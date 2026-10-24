//! When the app's panel window shows. The app lives in the tray, and a webtiles game's panel shows
//! in the browser's side panel, so the window shows when the player opens it or the app must ask
//! them something.

use copilot_core::{Connection, ViewModel};

/// Whether the app's panel window shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Presence {
    /// Only the tray icon shows.
    Hidden,
    /// The panel window shows.
    Shown,
}

/// Decides whether the panel window shows from what the core pushes, what the browsers' side
/// panels do and what the player does. Each rule acts as its moment arrives, so the player's own
/// choice holds until the next one.
#[derive(Debug)]
pub struct Lifecycle {
    presence: Presence,
    /// The last view asked the player something, or told them the relay couldn't start.
    asks: bool,
    /// The player opened the window themselves, so it stays until they close it.
    opened_by_player: bool,
}

impl Lifecycle {
    /// Starts `presence`: hidden when the app starts at login, shown when the player opens it.
    #[must_use]
    pub fn new(presence: Presence) -> Self {
        Self {
            presence,
            asks: false,
            opened_by_player: false,
        }
    }

    /// Whether the panel window shows now.
    #[must_use]
    pub fn presence(&self) -> Presence {
        self.presence
    }

    /// The core pushed `view`. The window shows to ask the player "Allow this browser?", which
    /// only the app may answer, or to say the relay couldn't start.
    pub fn viewed(&mut self, view: &ViewModel) {
        let asks = matches!(
            view.connection,
            Connection::AllowBrowser { .. } | Connection::RelayUnavailable { .. }
        );
        if asks && !self.asks {
            self.presence = Presence::Shown;
        }
        self.asks = asks;
    }

    /// A browser's side panel opened, or is open as the browser reconnects, so the panel shows
    /// there and the window gives way: unless it is asking the player something, or the player
    /// opened it themselves.
    pub fn side_panel_opened(&mut self) {
        if !self.asks && !self.opened_by_player {
            self.presence = Presence::Hidden;
        }
    }

    /// The player opened the panel window, from the tray or by launching the app again.
    pub fn show(&mut self) {
        self.presence = Presence::Shown;
        self.opened_by_player = true;
    }

    /// The player closed the panel window, to the tray.
    pub fn hide(&mut self) {
        self.presence = Presence::Hidden;
        self.opened_by_player = false;
    }
}

#[cfg(test)]
mod tests {
    use copilot_core::{Appearance, AssistanceLevel, ViewModel};
    use copilot_replay::replay;

    use super::*;

    const SESSION: &[u8] = include_bytes!(
        "../../../crates/replay/tests/recordings/session-tiv-reloads-saves-and-dies.dcssrec"
    );

    #[test]
    fn a_game_shows_in_the_side_panel_and_leaves_the_window_in_the_tray() {
        let mut lifecycle = Lifecycle::new(Presence::Hidden);

        // Joined, saved, played on from the save, died, and a new character.
        for view in replay(SESSION).unwrap().view_models {
            lifecycle.viewed(&view);
            assert_eq!(lifecycle.presence(), Presence::Hidden);
        }
    }

    #[test]
    fn the_window_shows_to_ask_the_player_and_gives_way_to_a_side_panel_after() {
        let mut lifecycle = Lifecycle::new(Presence::Hidden);
        let mut view = ViewModel::initial("1.2.3", Appearance::Tiles, AssistanceLevel::Hints);

        view.connection = Connection::AllowBrowser {
            browser: "chrome-extension://abc".to_owned(),
        };
        lifecycle.viewed(&view);
        assert_eq!(lifecycle.presence(), Presence::Shown);

        // Only the app answers, so a side panel opening meanwhile doesn't hide the question.
        lifecycle.side_panel_opened();
        assert_eq!(lifecycle.presence(), Presence::Shown);

        view.connection = Connection::Connected {
            buffer_overflowed: false,
        };
        lifecycle.viewed(&view);
        assert_eq!(lifecycle.presence(), Presence::Shown);
        lifecycle.side_panel_opened();
        assert_eq!(lifecycle.presence(), Presence::Hidden);

        view.connection = Connection::RelayUnavailable {
            reason: "Port 47821 is in use".to_owned(),
        };
        lifecycle.viewed(&view);
        assert_eq!(lifecycle.presence(), Presence::Shown);
    }

    #[test]
    fn the_window_the_app_started_with_gives_way_to_a_side_panel_but_one_the_player_opened_stays() {
        let mut lifecycle = Lifecycle::new(Presence::Shown);

        lifecycle.side_panel_opened();
        assert_eq!(lifecycle.presence(), Presence::Hidden);

        // Opened from the tray, it stays through the browser reconnecting.
        lifecycle.show();
        lifecycle.side_panel_opened();
        assert_eq!(lifecycle.presence(), Presence::Shown);
    }
}
