//! The fake-extension replayer: relays a Recording to the running app over the localhost websocket,
//! as the browser extension would, so the real panel can be designed and checked against real games.

use std::str::FromStr;
use std::time::Duration;

use copilot_webtiles::relay_protocol::{AppFrame, ExtensionFrame, RELAY_PROTOCOL_VERSION, Refusal};
use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::HeaderValue;
use tokio_tungstenite::tungstenite::{self, Message};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

use crate::{Applied, Recording, Replayer};

/// Where the app listens for the extension.
pub const APP_URL: &str = "ws://127.0.0.1:47821";

/// The origin the fake extension connects from. The app asks "Allow this browser?" for it once.
pub const FAKE_EXTENSION_ORIGIN: &str = "chrome-extension://dcss-copilot-fake-extension";

type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;

/// A browser page playing a Recording, and the extension relaying it to the app.
///
/// Like the real extension, it keeps every envelope the page has received: connecting replays
/// them all, then each move relays the envelopes it applies. A move made while disconnected only
/// moves the page along, and a backward move is a page reload: the connection is remade and the
/// buffer replayed from the start.
pub struct FakeExtension {
    page: Replayer,
    url: String,
    origin: String,
    socket: Option<Socket>,
}

/// Why the fake extension couldn't reach the app or relay to it.
#[derive(Debug, thiserror::Error)]
pub enum FakeExtensionError {
    /// The websocket failed: the app isn't running, or the connection dropped.
    #[error("websocket failed: {0}")]
    Socket(#[from] tungstenite::Error),
    /// The app sent a frame this build doesn't understand.
    #[error("the app sent a malformed frame: {0}")]
    Malformed(#[from] serde_json::Error),
    /// The app refused the connection.
    #[error("the app refused the connection: {0:?}")]
    Refused(Refusal),
    /// The app closed the connection before welcoming it, as when it asked "Allow this browser?"
    /// on a newer connection instead.
    #[error("the app closed the connection before welcoming it")]
    Closed,
    /// The origin isn't valid in an HTTP header.
    #[error("{0:?} can't be an Origin header")]
    Origin(String),
}

impl FakeExtension {
    /// A page that has received nothing yet, relaying to the app at `url` from `origin`.
    #[must_use]
    pub fn new(recording: Recording, url: &str, origin: &str) -> Self {
        Self {
            page: Replayer::new(recording),
            url: url.to_owned(),
            origin: origin.to_owned(),
            socket: None,
        }
    }

    /// What the page has received, replayed in process.
    #[must_use]
    pub fn replayer(&self) -> &Replayer {
        &self.page
    }

    /// The app has welcomed the connection and it hasn't dropped since.
    #[must_use]
    pub fn is_connected(&self) -> bool {
        self.socket.is_some()
    }

    /// Connects, says hello, waits for the app's welcome (through "Allow this browser?" if the
    /// app asks), then replays every envelope the page has received.
    ///
    /// # Errors
    /// If the app can't be reached, refuses or closes the connection, or the replay fails.
    pub async fn connect(&mut self) -> Result<(), FakeExtensionError> {
        self.disconnect().await;
        let mut request = self.url.as_str().into_client_request()?;
        let origin = HeaderValue::from_str(&self.origin)
            .map_err(|_| FakeExtensionError::Origin(self.origin.clone()))?;
        request.headers_mut().insert("Origin", origin);
        let (mut socket, _) = tokio_tungstenite::connect_async(request).await?;
        send(
            &mut socket,
            &ExtensionFrame::Hello {
                protocol: RELAY_PROTOCOL_VERSION,
            },
        )
        .await?;
        socket.flush().await?;
        welcomed(&mut socket).await?;
        self.socket = Some(socket);
        self.relay(0..self.page.position()).await
    }

    /// Closes the connection, if there is one, as when the browser closes.
    pub async fn disconnect(&mut self) {
        if let Some(mut socket) = self.socket.take() {
            // Waiting for the app's closing reply means the app has seen this connection end
            // before any new one starts.
            if socket.close(None).await.is_ok() {
                while let Some(Ok(_)) = socket.next().await {}
            }
        }
    }

    /// The page receives its next envelope.
    ///
    /// # Errors
    /// If relaying it fails; the connection is then dropped.
    pub async fn step(&mut self) -> Result<Applied, FakeExtensionError> {
        let applied = self.page.step();
        self.deliver(applied).await
    }

    /// The page receives envelopes up to the next one that changes the followed game's turn.
    ///
    /// # Errors
    /// If relaying them fails; the connection is then dropped.
    pub async fn step_turn(&mut self) -> Result<Applied, FakeExtensionError> {
        let applied = self.page.step_turn();
        self.deliver(applied).await
    }

    /// The page moves to just after `position` envelopes, reloading if that is backward.
    ///
    /// # Errors
    /// If reconnecting or relaying fails; the connection is then dropped.
    pub async fn seek(&mut self, position: usize) -> Result<Applied, FakeExtensionError> {
        let applied = self.page.seek(position);
        self.deliver(applied).await
    }

    /// The page moves to the first envelope at which the followed game's turn is at least `turn`,
    /// or stays put with `None` if the Recording never gets there.
    ///
    /// # Errors
    /// If reconnecting or relaying fails; the connection is then dropped.
    pub async fn seek_turn(&mut self, turn: u32) -> Result<Option<Applied>, FakeExtensionError> {
        match self.page.seek_turn(turn) {
            Some(applied) => self.deliver(applied).await.map(Some),
            None => Ok(None),
        }
    }

    async fn deliver(&mut self, applied: Applied) -> Result<Applied, FakeExtensionError> {
        if self.socket.is_none() {
            return Ok(applied);
        }
        if applied.restarted {
            // A reload: the new connection replays the buffer, which is exactly what was applied.
            self.connect().await?;
        } else {
            self.relay(applied.envelopes.clone()).await?;
        }
        Ok(applied)
    }

    async fn relay(&mut self, envelopes: std::ops::Range<usize>) -> Result<(), FakeExtensionError> {
        let Some(socket) = &mut self.socket else {
            return Ok(());
        };
        let mut relayed = Ok(());
        for envelope in &self.page.recording().envelopes[envelopes] {
            let frame = ExtensionFrame::Envelope {
                envelope: envelope.clone(),
            };
            relayed = send(socket, &frame).await;
            if relayed.is_err() {
                break;
            }
        }
        if relayed.is_ok() {
            relayed = socket.flush().await.map_err(FakeExtensionError::from);
        }
        if relayed.is_err() {
            self.socket = None;
        }
        relayed
    }
}

/// Queues a frame; the caller flushes.
async fn send(socket: &mut Socket, frame: &ExtensionFrame) -> Result<(), FakeExtensionError> {
    socket
        .feed(Message::text(serde_json::to_string(frame)?))
        .await?;
    Ok(())
}

/// Waits for the app's welcome, through any wait for the player's approval.
async fn welcomed(socket: &mut Socket) -> Result<(), FakeExtensionError> {
    while let Some(message) = socket.next().await {
        let Message::Text(text) = message? else {
            continue;
        };
        match serde_json::from_str(&text)? {
            AppFrame::Welcome { .. } => return Ok(()),
            AppFrame::AwaitingApproval => {}
            AppFrame::Refused { reason } => return Err(FakeExtensionError::Refused(reason)),
        }
    }
    Err(FakeExtensionError::Closed)
}

/// How fast the page receives a Recording's envelopes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Speed {
    /// The gaps between envelopes, divided by this: 1 is real time.
    Times(f64),
    /// No waiting at all.
    AsFastAsPossible,
}

impl Speed {
    /// How long to wait between envelopes received at `from_ms` and `to_ms`.
    #[must_use]
    pub fn wait(self, from_ms: u64, to_ms: u64) -> Duration {
        match self {
            // Envelope gaps fit easily in an f64's exact integer range.
            #[allow(clippy::cast_precision_loss)]
            Self::Times(times) => {
                Duration::from_secs_f64(to_ms.saturating_sub(from_ms) as f64 / 1000.0 / times)
            }
            Self::AsFastAsPossible => Duration::ZERO,
        }
    }
}

/// `max`, or a positive multiple of real time such as `1`, `4` or `0.5`.
impl FromStr for Speed {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        if text == "max" {
            return Ok(Self::AsFastAsPossible);
        }
        match text.trim_start_matches(['x', '×']).parse::<f64>() {
            Ok(times) if times.is_finite() && times > 0.0 => Ok(Self::Times(times)),
            _ => Err(format!(
                "{text:?} isn't a speed: use max, or a multiple of real time such as 1, 4 or 0.5"
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_speed_divides_the_gap_between_envelopes() {
        assert_eq!(
            Speed::Times(1.0).wait(1_000, 3_500),
            Duration::from_millis(2_500)
        );
        assert_eq!(
            Speed::Times(4.0).wait(1_000, 3_000),
            Duration::from_millis(500)
        );
        assert_eq!(Speed::AsFastAsPossible.wait(1_000, 9_000), Duration::ZERO);
        // Envelopes from two tabs can arrive out of timestamp order.
        assert_eq!(Speed::Times(1.0).wait(3_000, 1_000), Duration::ZERO);
    }

    #[test]
    fn a_speed_is_max_or_a_positive_multiple_of_real_time() {
        assert_eq!("max".parse(), Ok(Speed::AsFastAsPossible));
        assert_eq!("4".parse(), Ok(Speed::Times(4.0)));
        assert_eq!("x0.5".parse(), Ok(Speed::Times(0.5)));
        for bad in ["0", "-2", "inf", "fast"] {
            assert!(bad.parse::<Speed>().is_err(), "{bad}");
        }
    }
}
