//! The detectors: what the Advisor notices on its own at each Settled Update, by comparing it with
//! the one before. They tell when an Encounter starts and when the player is All Clear, raise
//! Tactical Alerts, show a fact card as a unique or Out of Depth monster comes into view, and
//! hold the Suggested Action steady through the Encounter.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::dangerous_status::DangerousStatus;
use crate::exceptional::{Exceptional, ExceptionalMonster};
use crate::fact_sheet::{FactSheet, ThreatLevel};
use crate::gaps::Fillable;
use crate::suggested_action::{SuggestedAction, SuggestionRule};
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
    /// At an All Clear, the panel's Gap prompt card asked the player to fill the Gaps in these
    /// facts, which no prompt card had asked for before in the game. The Advisor raises it
    /// rather than the detectors, as it asks of what the Advisor remembers.
    GapPrompt {
        /// What it asked for.
        asks: Vec<Fillable>,
    },
    /// The rules picked a new Suggested Action this Encounter, which the panel shows from now
    /// on, at DANGEROUS and LETHAL moments.
    SuggestedAction {
        /// The rule that picked it.
        rule: SuggestionRule,
        /// What it picked, in one line, such as `Leave by stairs: Stone staircase leading down ·
        /// 4 steps`.
        line: String,
    },
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

/// What raises a Tactical Alert. Each is raised at most once an Encounter; one that loses to a
/// more urgent Alert at a player action is held, and raised at a later one while it still holds.
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
    /// The player had a Way Out they could use this Encounter, and is cornered: DANGEROUS or
    /// worse, with none left. Losing it at CALM, as to a jackal that cuts off the stairs, is no
    /// Alert until the moment turns DANGEROUS.
    LastWayOutLost,
    /// A dangerous status landed on the player.
    DangerousStatus {
        /// The status.
        status: DangerousStatus,
    },
}

/// What kind of cause raised an Alert, whatever it was about, as the timeline writes its `cause`
/// and a Scenario's manifest names it, such as `threat-level-rose`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AlertKind {
    /// [`AlertCause::ThreatLevelRose`].
    ThreatLevelRose,
    /// [`AlertCause::UniqueInView`].
    UniqueInView,
    /// [`AlertCause::OutOfDepth`].
    OutOfDepth,
    /// [`AlertCause::HitPointsBelowHalf`].
    HitPointsBelowHalf,
    /// [`AlertCause::HitPointsBelowQuarter`].
    HitPointsBelowQuarter,
    /// [`AlertCause::LastWayOutLost`].
    LastWayOutLost,
    /// [`AlertCause::DangerousStatus`].
    DangerousStatus,
}

impl AlertCause {
    /// What kind of cause it is.
    #[must_use]
    pub fn kind(&self) -> AlertKind {
        match self {
            Self::ThreatLevelRose { .. } => AlertKind::ThreatLevelRose,
            Self::UniqueInView { .. } => AlertKind::UniqueInView,
            Self::OutOfDepth { .. } => AlertKind::OutOfDepth,
            Self::HitPointsBelowHalf => AlertKind::HitPointsBelowHalf,
            Self::HitPointsBelowQuarter => AlertKind::HitPointsBelowQuarter,
            Self::LastWayOutLost => AlertKind::LastWayOutLost,
            Self::DangerousStatus { .. } => AlertKind::DangerousStatus,
        }
    }

    /// How urgent it is, as the one Alert raised at a player action: the highest wins. The spec
    /// (#24) ranks none; this puts what can kill the player soonest first, and a unique or Out of
    /// Depth monster ahead of the rise in the Threat Level it brings.
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

    /// Whether raising it makes `other` say nothing new: a rise to LETHAL, being cornered, which
    /// is DANGEROUS or worse, and a monster that alone makes the moment DANGEROUS each cover a
    /// rise to DANGEROUS, and hit points below a quarter cover below half.
    fn covers(&self, other: &Self) -> bool {
        match other {
            Self::ThreatLevelRose {
                to: ThreatLevel::Dangerous,
            } => matches!(
                self,
                Self::ThreatLevelRose {
                    to: ThreatLevel::Lethal
                } | Self::LastWayOutLost
                    | Self::UniqueInView { .. }
                    | Self::OutOfDepth { .. }
            ),
            Self::HitPointsBelowHalf => *self == Self::HitPointsBelowQuarter,
            _ => false,
        }
    }

