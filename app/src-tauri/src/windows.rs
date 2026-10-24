//! The panel's windows: the docked panel and the Collapsed Pill, placed beside the browser window
//! holding the game, shown and hidden as the [`Lifecycle`] decides.

use std::sync::{Mutex, MutexGuard, PoisonError};

use copilot_core::ViewModel;
use copilot_webtiles::relay_protocol::{WindowBounds, WindowState};
use serde::Serialize;
use tauri::{AppHandle, LogicalPosition, LogicalSize, Manager, WebviewWindow};

use crate::dock::{self, Rect};
use crate::lifecycle::{Lifecycle, Presence};
use crate::live::Live;
use crate::relay::BrowserWindow;

/// The docked panel's window label.
pub const PANEL_WINDOW: &str = "main";
/// The Collapsed Pill's window label, which also names its route in the webview.
pub const PILL_WINDOW: &str = "pill";

/// The panel's width when its window can't be measured.
const PANEL_WIDTH: f64 = 420.0;

/// Whether the panel docks beside the browser, or floats where the desktop puts it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(
    tag = "state",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Docking {
    /// It follows the browser window, from the bounds the extension reports.
    Docks,
    /// It is a free floating window, as the desktop won't let apps place their windows.
    Floats {
        /// Why, for the player.
        reason: String,
    },
}

impl Docking {
    /// Whether this desktop lets the panel dock, judged from its environment `variable`s. Linux
    /// under Wayland lets no app read or set where windows are, so the panel floats there, unless
    /// GTK is made to run through XWayland (`GDK_BACKEND=x11`).
    #[must_use]
    pub fn detect(variable: impl Fn(&str) -> Option<String>) -> Self {
        let set = |name| variable(name).is_some_and(|value| !value.is_empty());
        let on_x11 = variable("GDK_BACKEND").is_some_and(|backend| backend.starts_with("x11"));
        if cfg!(target_os = "linux") && set("WAYLAND_DISPLAY") && !on_x11 {
            Self::Floats {
                reason: "Wayland doesn't let apps place their windows, so the panel floats \
                         wherever you put it."
                    .to_owned(),
            }
        } else {
            Self::Docks
        }
    }
}

/// The panel's windows and what places them.
pub struct Windows {
    docking: Docking,
    state: Mutex<State>,
}

struct State {
    lifecycle: Lifecycle,
    /// The browser window holding the game, as last reported.
    browser: Option<BrowserWindow>,
    /// The browser bounds Make Room last shrank the browser from, so it asks once for each.
    made_room_from: Option<WindowBounds>,
}

impl Windows {
    /// Starts `presence`, docking as `docking` allows.
    #[must_use]
    pub fn new(presence: Presence, docking: Docking) -> Self {
        Self {
            docking,
            state: Mutex::new(State {
                lifecycle: Lifecycle::new(presence),
                browser: None,
                made_room_from: None,
            }),
        }
    }

    /// Whether the panel docks or floats.
    pub fn docking(&self) -> &Docking {
        &self.docking
    }

    /// The core pushed `view`.
    pub fn viewed(&self, app: &AppHandle, view: &ViewModel) {
        let mut state = self.lock();
        let before = state.lifecycle.presence();
        state.lifecycle.viewed(view);
        if state.lifecycle.presence() != before {
            self.arrange(app, &mut state);
        }
    }

    /// The browser reported where its game window is now.
    pub fn browser_moved(&self, app: &AppHandle, window: BrowserWindow) {
        let mut state = self.lock();
        state.browser = Some(window);
        self.arrange(app, &mut state);
    }

    /// The player collapsed the panel to the pill.
    pub fn collapse(&self, app: &AppHandle) {
        let mut state = self.lock();
        state.lifecycle.collapse();
        self.arrange(app, &mut state);
    }

    /// The player opened the panel, from the pill or the tray.
    pub fn show(&self, app: &AppHandle) {
        let mut state = self.lock();
        state.lifecycle.show();
        self.arrange(app, &mut state);
        if let Some(panel) = app.get_webview_window(PANEL_WINDOW) {
            report(panel.set_focus(), "focus the panel");
        }
    }

    /// The player closed the panel, to the tray.
    pub fn hide(&self, app: &AppHandle) {
        let mut state = self.lock();
        state.lifecycle.hide();
        self.arrange(app, &mut state);
    }

    /// Places the windows afresh: as the app starts, or the player changed how the panel docks.
    /// Make Room may then shrink the browser again, even from bounds it shrank it from before.
    pub fn rearrange(&self, app: &AppHandle) {
        let mut state = self.lock();
        state.made_room_from = None;
        self.arrange(app, &mut state);
    }

