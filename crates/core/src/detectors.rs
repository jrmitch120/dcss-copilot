//! The detectors: what the Advisor notices on its own at each Settled Update, by comparing it with
//! the one before. They tell when an Encounter starts and when the player is All Clear, raise
//! Tactical Alerts, and show a fact card as a unique or Out of Depth monster comes into view.

use std::collections::BTreeSet;

use serde::Serialize;

use crate::dangerous_status::DangerousStatus;
use crate::exceptional::{Exceptional, ExceptionalMonster};
use crate::fact_sheet::{FactSheet, Threat, ThreatLevel};
use crate::view_model::capitalised;
use crate::visible_state::VisibleState;
use crate::ways_out::Usable;

/// How many player actions in a row must be clear for the player to be All Clear. From the spec
/// (#24, "Detectors and Suggested Action").
const ALL_CLEAR_ACTIONS: u32 = 3;

/// Something the detectors noticed at a Settled Update.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(
    tag = "event",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Detection {
    /// An Encounter started: a hostile came into view, the Threat Level left CALM, or a dangerous
    /// status landed.
    EncounterStarted,
    /// The player is All Clear, which ended the Encounter.
    AllClear,
    /// A unique or Out of Depth monster came into view, for the first time this Encounter.
    FactCard(ExceptionalMonster),
    /// A Tactical Alert.
    Alert(Alert),
}

/// A Tactical Alert: what it is about, and what it says.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Alert {
    /// What raised it.
    #[serde(flatten)]
    pub cause: AlertCause,
    /// What it says, in one sentence, such as `Threat rose to LETHAL.`.
    pub line: String,
}

/// What raises a Tactical Alert. Each is raised at most once an Encounter.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(
    tag = "cause",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase"
)]
pub enum AlertCause {
    /// The Threat Level rose to `to`.
    ThreatLevelRose {
        /// The level it rose to.
        to: ThreatLevel,
    },
    /// A unique that alone makes the moment DANGEROUS came into view.
    UniqueInView {
        /// The unique, as the game names it.
        monster: String,
    },
    /// An Out of Depth monster that alone makes the moment DANGEROUS came into view.
    OutOfDepth {
        /// The monster, as the game names it.
        monster: String,
    },
    /// The player's hit points fell below half their maximum during an Encounter.
    HitPointsBelowHalf,
    /// The player's hit points fell below a quarter of their maximum during an Encounter.
    HitPointsBelowQuarter,
    /// The player had a Way Out they could use, and has none now that the moment is DANGEROUS or
    /// worse: they are cornered. Losing it at CALM, as to a jackal that cuts off the stairs, is
    /// no Alert.
    LastWayOutLost,
    /// A dangerous status landed on the player.
    DangerousStatus {
        /// The status.
        status: DangerousStatus,
    },
}

impl AlertCause {
    /// How urgent it is, as the one Alert raised at a player action: the highest wins.
    fn urgency(&self) -> u8 {
        match self {
            Self::ThreatLevelRose {
                to: ThreatLevel::Lethal,
            } => 8,
            Self::LastWayOutLost => 7,
            Self::DangerousStatus { .. } => 6,
            Self::HitPointsBelowQuarter => 5,
            Self::UniqueInView { .. } => 4,
            Self::OutOfDepth { .. } => 3,
            Self::HitPointsBelowHalf => 2,
            Self::ThreatLevelRose { .. } => 1,
        }
    }
}

/// What the detectors remember of one game: the last Settled Update and the Encounter under way.
#[derive(Debug, Clone, Default)]
pub(crate) struct Detectors {
    /// What the last Settled Update showed, once there was one.
    last: Option<Reading>,
    /// The Encounter under way, if any.
    encounter: Option<Encounter>,
    /// The turn of the last player action an Alert was raised at.
    alerted_on: Option<u32>,
}

/// One Encounter, as far as the detectors need to remember it.
#[derive(Debug, Clone, Default)]
struct Encounter {
    /// The Alerts raised so far.
    raised: BTreeSet<AlertCause>,
    /// The monsters a fact card was shown for so far, by name.
    carded: BTreeSet<String>,
    /// The player actions in a row the player has been clear for.
    clear_actions: u32,
}

