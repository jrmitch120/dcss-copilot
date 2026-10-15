//! The Advisor: what it knows about the game it follows, and what it shows the player.

use crate::game_source::{GameIdentity, SourceEvent};
use crate::view_model::{
    AskBox, Character, Depiction, FocusCard, GameView, Listing, NowPill, RunPlanRail,
};
use crate::visible_state::{VisibleMonster, VisibleState};

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
            SourceEvent::Visible(visible) => self.visible = *visible,
            // The open screen is in Visible State too; remembering what screens said is
            // Accumulated State's job.
            SourceEvent::ScreenRead(_) => {}
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

    /// The panel for the followed game, once a Game Source has identified it.
    ///
    /// Each part shows only what the Advisor can tell: it can't yet judge a Threat Level or make
    /// a Run Plan, so it says so rather than guess.
    #[must_use]
    pub fn game_view(&self) -> Option<GameView> {
        let identity = self.identity.as_ref()?;
        Some(GameView {
            character: Character {
                name: identity.character.clone(),
                species: identity.species.clone(),
                hp: self.visible.stats.hp,
                hp_max: self.visible.stats.hp_max,
                turn: self.visible.turn,
            },
            now: NowPill::Unrated,
            focus: FocusCard::InView {
                monsters: self
                    .visible
                    .monsters
                    .as_ref()
                    .map(|monsters| monsters.iter().map(listing).collect()),
            },
            run_plan: RunPlanRail::NotPlanned,
            ask: AskBox::NeedsModel,
        })
    }
}

/// A monster's row in the panel. Tiles aren't named yet, so the panel draws its glyph.
fn listing(monster: &VisibleMonster) -> Listing {
    Listing {
        depiction: Depiction {
            glyph: monster.glyph.clone(),
            tile: None,
        },
        name: monster.name.clone(),
        detail: None,
    }
}
