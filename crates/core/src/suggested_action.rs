//! The Suggested Action: the Candidate Action the Advisor's own rules pick at a DANGEROUS or
//! LETHAL moment, before and without any LLM, named by the rule that picked it, and held steady
//! until something meaningful changes.

use copilot_gk::Relief;
use serde::{Deserialize, Serialize};

use crate::candidate_action::{
    CandidateAction, Odds, corridor, fight_action, same_plan, same_way_out, usable,
};
use crate::fact_sheet::{FactSheet, Threat, ThreatLevel};
use crate::fight_outcome::Fight;
use crate::gaps::Reader;
use crate::view_model::{MarkedValue, SuggestedActionCard};
use crate::ways_out::{Escape, WaysOut};

/// Fight on at DANGEROUS only when the best Attack Plan wins at least this often, in percent. From
/// the spec (#24, "Detectors and Suggested Action"), to be tuned on the corpus.
const FIGHT_ON_WIN_PERCENT: u64 = 90;

/// Fight on at DANGEROUS only when the best Attack Plan leaves the player at least this share of
/// their maximum hit points on average, in percent. From the spec (#24, "Detectors and Suggested
/// Action"), to be tuned on the corpus.
const FIGHT_ON_HP_LEFT_PERCENT: u64 = 30;

/// A fresh pick replaces the one held when it wins at least this many more fights in a hundred.
/// From the spec (#24, "Detectors and Suggested Action"), to be tuned on the corpus.
const CLEARLY_BETTER_WIN_POINTS: u64 = 10;

/// A held fight on is kept while its odds and the hit points it leaves dip by fewer than this many
/// points below the thresholds it was picked at: the margin the spec (#24) gives a clearly better
/// pick. Held any further below, the card would say "Fight on" over odds the rule would never
/// take.
const FIGHT_ON_HOLDS_WITHIN_POINTS: u64 = CLEARLY_BETTER_WIN_POINTS;

/// Falling back to a corridor helps only against more than one hostile: one alone attacks from
/// one side anywhere. The spec (#24) leaves it unsaid; this keeps the rule from walking the
/// player away from a lone monster that follows and hits them.
const CORRIDOR_HELPS_FROM_HOSTILES: u32 = 2;

/// The rule that picked a Suggested Action, as the timeline writes it and a Scenario's manifest
/// names it, such as `leave-by-stairs`. At a LETHAL moment the LETHAL rules are checked in the
/// order listed, and at a DANGEROUS one the DANGEROUS rules; an earlier rule outranks a later one,
/// so every LETHAL rule outranks every DANGEROUS one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SuggestionRule {
    /// LETHAL: take a staircase the player can use.
    LeaveByStairs,
    /// LETHAL: heal, with an item, ability or spell that heals.
    HealOutOfIt,
    /// LETHAL: escape, with one that takes the player away at once before one that takes them
    /// away after a delay.
    BestEscape,
    /// LETHAL: none of the rules above applies, so fight by the best Attack Plan, whatever its
    /// odds. It gives way as soon as one of them could.
    Fight,
    /// DANGEROUS: the best Attack Plan wins almost always and leaves hit points to spare.
    FightOn,
    /// DANGEROUS: among several hostiles, hold a corridor: the one the player stands in, or the
    /// nearest they can walk to.
    FallBackToACorridor,
    /// DANGEROUS: walk to a staircase the player can use.
    FallBackToStairs,
    /// DANGEROUS: fight by the best Attack Plan, with the Way Out to take should it turn LETHAL.
    FightCarefully,
}

impl SuggestionRule {
    /// The Threat Level it is checked at.
    fn level(self) -> ThreatLevel {
        match self {
            Self::LeaveByStairs | Self::HealOutOfIt | Self::BestEscape | Self::Fight => {
                ThreatLevel::Lethal
            }
            Self::FightOn
            | Self::FallBackToACorridor
            | Self::FallBackToStairs
            | Self::FightCarefully => ThreatLevel::Dangerous,
        }
    }

    /// Its name, as the panel labels the Suggested Action it picked.
    fn name(self) -> &'static str {
        match self {
            Self::LeaveByStairs => "Leave by stairs",
            Self::HealOutOfIt => "Heal out of it",
            Self::BestEscape => "Escape",
            Self::Fight => "Last resort: fight",
            Self::FightOn => "Fight on",
            Self::FallBackToACorridor => "Fall back to a corridor",
            Self::FallBackToStairs => "Fall back to stairs",
            Self::FightCarefully => "Fight carefully",
        }
    }
}

