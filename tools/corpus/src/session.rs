//! A webtiles session on a local server, played from here and recorded as the extension would
//! have relayed it.

use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Context, bail};
use copilot_replay::RecordingWriter;
use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use tokio::net::TcpStream;
use tokio::time::{Instant, timeout};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::HeaderValue;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

use crate::capture::Capture;

/// The browser tab every recorded game is played in.
const TAB: i64 = 1;

/// How long the server must stay quiet before the game counts as settled: drawn and waiting for
/// the next key.
const SETTLE: Duration = Duration::from_millis(1500);

/// The longest to wait for the server to answer a lobby request.
const LOBBY_TIMEOUT: Duration = Duration::from_secs(30);

type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;

/// A logged-in webtiles session whose every message is written to a Recording.
pub struct Session {
    socket: Socket,
    capture: Capture,
    recording: RecordingWriter<std::fs::File>,
    /// The game process has stopped and the server sent the player back to the lobby.
    game_over: bool,
    /// The first Lua error the game reported, such as from a bot in the rc file.
    lua_error: Option<String>,
}

/// Why waiting on the server stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stopped {
    /// The server went quiet.
    Settled,
    /// The game process stopped.
    GameOver,
    /// The deadline passed.
    Deadline,
}

impl Session {
    /// Connects to the webtiles websocket at `url`, recording into a new file at `out`.
    pub async fn connect(url: &str, out: &Path) -> anyhow::Result<Self> {
        let origin = page_origin(url)?;
        let mut request = url.into_client_request()?;
        // Uncompressed frames, so they are text the capture reads like the page does.
        request.headers_mut().insert(
            "Sec-WebSocket-Protocol",
            HeaderValue::from_static("no-compression"),
        );
        request
            .headers_mut()
            .insert("Origin", HeaderValue::from_str(&origin)?);
        let (socket, _) = tokio_tungstenite::connect_async(request)
            .await
            .with_context(|| format!("couldn't reach webtiles at {url}"))?;
        let file = std::fs::File::create(out)
            .with_context(|| format!("couldn't create {}", out.display()))?;
        Ok(Self {
            socket,
            capture: Capture::new(&origin, TAB),
            recording: RecordingWriter::new(file, env!("CARGO_PKG_VERSION"))?,
            game_over: false,
            lua_error: None,
        })
    }

    /// Registers a new account and waits until it is logged in.
    pub async fn register(&mut self, username: &str, password: &str) -> anyhow::Result<()> {
        self.send(&json!({
            "msg": "register",
            "username": username,
            "password": password,
            "email": "",
        }))
        .await?;
        self.wait_for(&["login_success"], &["register_fail", "login_fail"])
            .await
            .with_context(|| format!("couldn't register {username}"))
    }

    /// Replaces the account's rc file for `game`.
    pub async fn set_rc(&mut self, game: &str, contents: &str) -> anyhow::Result<()> {
        self.send(&json!({ "msg": "set_rc", "game_id": game, "contents": contents }))
            .await
    }

    /// Starts `game` and waits until it has started.
    pub async fn play(&mut self, game: &str) -> anyhow::Result<()> {
        self.send(&json!({ "msg": "play", "game_id": game }))
            .await?;
        self.wait_for(&["game_started"], &["go_lobby", "login_required"])
            .await
            .with_context(|| format!("couldn't start {game}"))
    }

    /// Types `text` into the game.
    pub async fn input(&mut self, text: &str) -> anyhow::Result<()> {
        self.send(&json!({ "msg": "input", "text": text })).await
    }

    /// Records until the server has been quiet for a moment, the game stops, or `deadline`.
    pub async fn settle(&mut self, deadline: Instant) -> anyhow::Result<Stopped> {
        while !self.game_over {
            let wait = SETTLE.min(deadline.saturating_duration_since(Instant::now()));
            if wait.is_zero() {
                return Ok(Stopped::Deadline);
            }
            match timeout(wait, self.socket.next()).await {
                Err(_) if Instant::now() >= deadline => return Ok(Stopped::Deadline),
                Err(_) => return Ok(Stopped::Settled),
                Ok(None) => bail!("the server closed the connection"),
                Ok(Some(message)) => {
                    self.receive(message?).await?;
                }
            }
        }
        Ok(Stopped::GameOver)
    }

    /// The first Lua error the game has reported, if any.
    pub fn lua_error(&self) -> Option<&str> {
        self.lua_error.as_deref()
    }

    /// Closes the connection and finishes the Recording.
    pub async fn finish(mut self) -> anyhow::Result<()> {
        // Closing mid-game makes the server save it, as closing the browser tab would.
        if self.socket.close(None).await.is_ok() {
            while let Some(Ok(_)) = self.socket.next().await {}
        }
        self.recording.finish()?;
        Ok(())
    }

    /// Records until one of `wanted` arrives; one of `refused` is an error.
    async fn wait_for(&mut self, wanted: &[&str], refused: &[&str]) -> anyhow::Result<()> {
        let deadline = Instant::now() + LOBBY_TIMEOUT;
        loop {
            let Ok(next) = tokio::time::timeout_at(deadline, self.socket.next()).await else {
                bail!("the server didn't answer: waited for {wanted:?}");
            };
            let Some(message) = next else {
                bail!("the server closed the connection");
            };
            for msg in self.receive(message?).await? {
                if wanted.contains(&msg.as_str()) {
                    return Ok(());
                }
                if refused.contains(&msg.as_str()) {
                    bail!("the server answered {msg}");
                }
            }
        }
    }

    /// Records one frame, answers a ping, and returns the types of the messages it held.
    async fn receive(&mut self, message: Message) -> anyhow::Result<Vec<String>> {
        let Message::Text(text) = message else {
            return Ok(Vec::new());
        };
        let captured = self.capture.frame(&text, now_ms());
        let mut types = Vec::with_capacity(captured.envelopes.len());
        for envelope in &captured.envelopes {
            self.recording.write(envelope)?;
            let msg = envelope.msg.get("msg").and_then(Value::as_str);
            match msg {
                Some("go_lobby") => self.game_over = true,
                Some("msgs") if self.lua_error.is_none() => {
                    self.lua_error = lua_error(&envelope.msg);
                }
                _ => {}
            }
            types.push(msg.unwrap_or_default().to_owned());
        }
        if captured.pinged {
            self.send(&json!({ "msg": "pong" })).await?;
        }
        Ok(types)
    }

    async fn send(&mut self, msg: &Value) -> anyhow::Result<()> {
        self.socket.send(Message::text(msg.to_string())).await?;
        Ok(())
    }
}

/// The first line of a `msgs` message that reports a Lua error, as crawl prints it.
fn lua_error(msg: &Value) -> Option<String> {
    msg.get("messages")?
        .as_array()?
        .iter()
        .filter_map(|line| line.get("text")?.as_str())
        .find(|text| text.contains("Lua error:"))
        .map(str::to_owned)
}

/// The origin of the page that serves the webtiles websocket at `url`, as the envelope's `origin`.
fn page_origin(url: &str) -> anyhow::Result<String> {
    let (scheme, rest) = url
        .split_once("://")
        .with_context(|| format!("{url:?} isn't a websocket URL"))?;
    let scheme = match scheme {
        "ws" => "http",
        "wss" => "https",
        _ => bail!("{url:?} isn't a ws:// or wss:// URL"),
    };
    let host = rest.split('/').next().unwrap_or(rest);
    Ok(format!("{scheme}://{host}"))
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| {
            u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX)
        })
}
