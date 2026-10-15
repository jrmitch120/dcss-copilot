//! The fake-extension replayer against the app's real relay: a Recording goes over the localhost
//! websocket as the extension would relay it.

// Helpers outside #[test] functions panic on a broken setup too.
#![allow(clippy::unwrap_used)]

use std::path::Path;
use std::sync::{Arc, Mutex};

use copilot_app_lib::relay::{BrowserApprovals, RelayEvent, serve};
use copilot_replay::{FAKE_EXTENSION_ORIGIN, FakeExtension, FakeExtensionError, Recording};
use copilot_webtiles::Envelope;
use copilot_webtiles::relay_protocol::Refusal;
use tokio::net::TcpListener;
use tokio::sync::mpsc;

const MINOTAUR: &[u8] =
    include_bytes!("../../../crates/replay/tests/recordings/minotaur-first-steps.dcssrec");

struct App {
    url: String,
    events: mpsc::Receiver<RelayEvent>,
}

/// Starts the app's relay, remembering browsers in `data_dir`.
async fn start_app(data_dir: &Path) -> App {
    let approvals = BrowserApprovals::load(&data_dir.join("browsers.json")).unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("ws://{}", listener.local_addr().unwrap());
    let (sender, events) = mpsc::channel(64);
    tokio::spawn(serve(listener, Arc::new(Mutex::new(approvals)), sender));
    App { url, events }
}

fn fake_extension(app: &App) -> FakeExtension {
    FakeExtension::new(
        Recording::read(MINOTAUR).unwrap(),
        &app.url,
        FAKE_EXTENSION_ORIGIN,
    )
    .unwrap()
}

fn envelopes() -> Vec<Envelope> {
    Recording::read(MINOTAUR).unwrap().envelopes
}

/// Answers "Allow this browser?" when the app asks.
async fn player_answers(events: &mut mpsc::Receiver<RelayEvent>, allow: bool) {
    match events.recv().await.unwrap() {
        RelayEvent::ApprovalNeeded(request) => request.answer(allow).unwrap(),
        other => panic!("expected an approval request, got {other:?}"),
    }
}

/// Connects the fake extension, allowing it as the player would.
async fn connect_allowed(extension: &mut FakeExtension, app: &mut App) {
    let (connected, ()) = tokio::join!(extension.connect(), player_answers(&mut app.events, true));
    connected.unwrap();
}

/// What the app hears next, envelopes by their index in the Recording.
async fn heard(app: &mut App, count: usize) -> Vec<String> {
    let all = envelopes();
    let mut heard = Vec::new();
    for _ in 0..count {
        heard.push(match app.events.recv().await.unwrap() {
            RelayEvent::Envelope(envelope) => {
                let index = all.iter().position(|e| *e == envelope).unwrap();
                format!("envelope {index}")
            }
            RelayEvent::Connected => "connected".to_owned(),
            RelayEvent::Disconnected => "disconnected".to_owned(),
            other => format!("{other:?}"),
        });
    }
    heard
}

#[tokio::test]
async fn on_connecting_it_shakes_hands_then_replays_its_buffer_then_relays_live() {
    let data_dir = tempfile::tempdir().unwrap();
    let mut app = start_app(data_dir.path()).await;
    let mut extension = fake_extension(&app);
    // The page has received three messages before the app is there to hear them.
    extension.seek(3).await.unwrap();

    connect_allowed(&mut extension, &mut app).await;
    extension.step().await.unwrap();

    assert_eq!(
        heard(&mut app, 5).await,
        [
            "connected",
            "envelope 0",
            "envelope 1",
            "envelope 2",
            "envelope 3"
        ]
    );
}

#[tokio::test]
async fn stepping_a_turn_relays_every_envelope_up_to_the_next_turn() {
    let data_dir = tempfile::tempdir().unwrap();
    let mut app = start_app(data_dir.path()).await;
    let mut extension = fake_extension(&app);
    extension.seek(5).await.unwrap();
    connect_allowed(&mut extension, &mut app).await;
    heard(&mut app, 6).await;

    extension.step_turn().await.unwrap();

    assert_eq!(
        heard(&mut app, 3).await,
        ["envelope 5", "envelope 6", "envelope 7"]
    );
    assert_eq!(extension.replayer().turn(), Some(3));
}

#[tokio::test]
async fn seeking_backward_reconnects_and_replays_from_the_start() {
    let data_dir = tempfile::tempdir().unwrap();
    let mut app = start_app(data_dir.path()).await;
    let mut extension = fake_extension(&app);
    connect_allowed(&mut extension, &mut app).await;
    extension.seek_turn(3).await.unwrap();
    heard(&mut app, 9).await;

    extension.seek(2).await.unwrap();

    assert_eq!(
        heard(&mut app, 4).await,
        ["disconnected", "connected", "envelope 0", "envelope 1"]
    );
}

#[tokio::test]
async fn a_browser_the_player_denies_is_refused() {
    let data_dir = tempfile::tempdir().unwrap();
    let mut app = start_app(data_dir.path()).await;
    let mut extension = fake_extension(&app);

    let (connected, ()) = tokio::join!(extension.connect(), player_answers(&mut app.events, false));

    assert!(matches!(
        connected,
        Err(FakeExtensionError::Refused(Refusal::BrowserDenied))
    ));
}
