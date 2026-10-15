//! The Advisor: what it knows about the game it follows, and what it shows the player.

use copilot_gk::{Bundle, Library};

use crate::fact_sheet::{FactSheet, Threat, ThreatAssessment};
use crate::game_source::{GameIdentity, SourceEvent};
use crate::view_model::{
    AskBox, Character, Depiction, FocusCard, GameView, Listing, NowPill, RunPlanRail,
};
use crate::visible_state::{VisibleMonster, VisibleState};

/// Follows one Game Source's game and turns what it learns into the Fact Sheet and the view
/// model.
#[derive(Debug, Clone)]
pub struct Advisor {
    knowledge: Library,
    identity: Option<GameIdentity>,
    visible: VisibleState,
    fact_sheet: FactSheet,
}

impl Advisor {
    /// An Advisor following no game yet, judging games with the Game Knowledge in `knowledge`.
    #[must_use]
    pub fn new(knowledge: Library) -> Self {
        let visible = VisibleState::default();
        Self {
            fact_sheet: FactSheet::compute(&visible, None),
            knowledge,
            identity: None,
            visible,
        }
    }

    /// Takes in what the Game Source learned.
    pub fn apply(&mut self, event: SourceEvent) {
        match event {
            SourceEvent::Identified(identity) => self.identity = Some(identity),
            SourceEvent::Visible(visible) => self.visible = *visible,
            // The open screen is in Visible State too. Remembering what screens said, and what a
            // game's sessions mean for what is remembered of it, is Accumulated State's job.
            SourceEvent::ScreenRead(_) | SourceEvent::Session(_) => return,
        }
        self.fact_sheet = FactSheet::compute(&self.visible, self.game_knowledge());
    }

    /// The Game Knowledge for the followed game's Crawl Version, if the Advisor has it.
    fn game_knowledge(&self) -> Option<&Bundle> {
        self.identity.as_ref()?.knowledge(&self.knowledge)
    }

    /// What the engine computed about this moment.
    #[must_use]
    pub fn fact_sheet(&self) -> &FactSheet {
        &self.fact_sheet
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
    /// Each part shows only what the Advisor can tell: without the facts to judge a Threat Level,
    /// and with no Run Plan yet, it says so rather than guess.
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
            now: now_pill(&self.fact_sheet.threat),
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

/// The smallest chance of death the now pill puts a number on, in percent: below it, the
/// Monte Carlo's sampling error is as large as the number.
const DEATH_SHOWN_FROM_PERCENT: f64 = 0.1;

/// The now pill for a moment's threat.
fn now_pill(threat: &Threat) -> NowPill {
    match threat {
        Threat::Unrated { .. } => NowPill::Unrated,
        Threat::Rated(assessment) => NowPill::Rated {
            level: assessment.level,
            line: threat_line(assessment),
            detail: threat_detail(assessment),
        },
    }
}

/// One sentence saying what threatens the player before their next action.
fn threat_line(assessment: &ThreatAssessment) -> String {
    match assessment.acting.as_slice() {
        [] if assessment.hostiles_in_view == 0 => "No hostiles in view.".to_owned(),
        [] => "Nothing hostile can reach you before your next action.".to_owned(),
        [one] => format!(
            "{} can attack before your next action.",
            capitalised(&one.name)
        ),
        many => format!(
            "{} monsters can attack before your next action.",
            many.len()
        ),
    }
}

/// The numbers behind the level, when anything can attack.
fn threat_detail(assessment: &ThreatAssessment) -> Option<String> {
    if assessment.acting.is_empty() {
        return None;
    }
    let percent = assessment.death.percent();
    let death = if percent < DEATH_SHOWN_FROM_PERCENT {
        format!("under {DEATH_SHOWN_FROM_PERCENT}%")
    } else {
        format!("{percent:.1}%")
    };
    Some(format!(
        "Death {death} · worst hit {} of {} HP",
        assessment.worst_hit, assessment.hp
    ))
}

fn capitalised(name: &str) -> String {
    let mut chars = name.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(chars).collect()
    })
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