/// A Suggested Action: the rule that picked it, and what it picked, with its facts as of the
/// Fact Sheet it was last brought up to date with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SuggestedAction {
    rule: SuggestionRule,
    action: CandidateAction,
    /// At fight carefully, the Way Out the LETHAL rules would take, should the moment turn
    /// LETHAL, if there is one.
    if_lethal: Option<Escape>,
}

impl SuggestedAction {
    /// What the rules pick at the moment of `fact_sheet`, for a player with `hp_max` hit points
    /// at most: `None` unless the moment is DANGEROUS or LETHAL and the character still lives.
    fn pick(fact_sheet: &FactSheet, hp_max: Option<i32>) -> Option<Self> {
        let Threat::Rated(assessment) = &fact_sheet.threat else {
            return None;
        };
        // Crawl shows hit points at or below zero once the character has died.
        if assessment.hp <= 0 {
            return None;
        }
        let ways_out = &fact_sheet.ways_out;
        let fight = || fight_action(&fact_sheet.fight, assessment.hp);
        let picked = |rule, action| Self {
            rule,
            action,
            if_lethal: None,
        };
        match assessment.level {
            ThreatLevel::Calm => None,
            ThreatLevel::Lethal => Some(match lethal_way_out(ways_out) {
                Some((rule, escape)) => picked(rule, CandidateAction::WayOut(escape.clone())),
                None => picked(SuggestionRule::Fight, fight()),
            }),
            ThreatLevel::Dangerous => Some(
                if let action @ CandidateAction::Fight { .. } = fight()
                    && action.fights_on(hp_max, 0)
                {
                    picked(SuggestionRule::FightOn, action)
                } else if let Some(corridor) = corridor(ways_out)
                    .filter(|_| assessment.hostiles_in_view >= CORRIDOR_HELPS_FROM_HOSTILES)
                {
                    picked(
                        SuggestionRule::FallBackToACorridor,
                        CandidateAction::Corridor(corridor),
                    )
                } else if let Some(stairs) = stairs(ways_out) {
                    picked(
                        SuggestionRule::FallBackToStairs,
                        CandidateAction::WayOut(stairs.clone()),
                    )
                } else {
                    Self {
                        rule: SuggestionRule::FightCarefully,
                        action: fight(),
                        if_lethal: lethal_way_out(ways_out).map(|(_, escape)| escape.clone()),
                    }
                },
            ),
        }
    }

    /// The Suggested Action at the moment of `fact_sheet`, for a player with `hp_max` hit points
    /// at most, given the one `held` so far this Encounter: `held` as of now, unless it can no
    /// longer be done, a rule that outranks it applies, or the rules' fresh pick wins clearly
    /// more fights. `None` unless the moment is DANGEROUS or LETHAL and the character still
    /// lives.
    pub(crate) fn steady(
        held: Option<&Self>,
        fact_sheet: &FactSheet,
        hp_max: Option<i32>,
    ) -> Option<Self> {
        let fresh = Self::pick(fact_sheet, hp_max)?;
        let Some(held) = held.and_then(|held| held.as_of(fact_sheet, hp_max)) else {
            return Some(fresh);
        };
        if fresh.rule < held.rule || fresh.clearly_better_than(&held) {
            Some(fresh)
        } else {
            Some(held)
        }
    }

