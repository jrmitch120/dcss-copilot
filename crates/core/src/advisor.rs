//! The Advisor: what it knows about the game it follows, and what it shows the player.

use std::collections::HashMap;
use std::sync::OnceLock;

use copilot_gk::{Bundle, Library, Monster};

use crate::accumulated_state::AccumulatedState;
use crate::detectors::{Alert, Detection, Detectors};
use crate::exceptional::{Exceptional, ExceptionalMonster};
use crate::fact_sheet::{FactSheet, Threat, ThreatAssessment, ThreatLevel, attacks_player};
use crate::game_source::{GameIdentity, SessionEvent, SourceEvent};
use crate::known_games::GameKey;
use crate::suggested_action::SuggestedAction;
use crate::view_model::{
    AskBox, AssistanceLevel, Character, Depiction, FactCard, FocusCard, GameView, Listing, NowPill,
    RunPlanRail, TileName, ToolbarBadge, capitalised,
};
use crate::visible_state::{VisibleMonster, VisibleState};

/// Follows one Game Source's game and turns what it learns into the Fact Sheet and the view
/// model.
#[derive(Debug, Clone)]
pub struct Advisor {
    knowledge: Library,
    identity: Option<GameIdentity>,
    visible: VisibleState,
    /// What is kept of each game followed so far, by its game key.
    games: HashMap<GameKey, Followed>,
    /// The Fact Sheet for this moment, computed when first asked for: playing a fight out is
    /// costly, and many events pass before anything reads it.
    fact_sheet: OnceLock<FactSheet>,
}

/// What the Advisor keeps of one game it has followed.
#[derive(Debug, Clone, Default)]
struct Followed {
    /// What it remembers of the game.
    accumulated: AccumulatedState,
    /// What its detectors remember.
    detectors: Detectors,
    /// The last Alert raised, with the turn it was raised on: the now pill shows it until the
    /// player acts.
    alert: Option<(Alert, Option<u32>)>,
    /// Its stream stopped or it ended, and it hasn't been picked up since.
    stopped: bool,
    /// Its turn when the Advisor last saw it.
    turn: Option<u32>,
    /// It was picked up again, last seen on this turn, and the Advisor hasn't seen it since.
    picked_up_from: Option<LastSeen>,
}

