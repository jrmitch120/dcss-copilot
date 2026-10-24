//! The localhost websocket the browser extension relays webtiles games over. The frames are
//! described in `docs/relay-protocol.md`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use copilot_webtiles::Envelope;
use copilot_webtiles::relay_protocol::{AppFrame, ExtensionFrame, RELAY_PROTOCOL_VERSION, Refusal};
pub use copilot_webtiles::relay_protocol::{RELAY_PORT, WindowBounds, WindowState};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, oneshot};
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::tungstenite::handshake::server::{ErrorResponse, Request, Response};
use tokio_tungstenite::tungstenite::http::StatusCode;
use tokio_tungstenite::tungstenite::{self, Message};

/// Only browser extensions may connect: never a web page, which could feed the app a fake game.
const EXTENSION_ORIGIN_SCHEMES: [&str; 2] = ["chrome-extension://", "moz-extension://"];

/// How long to wait before accepting again after a failed accept, such as running out of sockets.
const ACCEPT_RETRY: Duration = Duration::from_millis(100);

/// Something the relay needs the app to know or decide.
#[derive(Debug)]
pub enum RelayEvent {
    /// A browser connected for the first time: the player must be asked "Allow this browser?".
    ApprovalNeeded(ApprovalRequest),
    /// An allowed browser connected.
    Connected,
    /// An allowed browser relayed an envelope.
    Envelope(Envelope),
    /// A tab's buffer overflowed before it could be replayed, so its game is missing its start.
    Overflow,
    /// An allowed browser reported where the window holding its game is.
    BrowserWindow(BrowserWindow),
    /// An allowed browser disconnected.
    Disconnected,
}

/// A browser waiting for the player's answer to "Allow this browser?".
#[derive(Debug)]
pub struct ApprovalRequest {
    browser: String,
    approvals: Arc<Mutex<BrowserApprovals>>,
    answer: oneshot::Sender<bool>,
}

impl ApprovalRequest {
    /// The browser extension's origin, such as `chrome-extension://…`.
    #[must_use]
    pub fn browser(&self) -> &str {
        &self.browser
    }

    /// Remembers the player's answer for this browser and lets its connection go on or end.
    ///
    /// # Errors
    /// If the answer can't be saved. The connection is still answered.
    pub fn answer(self, allowed: bool) -> Result<(), ApprovalsError> {
        let saved = lock(&self.approvals).remember(&self.browser, allowed);
        // The browser may have gone away meanwhile; its next connect is answered from memory.
        let _ = self.answer.send(allowed);
        saved
    }
}

/// Where a browser's game window is, and the way to ask that browser to move it.
#[derive(Debug)]
pub struct BrowserWindow {
    /// Its outer bounds.
    pub bounds: WindowBounds,
    /// Whether it is minimized, maximized or fullscreen.
    pub state: WindowState,
    /// The browser that reported it.
    pub browser: BrowserLink,
}

/// The way back to one connected browser.
#[derive(Debug, Clone)]
pub struct BrowserLink(mpsc::UnboundedSender<AppFrame>);

impl BrowserLink {
    /// Asks the browser to move and resize its game window to `bounds`. A browser that has
    /// disconnected meanwhile is not asked.
    pub fn resize(&self, bounds: WindowBounds) {
        let _ = self.0.send(AppFrame::ResizeWindow { bounds });
    }
}