    /// The same pick as of `fact_sheet`, for a player with `hp_max` hit points at most, with its
    /// facts brought up to date: `None` once it can no longer be done, such as a staircase cut
    /// off or an Attack Plan whose wand ran out, or once a fight on falls clearly short of the
    /// odds it was picked at.
    fn as_of(&self, fact_sheet: &FactSheet, hp_max: Option<i32>) -> Option<Self> {
        let action = match &self.action {
            CandidateAction::WayOut(escape) => CandidateAction::WayOut(
                usable(&fact_sheet.ways_out)
                    .find(|way_out| same_way_out(&way_out.option, escape))?
                    .option
                    .clone(),
            ),
            CandidateAction::Corridor(_) => {
                CandidateAction::Corridor(corridor(&fact_sheet.ways_out)?)
            }
            CandidateAction::Fight { odds, .. } => {
                let hp = fact_sheet.threat_hp()?;
                match (&fact_sheet.fight, odds.plan()) {
                    (Fight::NoFight, _) => return None,
                    (Fight::Judged(outcome), Some(plan)) => CandidateAction::Fight {
                        odds: Odds::Now(
                            outcome
                                .plans
                                .iter()
                                .find(|now| same_plan(now, plan))?
                                .clone(),
                        ),
                        hp: outcome.hp,
                    },
                    (fight @ Fight::Judged(_), None) => fight_action(fight, hp),
                    (Fight::TooMany { .. } | Fight::Unjudged { .. }, plan) => {
                        CandidateAction::Fight {
                            odds: plan.cloned().map_or(Odds::Unjudged, Odds::LastJudged),
                            hp,
                        }
                    }
                }
            }
        };
        let gives_way = match self.rule {
            // Its card would say "Fight on" over odds the rule would never take.
            SuggestionRule::FightOn => {
                action.win().is_some() && !action.fights_on(hp_max, FIGHT_ON_HOLDS_WITHIN_POINTS)
            }
            // It is the last resort only while no rule above it could apply.
            SuggestionRule::Fight => lethal_way_out(&fact_sheet.ways_out).is_some(),
            _ => false,
        };
        (!gives_way).then(|| Self {
            rule: self.rule,
            action,
            if_lethal: (self.rule == SuggestionRule::FightCarefully)
                .then(|| lethal_way_out(&fact_sheet.ways_out).map(|(_, escape)| escape.clone()))
                .flatten(),
        })
    }

    /// Whether it fights by an Attack Plan that wins clearly more often than the one `held`
    /// fights by.
    fn clearly_better_than(&self, held: &Self) -> bool {
        match (self.action.win(), held.action.win()) {
            (Some(fresh), Some(held)) => fresh.ahead_by_points(held, CLEARLY_BETTER_WIN_POINTS),
            _ => false,
        }
    }

    /// Whether `other` is the same rule picking the same thing, whatever its facts now: the same
    /// staircase, item, ability or spell, a corridor, or the same Attack Plan.
    pub(crate) fn same_pick(&self, other: &Self) -> bool {
        self.rule == other.rule
            && match (&self.action, &other.action) {
                (CandidateAction::WayOut(mine), CandidateAction::WayOut(theirs)) => {
                    same_way_out(mine, theirs)
                }
                (CandidateAction::Corridor(_), CandidateAction::Corridor(_)) => true,
                (
                    CandidateAction::Fight { odds: mine, .. },
                    CandidateAction::Fight { odds: theirs, .. },
                ) => match (mine.plan(), theirs.plan()) {
                    (Some(mine), Some(theirs)) => same_plan(mine, theirs),
                    (mine, theirs) => mine.is_none() && theirs.is_none(),
                },
                _ => false,
            }
    }

    /// The rule that picked it.
    pub(crate) fn rule(&self) -> SuggestionRule {
        self.rule
    }

    /// It in one line, as the timeline records it, as of the moment `fight` judges, such as
    /// `Leave by stairs — Stone staircase leading down · 4 steps`.
    pub(crate) fn line(&self, fight: &Fight) -> String {
        let card = self.card(fight, Reader::Player);
        match card.if_lethal {
            Some(if_lethal) => format!(
                "{} — {}. If it turns LETHAL: {}",
                card.rule, card.action.text, if_lethal.text
            ),
            None => format!("{} — {}", card.rule, card.action.text),
        }
    }

    /// Its card in the panel, as of the moment `fight` judges, for `reader`: a pick that fights
    /// is marked with the Gaps that keep the fight from being judged or may hide a better plan.
    pub(crate) fn card(&self, fight: &Fight, reader: Reader) -> SuggestedActionCard {
        SuggestedActionCard {
            level: self.rule.level(),
            rule: self.rule.name().to_owned(),
            action: self.action.value(fight, reader),
            if_lethal: (self.rule == SuggestionRule::FightCarefully).then(|| {
                self.if_lethal.as_ref().map_or_else(
                    || MarkedValue::known("no Way Out left".to_owned()),
                    |escape| escape.value(reader),
                )
            }),
        }
    }
}

/// What fight on asks of a Candidate Action, kept with the rule.
impl CandidateAction {
    /// Whether it fights by an Attack Plan that wins and leaves hit points to spare out of
    /// `hp_max` as fight on asks, or within `slack` points of it.
    fn fights_on(&self, hp_max: Option<i32>, slack: u64) -> bool {
        let (
            Self::Fight {
                odds: Odds::Now(plan),
                hp,
            },
            Some(hp_max),
        ) = (self, hp_max)
        else {
            return false;
        };
        let wins = plan
            .win
            .at_least_percent(FIGHT_ON_WIN_PERCENT.saturating_sub(slack));
        let hp_left = u64::try_from(hp - plan.hp_lost).unwrap_or(0);
        let spares = hp_left * 100
            >= FIGHT_ON_HP_LEFT_PERCENT.saturating_sub(slack)
                * u64::try_from(hp_max).unwrap_or(u64::MAX);
        wins && spares
    }
}