/// What the detectors read off one Settled Update.
#[derive(Debug, Clone)]
struct Reading {
    turn: Option<u32>,
    /// The Threat Level, when the Advisor could judge one.
    level: Option<ThreatLevel>,
    hp: Option<i32>,
    hp_max: Option<i32>,
    /// Anything hostile is in view, as the panel tells.
    hostile: bool,
    /// A Way Out the player can use now is among their Ways Out.
    usable_way_out: bool,
    dangerous_statuses: Vec<DangerousStatus>,
    exceptional: Vec<ExceptionalMonster>,
}

impl Reading {
    fn of(fact_sheet: &FactSheet, visible: &VisibleState, hostile: bool) -> Self {
        Self {
            turn: visible.turn,
            level: match &fact_sheet.threat {
                Threat::Rated(assessment) => Some(assessment.level),
                Threat::Unrated { .. } => None,
            },
            hp: visible.stats.hp,
            hp_max: visible.stats.hp_max,
            hostile,
            usable_way_out: fact_sheet
                .ways_out
                .options
                .iter()
                .any(|way_out| way_out.usable == Usable::Yes),
            dangerous_statuses: fact_sheet.dangerous_statuses.clone(),
            exceptional: fact_sheet.exceptional.clone(),
        }
    }

    /// Whether it starts an Encounter: a hostile is in view, the Threat Level isn't CALM or a
    /// dangerous status is on the player.
    fn in_danger(&self) -> bool {
        self.hostile
            || self.level.is_some_and(|level| level > ThreatLevel::Calm)
            || !self.dangerous_statuses.is_empty()
    }

    /// Whether it counts toward All Clear: judged CALM, with nothing hostile in view and no
    /// dangerous status. A moment the Advisor can't judge never does.
    fn clear(&self) -> bool {
        self.level == Some(ThreatLevel::Calm) && !self.hostile && self.dangerous_statuses.is_empty()
    }

    /// Whether hit points are below `1 / fraction` of their maximum.
    fn hp_below(&self, fraction: i32) -> Option<bool> {
        Some(self.hp? * fraction < self.hp_max?)
    }
}

impl Detectors {
    /// What the detectors notice at a Settled Update: `fact_sheet` computed of `visible`, with
    /// `hostile` telling whether anything hostile is in view. The first Settled Update of a game
    /// is only where they start from: it may start an Encounter, but nothing has changed yet.
    pub(crate) fn detect(
        &mut self,
        fact_sheet: &FactSheet,
        visible: &VisibleState,
        hostile: bool,
    ) -> Vec<Detection> {
        let now = Reading::of(fact_sheet, visible, hostile);
        let before = self.last.replace(now.clone());
        let mut detections = Vec::new();
        if self.encounter.is_none() && now.in_danger() {
            self.encounter = Some(Encounter::default());
            detections.push(Detection::EncounterStarted);
        }
        let (Some(before), Some(encounter)) = (before, &mut self.encounter) else {
            // The first Settled Update: what an Encounter it starts finds in view didn't come
            // into view.
            if let Some(encounter) = &mut self.encounter {
                encounter
                    .carded
                    .extend(now.exceptional.iter().map(|monster| monster.name.clone()));
            }
            return detections;
        };

        let mut causes = Vec::new();
        for monster in came_into_view(&before, &now) {
            if encounter.carded.insert(monster.name.clone()) {
                detections.push(Detection::FactCard(monster.clone()));
            }
            causes.extend(exceptional_alert(monster));
        }
        causes.extend(alert_causes(&before, &now));
        // At most one Alert a player action: the most urgent not yet raised this Encounter.
        if now.turn.is_none() || self.alerted_on != now.turn {
            let raised = &encounter.raised;
            if let Some((cause, line)) = causes
                .into_iter()
                .filter(|(cause, _)| !raised.contains(cause))
                .max_by_key(|(cause, _)| cause.urgency())
            {
                encounter.raised.insert(cause.clone());
                self.alerted_on = now.turn;
                detections.push(Detection::Alert(Alert { cause, line }));
            }
        }

        let acted = matches!((before.turn, now.turn), (Some(was), Some(turn)) if turn > was);
        if !now.clear() {
            encounter.clear_actions = 0;
        } else if acted {
            encounter.clear_actions += 1;
        }
        if encounter.clear_actions >= ALL_CLEAR_ACTIONS {
            self.encounter = None;
            detections.push(Detection::AllClear);
        }
        detections
    }
}

