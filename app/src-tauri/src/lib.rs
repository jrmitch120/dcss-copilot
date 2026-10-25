//! The DCSS Copilot desktop app: wires the core to its windows, tray and keychain.

mod keys;
mod lifecycle;
mod live;
mod log;
mod models;
mod panels;
pub mod relay;
mod settings;

use std::net::Ipv4Addr;
use std::sync::{Arc, Mutex};

use copilot_core::{Appearance, AssistanceLevel, ViewModel};
use copilot_llm::{AnthropicProvider, ModelChoice, ModelSlot, Provider};
use copilot_store::Store;
use serde::Serialize;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, RunEvent, State, WebviewUrl, WebviewWindowBuilder, WindowEvent};
use tauri_plugin_autostart::{MacosLauncher, ManagerExt};
use tauri_specta::{Builder, Event, collect_commands, collect_events};
use tokio::net::TcpListener;
use tokio::sync::mpsc;

use crate::keys::{Keys, OsKeychain};
use crate::lifecycle::Presence;
use crate::live::Live;
use crate::models::{ModelsAndKeys, ModelsAndKeysView, ThinkingOption};
use crate::panels::{PANEL_WINDOW, Panels};
use crate::relay::{BrowserApprovals, PanelCommand, RELAY_PORT, RelayEvent, SidePanelEvent};
use crate::settings::Settings;

/// Where the generated TypeScript bindings for the webview live.
const BINDINGS_PATH: &str = "../src/lib/bindings.ts";

/// Relay events waiting for the app, before the relay waits for the app to catch up.
const RELAY_QUEUE: usize = 1024;

/// The core pushes a new view model to the panel.
#[derive(Debug, Clone, Serialize, specta::Type, tauri_specta::Event)]
pub struct ViewModelChanged(ViewModel);

/// The Settings window's label, which also names its route in the webview.
const SETTINGS_WINDOW: &str = "settings";

/// The tray menu's items.
const TRAY_SHOW: &str = "show";
/// See [`TRAY_SHOW`].
const TRAY_SETTINGS: &str = "settings";
/// See [`TRAY_SHOW`].
const TRAY_QUIT: &str = "quit";

/// What starting at login passes the app, so it starts in the tray rather than showing the panel.
const STARTED_AT_LOGIN: &str = "--started-at-login";

/// Everything the Settings window shows.
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SettingsView {
    /// The running app's version, for About.
    app_version: String,
    /// Settings → Models and keys.
    models: ModelsAndKeysView,
    /// Settings → Connection and window.
    window: WindowView,
}

/// How the app's window behaves, in Settings → Connection and window.
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct WindowView {
    /// The app starts in the tray at login.
    start_at_login: bool,
}

/// The core pushes new settings to the Settings window.
#[derive(Debug, Clone, Serialize, specta::Type, tauri_specta::Event)]
pub struct SettingsChanged(SettingsView);

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

/// The player chose to draw monsters and items as tiles or glyphs.
#[tauri::command]
#[specta::specta]
#[allow(clippy::needless_pass_by_value)] // Tauri passes command arguments by value.
fn set_appearance(
    appearance: Appearance,
    app: AppHandle,
    live: State<'_, Live>,
) -> Result<(), String> {
    let saved = live.set_appearance(appearance);
    push_view_model(&app, &live);
    saved.map_err(|error| error.to_string())
}

/// The player chose how much the Advisor offers beyond Information.
#[tauri::command]
#[specta::specta]
#[allow(clippy::needless_pass_by_value)] // Tauri passes command arguments by value.
fn set_assistance(
    assistance: AssistanceLevel,
    app: AppHandle,
    live: State<'_, Live>,
) -> Result<(), String> {
    let saved = live.set_assistance(assistance);
    push_view_model(&app, &live);
    saved.map_err(|error| error.to_string())
}

/// The player says the followed game is a new game, not the one the Advisor took it for.
#[tauri::command]
#[specta::specta]
#[allow(clippy::needless_pass_by_value)] // Tauri passes command arguments by value.
fn new_game(app: AppHandle, live: State<'_, Live>) -> Result<(), String> {
    live.new_game();
    push_view_model(&app, &live);
    Ok(())
}

