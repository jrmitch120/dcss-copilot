//! The Threat Level's Monte Carlo: every monster that can reach the player before their next
//! standard-speed action, fighting in melee through crawl's own dice, many times over.

use copilot_gk::{Formulas, Monster};

use super::melee::{
    self, ENERGY_THRESHOLD, apply_ac, attack_reach, energy_gained, evasion_margin, monster_damage,
    monster_to_hit, shield_blocks, starting_energy,
};
use super::rng::Rng;

/// How many times the next standard-speed action is played out. At 10,000 the chance of death
/// is within about 0.15 percentage points of its true value near the DANGEROUS threshold.
pub(crate) const TRIALS: u32 = 10_000;

/// The seed the Monte Carlo always starts from, so the same Game Situation always gives the same
/// Fact Sheet: the engine reads no clock and no entropy.
const SEED: (u64, u64) = (0x00dc_55c0_9170_7000, 0x0000_0000_0000_0034);

/// The worst-case hit is the total damage at this percentile of trials (spec #24, Engine).
const WORST_HIT_PERCENTILE: u32 = 99;

/// How many attacks the player's shield can block before their next action. Crawl allows one
/// without a shield item and two to four with one (`actor::shield_block_limit`, `actor.cc`), and
/// the Advisor can't yet tell which shield is worn, so it assumes the fewest.
const SHIELD_BLOCKS_PER_TURN: u32 = 1;

/// The player's defences, as the game reports them.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Defender {
    /// Current hit points.
    pub(crate) hp: i32,
    /// Armour class, as displayed.
    pub(crate) ac: i32,
    /// Evasion, as displayed.
    pub(crate) ev: i32,
    /// Shield class, as displayed.
    pub(crate) sh: i32,
}

/// A monster that may attack the player, and how far away it is.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Attacker<'a> {
    /// Its Game Knowledge.
    pub(crate) monster: &'a Monster,
    /// Squares between it and the player, counting diagonals as one, as crawl's `grid_distance`.
    pub(crate) distance: i32,
}

/// What playing out the player's next action many times showed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Outcome {
    /// Trials in which the player died.
    pub(crate) deaths: u32,
    /// Trials played out.
    pub(crate) trials: u32,
    /// The total damage taken at the 99th percentile of trials.
    pub(crate) worst_hit: i32,
}

/// Whether `attacker` can attack before the player's next standard-speed action at all: with the
/// most energy it could have, it closes the distance and still has an action left.
pub(crate) fn can_attack(formulas: Formulas, attacker: &Attacker<'_>) -> bool {
    let monster = attacker.monster;
    let reach = reach(formulas, monster);
    if reach == 0 {
        return false;
    }
    let mut energy = melee::most_starting_energy(formulas.monster_energy) + speed(monster);
    let mut distance = attacker.distance;
    while energy >= ENERGY_THRESHOLD {
        if distance <= reach {
            return true;
        }
        distance -= 1;
        energy -= cost(monster.energy.walk);
    }
    false
}

/// Plays out the player's next standard-speed action [`TRIALS`] times against `attackers`.
pub(crate) fn simulate(
    formulas: Formulas,
    defender: Defender,
    attackers: &[Attacker<'_>],
) -> Outcome {
    let mut rng = Rng::new(SEED.0, SEED.1);
    let mut totals: Vec<i32> = (0..TRIALS)
        .map(|_| one_action(formulas, &mut rng, defender, attackers))
        .collect();
    totals.sort_unstable();
    let deaths = totals.iter().filter(|&&total| total >= defender.hp).count();
    // The nearest-rank percentile: the smallest total at least 99% of trials stay within.
    let rank = (TRIALS * WORST_HIT_PERCENTILE).div_ceil(100);
    Outcome {
        deaths: u32::try_from(deaths).unwrap_or(TRIALS),
        trials: TRIALS,
        worst_hit: totals
            .get(usize::try_from(rank).unwrap_or(0).saturating_sub(1))
            .copied()
            .unwrap_or(0),
    }
}

/// The damage the player takes in one trial of their next action.
fn one_action(
    formulas: Formulas,
    rng: &mut Rng,
    defender: Defender,
    attackers: &[Attacker<'_>],
) -> i32 {
    let mut total = 0;
    let mut blocks_left = SHIELD_BLOCKS_PER_TURN;
    for attacker in attackers {
        let monster = attacker.monster;
        let reach = reach(formulas, monster);
        let hit_dice = i32::try_from(monster.hd).unwrap_or(i32::MAX);
        let mut distance = attacker.distance;
        let mut energy = starting_energy(formulas.monster_energy, rng)
            + energy_gained(formulas.monster_energy, rng, speed(monster));
        while energy >= ENERGY_THRESHOLD && reach > 0 {
            if distance > reach {
                // It walks straight at the player: walls and other monsters aren't modelled.
                distance -= 1;
                energy -= cost(monster.energy.walk);
                continue;
            }
            // Every attack it has, as one melee action makes them
            // (`melee_attack::run_monster_attack_set`, `melee-attack.cc:1569`).
            // An attack only made with a weapon (`AT_WEAP_ONLY`) is counted, as the Advisor can't
            // see what monsters wield.
            for attack in &monster.attacks {
                if attack.kind.0 == "AT_NONE"
                    || attack_reach(formulas.monster_reach, attack) < distance
                {
                    continue;
                }
                let to_hit =
                    monster_to_hit(formulas.monster_to_hit, rng, hit_dice, monster.fighter);
                let margin = evasion_margin(formulas.evasion_roll, rng, to_hit, defender.ev);
                let blocked =
                    shield_blocks(formulas.shield_block, rng, to_hit, hit_dice, defender.sh)
                        && blocks_left > 0;
                if blocked {
                    blocks_left -= 1;
                } else if margin >= 0 {
                    let rolled = monster_damage(formulas.monster_damage, rng, attack);
                    total += apply_ac(formulas.armour, rng, rolled, defender.ac);
                }
            }
            energy -= cost(monster.energy.attack);
        }
    }
    total
}

/// How far the monster's melee reaches: its farthest-reaching attack that deals damage, as
/// `monster::reach_range` (`monster.cc:6224`). 0 when it has no melee attack.
fn reach(formulas: Formulas, monster: &Monster) -> i32 {
    monster
        .attacks
        .iter()
        .filter(|attack| attack.kind.0 != "AT_NONE" && attack.damage > 0)
        .map(|attack| attack_reach(formulas.monster_reach, attack))
        .max()
        .unwrap_or(0)
}

fn speed(monster: &Monster) -> i32 {
    i32::try_from(monster.speed).unwrap_or(i32::MAX)
}

/// An action's energy cost, never zero (as `monster::action_energy`, `monster.cc:5897`).
fn cost(energy: u32) -> i32 {
    i32::try_from(energy).unwrap_or(i32::MAX).max(1)
}
