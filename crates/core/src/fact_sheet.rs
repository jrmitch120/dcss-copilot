//! The Fact Sheet: everything the Advisor has computed about one moment, and the Gaps that kept
//! it from computing more.

use std::collections::BTreeSet;

use copilot_gk::Bundle;
use serde::Serialize;
use specta::Type;

use crate::accumulated_state::AccumulatedState;
use crate::engine::threat::{self, Attacker, Defender, Outcome};
use crate::visible_state::{Attitude, Position, VisibleMonster, VisibleState};

/// LETHAL from this chance of death before the player's next action, per mille: 5%. A starting
/// threshold from the spec (#24, "Engine (ADR 0003)"), to be tuned on the corpus.
const LETHAL_DEATHS_PER_MILLE: u32 = 50;

/// DANGEROUS from this chance of death, per mille: 0.5%. A starting threshold from the spec (#24,
/// "Engine (ADR 0003)"), to be tuned on the corpus.
const DANGEROUS_DEATHS_PER_MILLE: u32 = 5;

/// DANGEROUS when the worst-case hit is at least this percentage of current HP. A starting
/// threshold from the spec (#24, "Engine (ADR 0003)"), to be tuned on the corpus.
const DANGEROUS_WORST_HIT_PERCENT_OF_HP: i32 = 40;

/// Everything the Advisor has computed about one moment of the game it follows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FactSheet {
    /// How dangerous this moment is.
    pub threat: Threat,
}

/// How dangerous this moment is, or why the Advisor can't tell.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(
    tag = "state",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Threat {
    /// The Advisor judged the moment.
    Rated(Box<ThreatAssessment>),
    /// The Advisor lacks facts it needs to judge the moment, so it judges nothing.
    Unrated {
        /// What it lacks.
        gaps: Vec<Gap>,
    },
}

/// The Threat Level and the numbers it was judged from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreatAssessment {
    /// How dangerous the moment is.
    pub level: ThreatLevel,
    /// The player's hit points it was judged against.
    pub hp: i32,
    /// The chance the player dies before their next standard-speed action. Nothing is played
    /// out when nothing can attack.
    pub death: Chance,
    /// The total damage the player takes before their next standard-speed action at the 99th
    /// percentile: as bad as it gets, barring one time in a hundred.
    pub worst_hit: i32,
    /// How many monsters in view would attack the player, whether or not they can yet.
    pub hostiles_in_view: u32,
    /// The monsters that can attack before the player's next standard-speed action, in map
    /// order.
    pub acting: Vec<ActingMonster>,
    /// What the judgement leaves out, so the numbers are read for what they are.
    pub not_modelled: Vec<NotModelled>,
}

/// How dangerous a moment is: the Threat Level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum ThreatLevel {
    /// Neither likely to die nor to be hit hard before the next action.
    Calm,
    /// A real chance of death, or a hard hit, before the next action.
    Dangerous,
    /// Death before the next action is likely enough, or a hit could take every hit point.
    Lethal,
}

/// How often something happened when the engine played a moment out many times: exact, as
/// counted, rather than rounded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Chance {
    /// Trials in which it happened.
    pub times: u32,
    /// Trials played out.
    pub trials: u32,
}

impl Chance {
    /// No chance at all: nothing could happen, so nothing was played out.
    const NONE: Self = Self {
        times: 0,
        trials: 0,
    };

    /// Whether the chance is at least `per_mille` in a thousand, for a `per_mille` above 0.
    fn at_least_per_mille(self, per_mille: u32) -> bool {
        self.times > 0
            && u64::from(self.times) * 1000 >= u64::from(per_mille) * u64::from(self.trials)
    }

    /// The chance as a percentage.
    #[must_use]
    pub fn percent(self) -> f64 {
        if self.trials == 0 {
            return 0.0;
        }
        f64::from(self.times) * 100.0 / f64::from(self.trials)
    }
}

/// A monster that can attack the player before their next standard-speed action.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActingMonster {
    /// Its name, as the game names it.
    pub name: String,
    /// The square it is on.
    pub position: Position,
    /// Squares between it and the player.
    pub distance: i32,
}