/// When the Advisor last saw a game it picks up again.
#[derive(Debug, Clone, Copy)]
struct LastSeen {
    turn: Option<u32>,
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
            games: HashMap::new(),
        }
    }

    /// Takes in what the Game Source learned, and returns what the detectors noticed: at a
    /// Settled Update, and only then, they judge it against the one before.
    pub fn apply(&mut self, event: SourceEvent) -> Vec<Detection> {
        match event {
            SourceEvent::Identified(identity) => {
                self.games.entry(identity.key()).or_default();
                self.identity = Some(identity);
            }
            SourceEvent::Visible(visible) => {
                let knowledge = self
                    .identity
                    .as_ref()
                    .and_then(|identity| identity.knowledge(&self.knowledge));
                if let Some(game) = self
                    .identity
                    .as_ref()
                    .map(GameIdentity::key)
                    .and_then(|key| self.games.get_mut(&key))
                {
                    match game.picked_up_from.take() {
                        Some(last) => game.accumulated.pick_up(last.turn, &visible),
                        None => game.accumulated.update(&self.visible, &visible, knowledge),
                    }
                    game.turn = visible.turn;
                }
                self.visible = *visible;
            }
            SourceEvent::ScreenRead(read) => {
                let knowledge = self
                    .identity
                    .as_ref()
                    .and_then(|identity| identity.knowledge(&self.knowledge));
                if let Some(game) = self.followed_key().and_then(|key| self.games.get_mut(&key)) {
                    game.accumulated.read(&read, &self.visible, knowledge);
                }
            }
            // A new game has a key of its own; a game resynced is picked up again.
            SourceEvent::Session(session) => {
                if let Some(game) = self.followed_game_mut() {
                    game.stopped = matches!(
                        session,
                        SessionEvent::ConnectionLost | SessionEvent::GameEnded { .. }
                    );
                    if session == SessionEvent::Resynced {
                        let turn = game.turn;
                        game.picked_up_from.get_or_insert(LastSeen { turn });
                    }
                }
                return Vec::new();
            }
            SourceEvent::Settled => return self.settle(),
        }
        self.fact_sheet = OnceLock::new();
        Vec::new()
    }

    /// Whether the Advisor keeps anything of the game `key` names: it has followed it, or
    /// recalled it.
    #[must_use]
    pub fn knows(&self, key: &GameKey) -> bool {
        self.games.contains_key(key)
    }

    /// Takes back what was `remembered` of the game `key` names, last seen on `turn`, such as
    /// before the app last closed, for the Game Source to pick it up again. What it remembers is
    /// kept as it was while the game is no further on, and goes Stale once it is: the Advisor
    /// didn't see the turns between.
    pub fn recall(&mut self, key: GameKey, turn: Option<u32>, remembered: AccumulatedState) {
        self.games.insert(
            key,
            Followed {
                accumulated: remembered,
                turn,
                picked_up_from: Some(LastSeen { turn }),
                ..Followed::default()
            },
        );
    }

    /// Runs the detectors on the followed game's Settled Update, and keeps the Alert they raise
    /// for the now pill.
    fn settle(&mut self) -> Vec<Detection> {
        let Some(key) = self.followed_key() else {
            return Vec::new();
        };
        let hostile = self.hostile_in_view();
        let fact_sheet = self.fact_sheet.get_or_init(|| self.compute_fact_sheet());
        let Some(game) = self.games.get_mut(&key) else {
            return Vec::new();
        };
        let detections = game.detectors.detect(fact_sheet, &self.visible, hostile);
        if let Some(alert) = detections.iter().find_map(|detection| match detection {
            Detection::Alert(alert) => Some(alert),
            _ => None,
        }) {
            game.alert = Some((alert.clone(), self.visible.turn));
        }
        detections
    }

    /// What is remembered of the followed game, once a Game Source has identified it.
    #[must_use]
    pub fn accumulated(&self) -> Option<&AccumulatedState> {
        Some(&self.followed_game()?.accumulated)
    }

    /// The Alert the now pill shows: the last one raised, until the player acts.
    fn current_alert(&self) -> Option<&Alert> {
        let (alert, turn) = self.followed_game()?.alert.as_ref()?;
        (*turn == self.visible.turn).then_some(alert)
    }

    fn followed_key(&self) -> Option<GameKey> {
        self.identity.as_ref().map(GameIdentity::key)
    }

    /// What is kept of the followed game, once a Game Source has identified it.
    fn followed_game(&self) -> Option<&Followed> {
        self.games.get(&self.followed_key()?)
    }

    /// As [`Advisor::followed_game`], to change.
    fn followed_game_mut(&mut self) -> Option<&mut Followed> {
        let key = self.followed_key()?;
        self.games.get_mut(&key)
    }

    /// The Game Knowledge for the followed game's Crawl Version, if the Advisor has it.
    fn game_knowledge(&self) -> Option<&Bundle> {
        self.identity.as_ref()?.knowledge(&self.knowledge)
    }

    /// What the engine computed about this moment.
    #[must_use]
    pub fn fact_sheet(&self) -> &FactSheet {
        self.fact_sheet.get_or_init(|| self.compute_fact_sheet())
    }

    fn compute_fact_sheet(&self) -> FactSheet {
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

    /// The panel for the followed game, once a Game Source has identified it, at the player's
    /// `assistance` level.
    ///
    /// Each part shows only what the Advisor can tell: without the facts to judge a Threat Level,
    /// and with no Run Plan yet, it says so rather than guess.
    #[must_use]
    pub fn game_view(&self, assistance: AssistanceLevel) -> Option<GameView> {
        let identity = self.identity.as_ref()?;
        Some(GameView {
            character: Character {
                name: identity.character.clone(),
                species: identity.species.clone(),
                hp: self.visible.stats.hp,
                hp_max: self.visible.stats.hp_max,
                turn: self.visible.turn,
            },
            now: now_pill(
                &self.fact_sheet().threat,
                self.current_alert().map(|alert| alert.line.clone()),
            ),
            suggested_action: assistance
                .suggests()
                .then(|| self.suggested_action())
                .flatten()
                .map(|suggested| suggested.card()),
            focus: self.focus(),
            fact_cards: self.fact_cards(),
            ways_out: self
                .hostile_in_view()
                .then(|| self.fact_sheet().ways_out.list()),
            run_plan: RunPlanRail::NotPlanned,
            ask: AskBox::NeedsModel,
            live: !self.followed_game().is_some_and(|game| game.stopped),
            badge: ToolbarBadge {
                decisions_waiting: 0,
                calls_for_panel: self.fact_sheet().threat.level() == Some(ThreatLevel::Lethal)
                    && !self.fact_sheet().ways_out.any_trusted(),
            },
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

    /// The Suggested Action at this moment: the one held through the Encounter as of now, unless
    /// something meaningful changed since the last Settled Update, when the detectors will pick
    /// anew.
    fn suggested_action(&self) -> Option<SuggestedAction> {
        let held = self
            .followed_game()
            .and_then(|game| game.detectors.held_suggestion());
        SuggestedAction::steady(held, self.fact_sheet(), self.visible.stats.hp_max)
    }

    /// A fact card for each kind of unique or Out of Depth monster in view, in map order.
    fn fact_cards(&self) -> Vec<FactCard> {
        let mut cards: Vec<FactCard> = Vec::new();
        for exceptional in &self.fact_sheet().exceptional {
            if cards
                .iter()
                .any(|card| card.name == capitalised(&exceptional.name))
            {
                continue;
            }
            // The Fact Sheet found it among the monsters in view, where it is drawn from.
            let Some(seen) = self.visible.monsters.iter().flatten().find(|monster| {
                monster.position == exceptional.position && monster.name == exceptional.name
            }) else {
                continue;
            };
            let kind = seen
                .id
                .as_ref()
                .and_then(|id| self.game_knowledge()?.monster_by_id(id));
            cards.push(fact_card(exceptional, listing(seen, kind).depiction));
        }
        cards
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

/// The fact card for `exceptional`, drawn as `depiction`.
fn fact_card(exceptional: &ExceptionalMonster, depiction: Depiction) -> FactCard {
    FactCard {
        depiction,
        name: capitalised(&exceptional.name),
        why: match exceptional.why {
            Exceptional::Unique => "Unique".to_owned(),
            Exceptional::OutOfDepth { levels, .. } => {
                format!("Out of depth: usually met {levels} levels deeper")
            }
        },
        alone: exceptional.alone,
    }
}

/// The smallest chance of death the now pill puts a number on, in percent: below it, the
/// Monte Carlo's sampling error is as large as the number.
const DEATH_SHOWN_FROM_PERCENT: f64 = 0.1;

/// The now pill for a moment's threat, with the `alert` raised at it.
fn now_pill(threat: &Threat, alert: Option<String>) -> NowPill {
    match threat {
        Threat::Unrated { .. } => NowPill::Unrated { alert },
        Threat::Rated(assessment) => NowPill::Rated {
            level: assessment.level,
            alert,
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
