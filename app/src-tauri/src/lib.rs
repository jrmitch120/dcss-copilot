//! The DCSS Copilot desktop app: wires the core to its windows, tray and keychain.

mod live;
pub mod relay;

use std::net::Ipv4Addr;
use std::sync::{Arc, Mutex};

use copilot_core::ViewModel;
use serde::Serialize;
use tauri::{AppHandle, Manager, RunEvent, State};
use tauri_specta::{Builder, Event, collect_commands, collect_events};
use tokio::net::TcpListener;
use tokio::sync::mpsc;

use crate::live::Live;
use crate::relay::{BrowserApprovals, RELAY_PORT};

/// Where the generated TypeScript bindings for the webview live.
const BINDINGS_PATH: &str = "../src/lib/bindings.ts";

/// Relay events waiting for the app, before the relay waits for the app to catch up.
const RELAY_QUEUE: usize = 1024;

/// The core pushes a new view model to the panel.
#[derive(Debug, Clone, Serialize, specta::Type, tauri_specta::Event)]
pub struct ViewModelChanged(ViewModel);

/// The view model as it is now, for a panel that has just loaded.
#[tauri::command]
#[specta::specta]
#[allow(clippy::needless_pass_by_value)] // Tauri passes command arguments by value.
fn view_model(live: State<'_, Live>) -> ViewModel {
    live.view_model()
}

/// The player answered "Allow this browser?".
#[tauri::command]
#[specta::specta]
#[allow(clippy::needless_pass_by_value)] // Tauri passes command arguments by value.
fn answer_browser(allow: bool, app: AppHandle, live: State<'_, Live>) -> Result<(), String> {
    let answered = live.answer_browser(allow);
    push_view_model(&app, &live);
    answered
}

fn specta_builder() -> Builder<tauri::Wry> {
    Builder::<tauri::Wry>::new()
        .commands(collect_commands![view_model, answer_browser])
        .events(collect_events![ViewModelChanged])
}

/// Writes the webview's TypeScript bindings. Run by the `bindings` test.
///
/// # Errors
/// If the bindings can't be generated or written.
pub fn export_bindings() -> Result<(), specta_typescript::Error> {
    specta_builder().export(specta_typescript::Typescript::default(), BINDINGS_PATH)
}

/// Starts the app.
///
/// # Panics
/// If Tauri fails to start, which leaves nothing to recover.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
#[allow(clippy::expect_used)]
pub fn run() {
    let builder = specta_builder();
    tauri::Builder::default()
        .invoke_handler(builder.invoke_handler())
        .setup(move |app| {
            builder.mount_events(app);
            // Dev builds save what they receive, so a session can be replayed.
            let recordings = if cfg!(debug_assertions) {
                Some(app.path().app_data_dir()?.join("recordings"))
            } else {
                None
            };
            app.manage(Live::new(env!("CARGO_PKG_VERSION"), recordings));
            let approvals = app.path().app_config_dir()?.join("browsers.json");
            tauri::async_runtime::spawn(run_relay(app.handle().clone(), approvals));
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while starting DCSS Copilot")
        .run(|app, event| {
            if let RunEvent::Exit = event {
                app.state::<Live>().finish_recording();
            }
        });
}

/// Listens for the browser extension and feeds what it relays to the live game.
async fn run_relay(app: AppHandle, approvals: std::path::PathBuf) {
    let live = app.state::<Live>();
    let started = match BrowserApprovals::load(&approvals) {
        Ok(approvals) => TcpListener::bind((Ipv4Addr::LOCALHOST, RELAY_PORT))
            .await
            .map(|listener| (listener, approvals))
            .map_err(|error| format!("Port {RELAY_PORT} is in use or blocked: {error}")),
        Err(error) => Err(error.to_string()),
    };
    let (listener, approvals) = match started {
        Ok(started) => started,
        Err(reason) => {
            live.relay_unavailable(reason);
            push_view_model(&app, &live);
            return;
        }
    };

    let (sender, mut events) = mpsc::channel(RELAY_QUEUE);
    tauri::async_runtime::spawn(relay::serve(
        listener,
        Arc::new(Mutex::new(approvals)),
        sender,
    ));
    while let Some(event) = events.recv().await {
        live.handle(event);
        push_view_model(&app, &live);
    }
}

fn push_view_model(app: &AppHandle, live: &Live) {
    if let Err(error) = ViewModelChanged(live.view_model()).emit(app) {
        eprintln!("could not push the view model to the panel: {error}");
    }
}

#[cfg(test)]
mod tests {
    /// Regenerates the bindings; CI fails if this leaves the committed file changed.
    #[test]
    fn bindings() {
        super::export_bindings().unwrap();
    }
}