/// Opens the Settings window, or brings it to the front.
#[tauri::command]
#[specta::specta]
#[allow(clippy::needless_pass_by_value)] // Tauri passes command arguments by value.
fn open_settings(app: AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window(SETTINGS_WINDOW) {
        return window
            .show()
            .and_then(|()| window.set_focus())
            .map_err(|error| error.to_string());
    }
    WebviewWindowBuilder::new(
        &app,
        SETTINGS_WINDOW,
        WebviewUrl::App(SETTINGS_WINDOW.into()),
    )
    .title("DCSS Copilot Settings")
    .inner_size(880.0, 600.0)
    .min_inner_size(560.0, 420.0)
    .build()
    .map(drop)
    .map_err(|error| error.to_string())
}

/// The player turned start at login on or off.
#[tauri::command]
#[specta::specta]
#[allow(clippy::needless_pass_by_value)] // Tauri passes command arguments by value.
fn set_start_at_login(on: bool, app: AppHandle) -> Result<(), String> {
    let autostart = app.autolaunch();
    let changed = if on {
        autostart.enable()
    } else {
        autostart.disable()
    };
    push_settings(&app);
    changed.map_err(|error| format!("Start at login couldn't be changed: {error}"))
}

/// The settings as they are now, for a Settings window that has just loaded.
#[tauri::command]
#[specta::specta]
#[allow(clippy::needless_pass_by_value)] // Tauri passes command arguments by value.
fn settings_view(app: AppHandle) -> SettingsView {
    current_settings(&app)
}

/// The player chose a slot's model and how it thinks.
#[tauri::command]
#[specta::specta]
#[allow(clippy::needless_pass_by_value)] // Tauri passes command arguments by value.
fn set_model(
    slot: ModelSlot,
    model: String,
    thinking: ThinkingOption,
    app: AppHandle,
    live: State<'_, Live>,
) -> Result<(), String> {
    let provider = live.model(slot).provider();
    let choice =
        ModelChoice::new(provider, &model, thinking.into()).map_err(|error| error.to_string())?;
    let saved = live.set_model(slot, choice);
    push_settings(&app);
    saved.map_err(unsaved)
}

/// The player pasted a key for a provider.
#[tauri::command]
#[specta::specta]
#[allow(clippy::needless_pass_by_value)] // Tauri passes command arguments by value.
fn save_key(
    provider: String,
    key: String,
    app: AppHandle,
    models: State<'_, ModelsAndKeys>,
) -> Result<(), String> {
    let saved = models.save_key(provider_named(&provider)?, &key);
    push_settings(&app);
    saved.map_err(|error| error.to_string())
}

/// The player removed their key for a provider.
#[tauri::command]
#[specta::specta]
#[allow(clippy::needless_pass_by_value)] // Tauri passes command arguments by value.
fn remove_key(
    provider: String,
    app: AppHandle,
    models: State<'_, ModelsAndKeys>,
) -> Result<(), String> {
    let removed = models.remove_key(provider_named(&provider)?);
    push_settings(&app);
    removed.map_err(|error| error.to_string())
}

/// The player asked to test a provider's key. The outcome arrives in the settings.
#[tauri::command]
#[specta::specta]
async fn test_key(provider: String, app: AppHandle) -> Result<(), String> {
    let provider = provider_named(&provider)?;
    let models = app.state::<ModelsAndKeys>();
    models.testing(provider);
    push_settings(&app);
    let sidecar = app.state::<Live>().llm_sidecar();
    models.test_key(provider, sidecar).await;
    push_settings(&app);
    Ok(())
}

/// What the player is told when a choice of theirs couldn't be saved.
#[allow(clippy::needless_pass_by_value)] // Called through `map_err`.
fn unsaved(error: settings::SettingsError) -> String {
    format!("This choice lasts only until DCSS Copilot closes: {error}")
}

fn provider_named(id: &str) -> Result<Provider, String> {
    Provider::from_id(id).ok_or_else(|| format!("there is no provider called {id}"))
}