/// Something the Threat Level's simulation doesn't model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum NotModelled {
    /// Monsters fight as if unarmed and unarmoured, with no weapon damage or brands.
    MonsterEquipment,
    /// Attack flavours count only their base damage: no poison, fire, draining and the like.
    AttackFlavours,
    /// Only melee: no spells, breath, wands or missiles.
    RangedAndSpells,
    /// Statuses on either side, such as haste, might or confusion.
    Statuses,
    /// Monsters walk straight at the player, through walls and each other.
    Paths,
    /// Monsters are assumed awake and hunting the player.
    Awareness,
    /// The player's shield is assumed to block at most one attack before their next action, as
    /// without a shield, because the Advisor doesn't know which shield is worn. The character
    /// overview (`%`) tells it.
    WornShield,
}

/// What the melee simulation leaves out, whatever is in view.
const MELEE_NOT_MODELLED: [NotModelled; 6] = [
    NotModelled::MonsterEquipment,
    NotModelled::AttackFlavours,
    NotModelled::RangedAndSpells,
    NotModelled::Statuses,
    NotModelled::Paths,
    NotModelled::Awareness,
];

/// A fact the Advisor needs about this moment but doesn't have.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(tag = "gap", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Gap {
    /// The player's hit points haven't been reported.
    HitPoints,
    /// The player's AC, EV or SH haven't been reported.
    Defences,
    /// The game hasn't sent its map, so the monsters in view aren't known.
    MonstersInView,
    /// Where the player stands hasn't been reported.
    Position,
    /// There is no Game Knowledge for the game's Crawl Version, so monsters can't be judged.
    GameKnowledge,
    /// A monster in view isn't in the Game Knowledge for the game's Crawl Version.
    MonsterKnowledge {
        /// The monster, as the game names it.
        monster: String,
    },
}

impl FactSheet {
    /// What the engine makes of `visible` and what the Advisor remembers of the game, with
    /// `knowledge` for the game's Crawl Version if the Advisor has it.
    #[must_use]
    pub fn compute(
        visible: &VisibleState,
        remembered: Option<&AccumulatedState>,
        knowledge: Option<&Bundle>,
    ) -> Self {
        Self {
            threat: threat(visible, remembered, knowledge),
        }
    }
}

/// Monsters on these sides attack the player: crawl's `mons_att_wont_attack` (`mon-util.cc`)
/// spares only friends, good neutrals and marionettes, and strict neutrals don't hunt the player.
fn attacks_player(attitude: Attitude) -> bool {
    matches!(attitude, Attitude::Hostile | Attitude::Neutral)
}

