//! The app's end of the extension relay: a fake extension connects to the real localhost websocket.

// Helpers outside #[test] functions panic on a broken setup too.
#![allow(clippy::unwrap_used)]

use std::net::SocketAddr;
use std::path::Path;
use std::sync::{Arc, Mutex};

use copilot_app_lib::relay::{BrowserApprovals, RelayEvent, serve};
use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::{Error as WsError, Message};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async};

const BROWSER: &str = "chrome-extension://abcdefghijklmnop";

type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;

struct App {
    addr: SocketAddr,
    events: mpsc::Receiver<RelayEvent>,
}

/// Starts the app's relay, remembering browsers in `data_dir`.
async fn start_app(data_dir: &Path) -> App {
    let approvals = BrowserApprovals::load(&data_dir.join("browsers.json")).unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let (sender, events) = mpsc::channel(64);
    tokio::spawn(serve(listener, Arc::new(Mutex::new(approvals)), sender));
    App { addr, events }
}

async fn connect(app: &App, origin: Option<&str>) -> Result<Socket, WsError> {
    let mut request = format!("ws://{}", app.addr).into_client_request().unwrap();
    if let Some(origin) = origin {
        request
            .headers_mut()
            .insert("Origin", origin.parse().unwrap());
    }
    connect_async(request).await.map(|(socket, _)| socket)
}

async fn send(socket: &mut Socket, frame: Value) {
    socket.send(Message::text(frame.to_string())).await.unwrap();
}

async fn receive(socket: &mut Socket) -> Option<Value> {
    loop {
        match socket.next().await? {
            Ok(Message::Text(text)) => return Some(serde_json::from_str(&text).unwrap()),
            Ok(Message::Close(_)) | Err(_) => return None,
            Ok(_) => {}
        }
    }
}

/// Connects as the extension and says hello.
async fn hello(app: &App, protocol: u32) -> Socket {
    let mut socket = connect(app, Some(BROWSER)).await.unwrap();
    send(
        &mut socket,
        json!({ "type": "hello", "protocol": protocol }),
    )
    .await;
    socket
}

/// Waits for the app to be asked "Allow this browser?" and answers.
async fn player_answers(app: &mut App, allow: bool) {
    match app.events.recv().await.unwrap() {
        RelayEvent::ApprovalNeeded(request) => {
            assert_eq!(request.browser(), BROWSER);
            request.answer(allow).unwrap();
        }
        other => panic!("expected an approval request, got {other:?}"),
    }
}

fn status_code(error: WsError) -> Option<u16> {
    match error {
        WsError::Http(response) => Some(response.status().as_u16()),
        _ => None,
    }
}

#[tokio::test]
async fn web_pages_cannot_connect() {
    let data_dir = tempfile::tempdir().unwrap();
    let app = start_app(data_dir.path()).await;

    for origin in [
        Some("https://evil.example"),
        Some("http://localhost:8080"),
        Some("null"),
        None,
    ] {
        let error = connect(&app, origin).await.unwrap_err();
        assert_eq!(status_code(error), Some(403), "origin {origin:?}");
    }
}

#[tokio::test]
async fn a_new_browser_waits_for_the_player_to_allow_it_then_relays_envelopes() {
    let data_dir = tempfile::tempdir().unwrap();
    let mut app = start_app(data_dir.path()).await;
    let mut socket = hello(&app, 1).await;

    assert_eq!(
        receive(&mut socket).await,
        Some(json!({ "type": "awaiting-approval" }))
    );
    player_answers(&mut app, true).await;
    assert_eq!(
        receive(&mut socket).await,
        Some(json!({ "type": "welcome", "protocol": 1 }))
    );
    assert!(matches!(
        app.events.recv().await,
        Some(RelayEvent::Connected)
    ));

    let envelope = json!({
        "origin": "https://crawl.example.org",
        "tab": 7,
        "ts": 1_760_486_400_000_u64,
        "gameVersion": "Dungeon Crawl Stone Soup 0.34.1",
        "msg": { "msg": "player", "hp": 18 },
    });
    send(&mut socket, json!({ "type": "ping" })).await;
    send(
        &mut socket,
        json!({ "type": "envelope", "envelope": envelope }),
    )
    .await;
    send(
        &mut socket,
        json!({ "type": "overflow", "origin": "https://crawl.example.org", "tab": 7 }),
    )
    .await;

    match app.events.recv().await {
        Some(RelayEvent::Envelope(received)) => {
            assert_eq!(serde_json::to_value(received).unwrap(), envelope);
        }
        other => panic!("expected an envelope, got {other:?}"),
    }
    assert!(matches!(
        app.events.recv().await,
        Some(RelayEvent::Overflow { tab: 7, .. })
    ));

    drop(socket);
    assert!(matches!(
        app.events.recv().await,
        Some(RelayEvent::Disconnected)
    ));
}

#[tokio::test]
async fn the_players_answer_is_remembered_across_restarts() {
    let data_dir = tempfile::tempdir().unwrap();
    let mut app = start_app(data_dir.path()).await;
    let mut socket = hello(&app, 1).await;
    receive(&mut socket).await;
    player_answers(&mut app, true).await;

    let restarted = start_app(data_dir.path()).await;
    let mut socket = hello(&restarted, 1).await;

    assert_eq!(
        receive(&mut socket).await,
        Some(json!({ "type": "welcome", "protocol": 1 }))
    );
}

#[tokio::test]
async fn a_denied_browser_is_refused_now_and_later() {
    let data_dir = tempfile::tempdir().unwrap();
    let mut app = start_app(data_dir.path()).await;
    let mut socket = hello(&app, 1).await;
    receive(&mut socket).await;

    player_answers(&mut app, false).await;
    assert_eq!(
        receive(&mut socket).await,
        Some(json!({ "type": "refused", "reason": "browser-denied" }))
    );
    assert_eq!(receive(&mut socket).await, None);

    let mut again = hello(&app, 1).await;
    assert_eq!(
        receive(&mut again).await,
        Some(json!({ "type": "refused", "reason": "browser-denied" }))
    );
}

#[tokio::test]
async fn the_current_and_previous_relay_protocol_versions_are_accepted() {
    let data_dir = tempfile::tempdir().unwrap();
    let mut app = start_app(data_dir.path()).await;
    let mut socket = hello(&app, 1).await;
    receive(&mut socket).await;
    player_answers(&mut app, true).await;

    let mut previous = hello(&app, 0).await;
    assert_eq!(
        receive(&mut previous).await,
        Some(json!({ "type": "welcome", "protocol": 1 }))
    );

    let mut newer = hello(&app, 2).await;
    assert_eq!(
        receive(&mut newer).await,
        Some(json!({ "type": "refused", "reason": "unsupported-protocol" }))
    );
    assert_eq!(receive(&mut newer).await, None);
}