fn specta_builder() -> Builder<tauri::Wry> {
    Builder::<tauri::Wry>::new()
        .commands(collect_commands![
            view_model,
            answer_browser,
            set_appearance,
            set_assistance,
            new_game,
            open_settings,
            settings_view,
            set_start_at_login,
            set_model,
            save_key,
            remove_key,
            test_key
        ])
        .events(collect_events![ViewModelChanged, SettingsChanged])
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
        // Registered first, so a second launch hands over before anything else starts.
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            app.state::<Panels>().show(app);
        }))
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            Some(vec![STARTED_AT_LOGIN]),
        ))
        .invoke_handler(builder.invoke_handler())
        .on_window_event(|window, event| {
            // Closing the panel hides it to the tray; Quit in the tray menu exits.
            if let WindowEvent::CloseRequested { api, .. } = event
                && window.label() == PANEL_WINDOW
            {
                api.prevent_close();
                window
                    .app_handle()
                    .state::<Panels>()
                    .hide(window.app_handle());
            }
        })
        .setup(move |app| {
            builder.mount_events(app);
            let presence = if std::env::args().any(|arg| arg == STARTED_AT_LOGIN) {
                Presence::Hidden
            } else {
                Presence::Shown
            };
            app.manage(Panels::new(presence));
            tray(app.handle())?;
            // Dev builds save what they receive, so a session can be replayed.
            let recordings = if cfg!(debug_assertions) {
                Some(app.path().app_data_dir()?.join("recordings"))
            } else {
                None
            };
            let settings_path = app.path().app_config_dir()?.join("settings.json");
            // An unreadable settings file is replaced by the defaults the next time one changes.
            let settings = Settings::load(&settings_path).unwrap_or_else(|error| {
                crate::log::error(format_args!("{error}; using the default settings"));
                Settings::defaults(&settings_path)
            });
            let history = Store::open(&app.path().app_data_dir()?.join("games.sqlite"));
            app.manage(Live::new(
                env!("CARGO_PKG_VERSION"),
                recordings,
                settings,
                history,
            ));
            let keys = Keys::load(Box::new(OsKeychain), |variable| {
                std::env::var(variable).ok()
            });
            app.manage(ModelsAndKeys::new(
                keys,
                Box::new(|provider, key| match provider {
                    Provider::Anthropic => Box::new(AnthropicProvider::new(key)),
                }),
            ));
            let approvals = app.path().app_config_dir()?.join("browsers.json");
            tauri::async_runtime::spawn(run_relay(app.handle().clone(), approvals));
            app.state::<Panels>().arrange(app.handle());
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while starting DCSS Copilot")
        .run(|app, event| match event {
            RunEvent::Exit => app.state::<Live>().finish_recording(),
            // Clicking the Dock icon of an app whose panel is in the tray.
            #[cfg(target_os = "macos")]
            RunEvent::Reopen { .. } => app.state::<Panels>().show(app),
            _ => {}
        });
}

/// The tray icon the app lives in: a click shows the panel, and its menu opens Settings or quits.
fn tray(app: &AppHandle) -> tauri::Result<()> {
    let menu = Menu::with_items(
        app,
        &[
            &MenuItem::with_id(app, TRAY_SHOW, "Show DCSS Copilot", true, None::<&str>)?,
            &MenuItem::with_id(app, TRAY_SETTINGS, "Settings…", true, None::<&str>)?,
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(app, TRAY_QUIT, "Quit DCSS Copilot", true, None::<&str>)?,
        ],
    )?;
    let mut tray = TrayIconBuilder::with_id("tray")
        .tooltip("DCSS Copilot")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            TRAY_SHOW => app.state::<Panels>().show(app),
            TRAY_SETTINGS => {
                if let Err(error) = open_settings(app.clone()) {
                    crate::log::error(format_args!("could not open Settings: {error}"));
                }
            }
            TRAY_QUIT => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle();
                app.state::<Panels>().show(app);
            }
        });
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    tray.build(app).map(drop)
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
        match event {
            RelayEvent::SidePanel(event) => side_panel(&app, event),
            event => {
                if let RelayEvent::Connected(browser) = &event {
                    app.state::<Panels>().connected(browser.clone());
                }
                live.handle(event);
                push_view_model(&app, &live);
            }
        }
    }
}