    /// Shows, hides and places the windows as the panel's presence now says.
    fn arrange(&self, app: &AppHandle, state: &mut State) {
        let (Some(panel), Some(pill)) = (
            app.get_webview_window(PANEL_WINDOW),
            app.get_webview_window(PILL_WINDOW),
        ) else {
            return;
        };
        let (side, make_room) = app.state::<Live>().docking();
        let browser = state
            .browser
            .as_ref()
            .filter(|_| self.docking == Docking::Docks);
        // The panel goes with a minimized browser, and comes back with it.
        let minimized = browser.is_some_and(|browser| browser.state == WindowState::Minimized);
        let browser = browser.filter(|_| !minimized).and_then(|browser| {
            let bounds = Rect::from(browser.bounds);
            Some((browser, bounds, dock::screen_of(bounds, &screens(app))?))
        });
        let presence = if minimized {
            Presence::Hidden
        } else {
            state.lifecycle.presence()
        };
        match presence {
            Presence::Hidden => {
                report(panel.hide(), "hide the panel");
                report(pill.hide(), "hide the pill");
            }
            Presence::Docked => {
                if let Some((browser, bounds, screen)) = browser {
                    // A fullscreen browser is the player's choice not to share the screen.
                    let make_room = make_room && browser.state != WindowState::Fullscreen;
                    let docked = dock::dock(bounds, screen, side, width(&panel), make_room);
                    place(&panel, docked.panel);
                    report(
                        panel.set_always_on_top(docked.over_browser),
                        "keep the panel on top",
                    );
                    if let Some(room) = docked.make_room
                        && state.made_room_from != Some(browser.bounds)
                    {
                        state.made_room_from = Some(browser.bounds);
                        browser.browser.resize(room);
                    }
                }
                report(pill.hide(), "hide the pill");
                show_unfocused(&panel);
            }
            Presence::Collapsed => {
                if let Some((_, bounds, screen)) = browser {
                    place(&pill, dock::pill(bounds, screen, side));
                }
                report(panel.hide(), "hide the panel");
                show_unfocused(&pill);
            }
        }
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        // Each update leaves the state whole, so a panic elsewhere can't leave it half-changed.
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Every screen's work area, in logical pixels.
fn screens(app: &AppHandle) -> Vec<Rect> {
    let monitors = app.available_monitors().unwrap_or_else(|error| {
        crate::log::error(format_args!("could not list the screens: {error}"));
        Vec::new()
    });
    monitors
        .iter()
        .map(|monitor| {
            let area = monitor.work_area();
            let position = area.position.to_logical::<f64>(monitor.scale_factor());
            let size = area.size.to_logical::<f64>(monitor.scale_factor());
            Rect {
                left: position.x,
                top: position.y,
                width: size.width,
                height: size.height,
            }
        })
        .collect()
}

/// The panel's outer width now, which the player may have changed.
fn width(panel: &WebviewWindow) -> f64 {
    panel
        .outer_size()
        .and_then(|size| Ok(size.to_logical::<f64>(panel.scale_factor()?).width))
        .unwrap_or(PANEL_WIDTH)
}

/// Moves and resizes `window` so its outer bounds are `rect`.
fn place(window: &WebviewWindow, rect: Rect) {
    // Window sizes are set inside the frame, which the title bar and borders add to.
    let frame = window.scale_factor().and_then(|scale| {
        let outer = window.outer_size()?.to_logical::<f64>(scale);
        let inner = window.inner_size()?.to_logical::<f64>(scale);
        Ok((outer.width - inner.width, outer.height - inner.height))
    });
    let (frame_width, frame_height) = frame.unwrap_or((0.0, 0.0));
    report(
        window.set_size(LogicalSize::new(
            rect.width - frame_width,
            rect.height - frame_height,
        )),
        "resize a window",
    );
    report(
        window.set_position(LogicalPosition::new(rect.left, rect.top)),
        "move a window",
    );
}

/// Shows `window` without taking the keyboard from the game, where the desktop allows.
fn show_unfocused(window: &WebviewWindow) {
    if window.is_visible().unwrap_or(false) {
        return;
    }
    report(window.set_focusable(false), "show a window unfocused");
    report(window.show(), "show a window");
    report(window.set_focusable(true), "make a window focusable");
}

fn report(result: tauri::Result<()>, what: &str) {
    if let Err(error) = result {
        crate::log::error(format_args!("could not {what}: {error}"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn environment(variables: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let variables: Vec<(String, String)> = variables
            .iter()
            .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
            .collect();
        move |name| {
            variables
                .iter()
                .find(|(variable, _)| variable == name)
                .map(|(_, value)| value.clone())
        }
    }

    #[test]
    fn the_panel_floats_only_under_wayland_on_linux() {
        let wayland = Docking::detect(environment(&[("WAYLAND_DISPLAY", "wayland-0")]));
        let xwayland = Docking::detect(environment(&[
            ("WAYLAND_DISPLAY", "wayland-0"),
            ("GDK_BACKEND", "x11"),
        ]));
        let x11 = Docking::detect(environment(&[("DISPLAY", ":0")]));

        if cfg!(target_os = "linux") {
            assert!(matches!(wayland, Docking::Floats { .. }));
        } else {
            assert_eq!(wayland, Docking::Docks);
        }
        assert_eq!(xwayland, Docking::Docks);
        assert_eq!(x11, Docking::Docks);
    }
}
