//! The Advisor: what it knows about the game it follows, and what it shows the player.

use std::collections::HashMap;
use std::sync::OnceLock;

use copilot_gk::{Bundle, Library, Monster};

use crate::accumulated_state::AccumulatedState;
use crate::fact_sheet::{FactSheet, Threat, ThreatAssessment, attacks_player};
use crate::game_source::{GameIdentity, SourceEvent};
use crate::view_model::{
    AskBox, Character, Depiction, FocusCard, GameView, Listing, NowPill, RunPlanRail, TileName,
    capitalised,
};
use crate::visible_state::{VisibleMonster, VisibleState};

/// Follows one Game Source's game and turns what it learns into the Fact Sheet and the view
/// model.
#[derive(Debug, Clone)]
pub struct Advisor {
    knowledge: Library,
    identity: Option<GameIdentity>,
    visible: VisibleState,
    /// What is remembered of each game followed so far, by its game key.
    remembered: HashMap<GameKey, AccumulatedState>,
    /// The Fact Sheet for this moment, computed when first asked for: playing a fight out is
    /// costly, and many events pass before anything reads it.
    fact_sheet: OnceLock<FactSheet>,
}

/// What tells one game apart from every other, for as long as it is played: the spec's game key.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct GameKey {
    server: String,
    account: Option<String>,
    character: String,
    species: String,
    first_seen_ms: u64,
}

impl GameKey {
    fn of(identity: &GameIdentity) -> Self {
        Self {
            server: identity.server.clone(),
            account: identity.account.clone(),
            character: identity.character.clone(),
            species: identity.species.clone(),
            first_seen_ms: identity.first_seen_ms,
        }
    }
}

impl Advisor {
    /// An Advisor following no game yet, judging games with the Game Knowledge in `knowledge`.
    #[must_use]
    pub fn new(knowledge: Library) -> Self {
        Self {
            fact_sheet: OnceLock::new(),
            knowledge,
            identity: None,
            visible: VisibleState::default(),
            remembered: HashMap::new(),
        }
    }

    /// Takes in what the Game Source learned.
    pub fn apply(&mut self, event: SourceEvent) {
        match event {
            SourceEvent::Identified(identity) => {
                self.remembered.entry(GameKey::of(&identity)).or_default();
                self.identity = Some(identity);
            }
            SourceEvent::Visible(visible) => {
                let knowledge = self
                    .identity
                    .as_ref()
                    .and_then(|identity| identity.knowledge(&self.knowledge));
                if let Some(remembered) = self
                    .identity
                    .as_ref()
                    .map(GameKey::of)
                    .and_then(|key| self.remembered.get_mut(&key))
                {
                    remembered.update(&self.visible, &visible, knowledge);
                }
                self.visible = *visible;
            }
            SourceEvent::ScreenRead(read) => {
                let knowledge = self
                    .identity
                    .as_ref()
                    .and_then(|identity| identity.knowledge(&self.knowledge));
                if let Some(remembered) = self
                    .followed_key()
                    .and_then(|key| self.remembered.get_mut(&key))
                {
                    remembered.read(&read, &self.visible, knowledge);
                }
            }
            // A game's sessions don't change what is remembered of it: a resynced game is the
            // same game, and a new game has its own game key. Nothing judges a Settled Update yet.
            SourceEvent::Session(_) | SourceEvent::Settled => return,
        }
        self.fact_sheet = OnceLock::new();
    }

    /// What is remembered of the followed game, once a Game Source has identified it.
    #[must_use]
    pub fn accumulated(&self) -> Option<&AccumulatedState> {
        self.remembered.get(&self.followed_key()?)
    }

    fn followed_key(&self) -> Option<GameKey> {
        self.identity.as_ref().map(GameKey::of)
    }

    /// The Game Knowledge for the followed game's Crawl Version, if the Advisor has it.
    fn game_knowledge(&self) -> Option<&Bundle> {
        self.identity.as_ref()?.knowledge(&self.knowledge)
    }

    /// What the engine computed about this moment.
    #[must_use]
    pub fn fact_sheet(&self) -> &FactSheet {
        self.fact_sheet.get_or_init(|| {
            let species = self
                .identity
                .as_ref()
                .map(|identity| identity.species.as_str());
            FactSheet::compute(
                &self.visible,
                self.accumulated(),
                species,
                self.game_knowledge(),
            )
        })
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
            now: now_pill(&self.fact_sheet().threat),
            focus: self.focus(),
            ways_out: self
                .hostile_in_view()
                .then(|| self.fact_sheet().ways_out.list()),
            run_plan: RunPlanRail::NotPlanned,
            ask: AskBox::NeedsModel,
        })
    }
}