/// The player's answers to "Allow this browser?", saved so each browser is asked once.
#[derive(Debug)]
pub struct BrowserApprovals {
    path: PathBuf,
    answers: BTreeMap<String, Answer>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Answer {
    Allowed,
    Denied,
}

/// Why the saved answers couldn't be read or written.
#[derive(Debug, thiserror::Error)]
pub enum ApprovalsError {
    /// The file couldn't be read or written.
    #[error("browser approvals at {path} can't be read or written: {source}")]
    Io {
        /// The approvals file.
        path: PathBuf,
        /// What went wrong.
        source: std::io::Error,
    },
    /// The file isn't the JSON the app writes.
    #[error("browser approvals at {path} are malformed: {source}")]
    Malformed {
        /// The approvals file.
        path: PathBuf,
        /// What was wrong with it.
        source: serde_json::Error,
    },
}

impl BrowserApprovals {
    /// Reads the saved answers, or starts with none if the file doesn't exist yet.
    ///
    /// # Errors
    /// If the file exists but can't be read or parsed.
    pub fn load(path: &Path) -> Result<Self, ApprovalsError> {
        let answers = match std::fs::read(path) {
            Ok(bytes) => {
                serde_json::from_slice(&bytes).map_err(|source| ApprovalsError::Malformed {
                    path: path.to_owned(),
                    source,
                })?
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => BTreeMap::new(),
            Err(source) => {
                return Err(ApprovalsError::Io {
                    path: path.to_owned(),
                    source,
                });
            }
        };
        Ok(Self {
            path: path.to_owned(),
            answers,
        })
    }

    fn allowed(&self, browser: &str) -> Option<bool> {
        self.answers
            .get(browser)
            .map(|answer| *answer == Answer::Allowed)
    }

    fn remember(&mut self, browser: &str, allowed: bool) -> Result<(), ApprovalsError> {
        let answer = if allowed {
            Answer::Allowed
        } else {
            Answer::Denied
        };
        self.answers.insert(browser.to_owned(), answer);
        let io = |source| ApprovalsError::Io {
            path: self.path.clone(),
            source,
        };
        let json = serde_json::to_vec_pretty(&self.answers)
            .map_err(std::io::Error::other)
            .map_err(io)?;
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir).map_err(io)?;
        }
        std::fs::write(&self.path, json).map_err(io)
    }
}

/// Accepts extension connections on `listener` until the app exits, reporting to `events`.
pub async fn serve(
    listener: TcpListener,
    approvals: Arc<Mutex<BrowserApprovals>>,
    events: mpsc::Sender<RelayEvent>,
) {
    loop {
        match listener.accept().await {
            Ok((stream, _)) => {
                tokio::spawn(connection(stream, Arc::clone(&approvals), events.clone()));
            }
            Err(error) => {
                crate::log::error(format_args!(
                    "relay: could not accept a connection: {error}"
                ));
                tokio::time::sleep(ACCEPT_RETRY).await;
            }
        }
    }
}