impl FactSheet {
    /// The player's hit points the Threat Level was judged against, when it was judged.
    fn threat_hp(&self) -> Option<i32> {
        match &self.threat {
            Threat::Rated(assessment) => Some(assessment.hp),
            Threat::Unrated { .. } => None,
        }
    }
}

/// The Way Out the LETHAL rules take, with the rule that takes it: a staircase, else something
/// that heals, else the best escape.
fn lethal_way_out(ways_out: &WaysOut) -> Option<(SuggestionRule, &Escape)> {
    stairs(ways_out)
        .map(|escape| (SuggestionRule::LeaveByStairs, escape))
        .or_else(|| {
            carried(ways_out, Relief::Heals).map(|escape| (SuggestionRule::HealOutOfIt, escape))
        })
        .or_else(|| {
            carried(ways_out, Relief::Escapes)
                .or_else(|| carried(ways_out, Relief::EscapesLater))
                .map(|escape| (SuggestionRule::BestEscape, escape))
        })
}

/// The staircase to take of those the player can use: one judged on what the game shows now
/// before one judged over remembered squares, then the nearest.
fn stairs(ways_out: &WaysOut) -> Option<&Escape> {
    usable(ways_out)
        .filter_map(|way_out| match &way_out.option {
            Escape::Stairs { steps, .. } => Some((way_out, *steps)),
            Escape::Item { .. } | Escape::Ability { .. } | Escape::Spell { .. } => None,
        })
        .min_by_key(|(way_out, steps)| (!way_out.trusted(), *steps))
        .map(|(way_out, _)| &way_out.option)
}

/// The item, ability or spell to use of those the player can that bring `relief`: one trusted
/// before one read off a Stale screen, then the least likely to fail, then the first listed.
fn carried(ways_out: &WaysOut, relief: Relief) -> Option<&Escape> {
    usable(ways_out)
        .filter_map(|way_out| {
            let failure_percent = match &way_out.option {
                Escape::Item { effect, .. } => (*effect == relief).then_some(0)?,
                Escape::Ability {
                    effect,
                    failure_percent,
                    ..
                }
                | Escape::Spell {
                    effect,
                    failure_percent,
                    ..
                } => (*effect == relief).then_some(*failure_percent)?,
                Escape::Stairs { .. } => return None,
            };
            Some((way_out, failure_percent))
        })
        .min_by_key(|(way_out, failure_percent)| (!way_out.trusted(), *failure_percent))
        .map(|(way_out, _)| &way_out.option)
}

#[cfg(test)]
mod tests {
    use copilot_gk::StairDirection;

    use super::*;
    use crate::fact_sheet::{Chance, ThreatAssessment};
    use crate::fight_outcome::{AttackPlan, FightOutcome, PlanBase, PlanOpening, PlanOutcome};
    use crate::visible_state::Position;
    use crate::ways_out::{CorridorSquare, Ground, ReadOn, Usable, WayOut};

    const TRIALS: u32 = 1000;
    const HP: i32 = 50;
    const HP_MAX: i32 = 100;

    /// A moment at `level`, with `hostiles` in view, the player at `HP` of `HP_MAX`, the
    /// `ways_out` and `fight` given, on open ground with no corridor in reach.
    fn moment(level: ThreatLevel, hostiles: u32, ways_out: Vec<WayOut>, fight: Fight) -> FactSheet {
        FactSheet {
            threat: Threat::Rated(Box::new(ThreatAssessment {
                level,
                hp: HP,
                death: Chance {
                    times: 0,
                    trials: TRIALS,
                },
                worst_hit: 0,
                hostiles_in_view: hostiles,
                best_plan_win: None,
                acting: Vec::new(),
                gear: Vec::new(),
                lighting: None,
                resistances: None,
                not_modelled: Vec::new(),
                gaps: Vec::new(),
            })),
            ways_out: WaysOut {
                options: ways_out,
                ground: Some(Ground::OpenGround {
                    open_sides: 8,
                    nearest_corridor: None,
                }),
                ..WaysOut::default()
            },
            fight,
            exceptional: Vec::new(),
            dangerous_statuses: Vec::new(),
            gaps: Vec::new(),
        }
    }