/// A browser opened a side panel, or the player pressed something in one.
fn side_panel(app: &AppHandle, event: SidePanelEvent) {
    match event {
        SidePanelEvent::Opened => app.state::<Panels>().side_panel_opened(app),
        SidePanelEvent::Command(request) => {
            let done = match request.command {
                PanelCommand::SetAppearance { appearance } => {
                    set_appearance(appearance, app.clone(), app.state())
                }
                PanelCommand::SetAssistance { assistance } => {
                    set_assistance(assistance, app.clone(), app.state())
                }
                PanelCommand::NewGame => new_game(app.clone(), app.state()),
                PanelCommand::OpenSettings => open_settings(app.clone()),
            };
            request.answer(done);
        }
    }
}

fn current_settings(app: &AppHandle) -> SettingsView {
    let live = app.state::<Live>();
    let start_at_login = app.autolaunch().is_enabled().unwrap_or_else(|error| {
        crate::log::error(format_args!(
            "could not tell if the app starts at login: {error}"
        ));
        false
    });
    SettingsView {
        app_version: env!("CARGO_PKG_VERSION").to_owned(),
        models: models_view(&live, &app.state::<ModelsAndKeys>()),
        window: WindowView { start_at_login },
    }
}

fn models_view(live: &Live, models: &ModelsAndKeys) -> ModelsAndKeysView {
    models.view(&live.model(ModelSlot::Quick), &live.model(ModelSlot::Deep))
}

fn push_settings(app: &AppHandle) {
    let view = current_settings(app);
    if let Err(error) = SettingsChanged(view).emit(app) {
        crate::log::error(format_args!(
            "could not push the settings to the Settings window: {error}"
        ));
    }
}