    /// What its Alert says at `now`, or `None` once what raised it no longer holds.
    fn line(&self, now: &Reading) -> Option<String> {
        match self {
            Self::ThreatLevelRose { to } => {
                (now.level >= Some(*to)).then(|| format!("Threat rose to {}.", shouted(*to)))
            }
            Self::UniqueInView { monster } | Self::OutOfDepth { monster } => {
                let seen = now.exceptional.iter().find(|seen| seen.name == *monster)?;
                let alone = seen
                    .alone
                    .filter(|&level| level >= ThreatLevel::Dangerous)?;
                let name = capitalised(monster);
                Some(match seen.why {
                    Exceptional::Unique => {
                        format!("{name}, a unique, alone makes this {}.", shouted(alone))
                    }
                    Exceptional::OutOfDepth { levels, .. } => format!(
                        "{name}, out of depth by {levels} levels, alone makes this {}.",
                        shouted(alone)
                    ),
                })
            }
            Self::HitPointsBelowHalf => hp_line(now, 2, "half"),
            Self::HitPointsBelowQuarter => hp_line(now, 4, "a quarter"),
            Self::LastWayOutLost => now
                .cornered()
                .then(|| "No Way Out is left that you can use.".to_owned()),
            Self::DangerousStatus { status } => now
                .dangerous_statuses
                .contains(status)
                .then(|| status_line(*status).to_owned()),
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
    /// An Alert was raised since the player last acted.
    alerted_this_action: bool,
}

/// One Encounter, as far as the detectors need to remember it.
#[derive(Debug, Clone, Default)]
struct Encounter {
    /// The Alerts raised so far.
    raised: BTreeSet<AlertCause>,
    /// What would have raised an Alert at an action that raised another, to raise at a later
    /// action while it still holds.
    held: BTreeSet<AlertCause>,
    /// The monsters a fact card was shown for so far, by name.
    carded: BTreeSet<String>,
    /// The player has had a Way Out they could use at some moment of it, or just before it.
    had_way_out: bool,
    /// The player actions in a row the player has been clear for.
    clear_actions: u32,
    /// The Suggested Action picked last, held steady while it can still be done, through any
    /// lull below DANGEROUS.
    suggested: Option<SuggestedAction>,
}

/// What the detectors read off one Settled Update.
#[derive(Debug, Clone, Default)]
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
            level: fact_sheet.threat.level(),
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

    /// Whether the player is cornered: DANGEROUS or worse, with no Way Out they can use.
    fn cornered(&self) -> bool {
        !self.usable_way_out && self.level >= Some(ThreatLevel::Dangerous)
    }

    /// Whether hit points are below `1 / fraction` of their maximum.
    fn hp_below(&self, fraction: i32) -> Option<bool> {
        Some(self.hp? * fraction < self.hp_max?)
    }
}

impl Detectors {
    /// What the detectors notice at a Settled Update: `fact_sheet` computed of `visible`, with
    /// `hostile` telling whether anything hostile is in view. The first Settled Update of a game
    /// is judged against an empty one, so the Advisor joining a fight in progress says so.
    pub(crate) fn detect(
        &mut self,
        fact_sheet: &FactSheet,
        visible: &VisibleState,
        hostile: bool,
    ) -> Vec<Detection> {
        let now = Reading::of(fact_sheet, visible, hostile);
        let before = self.last.replace(now.clone()).unwrap_or_default();
        let acted = matches!((before.turn, now.turn), (Some(was), Some(turn)) if turn > was);
        if acted {
            self.alerted_this_action = false;
        }
        let mut detections = Vec::new();
        if self.encounter.is_none() && now.in_danger() {
            self.encounter = Some(Encounter {
                had_way_out: before.usable_way_out,
                ..Encounter::default()
            });
            detections.push(Detection::EncounterStarted);
        }
        let Some(encounter) = &mut self.encounter else {
            return detections;
        };

        for monster in came_into_view(&before, &now) {
            if encounter.carded.insert(monster.name.clone()) {
                detections.push(Detection::FactCard(monster.clone()));
            }
        }
        let mut causes = std::mem::take(&mut encounter.held);
        causes.extend(alert_causes(&before, &now, encounter.had_way_out));
        encounter.had_way_out |= now.usable_way_out;
        // Each still holding and saying something new this Encounter, with what it says.
        let mut pending: Vec<(AlertCause, String)> = causes
            .into_iter()
            .filter(|cause| {
                !encounter
                    .raised
                    .iter()
                    .any(|raised| raised == cause || raised.covers(cause))
            })
            .filter_map(|cause| {
                let line = cause.line(&now)?;
                Some((cause, line))
            })
            .collect();
        // At most one Alert a player action: the most urgent.
        if !self.alerted_this_action {
            pending.sort_by_key(|(cause, _)| std::cmp::Reverse(cause.urgency()));
            if !pending.is_empty() {
                let (cause, line) = pending.remove(0);
                pending.retain(|(held, _)| !cause.covers(held));
                encounter.raised.insert(cause.clone());
                self.alerted_this_action = true;
                detections.push(Detection::Alert(Alert { cause, line }));
            }
        }
        encounter.held = pending.into_iter().map(|(cause, _)| cause).collect();

        if let Some(suggested) = SuggestedAction::steady(
            encounter.suggested.as_ref(),
            fact_sheet,
            visible.stats.hp_max,
        ) {
            if !encounter
                .suggested
                .as_ref()
                .is_some_and(|held| held.same_pick(&suggested))
            {
                detections.push(Detection::SuggestedAction {
                    rule: suggested.rule(),
                    line: suggested.line(),
                });
            }
            encounter.suggested = Some(suggested);
        }

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

    /// The Suggested Action held through the Encounter under way, as the last Settled Update
    /// left it.
    pub(crate) fn held_suggestion(&self) -> Option<&SuggestedAction> {
        self.encounter.as_ref()?.suggested.as_ref()
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

/// What changed from `before` to `now` that may raise an Alert: the Threat Level rising past
/// DANGEROUS or LETHAL, a unique or Out of Depth monster coming into view, hit points falling past
/// half or a quarter, the player cornered when they `had_way_out` this Encounter, and a Dangerous
/// Status landing. Whether each says anything is up to [`AlertCause::line`].
fn alert_causes(before: &Reading, now: &Reading, had_way_out: bool) -> Vec<AlertCause> {
    let mut causes = Vec::new();
    for to in [ThreatLevel::Dangerous, ThreatLevel::Lethal] {
        if now.level >= Some(to) && before.level < Some(to) {
            causes.push(AlertCause::ThreatLevelRose { to });
        }
    }
    for monster in came_into_view(before, now) {
        let monster_name = monster.name.clone();
        causes.push(match monster.why {
            Exceptional::Unique => AlertCause::UniqueInView {
                monster: monster_name,
            },
            Exceptional::OutOfDepth { .. } => AlertCause::OutOfDepth {
                monster: monster_name,
            },
        });
    }
    for (fraction, cause) in [
        (2, AlertCause::HitPointsBelowHalf),
        (4, AlertCause::HitPointsBelowQuarter),
    ] {
        if before.hp_below(fraction) == Some(false) && now.hp_below(fraction) == Some(true) {
            causes.push(cause);
        }
    }
    if had_way_out && now.cornered() && !before.cornered() {
        causes.push(AlertCause::LastWayOutLost);
    }
    for &status in &now.dangerous_statuses {
        if !before.dangerous_statuses.contains(&status) {
            causes.push(AlertCause::DangerousStatus { status });
        }
    }
    causes
}

/// What an Alert that hit points fell below `1 / fraction`, in `words`, says at `now`, while they
/// are still below it.
fn hp_line(now: &Reading, fraction: i32, words: &str) -> Option<String> {
    let (hp, hp_max) = (now.hp?, now.hp_max?);
    now.hp_below(fraction)?
        .then(|| format!("HP {hp} of {hp_max}: below {words}."))
}

/// What an Alert for `status` landing says.
fn status_line(status: DangerousStatus) -> &'static str {
    match status {
        DangerousStatus::LethalPoison => "The poison in you will kill you unless you cure it.",
        DangerousStatus::Petrification => "You are turning, or have turned, to stone.",
        DangerousStatus::LiquidFire => "You are burning in liquid fire.",
        DangerousStatus::ConfusionBesideDeepWaterOrLava => {
            "You are confused beside deep water or lava."
        }
    }
}

/// A Threat Level as the panel labels it, such as `DANGEROUS`.
fn shouted(level: ThreatLevel) -> &'static str {
    match level {
        ThreatLevel::Calm => "CALM",
        ThreatLevel::Dangerous => "DANGEROUS",
        ThreatLevel::Lethal => "LETHAL",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_cause_is_written_as_its_kind_is_named() {
        let causes = [
            AlertCause::ThreatLevelRose {
                to: ThreatLevel::Lethal,
            },
            AlertCause::UniqueInView {
                monster: "Sigmund".to_owned(),
            },
            AlertCause::OutOfDepth {
                monster: "ogre".to_owned(),
            },
            AlertCause::HitPointsBelowHalf,
            AlertCause::HitPointsBelowQuarter,
            AlertCause::LastWayOutLost,
            AlertCause::DangerousStatus {
                status: DangerousStatus::LiquidFire,
            },
        ];
        for cause in causes {
            let written = serde_json::to_value(&cause).unwrap();
            assert_eq!(
                written["cause"],
                serde_json::to_value(cause.kind()).unwrap(),
                "{cause:?}"
            );
        }
    }
}