    fn usable(option: Escape) -> WayOut {
        WayOut {
            option,
            usable: Usable::Yes,
        }
    }

    fn unusable(option: Escape) -> WayOut {
        WayOut {
            option,
            usable: Usable::No {
                because: vec![crate::ways_out::Blocker::Useless],
            },
        }
    }

    /// A staircase down at `x`, `steps` away, judged over remembered squares or not.
    fn stairs(x: i32, steps: u32, over_remembered_squares: bool) -> Escape {
        Escape::Stairs {
            name: "stone staircase leading down".to_owned(),
            direction: StairDirection::Down,
            position: Position { x, y: 0 },
            steps,
            over_remembered_squares,
        }
    }

    fn item(letter: char, kind_name: &str, effect: Relief) -> Escape {
        Escape::Item {
            letter,
            name: kind_name.to_owned(),
            kind_name: kind_name.to_owned(),
            quantity: 1,
            effect,
        }
    }

    fn spell(name: &str, effect: Relief, failure_percent: u32) -> Escape {
        Escape::Spell {
            name: name.to_owned(),
            effect,
            failure_percent,
            read: ReadOn {
                turn: Some(1),
                stale: false,
            },
        }
    }

    /// An Attack Plan opening with the spell `opening`, or melee throughout, that wins `wins` of
    /// `TRIALS` fights and costs `hp_lost` hit points on average.
    fn plan(opening: Option<&str>, wins: u32, hp_lost: i32) -> PlanOutcome {
        PlanOutcome {
            plan: AttackPlan {
                opening: opening.map(|name| PlanOpening::Spell {
                    name: name.to_owned(),
                }),
                base: PlanBase::Melee,
            },
            win: Chance {
                times: wins,
                trials: TRIALS,
            },
            death: Chance {
                times: TRIALS - wins,
                trials: TRIALS,
            },
            hp_lost,
        }
    }

    /// A fight judged with `plans`, the best first.
    fn judged(plans: Vec<PlanOutcome>) -> Fight {
        Fight::Judged(Box::new(FightOutcome {
            hp: HP,
            plans,
            not_played: Vec::new(),
            gaps: Vec::new(),
            not_modelled: Vec::new(),
        }))
    }

    fn rule_at(fact_sheet: &FactSheet) -> Option<SuggestionRule> {
        SuggestedAction::pick(fact_sheet, Some(HP_MAX)).map(|picked| picked.rule)
    }

    /// The pick at `now`, after `held` was picked.
    fn after(held: &FactSheet, now: &FactSheet) -> SuggestedAction {
        let held = SuggestedAction::pick(held, Some(HP_MAX)).unwrap();
        SuggestedAction::steady(Some(&held), now, Some(HP_MAX)).unwrap()
    }

    fn with_corridor(mut fact_sheet: FactSheet, steps: u32) -> FactSheet {
        fact_sheet.ways_out.ground = Some(Ground::OpenGround {
            open_sides: 8,
            nearest_corridor: Some(CorridorSquare {
                position: Position { x: 9, y: 9 },
                steps,
            }),
        });
        fact_sheet
    }

    #[test]
    fn at_lethal_the_stairs_come_first_then_healing_then_an_escape_at_once_then_a_later_one() {
        let lethal = |ways_out| {
            moment(
                ThreatLevel::Lethal,
                1,
                ways_out,
                judged(vec![plan(None, 300, 40)]),
            )
        };
        let teleport = usable(item('t', "teleportation", Relief::EscapesLater));
        let blink = usable(spell("Blink", Relief::Escapes, 20));
        let heal = usable(item('h', "heal wounds", Relief::Heals));
        let down = usable(stairs(1, 6, false));

        let all = vec![teleport.clone(), blink.clone(), heal.clone(), down];
        assert_eq!(rule_at(&lethal(all)), Some(SuggestionRule::LeaveByStairs));
        let no_stairs = vec![teleport.clone(), blink.clone(), heal];
        assert_eq!(
            rule_at(&lethal(no_stairs)),
            Some(SuggestionRule::HealOutOfIt)
        );

        let picked =
            SuggestedAction::pick(&lethal(vec![teleport.clone(), blink]), Some(HP_MAX)).unwrap();
        assert_eq!(picked.rule, SuggestionRule::BestEscape);
        assert_eq!(
            picked.card(&Fight::NoFight, Reader::Player).action.text,
            "Blink · spell · 20% fail"
        );
        assert_eq!(
            rule_at(&lethal(vec![teleport])),
            Some(SuggestionRule::BestEscape)
        );

        let cornered =
            SuggestedAction::pick(&lethal(vec![unusable(stairs(1, 6, false))]), Some(HP_MAX))
                .unwrap();
        assert_eq!(cornered.rule, SuggestionRule::Fight);
        assert_eq!(
            cornered.card(&Fight::NoFight, Reader::Player).action.text,
            "Melee: ~30% win, ~80% HP lost"
        );
    }

