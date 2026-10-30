//! Candidate Actions: what the player could legally do right now, each with its facts from the
//! Fact Sheet. The Advisor only ever recommends one.

use copilot_gk::Relief;

use crate::fact_sheet::{Chance, FactSheet, ThreatLevel};
use crate::fight_outcome::{Fight, PlanOutcome};
use crate::gaps::Reader;
use crate::view_model::{MarkedValue, capitalised};
use crate::ways_out::{CorridorSquare, Escape, Ground, Usable, WayOut, WaysOut};

/// The Candidate Actions at a moment, best first by the engine's own rank, each with the id an
/// LLM picks it by.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Candidates(pub(crate) Vec<Candidate>);

/// A Candidate Action, listed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Candidate {
    /// What an LLM picks it by, such as `a1`.
    pub(crate) id: String,
    /// What it is.
    pub(crate) action: CandidateAction,
    /// It is the Suggested Action.
    pub(crate) suggested: bool,
}

impl Candidates {
    /// What the player could do at the moment of `fact_sheet`, with `suggested`, the Suggested
    /// Action's pick, first, then the rest in the order the rules at the moment's Threat Level
    /// weigh them: at LETHAL the stairs, healing, escapes at once, escapes later, the rest of the
    /// Ways Out, a corridor and fighting; at DANGEROUS fighting, a corridor, then the Ways Out in
    /// that order. Within each, those the Advisor trusts come first, then the Fact Sheet's order:
    /// the nearest staircase, and the Attack Plan likeliest to win.
    pub(crate) fn at(fact_sheet: &FactSheet, suggested: &CandidateAction) -> Self {
        let level = fact_sheet.threat.level().unwrap_or(ThreatLevel::Dangerous);
        let ways_out = usable(&fact_sheet.ways_out).map(|way_out| {
            (
                way_out.trusted(),
                CandidateAction::WayOut(way_out.option.clone()),
            )
        });
        let corridor = corridor(&fact_sheet.ways_out)
            .map(|corridor| (true, CandidateAction::Corridor(corridor)));
        let fights: Vec<CandidateAction> = match &fact_sheet.fight {
            Fight::Judged(outcome) => outcome
                .plans
                .iter()
                .map(|plan| CandidateAction::Fight {
                    odds: Odds::Now(plan.clone()),
                    hp: outcome.hp,
                })
                .collect(),
            // While the fight can't be judged now, the Suggested Action's fight, if it fights,
            // is the one fight listed.
            Fight::TooMany { .. } | Fight::Unjudged { .. }
                if !matches!(suggested, CandidateAction::Fight { .. }) =>
            {
                fact_sheet
                    .threat_hp()
                    .map(|hp| fight_action(&fact_sheet.fight, hp))
                    .into_iter()
                    .collect()
            }
            Fight::NoFight | Fight::TooMany { .. } | Fight::Unjudged { .. } => Vec::new(),
        };
        let mut rest: Vec<(bool, CandidateAction)> = ways_out
            .chain(corridor)
            .chain(fights.into_iter().map(|fight| (true, fight)))
            .filter(|(_, action)| !action.same_as(suggested))
            .collect();
        rest.sort_by_key(|(trusted, action)| (rank(level, action), !trusted));
        Self(
            std::iter::once((true, suggested.clone()))
                .chain(rest)
                .enumerate()
                .map(|(index, (_, action))| Candidate {
                    id: format!("a{}", index + 1),
                    suggested: index == 0,
                    action,
                })
                .collect(),
        )
    }
}

/// Where the rules at `level` weigh `action`, the first first.
fn rank(level: ThreatLevel, action: &CandidateAction) -> u8 {
    let way_out = |escape: &Escape| match escape {
        Escape::Stairs { .. } => 0,
        Escape::Item { effect, .. }
        | Escape::Ability { effect, .. }
        | Escape::Spell { effect, .. } => match effect {
            Relief::Heals => 1,
            Relief::Escapes => 2,
            Relief::EscapesLater => 3,
            Relief::Hinders | Relief::Hastens | Relief::Strengthens => 4,
        },
    };
    match (level, action) {
        (ThreatLevel::Lethal, CandidateAction::WayOut(escape)) => way_out(escape),
        (ThreatLevel::Lethal, CandidateAction::Corridor(_)) => 5,
        (ThreatLevel::Lethal, CandidateAction::Fight { .. }) => 6,
        (ThreatLevel::Calm | ThreatLevel::Dangerous, CandidateAction::Fight { .. }) => 0,
        (ThreatLevel::Calm | ThreatLevel::Dangerous, CandidateAction::Corridor(_)) => 1,
        (ThreatLevel::Calm | ThreatLevel::Dangerous, CandidateAction::WayOut(escape)) => {
            2 + way_out(escape)
        }
    }
}

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
    /// Whether `other` is the same thing to do, whatever its facts now: the same staircase, item,
    /// ability or spell, a corridor, or the same Attack Plan.
    pub(crate) fn same_as(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::WayOut(mine), Self::WayOut(theirs)) => same_way_out(mine, theirs),
            (Self::Corridor(_), Self::Corridor(_)) => true,
            (Self::Fight { odds: mine, .. }, Self::Fight { odds: theirs, .. }) => {
                match (mine.plan(), theirs.plan()) {
                    (Some(mine), Some(theirs)) => same_plan(mine, theirs),
                    (mine, theirs) => mine.is_none() && theirs.is_none(),
                }
            }
            _ => false,
        }
    }

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