#[derive(Debug, thiserror::Error)]
enum SessionError {
    #[error("websocket failed: {0}")]
    Socket(#[from] tungstenite::Error),
    #[error("the extension sent a malformed frame: {0}")]
    Malformed(#[from] serde_json::Error),
}

type Socket = WebSocketStream<TcpStream>;

async fn connection(
    stream: TcpStream,
    approvals: Arc<Mutex<BrowserApprovals>>,
    events: mpsc::Sender<RelayEvent>,
) {
    let mut browser = None;
    // tungstenite's handshake callback fixes the error type.
    #[allow(clippy::result_large_err)]
    let check_origin = |request: &Request, response: Response| match extension_origin(request) {
        Some(origin) => {
            browser = Some(origin);
            Ok(response)
        }
        None => Err(forbidden()),
    };
    let Ok(socket) = tokio_tungstenite::accept_hdr_async(stream, check_origin).await else {
        return;
    };
    let Some(browser) = browser else {
        return;
    };
    if let Err(error) = session(socket, browser, approvals, events).await {
        crate::log::error(format_args!("relay: {error}"));
    }
}

/// The browser extension's origin, if the request comes from one.
fn extension_origin(request: &Request) -> Option<String> {
    let origin = request.headers().get("Origin")?.to_str().ok()?;
    EXTENSION_ORIGIN_SCHEMES
        .iter()
        .any(|scheme| origin.starts_with(scheme))
        .then(|| origin.to_owned())
}

fn forbidden() -> ErrorResponse {
    let mut response = ErrorResponse::new(Some(
        "Only the DCSS Copilot extension may connect.".to_owned(),
    ));
    *response.status_mut() = StatusCode::FORBIDDEN;
    response
}

fn supported(protocol: u32) -> bool {
    protocol == RELAY_PROTOCOL_VERSION || protocol.checked_add(1) == Some(RELAY_PROTOCOL_VERSION)
}

async fn session(
    mut socket: Socket,
    browser: String,
    approvals: Arc<Mutex<BrowserApprovals>>,
    events: mpsc::Sender<RelayEvent>,
) -> Result<(), SessionError> {
    let Some(ExtensionFrame::Hello { protocol }) = next_frame(&mut socket).await? else {
        return Ok(());
    };
    if !supported(protocol) {
        return refuse(socket, Refusal::UnsupportedProtocol).await;
    }

    let remembered = lock(&approvals).allowed(&browser);
    let allowed = match remembered {
        Some(allowed) => allowed,
        None => {
            send(&mut socket, &AppFrame::AwaitingApproval).await?;
            let (answer, answered) = oneshot::channel();
            let request = ApprovalRequest {
                browser,
                approvals,
                answer,
            };
            if events
                .send(RelayEvent::ApprovalNeeded(request))
                .await
                .is_err()
            {
                return Ok(());
            }
            // No answer means the app is closing or asked again on a newer connection.
            let Ok(allowed) = answered.await else {
                return Ok(());
            };
            allowed
        }
    };
    if !allowed {
        return refuse(socket, Refusal::BrowserDenied).await;
    }

    send(
        &mut socket,
        &AppFrame::Welcome {
            protocol: RELAY_PROTOCOL_VERSION,
        },
    )
    .await?;
    let _ = events.send(RelayEvent::Connected).await;
    let relayed = relay(&mut socket, &events).await;
    let _ = events.send(RelayEvent::Disconnected).await;
    relayed
}

/// Passes the extension's frames on to the app, and the app's requests back, until the browser
/// disconnects.
async fn relay(socket: &mut Socket, events: &mpsc::Sender<RelayEvent>) -> Result<(), SessionError> {
    let (link, mut requests) = mpsc::unbounded_channel();
    let link = BrowserLink(link);
    loop {
        let frame = tokio::select! {
            frame = next_frame(socket) => frame?,
            Some(request) = requests.recv() => {
                send(socket, &request).await?;
                continue;
            }
        };
        let Some(frame) = frame else {
            break;
        };
        let event = match frame {
            ExtensionFrame::Envelope { envelope } => RelayEvent::Envelope(envelope),
            ExtensionFrame::Overflow { .. } => RelayEvent::Overflow,
            ExtensionFrame::Window { bounds, state } => RelayEvent::BrowserWindow(BrowserWindow {
                bounds,
                state,
                browser: link.clone(),
            }),
            ExtensionFrame::Hello { .. } | ExtensionFrame::Ping => continue,
        };
        if events.send(event).await.is_err() {
            break;
        }
    }
    Ok(())
}

async fn refuse(mut socket: Socket, reason: Refusal) -> Result<(), SessionError> {
    send(&mut socket, &AppFrame::Refused { reason }).await?;
    socket.close(None).await?;
    Ok(())
}

async fn send(socket: &mut Socket, frame: &AppFrame) -> Result<(), SessionError> {
    socket
        .send(Message::text(serde_json::to_string(frame)?))
        .await?;
    Ok(())
}

/// The extension's next frame, or `None` once it has disconnected.
async fn next_frame(socket: &mut Socket) -> Result<Option<ExtensionFrame>, SessionError> {
    while let Some(message) = socket.next().await {
        match message? {
            Message::Text(text) => return Ok(Some(serde_json::from_str(&text)?)),
            Message::Close(_) => return Ok(None),
            Message::Binary(_) | Message::Ping(_) | Message::Pong(_) | Message::Frame(_) => {}
        }
    }
    Ok(None)
}

/// The approvals stay consistent even if a thread panicked holding them: each update is one insert.
fn lock(approvals: &Mutex<BrowserApprovals>) -> std::sync::MutexGuard<'_, BrowserApprovals> {
    approvals.lock().unwrap_or_else(PoisonError::into_inner)
}