    #[test]
    fn a_staircase_judged_on_what_the_game_shows_is_taken_before_a_nearer_one_judged_from_memory() {
        let lethal = moment(
            ThreatLevel::Lethal,
            1,
            vec![
                usable(stairs(1, 3, true)),
                usable(stairs(2, 9, false)),
                usable(stairs(3, 5, false)),
            ],
            Fight::NoFight,
        );

        let picked = SuggestedAction::pick(&lethal, Some(HP_MAX)).unwrap();

        assert_eq!(
            picked.card(&Fight::NoFight, Reader::Player).action.text,
            "Stone staircase leading down · 5 steps"
        );
    }

    #[test]
    fn of_two_escapes_alike_the_one_least_likely_to_fail_is_used() {
        let lethal = moment(
            ThreatLevel::Lethal,
            1,
            vec![
                usable(spell("Blink", Relief::Escapes, 30)),
                usable(spell("Passage of Golubria", Relief::Escapes, 10)),
            ],
            Fight::NoFight,
        );

        let picked = SuggestedAction::pick(&lethal, Some(HP_MAX)).unwrap();

        assert_eq!(
            picked.card(&Fight::NoFight, Reader::Player).action.text,
            "Passage of Golubria · spell · 10% fail"
        );
    }

    #[test]
    fn at_dangerous_fight_on_needs_ninety_percent_to_win_and_thirty_percent_of_hp_left() {
        let dangerous = |wins, hp_lost| {
            moment(
                ThreatLevel::Dangerous,
                1,
                vec![usable(stairs(1, 6, false))],
                judged(vec![plan(None, wins, hp_lost)]),
            )
        };

        // 50 HP less 20 leaves 30 of 100.
        assert_eq!(rule_at(&dangerous(900, 20)), Some(SuggestionRule::FightOn));
        assert_eq!(
            rule_at(&dangerous(899, 20)),
            Some(SuggestionRule::FallBackToStairs)
        );
        assert_eq!(
            rule_at(&dangerous(900, 21)),
            Some(SuggestionRule::FallBackToStairs)
        );
    }

    #[test]
    fn at_dangerous_a_corridor_is_fallen_back_to_or_held_only_among_several_hostiles() {
        let dangerous = |hostiles| {
            with_corridor(
                moment(
                    ThreatLevel::Dangerous,
                    hostiles,
                    vec![usable(stairs(1, 6, false))],
                    judged(vec![plan(None, 500, 40)]),
                ),
                3,
            )
        };

        let picked = SuggestedAction::pick(&dangerous(2), Some(HP_MAX)).unwrap();
        assert_eq!(picked.rule, SuggestionRule::FallBackToACorridor);
        assert_eq!(
            picked.card(&Fight::NoFight, Reader::Player).action.text,
            "The corridor 3 steps away"
        );
        assert_eq!(
            rule_at(&dangerous(1)),
            Some(SuggestionRule::FallBackToStairs)
        );
        let mut in_one = dangerous(2);
        in_one.ways_out.ground = Some(Ground::Corridor { open_sides: 2 });
        let held = SuggestedAction::pick(&in_one, Some(HP_MAX)).unwrap();
        assert_eq!(held.rule, SuggestionRule::FallBackToACorridor);
        assert_eq!(
            held.card(&Fight::NoFight, Reader::Player).action.text,
            "The corridor you stand in"
        );
        assert!(held.same_pick(&picked));
    }