/// The exceptional monsters in view `now` that weren't `before`.
fn came_into_view<'a>(
    before: &'a Reading,
    now: &'a Reading,
) -> impl Iterator<Item = &'a ExceptionalMonster> {
    now.exceptional.iter().filter(|monster| {
        !before
            .exceptional
            .iter()
            .any(|seen| seen.name == monster.name)
    })
}

/// The Alerts what changed from `before` to `now` may raise, besides exceptional monsters coming
/// into view, each with what it says.
fn alert_causes(before: &Reading, now: &Reading) -> Vec<(AlertCause, String)> {
    let mut causes = Vec::new();
    if let (Some(was), Some(level)) = (before.level, now.level)
        && level > was
    {
        causes.push((
            AlertCause::ThreatLevelRose { to: level },
            format!("Threat rose to {}.", shouted(level)),
        ));
    }
    for (fraction, cause, words) in [
        (2, AlertCause::HitPointsBelowHalf, "half"),
        (4, AlertCause::HitPointsBelowQuarter, "a quarter"),
    ] {
        if before.hp_below(fraction) == Some(false)
            && now.hp_below(fraction) == Some(true)
            && let (Some(hp), Some(hp_max)) = (now.hp, now.hp_max)
        {
            causes.push((cause, format!("HP {hp} of {hp_max}: below {words}.")));
        }
    }
    if before.usable_way_out
        && !now.usable_way_out
        && now
            .level
            .is_some_and(|level| level >= ThreatLevel::Dangerous)
    {
        causes.push((
            AlertCause::LastWayOutLost,
            "No Way Out is left that you can use.".to_owned(),
        ));
    }
    for &status in &now.dangerous_statuses {
        if !before.dangerous_statuses.contains(&status) {
            causes.push((AlertCause::DangerousStatus { status }, status_line(status)));
        }
    }
    causes
}

/// The Alert an exceptional monster coming into view raises: only when it alone makes the moment
/// DANGEROUS or worse.
fn exceptional_alert(monster: &ExceptionalMonster) -> Option<(AlertCause, String)> {
    let alone = monster
        .alone
        .filter(|&level| level >= ThreatLevel::Dangerous)?;
    let name = capitalised(&monster.name);
    Some(match monster.why {
        Exceptional::Unique => (
            AlertCause::UniqueInView {
                monster: monster.name.clone(),
            },
            format!("{name}, a unique, alone makes this {}.", shouted(alone)),
        ),
        Exceptional::OutOfDepth { levels, .. } => (
            AlertCause::OutOfDepth {
                monster: monster.name.clone(),
            },
            format!(
                "{name}, out of depth by {levels} levels, alone makes this {}.",
                shouted(alone)
            ),
        ),
    })
}

/// What an Alert for `status` landing says.
fn status_line(status: DangerousStatus) -> String {
    match status {
        DangerousStatus::LethalPoison => "The poison in you will kill you unless you cure it.",
        DangerousStatus::Petrification => "You are turning, or have turned, to stone.",
        DangerousStatus::LiquidFire => "You are burning in liquid fire.",
        DangerousStatus::ConfusionBesideDeepWaterOrLava => {
            "You are confused beside deep water or lava."
        }
    }
    .to_owned()
}

/// A Threat Level as the panel labels it, such as `DANGEROUS`.
fn shouted(level: ThreatLevel) -> &'static str {
    match level {
        ThreatLevel::Calm => "CALM",
        ThreatLevel::Dangerous => "DANGEROUS",
        ThreatLevel::Lethal => "LETHAL",
    }
}
