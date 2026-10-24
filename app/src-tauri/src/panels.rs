//! Where the panel shows: the app's panel window, shown and hidden as the [`Lifecycle`] decides,
//! and each connected browser, sent the view for its side panels and Toolbar Badge.

use std::sync::{Mutex, MutexGuard, PoisonError};

use tauri::{AppHandle, Manager};
use tauri_specta::Event;

use crate::ViewModelChanged;
use crate::lifecycle::{Lifecycle, Presence};
use crate::live::Live;
use crate::relay::BrowserLink;

/// The panel window's label.
pub const PANEL_WINDOW: &str = "main";

/// The app's panel window and the connected browsers.
pub struct Panels {
    state: Mutex<State>,
}

struct State {
    lifecycle: Lifecycle,
    /// Each connected browser.
    browsers: Vec<BrowserLink>,
    /// The view last sent to the browsers, so each is sent only what changed.
    sent: Option<serde_json::Value>,
}

impl Panels {
    /// Starts with the panel window `presence`, and no browser connected.
    #[must_use]
    pub fn new(presence: Presence) -> Self {
        Self {
            state: Mutex::new(State {
                lifecycle: Lifecycle::new(presence),
                browsers: Vec::new(),
                sent: None,
            }),
        }
    }

    /// `browser` connected. The next push goes to every browser, changed or not, so it reaches
    /// this one too.
    pub fn connected(&self, browser: BrowserLink) {
        let mut state = self.lock();
        state.browsers.push(browser);
        state.sent = None;
    }

    /// Sends `live`'s view model to the panel window and, if it changed, to each browser, and
    /// shows the window if it asks the player something.
    pub fn push(&self, app: &AppHandle, live: &Live) {
        let mut state = self.lock();
        // Taken under the lock, so every window and browser ends on the latest view.
        let view = live.view_model();
        if let Err(error) = ViewModelChanged(view.clone()).emit(app) {
            crate::log::error(format_args!(
                "could not push the view model to the panel: {error}"
            ));
        }
        if let Some(sent) = browser_view(&view)
            && state.sent.as_ref() != Some(&sent)
        {
            state.browsers.retain(|browser| browser.show(&sent));
            state.sent = Some(sent);
        }
        self.change(app, &mut state, |lifecycle| lifecycle.viewed(&view));
    }

    /// A browser opened a side panel, where the panel now shows.
    pub fn side_panel_opened(&self, app: &AppHandle) {
        let mut state = self.lock();
        self.change(app, &mut state, Lifecycle::side_panel_opened);
    }

    /// The player opened the panel window, from the tray or by launching the app again.
    pub fn show(&self, app: &AppHandle) {
        let mut state = self.lock();
        state.lifecycle.show();
        // Showing already or not, the player wants it in front.
        arrange(app, state.lifecycle.presence());
    }

    /// The player closed the panel window, to the tray.
    pub fn hide(&self, app: &AppHandle) {
        let mut state = self.lock();
        self.change(app, &mut state, Lifecycle::hide);
    }

    /// Shows or hides the window as the lifecycle now says, as the app starts.
    pub fn arrange(&self, app: &AppHandle) {
        let state = self.lock();
        arrange(app, state.lifecycle.presence());
    }

    /// Applies `rule` to the lifecycle, and shows or hides the window if it changed.
    fn change(&self, app: &AppHandle, state: &mut State, rule: impl FnOnce(&mut Lifecycle)) {
        let before = state.lifecycle.presence();
        rule(&mut state.lifecycle);
        if state.lifecycle.presence() != before {
            arrange(app, state.lifecycle.presence());
        }
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        // Each update leaves the state whole, so a panic elsewhere can't leave it half-changed.
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Shows or hides the panel window as `presence` says. A window that shows comes to the front,
/// so a question for the player isn't left behind the browser.
fn arrange(app: &AppHandle, presence: Presence) {
    let Some(window) = app.get_webview_window(PANEL_WINDOW) else {
        return;
    };
    match presence {
        Presence::Hidden => report(window.hide(), "hide the panel"),
        Presence::Shown => {
            report(window.show(), "show the panel");
            report(window.set_focus(), "bring the panel to the front");
        }
    }
}

/// `view` as the relay sends it to browsers, or `None`, logged, if it can't be.
fn browser_view(view: &copilot_core::ViewModel) -> Option<serde_json::Value> {
    serde_json::to_value(view)
        .map_err(|error| {
            crate::log::error(format_args!(
                "could not send the view model to the browsers: {error}"
            ));
        })
        .ok()
}

fn report(result: tauri::Result<()>, what: &str) {
    if let Err(error) = result {
        crate::log::error(format_args!("could not {what}: {error}"));
    }
}