    #[test]
    fn fighting_carefully_lists_the_way_out_to_take_should_it_turn_lethal() {
        let dangerous = |ways_out| {
            moment(
                ThreatLevel::Dangerous,
                1,
                ways_out,
                judged(vec![plan(None, 600, 30)]),
            )
        };

        let card = SuggestedAction::pick(
            &dangerous(vec![usable(item(
                't',
                "teleportation",
                Relief::EscapesLater,
            ))]),
            Some(HP_MAX),
        )
        .unwrap()
        .card(&Fight::NoFight, Reader::Player);
        assert_eq!(card.rule, "Fight carefully");
        assert_eq!(card.action.text, "Melee: ~60% win, ~60% HP lost");
        assert_eq!(
            card.if_lethal.map(|value| value.text).as_deref(),
            Some("Teleportation · escapes after a delay")
        );

        let cornered = SuggestedAction::pick(&dangerous(Vec::new()), Some(HP_MAX)).unwrap();
        assert_eq!(
            cornered
                .card(&Fight::NoFight, Reader::Player)
                .if_lethal
                .map(|value| value.text)
                .as_deref(),
            Some("no Way Out left")
        );
    }

    #[test]
    fn nothing_is_suggested_once_the_character_is_dead() {
        let mut dead = moment(
            ThreatLevel::Lethal,
            1,
            Vec::new(),
            judged(vec![plan(None, 0, HP)]),
        );
        if let Threat::Rated(assessment) = &mut dead.threat {
            assessment.hp = -2;
        }

        assert_eq!(rule_at(&dead), None);
    }

    #[test]
    fn nothing_is_suggested_at_calm_or_when_the_moment_isnt_judged() {
        let calm = moment(
            ThreatLevel::Calm,
            1,
            vec![usable(stairs(1, 6, false))],
            Fight::NoFight,
        );
        let mut unrated = calm.clone();
        unrated.threat = Threat::Unrated { gaps: Vec::new() };

        assert_eq!(rule_at(&calm), None);
        assert_eq!(rule_at(&unrated), None);
    }

    #[test]
    fn a_pick_holds_while_it_can_be_done_though_the_rules_would_pick_otherwise_now() {
        let lethal = |ways_out| moment(ThreatLevel::Lethal, 1, ways_out, Fight::NoFight);
        let far = usable(stairs(1, 6, false));
        let near = usable(stairs(2, 2, false));

        // A nearer staircase comes into reach.
        let held = after(&lethal(vec![far.clone()]), &lethal(vec![near, far.clone()]));
        assert_eq!(
            held.card(&Fight::NoFight, Reader::Player).action.text,
            "Stone staircase leading down · 6 steps"
        );

        // The moment eases to DANGEROUS, where the fight would be fought on.
        let eased = moment(
            ThreatLevel::Dangerous,
            1,
            vec![far.clone()],
            judged(vec![plan(None, 950, 5)]),
        );
        assert_eq!(
            after(&lethal(vec![far]), &eased).rule,
            SuggestionRule::LeaveByStairs
        );

        // The fight on wins less often now, but no rule above it applies.
        let fight = |wins| {
            with_corridor(
                moment(
                    ThreatLevel::Dangerous,
                    2,
                    Vec::new(),
                    judged(vec![plan(None, wins, 5)]),
                ),
                3,
            )
        };
        assert_eq!(
            after(&fight(950), &fight(850)).rule,
            SuggestionRule::FightOn
        );
    }

    #[test]
    fn fighting_on_holds_through_a_dip_of_under_ten_points_below_its_thresholds() {
        let fight = |wins, hp_lost| {
            with_corridor(
                moment(
                    ThreatLevel::Dangerous,
                    2,
                    Vec::new(),
                    judged(vec![plan(None, wins, hp_lost)]),
                ),
                3,
            )
        };

        assert_eq!(
            after(&fight(950, 5), &fight(800, 5)).rule,
            SuggestionRule::FightOn
        );
        assert_eq!(
            after(&fight(950, 5), &fight(799, 5)).rule,
            SuggestionRule::FallBackToACorridor
        );
        // 50 HP less 30 leaves 20 of 100.
        assert_eq!(
            after(&fight(950, 5), &fight(950, 30)).rule,
            SuggestionRule::FightOn
        );
        assert_eq!(
            after(&fight(950, 5), &fight(950, 31)).rule,
            SuggestionRule::FallBackToACorridor
        );
    }

