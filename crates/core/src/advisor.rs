//! The Advisor: what it knows about the game it follows, and what it shows the player.

use crate::game_source::{GameIdentity, SourceEvent, VisibleState};
use crate::view_model::{Character, Connection, ViewModel};

/// Follows one Game Source's game and turns what it learns into the view model.
#[derive(Debug, Clone, Default)]
pub struct Advisor {
    identity: Option<GameIdentity>,
    visible: VisibleState,
}

impl Advisor {
    /// An Advisor following no game yet.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Takes in what the Game Source learned.
    pub fn apply(&mut self, event: SourceEvent) {
        match event {
            SourceEvent::Identified(identity) => self.identity = Some(identity),
            SourceEvent::Visible(visible) => self.visible = visible,
        }
    }

    /// The game being followed, once a Game Source has identified it.
    #[must_use]
    pub fn identity(&self) -> Option<&GameIdentity> {
        self.identity.as_ref()
    }

    /// What the game currently reports.
    #[must_use]
    pub fn visible(&self) -> &VisibleState {
        &self.visible
    }

    /// Everything the panel shows, given the app's version and its connection to the browser.
    #[must_use]
    pub fn view_model(&self, app_version: &str, connection: Connection) -> ViewModel {
        ViewModel {
            app_version: app_version.to_owned(),
            connection,
            character: self.identity.as_ref().map(|identity| Character {
                name: identity.character.clone(),
                species: identity.species.clone(),
                hp: self.visible.hp,
                hp_max: self.visible.hp_max,
                turn: self.visible.turn,
            }),
        }
    }
}