/// Pushes the view model to the panel window and the browsers.
fn push_view_model(app: &AppHandle, live: &Live) {
    app.state::<Panels>().push(app, live);
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use copilot_core::Appearance;
    use copilot_llm::{Effort, ModelChoice, ModelSlot, Provider, Thinking};
    use copilot_replay::Recording;
    use copilot_store::Store;
    use copilot_webtiles::Envelope;
    use serde_json::json;

    use super::models_view;
    use crate::live::Live;
    use copilot_llm::{LlmError, LlmProvider, LlmRequest, LlmStream};
    use futures_util::StreamExt;

    use crate::models::tests::{KEY, MemoryKeychain, WORKS, models, replaying};
    use crate::models::{Connect, KeyState, KeyTest, ModelsAndKeys};
    use crate::relay::{BrowserLink, RelayEvent};
    use crate::settings::Settings;

    /// Regenerates the bindings; CI fails if this leaves the committed file changed.
    #[test]
    fn bindings() {
        super::export_bindings().unwrap();
    }

    /// A provider whose every call fails with `error`.
    struct Failing(LlmError);

    impl LlmProvider for Failing {
        fn stream(&self, _: LlmRequest) -> LlmStream {
            futures_util::stream::once(std::future::ready(Err(self.0.clone()))).boxed()
        }
    }

    fn failing(error: LlmError) -> Connect {
        Box::new(move |_, _| Box::new(Failing(error.clone())))
    }

    /// Every message the Settings window shows about `models`' Anthropic key.
    fn key_messages(models: &ModelsAndKeys) -> Vec<String> {
        let quick = ModelChoice::default_for(ModelSlot::Quick);
        let deep = ModelChoice::default_for(ModelSlot::Deep);
        let view = models.view(&quick, &deep).keys.remove(0);
        let mut messages = vec![serde_json::to_string(&view).unwrap()];
        match view.test {
            KeyTest::Rejected { message } | KeyTest::Failed { message } => messages.push(message),
            _ => {}
        }
        if let KeyState::Stored { masked, .. } = view.key {
            messages.push(masked);
        }
        messages
    }

    #[tokio::test]
    async fn a_key_never_reaches_the_log_or_an_error_from_saving_testing_or_removing_it() {
        let mut said = Vec::new();

        // No keychain: reading keys at launch is logged, and the key is held for the session.
        let without_keychain = models(&MemoryKeychain::unusable(), replaying(WORKS));
        without_keychain.save_key(Provider::Anthropic, KEY).unwrap();
        without_keychain.remove_key(Provider::Anthropic).unwrap();

        // A provider that fails, even one that echoes the key it was called with.
        for error in [
            LlmError::Unreachable("connection refused".to_owned()),
            LlmError::BadRequest {
                status: 400,
                message: format!("x-api-key {KEY} is malformed"),
            },
        ] {
            let models = models(&MemoryKeychain::default(), failing(error));
            models.save_key(Provider::Anthropic, KEY).unwrap();
            models.test_key(Provider::Anthropic, None).await;
            said.extend(key_messages(&models));
        }

        // A keychain that won't delete the key.
        let locked = models(&MemoryKeychain::refusing_delete(), replaying(WORKS));
        locked.save_key(Provider::Anthropic, KEY).unwrap();
        said.push(
            locked
                .remove_key(Provider::Anthropic)
                .unwrap_err()
                .to_string(),
        );

        said.extend(crate::log::tests::logged());
        assert!(
            said.iter()
                .any(|line| line.contains("no Secret Service is running")),
            "{said:?}"
        );
        for line in &said {
            assert!(!line.contains(KEY), "the key reached: {line}");
        }
    }

    /// Every file under `dir`, as text: Recordings decompressed, everything else as written.
    fn written_under(dir: &Path) -> Vec<(String, String)> {
        let mut files = Vec::new();
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                files.extend(written_under(&path));
                continue;
            }
            let bytes = std::fs::read(&path).unwrap();
            let text = if path.extension().is_some_and(|ext| ext == "dcssrec") {
                format!("{:?}", Recording::read(bytes.as_slice()).unwrap())
            } else {
                String::from_utf8_lossy(&bytes).into_owned()
            };
            files.push((path.display().to_string(), text));
        }
        files
    }

    #[tokio::test]
    async fn a_key_never_reaches_settings_recordings_the_llm_sidecar_or_the_windows() {
        let dir = tempfile::tempdir().unwrap();
        let live = Live::new(
            "1.2.3",
            Some(dir.path().join("recordings")),
            Settings::load(&dir.path().join("settings.json")).unwrap(),
            Store::open(&dir.path().join("games.sqlite")),
        );
        let keychain = MemoryKeychain::default();
        let models = models(&keychain, replaying(WORKS));

        // A session that touches everything a key could leak into.
        live.handle(RelayEvent::Connected(BrowserLink::unconnected()));
        live.handle(RelayEvent::Envelope(Envelope {
            origin: "https://crawl.example.org".to_owned(),
            tab: 7,
            ts: 1_760_486_400_000,
            game_version: None,
            msg: json!({ "msg": "game_started" }),
        }));
        models.save_key(Provider::Anthropic, KEY).unwrap();
        live.set_appearance(Appearance::Glyphs).unwrap();
        let deep = ModelChoice::new(
            Provider::Anthropic,
            "claude-opus-5-5",
            Thinking::Adaptive(Effort::High),
        )
        .unwrap();
        live.set_model(ModelSlot::Deep, deep).unwrap();
        models
            .test_key(Provider::Anthropic, live.llm_sidecar())
            .await;
        let settings_window = serde_json::to_string(&models_view(&live, &models)).unwrap();
        let panel = serde_json::to_string(&live.view_model()).unwrap();
        live.finish_recording();

        let files = written_under(dir.path());
        let names: Vec<&str> = files.iter().map(|(name, _)| name.as_str()).collect();
        assert!(
            names.iter().any(|name| name.ends_with("settings.json")),
            "{names:?}"
        );
        assert!(
            names.iter().any(|name| name.ends_with(".dcssrec")),
            "{names:?}"
        );
        assert!(
            names.iter().any(|name| name.ends_with(".llm.jsonl")),
            "{names:?}"
        );
        for (name, text) in &files {
            assert!(!text.contains(KEY), "the key was written to {name}");
        }
        assert!(!settings_window.contains(KEY));
        assert!(settings_window.contains("sk-ant-…a1b2"));
        assert!(!panel.contains(KEY));
        assert!(
            !format!(
                "{:?}",
                models.view(&live.model(ModelSlot::Quick), &live.model(ModelSlot::Deep))
            )
            .contains(KEY)
        );
    }
}
