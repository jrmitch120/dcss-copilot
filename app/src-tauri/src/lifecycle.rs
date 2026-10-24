//! When the panel shows: the app lives in the tray, the panel appears when a game is detected and
//! hides after it, and collapses to the Collapsed Pill when the player wants it out of the way.

use copilot_core::{Connection, ViewModel};

/// How the panel is shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Presence {
    /// Only the tray icon shows.
    Hidden,
    /// The panel, docked beside the browser.
    Docked,
    /// The Collapsed Pill, always on top.
    Collapsed,
}

/// Decides how the panel is shown from what the core pushes and what the player does. Each rule
/// acts as its moment arrives, so the player's own choice holds until the next one: a panel they
/// hid mid-game stays hidden, and a pill they collapse again on a LETHAL moment stays collapsed.
#[derive(Debug)]
pub struct Lifecycle {
    presence: Presence,
    /// The last view had a game being played, from a connected browser.
    live: bool,
    /// The last view's moment calls the Collapsed Pill to expand.
    expands: bool,
    /// The last view asked the player something, or told them the relay couldn't start.
    asks: bool,
}

impl Lifecycle {
    /// Starts `presence`: hidden when the app starts at login, docked when the player opens it.
    #[must_use]
    pub fn new(presence: Presence) -> Self {
        Self {
            presence,
            live: false,
            expands: false,
            asks: false,
        }
    }

    /// How the panel is shown now.
    #[must_use]
    pub fn presence(&self) -> Presence {
        self.presence
    }

    /// The core pushed `view`. A game coming live shows the panel and its end hides it again; a
    /// moment that calls for the panel expands the pill; and the panel shows to ask the player
    /// "Allow this browser?" or to say the relay couldn't start.
    pub fn viewed(&mut self, view: &ViewModel) {
        let connected = matches!(view.connection, Connection::Connected { .. });
        let game = view.game.as_ref().filter(|game| connected && game.live);
        let live = game.is_some();
        let expands = game.is_some_and(|game| game.pill.expands);
        let asks = matches!(
            view.connection,
            Connection::AllowBrowser { .. } | Connection::RelayUnavailable { .. }
        );

        let appears =
            (asks && !self.asks) || (live && !self.live && self.presence == Presence::Hidden);
        if appears {
            self.presence = Presence::Docked;
        } else if !live && self.live && !asks {
            self.presence = Presence::Hidden;
        }
        if expands && !self.expands && self.presence == Presence::Collapsed {
            self.presence = Presence::Docked;
        }
        self.live = live;
        self.expands = expands;
        self.asks = asks;
    }

    /// The player collapsed the panel to the pill.
    pub fn collapse(&mut self) {
        self.presence = Presence::Collapsed;
    }

    /// The player opened the panel, from the pill or the tray.
    pub fn show(&mut self) {
        self.presence = Presence::Docked;
    }

    /// The player closed the panel, to the tray.
    pub fn hide(&mut self) {
        self.presence = Presence::Hidden;
    }
}

#[cfg(test)]
mod tests {
    use copilot_core::{Appearance, ViewModel};
    use copilot_replay::replay;

    use super::*;

    const SESSION: &[u8] = include_bytes!(
        "../../../crates/replay/tests/recordings/session-tiv-reloads-saves-and-dies.dcssrec"
    );
    const JACKALS_FALL: &[u8] =
        include_bytes!("../../../crates/replay/tests/recordings/jackals-close-in-and-fall.dcssrec");

    /// Each change of presence as `views` are pushed, with the game's turn then.
    fn presences(
        lifecycle: &mut Lifecycle,
        views: Vec<ViewModel>,
        mut player: impl FnMut(&mut Lifecycle, Option<u32>),
    ) -> Vec<(Option<u32>, Presence)> {
        let mut changes = Vec::new();
        for view in views {
            let turn = view.game.as_ref().and_then(|game| game.character.turn);
            let before = lifecycle.presence();
            lifecycle.viewed(&view);
            if lifecycle.presence() != before {
                changes.push((turn, lifecycle.presence()));
                player(lifecycle, turn);
            }
        }
        changes
    }

    #[test]
    fn the_panel_appears_when_a_game_is_detected_and_hides_after_it_stops_or_ends() {
        let mut lifecycle = Lifecycle::new(Presence::Hidden);

        let changes = presences(
            &mut lifecycle,
            replay(SESSION).unwrap().view_models,
            |_, _| {},
        );

        // Joined, saved, played on from the save, died, and a new character.
        assert_eq!(
            changes,
            [
                (Some(1234), Presence::Docked),
                (Some(1300), Presence::Hidden),
                (Some(1300), Presence::Docked),
                (Some(1410), Presence::Hidden),
                (Some(0), Presence::Docked),
            ]
        );
    }

    #[test]
    fn the_collapsed_pill_expands_itself_as_a_lethal_moment_with_no_trusted_way_out_arrives() {
        let mut lifecycle = Lifecycle::new(Presence::Hidden);

        // The player collapses the panel each time it shows.
        let changes = presences(
            &mut lifecycle,
            replay(JACKALS_FALL).unwrap().view_models,
            |lifecycle, _| {
                if lifecycle.presence() == Presence::Docked {
                    lifecycle.collapse();
                }
            },
        );

        // Turn 103 turns LETHAL with nowhere to go; it stays so until the last jackal falls at
        // 105, and the pill collapsed again stays collapsed.
        let first_turn = changes[0].0;
        assert_eq!(
            changes,
            [
                (first_turn, Presence::Docked),
                (Some(103), Presence::Docked)
            ]
        );
    }

    #[test]
    fn a_panel_the_player_hid_mid_game_stays_hidden_until_the_next_game() {
        let mut lifecycle = Lifecycle::new(Presence::Hidden);
        let views = replay(SESSION).unwrap().view_models;
        let mut hid = false;

        let changes = presences(&mut lifecycle, views, |lifecycle, _| {
            if !hid {
                lifecycle.hide();
                hid = true;
            }
        });

        // Hidden as it first showed, it shows again only as the saved game is played on.
        assert_eq!(
            changes,
            [
                (Some(1234), Presence::Docked),
                (Some(1300), Presence::Docked),
                (Some(1410), Presence::Hidden),
                (Some(0), Presence::Docked),
            ]
        );
    }

    #[test]
    fn the_panel_shows_to_ask_the_player_and_hides_when_the_browser_goes() {
        let mut lifecycle = Lifecycle::new(Presence::Hidden);
        let mut view = ViewModel::initial("1.2.3", Appearance::Tiles);

        view.connection = Connection::AllowBrowser {
            browser: "chrome-extension://abc".to_owned(),
        };
        lifecycle.viewed(&view);
        assert_eq!(lifecycle.presence(), Presence::Docked);

        // Answered with no game yet: the panel stays until the player closes it.
        view.connection = Connection::Connected {
            buffer_overflowed: false,
        };
        lifecycle.viewed(&view);
        assert_eq!(lifecycle.presence(), Presence::Docked);
        lifecycle.hide();

        view.game = replay(SESSION)
            .unwrap()
            .view_models
            .into_iter()
            .find_map(|view| view.game);
        lifecycle.viewed(&view);
        assert_eq!(lifecycle.presence(), Presence::Docked);

        view.connection = Connection::NoBrowser;
        lifecycle.viewed(&view);
        assert_eq!(lifecycle.presence(), Presence::Hidden);

        view.connection = Connection::RelayUnavailable {
            reason: "Port 47821 is in use".to_owned(),
        };
        lifecycle.viewed(&view);
        assert_eq!(lifecycle.presence(), Presence::Docked);
    }
}
