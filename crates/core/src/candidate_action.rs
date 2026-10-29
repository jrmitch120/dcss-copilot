//! Candidate Actions: what the player could legally do right now, each with its facts from the
//! Fact Sheet. The Advisor only ever recommends one.

use crate::fact_sheet::Chance;
use crate::fight_outcome::{Fight, PlanOutcome};
use crate::gaps::Reader;
use crate::view_model::{MarkedValue, capitalised};
use crate::ways_out::{CorridorSquare, Escape, Ground, Usable, WayOut, WaysOut};

/// Something the player could do right now that a rule can pick, with its facts from the Fact
/// Sheet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CandidateAction {
    /// Take a staircase, or use an item, ability or spell: one of the Ways Out.
    WayOut(Escape),
    /// Hold a corridor.
    Corridor(CorridorToHold),
    /// Fight what is in view.
    Fight {
        /// The Attack Plan to fight by, and how it comes out.
        odds: Odds,
        /// The player's hit points the fight starts from.
        hp: i32,
    },
}

/// The corridor a Candidate Action holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CorridorToHold {
    /// The one the player stands in.
    StandingIn,
    /// The nearest square of one, which the player can walk to from the open ground they stand
    /// on.
    Near(CorridorSquare),
}

/// The Attack Plan a fight is fought by, and how it comes out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Odds {
    /// As the Fight Outcome judges it now.
    Now(PlanOutcome),
    /// As the Fight Outcome last judged it, while it can't judge the fight now, such as with too
    /// many hostiles in view.
    LastJudged(PlanOutcome),
    /// The Fight Outcome hasn't judged a plan to fight by.
    Unjudged,
}

impl Odds {
    /// The plan, whether or not its odds are current.
    pub(crate) fn plan(&self) -> Option<&PlanOutcome> {
        match self {
            Self::Now(plan) | Self::LastJudged(plan) => Some(plan),
            Self::Unjudged => None,
        }
    }
}

impl CandidateAction {
    /// How often its Attack Plan wins now, when it fights by one the Fight Outcome judges.
    pub(crate) fn win(&self) -> Option<Chance> {
        match self {
            Self::Fight {
                odds: Odds::Now(plan),
                ..
            } => Some(plan.win),
            Self::WayOut(_) | Self::Corridor(_) | Self::Fight { .. } => None,
        }
    }

    /// It in one line, as of the moment `fight` judges, for `reader`, such as `Stone staircase
    /// leading down · 4 steps`: a fight is marked with the Gaps that keep it from being judged or
    /// may hide a better plan.
    pub(crate) fn value(&self, fight: &Fight, reader: Reader) -> MarkedValue {
        match self {
            Self::WayOut(escape) => escape.value(reader),
            Self::Corridor(CorridorToHold::StandingIn) => {
                MarkedValue::known("The corridor you stand in".to_owned())
            }
            Self::Corridor(CorridorToHold::Near(square)) => {
                MarkedValue::known(match square.steps {
                    1 => "The corridor 1 step away".to_owned(),
                    steps => format!("The corridor {steps} steps away"),
                })
            }
            Self::Fight {
                odds: Odds::Now(plan),
                hp,
            } => fight.marked(plan.summary(*hp), reader),
            Self::Fight {
                odds: Odds::LastJudged(plan),
                ..
            } => fight.marked(
                format!("{}: can't be judged now", capitalised(&plan.plan.name())),
                reader,
            ),
            Self::Fight {
                odds: Odds::Unjudged,
                ..
            } => fight.marked("The Advisor can't judge this fight".to_owned(), reader),
        }
    }
}

/// Fighting by the best Attack Plan in `fight`, or unjudged from `hp`.
pub(crate) fn fight_action(fight: &Fight, hp: i32) -> CandidateAction {
    match fight {
        Fight::Judged(outcome) => CandidateAction::Fight {
            odds: outcome
                .plans
                .first()
                .cloned()
                .map_or(Odds::Unjudged, Odds::Now),
            hp: outcome.hp,
        },
        Fight::NoFight | Fight::TooMany { .. } | Fight::Unjudged { .. } => CandidateAction::Fight {
            odds: Odds::Unjudged,
            hp,
        },
    }
}

/// The Ways Out the player can use now.
pub(crate) fn usable(ways_out: &WaysOut) -> impl Iterator<Item = &WayOut> {
    ways_out
        .options
        .iter()
        .filter(|way_out| way_out.usable == Usable::Yes)
}

/// The corridor to hold, when the player stands in one or can walk to one.
pub(crate) fn corridor(ways_out: &WaysOut) -> Option<CorridorToHold> {
    match ways_out.ground {
        Some(Ground::Corridor { .. }) => Some(CorridorToHold::StandingIn),
        Some(Ground::OpenGround {
            nearest_corridor, ..
        }) => nearest_corridor.map(CorridorToHold::Near),
        None => None,
    }
}

/// Whether two outcomes are of the same Attack Plan, by name: a plan's wand charges and missiles
/// count down as it is fought, and it is still the same plan.
pub(crate) fn same_plan(one: &PlanOutcome, other: &PlanOutcome) -> bool {
    one.plan.name() == other.plan.name()
}

/// Whether two Ways Out are the same one, whatever their facts now: the same staircase, the item
/// in the same inventory slot, or the ability or spell of the same name.
pub(crate) fn same_way_out(one: &Escape, other: &Escape) -> bool {
    match (one, other) {
        (
            Escape::Stairs { position: one, .. },
            Escape::Stairs {
                position: other, ..
            },
        ) => one == other,
        (
            Escape::Item {
                letter: one,
                kind_name: one_kind,
                ..
            },
            Escape::Item {
                letter: other,
                kind_name: other_kind,
                ..
            },
        ) => one == other && one_kind == other_kind,
        (Escape::Ability { name: one, .. }, Escape::Ability { name: other, .. })
        | (Escape::Spell { name: one, .. }, Escape::Spell { name: other, .. }) => one == other,
        _ => false,
    }
}