impl Advisor {
    /// The focus card: a small fight while anything hostile is in view, and what is in view
    /// otherwise.
    fn focus(&self) -> FocusCard {
        let monsters = self
            .visible
            .monsters
            .as_ref()
            .map(|monsters| in_view(monsters, self.game_knowledge()));
        let fight = self
            .hostile_in_view()
            .then(|| self.fact_sheet().fight.card(monsters.clone()))
            .flatten();
        fight.unwrap_or(FocusCard::InView { monsters })
    }

    /// Whether a monster in view would attack the player, other than Firewood, or one whose
    /// side or kind the Advisor can't tell, which might.
    fn hostile_in_view(&self) -> bool {
        let knowledge = self.game_knowledge();
        self.visible.monsters.iter().flatten().any(|monster| {
            let firewood = monster
                .id
                .as_ref()
                .and_then(|id| knowledge?.monster_by_id(id))
                .is_some_and(|kind| kind.firewood);
            monster.attitude.is_none_or(attacks_player) && !firewood
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

/// One sentence saying what threatens the player before their next action, and another when
/// a fight with what is in view looks doubtful.
fn threat_line(assessment: &ThreatAssessment) -> String {
    let line = acting_line(assessment);
    match assessment
        .best_plan_win
        .filter(|_| assessment.fight_doubtful())
    {
        Some(win) => format!(
            "{line} Your best plan wins only ~{:.0}% of fights.",
            win.percent()
        ),
        None => line,
    }
}

/// One sentence saying which monsters can attack before the player's next action.
fn acting_line(assessment: &ThreatAssessment) -> String {
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

/// The numbers behind the level when anything can attack, and which monsters' gear they guess
/// from their kind.
fn threat_detail(assessment: &ThreatAssessment) -> Option<String> {
    let numbers = (!assessment.acting.is_empty()).then(|| {
        let percent = assessment.death.percent();
        let death = if percent < DEATH_SHOWN_FROM_PERCENT {
            format!("under {DEATH_SHOWN_FROM_PERCENT}%")
        } else {
            format!("{percent:.1}%")
        };
        format!(
            "Death {death} · worst hit {} of {} HP",
            assessment.worst_hit, assessment.hp
        )
    });
    let shooters = may_shoot_note(assessment);
    match (numbers, shooters) {
        (Some(numbers), Some(shooters)) => Some(format!("{numbers} · {shooters}")),
        (numbers, shooters) => numbers.or(shooters),
    }
}

/// Which monsters may shoot or zap, whose gear the numbers guess from what crawl gives their
/// kind, as their Carried Gear isn't read: each kind named once.
fn may_shoot_note(assessment: &ThreatAssessment) -> Option<String> {
    let mut names: Vec<&str> = Vec::new();
    for monster in &assessment.gear {
        if monster.guessed_shooter() && !names.contains(&monster.name.as_str()) {
            names.push(&monster.name);
        }
    }
    if names.is_empty() {
        return None;
    }
    Some(format!("Gear guessed from kind: {}", names.join(", ")))
}

/// The rows listing `monsters` in view: Firewood left out, since the player sees it as scenery,
/// and identical monsters in one row. Without `knowledge` nothing is known to be Firewood, so
/// nothing is left out.
fn in_view(monsters: &[VisibleMonster], knowledge: Option<&Bundle>) -> Vec<Listing> {
    Listing::grouped(
        monsters
            .iter()
            .map(|monster| {
                let kind = monster
                    .id
                    .as_ref()
                    .and_then(|id| knowledge?.monster_by_id(id));
                (monster, kind)
            })
            .filter(|(_, kind)| kind.is_none_or(|kind| !kind.firewood))
            .map(|(monster, kind)| listing(monster, kind)),
    )
}

/// A monster's row in the panel, with its tile when its `kind` names one.
fn listing(monster: &VisibleMonster, kind: Option<&Monster>) -> Listing {
    Listing {
        depiction: Depiction {
            glyph: monster.glyph.clone(),
            tile: kind.and_then(|kind| kind.tile.clone()).map(TileName),
        },
        name: monster.name.clone(),
        detail: None,
        count: 1,
    }
}
