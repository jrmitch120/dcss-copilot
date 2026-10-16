//! The crawl oracle's seam (`CODING_STANDARDS.md`, Testing): the engine's ported melee formulas,
//! run on the inputs a crawl harness reports, tallied as the harness tallies crawl's own rolls,
//! so `tools/crawl-oracle` can diff the two.

use std::collections::BTreeMap;

use copilot_gk::Formulas;

use super::melee::{self, Attack, Defences, Flavour};
use super::rng::Rng;

/// One monster attack on the player, in the terms the ported formulas take.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MeleeCase {
    /// The attacker's hit dice.
    pub hit_dice: i32,
    /// Whether the attacker is a fighter (`M_FIGHTER`), which raises its accuracy.
    pub fighter: bool,
    /// The attack's damage before armour.
    pub damage: i32,
    /// Whether the attack crushes (`AF_CRUSH`), grabbing and dealing no damage itself.
    pub crushes: bool,
    /// The player's AC, as displayed.
    pub ac: i32,
    /// The player's EV, as displayed.
    pub ev: i32,
    /// The player's SH, as displayed.
    pub sh: i32,
}

/// What many runs of one attack came to.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MeleeTally {
    /// How often each to-hit roll came up.
    pub to_hit: BTreeMap<i32, u32>,
    /// Attacks that got past the player's evasion.
    pub hits: u32,
    /// Attacks the player's shield blocked, whether or not they would have hit.
    pub blocks: u32,
    /// How often each amount of damage, after armour, was taken from an attack that hit and
    /// wasn't blocked.
    pub damage: BTreeMap<i32, u32>,
}

/// Runs `case` `trials` times through the ported formulas, from a generator seeded with
/// `seed`, with a fresh shield block for every attack, as the harness resets crawl's.
#[must_use]
pub fn melee(formulas: Formulas, case: &MeleeCase, seed: (u64, u64), trials: u32) -> MeleeTally {
    let mut rng = Rng::new(seed.0, seed.1);
    let attack = Attack {
        flavour: if case.crushes {
            Flavour::Crush
        } else {
            Flavour::Other
        },
        damage: case.damage,
    };
    let defences = Defences {
        ac: case.ac,
        ev: case.ev,
        sh: case.sh,
    };
    let mut tally = MeleeTally::default();
    for _ in 0..trials {
        let swing = melee::swing(
            formulas,
            &mut rng,
            case.hit_dice,
            case.fighter,
            attack,
            defences,
            // The harness resets crawl's shield blocks before every attack.
            true,
        );
        *tally.to_hit.entry(swing.to_hit).or_default() += 1;
        tally.hits += u32::from(swing.hits);
        tally.blocks += u32::from(swing.blocked);
        if swing.landed() {
            *tally.damage.entry(swing.damage).or_default() += 1;
        }
    }
    tally
}