    #[test]
    fn a_fight_that_cant_be_judged_for_a_moment_keeps_its_plan() {
        let judged_fight = |wins| {
            moment(
                ThreatLevel::Lethal,
                1,
                Vec::new(),
                judged(vec![plan(Some("Fireball"), wins, 30)]),
            )
        };
        let too_many = moment(
            ThreatLevel::Lethal,
            4,
            Vec::new(),
            Fight::TooMany { hostiles: 4 },
        );

        let held = after(&judged_fight(600), &too_many);
        assert_eq!(
            held.card(&Fight::NoFight, Reader::Player).action.text,
            "Fireball, then melee: can't be judged now"
        );
        let judged_again =
            SuggestedAction::steady(Some(&held), &judged_fight(500), Some(HP_MAX)).unwrap();
        assert!(judged_again.same_pick(&held));
        assert_eq!(
            judged_again
                .card(&Fight::NoFight, Reader::Player)
                .action
                .text,
            "Fireball, then melee: ~50% win, ~60% HP lost"
        );
    }

    #[test]
    fn a_last_resort_fight_gives_way_once_a_way_out_the_lethal_rules_take_opens() {
        let lethal = moment(
            ThreatLevel::Lethal,
            1,
            vec![unusable(stairs(1, 6, false))],
            judged(vec![plan(None, 950, 5)]),
        );
        let eased = moment(
            ThreatLevel::Dangerous,
            1,
            vec![usable(stairs(1, 5, false))],
            judged(vec![plan(None, 600, 30)]),
        );

        let held = SuggestedAction::pick(&lethal, Some(HP_MAX)).unwrap();
        assert_eq!(
            held.card(&Fight::NoFight, Reader::Player).rule,
            "Last resort: fight"
        );
        assert_eq!(
            after(&lethal, &eased).rule,
            SuggestionRule::FallBackToStairs
        );
    }

    #[test]
    fn a_pick_changes_once_it_can_no_longer_be_done() {
        let lethal = |ways_out| moment(ThreatLevel::Lethal, 1, ways_out, Fight::NoFight);
        let blink = usable(spell("Blink", Relief::Escapes, 20));

        let now = after(
            &lethal(vec![usable(stairs(1, 6, false)), blink.clone()]),
            &lethal(vec![unusable(stairs(1, 5, false)), blink]),
        );

        assert_eq!(now.rule, SuggestionRule::BestEscape);
    }

    #[test]
    fn a_pick_changes_when_a_rule_that_outranks_it_applies() {
        let lethal = |ways_out| moment(ThreatLevel::Lethal, 1, ways_out, Fight::NoFight);
        let teleport = usable(item('t', "teleportation", Relief::EscapesLater));
        let heal = item('h', "heal wounds", Relief::Heals);

        let now = after(
            &lethal(vec![teleport.clone(), unusable(heal.clone())]),
            &lethal(vec![teleport, usable(heal)]),
        );
        assert_eq!(now.rule, SuggestionRule::HealOutOfIt);

        let dangerous = |level| moment(level, 1, Vec::new(), judged(vec![plan(None, 950, 5)]));
        let risen = after(
            &dangerous(ThreatLevel::Dangerous),
            &dangerous(ThreatLevel::Lethal),
        );
        assert_eq!(risen.rule, SuggestionRule::Fight);
    }

    #[test]
    fn a_fight_changes_plan_for_one_that_wins_ten_points_more_and_not_for_less() {
        let fight = |melee, fireball| {
            let mut plans = vec![plan(None, melee, 30), plan(Some("Fireball"), fireball, 30)];
            plans.sort_by_key(|plan| std::cmp::Reverse(plan.win.times));
            moment(ThreatLevel::Dangerous, 1, Vec::new(), judged(plans))
        };

        let nine_more = after(&fight(700, 600), &fight(650, 740));
        assert_eq!(
            nine_more.card(&Fight::NoFight, Reader::Player).action.text,
            "Melee: ~65% win, ~60% HP lost"
        );
        let ten_more = after(&fight(700, 600), &fight(650, 750));
        assert_eq!(
            ten_more.card(&Fight::NoFight, Reader::Player).action.text,
            "Fireball, then melee: ~75% win, ~60% HP lost"
        );
    }

    #[test]
    fn the_same_pick_is_the_same_rule_and_thing_whatever_its_facts_now() {
        let lethal = |ways_out| moment(ThreatLevel::Lethal, 1, ways_out, Fight::NoFight);
        let pick = |ways_out| SuggestedAction::pick(&lethal(ways_out), Some(HP_MAX)).unwrap();

        let six = pick(vec![usable(stairs(1, 6, false))]);
        assert!(six.same_pick(&pick(vec![usable(stairs(1, 2, false))])));
        assert!(!six.same_pick(&pick(vec![usable(stairs(2, 6, false))])));
    }
}