fn threat(
    visible: &VisibleState,
    remembered: Option<&AccumulatedState>,
    knowledge: Option<&Bundle>,
) -> Threat {
    let mut gaps = BTreeSet::new();
    let hp = visible.stats.hp;
    if hp.is_none() {
        gaps.insert(Gap::HitPoints);
    }
    let Some(monsters) = &visible.monsters else {
        gaps.insert(Gap::MonstersInView);
        return Threat::Unrated {
            gaps: gaps.into_iter().collect(),
        };
    };

    // The monsters that may attack, with their Game Knowledge.
    let mut hostile: Vec<(&VisibleMonster, &copilot_gk::Monster)> = Vec::new();
    for monster in monsters {
        let Some(attitude) = monster.attitude else {
            gaps.insert(Gap::GameKnowledge);
            continue;
        };
        if !attacks_player(attitude) {
            continue;
        }
        let Some(bundle) = knowledge else {
            gaps.insert(Gap::GameKnowledge);
            continue;
        };
        match monster.id.as_ref().and_then(|id| bundle.monster_by_id(id)) {
            Some(known) => hostile.push((monster, known)),
            None => {
                gaps.insert(Gap::MonsterKnowledge {
                    monster: monster.name.clone(),
                });
            }
        }
    }

    let stats = &visible.stats;
    let defences = match (stats.ac, stats.ev, stats.sh) {
        (Some(ac), Some(ev), Some(sh)) => Some((ac, ev, sh)),
        _ => None,
    };
    if !hostile.is_empty() {
        if defences.is_none() {
            gaps.insert(Gap::Defences);
        }
        if visible.position.is_none() {
            gaps.insert(Gap::Position);
        }
    }
    let (Some(hp), true) = (hp, gaps.is_empty()) else {
        return Threat::Unrated {
            gaps: gaps.into_iter().collect(),
        };
    };
    let (false, Some(bundle), Some(player), Some((ac, ev, sh))) =
        (hostile.is_empty(), knowledge, visible.position, defences)
    else {
        // Nothing in view attacks the player: a hostile would have needed the rest, or been a Gap.
        return Threat::Rated(Box::new(ThreatAssessment {
            level: ThreatLevel::Calm,
            hp,
            death: Chance::NONE,
            worst_hit: 0,
            hostiles_in_view: 0,
            acting: Vec::new(),
            not_modelled: Vec::new(),
        }));
    };

    let formulas = bundle.formulas;
    let worn_shield = remembered.and_then(|state| state.worn_shield.known().copied());
    let mut not_modelled = MELEE_NOT_MODELLED.to_vec();
    if sh > 0 && worn_shield.is_none() {
        not_modelled.push(NotModelled::WornShield);
    }
    let defender = Defender {
        hp,
        ac,
        ev,
        sh,
        shield_blocks: threat::shield_blocks_per_action(formulas, worn_shield.flatten()),
    };
    let hostiles_in_view = u32::try_from(hostile.len()).unwrap_or(u32::MAX);
    let acting: Vec<(&VisibleMonster, Attacker<'_>)> = hostile
        .into_iter()
        .map(|(seen, monster)| {
            let attacker = Attacker {
                monster,
                distance: distance(player, seen.position),
            };
            (seen, attacker)
        })
        .filter(|(_, attacker)| threat::can_attack(formulas, attacker))
        .collect();
    let attackers: Vec<Attacker<'_>> = acting.iter().map(|(_, attacker)| *attacker).collect();
    let outcome = if attackers.is_empty() {
        Outcome {
            deaths: 0,
            trials: 0,
            worst_hit: 0,
        }
    } else {
        threat::simulate(formulas, defender, &attackers)
    };
    let death = Chance {
        times: outcome.deaths,
        trials: outcome.trials,
    };
    Threat::Rated(Box::new(ThreatAssessment {
        level: level(hp, death, outcome.worst_hit),
        hp,
        death,
        worst_hit: outcome.worst_hit,
        hostiles_in_view,
        acting: acting
            .iter()
            .map(|(seen, attacker)| ActingMonster {
                name: seen.name.clone(),
                position: seen.position,
                distance: attacker.distance,
            })
            .collect(),
        not_modelled,
    }))
}

/// The Threat Level for a chance of death and a worst-case hit against `hp`.
fn level(hp: i32, death: Chance, worst_hit: i32) -> ThreatLevel {
    if death.at_least_per_mille(LETHAL_DEATHS_PER_MILLE) || worst_hit >= hp {
        ThreatLevel::Lethal
    } else if death.at_least_per_mille(DANGEROUS_DEATHS_PER_MILLE)
        || worst_hit * 100 >= hp * DANGEROUS_WORST_HIT_PERCENT_OF_HP
    {
        ThreatLevel::Dangerous
    } else {
        ThreatLevel::Calm
    }
}

/// Squares between two positions, counting a diagonal step as one: crawl's `grid_distance`.
fn distance(a: Position, b: Position) -> i32 {
    (a.x - b.x).abs().max((a.y - b.y).abs())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chance(times: u32) -> Chance {
        Chance {
            times,
            trials: 10_000,
        }
    }

    #[test]
    fn a_five_percent_chance_of_death_is_lethal_and_just_under_it_dangerous() {
        assert_eq!(level(100, chance(500), 0), ThreatLevel::Lethal);
        assert_eq!(level(100, chance(499), 0), ThreatLevel::Dangerous);
    }

    #[test]
    fn a_half_percent_chance_of_death_is_dangerous_and_just_under_it_calm() {
        assert_eq!(level(100, chance(50), 0), ThreatLevel::Dangerous);
        assert_eq!(level(100, chance(49), 0), ThreatLevel::Calm);
    }

    #[test]
    fn nothing_able_to_attack_is_calm() {
        assert_eq!(level(1, Chance::NONE, 0), ThreatLevel::Calm);
    }

    #[test]
    fn a_worst_hit_of_all_hp_is_lethal_and_of_forty_percent_dangerous() {
        assert_eq!(level(50, chance(0), 50), ThreatLevel::Lethal);
        assert_eq!(level(50, chance(0), 49), ThreatLevel::Dangerous);
        assert_eq!(level(50, chance(0), 20), ThreatLevel::Dangerous);
        assert_eq!(level(50, chance(0), 19), ThreatLevel::Calm);
    }
}
