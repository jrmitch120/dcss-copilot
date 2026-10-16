//! The crawl oracle's seam (`CODING_STANDARDS.md`, Testing): the engine's ported melee and spell
//! formulas, run on the inputs a crawl harness reports, tallied as the harness tallies crawl's
//! own rolls, so `tools/crawl-oracle` can diff the two.

use std::collections::BTreeMap;

use copilot_gk::{Bundle, Formulas, MonsterSpells};

use super::melee::{self, Attack, Defences, Flavour};
use super::rng::Rng;
use super::spells;

pub use super::melee::AUTOMATIC_HIT;

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

/// Why a spell case can't be run through the engine.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SpellCaseError {
    /// The bundle has no monster with this `monster_type` value.
    #[error("the Game Knowledge has no monster numbered {0}")]
    NoMonster(u16),
    /// The bundle has no spell with this `spell_type` value.
    #[error("the Game Knowledge has no spell numbered {0}")]
    NoSpell(u16),
    /// A spell in the monster's book isn't in the Game Knowledge.
    #[error("the Game Knowledge has no spell {0}, which a monster's book names")]
    UnknownSpell(String),
    /// The spell isn't one the engine ports, or not in the monster's spellbook.
    #[error("the engine doesn't port spell {spell} as monster {monster} casts it")]
    NotPorted {
        /// The monster's `monster_type` value.
        monster: u16,
        /// The spell's `spell_type` value.
        spell: u16,
    },
}

/// The `spell_type` value of every spell the engine ports when a monster casts it, so a crawl
/// harness can draw its cases from them.
#[must_use]
pub fn ported_spells(bundle: &Bundle) -> Vec<u16> {
    bundle
        .spells
        .iter()
        .filter(|(_, spell)| spells::is_ported(bundle.formulas.monster_spell_beam, spell))
        .map(|(&value, _)| value)
        .collect()
}

/// One monster casting one of its spells at the player, in the terms the ported formulas take.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpellCase {
    /// The caster's `monster_type` value.
    pub monster: u16,
    /// The spell's `spell_type` value.
    pub spell: u16,
    /// The caster's hit dice.
    pub hit_dice: i32,
    /// The player's AC, as displayed.
    pub ac: i32,
    /// The player's EV, as displayed.
    pub ev: i32,
    /// The player's SH, as displayed.
    pub sh: i32,
}

/// What many casts of one spell came to.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SpellTally {
    /// The power it was cast with.
    pub power: i32,
    /// How many squares it reaches.
    pub range: i32,
    /// Its beam's accuracy, `None` when it never misses.
    pub to_hit: Option<i32>,
    /// Beams that got past the player's shield and evasion.
    pub hits: u32,
    /// Beams the player's shield blocked.
    pub blocks: u32,
    /// How often each amount of damage, after armour, was taken from a beam that hit.
    pub damage: BTreeMap<i32, u32>,
}

/// Casts `case`'s spell `trials` times through the ported formulas, from a generator seeded with
/// `seed`, with a fresh shield block for every beam, as the harness resets crawl's.
///
/// # Errors
/// If the bundle lacks the monster or spell, or the engine doesn't port the spell as the monster
/// casts it.
pub fn spell(
    bundle: &Bundle,
    case: &SpellCase,
    seed: (u64, u64),
    trials: u32,
) -> Result<SpellTally, SpellCaseError> {
    let formulas = bundle.formulas;
    let cast = damaging_spell(bundle, case.monster, case.spell)?;
    let power = spells::power(formulas.monster_spell_power, &cast.spell.id, case.hit_dice);
    let range = cast.spell.range.map_or(0, |range| {
        spells::range(formulas.monster_spell_power, range, power)
    });
    let defences = Defences {
        ac: case.ac,
        ev: case.ev,
        sh: case.sh,
    };
    let mut rng = Rng::new(seed.0, seed.1);
    let mut tally = SpellTally {
        power,
        range,
        ..SpellTally::default()
    };
    for _ in 0..trials {
        let bolt = spells::bolt(formulas.beam_damage, &mut rng, &cast, power);
        tally.to_hit = bolt.to_hit;
        let shot = spells::shoot(
            formulas.beam_hit,
            formulas.beam_damage,
            &mut rng,
            bolt,
            defences,
            true,
        );
        tally.blocks += u32::from(shot.blocked);
        if shot.hits {
            tally.hits += 1;
            *tally.damage.entry(shot.damage).or_default() += 1;
        }
    }
    Ok(tally)
}

/// A monster choosing which spell to cast at a player some squares away.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpellChoiceCase {
    /// The caster's `monster_type` value.
    pub monster: u16,
    /// The caster's hit dice.
    pub hit_dice: i32,
    /// Squares between it and the player.
    pub distance: i32,
}

/// How often a monster chose each of the spells the engine ports from its book, by `spell_type`
/// value, `trials` times from a generator seeded with `seed`. Actions it chose none of them are
/// counted under `None`.
///
/// # Errors
/// If the bundle lacks the monster or a spell in its book.
pub fn spell_choice(
    bundle: &Bundle,
    case: &SpellChoiceCase,
    seed: (u64, u64),
    trials: u32,
) -> Result<BTreeMap<Option<u16>, u32>, SpellCaseError> {
    let formulas = bundle.formulas;
    let monster = bundle
        .monster(case.monster)
        .ok_or(SpellCaseError::NoMonster(case.monster))?;
    let MonsterSpells::Book { spells: book } = &monster.spells else {
        return Ok(BTreeMap::new());
    };
    let mut castable = Vec::new();
    for slot in book {
        let (value, spell) = bundle
            .spells
            .iter()
            .find(|(_, spell)| spell.id == slot.spell)
            .ok_or_else(|| SpellCaseError::UnknownSpell(slot.spell.0.clone()))?;
        if let Some(cast) = spells::damaging(formulas.monster_spell_beam, spell) {
            let book = spells::Slot::of(slot);
            let power = spells::power(formulas.monster_spell_power, &spell.id, case.hit_dice);
            let in_range = spell.range.is_some_and(|range| {
                let range = spells::range(formulas.monster_spell_power, range, power);
                spells::reach(formulas.monster_spell_beam, &cast, range).casts >= case.distance
            });
            castable.push((
                *value,
                spells::Prospect {
                    frequency: book.frequency,
                    when: book.when,
                    in_range,
                },
            ));
        }
    }
    let prospects: Vec<_> = castable.iter().map(|&(_, prospect)| prospect).collect();
    let mut rng = Rng::new(seed.0, seed.1);
    let mut tally = BTreeMap::new();
    for _ in 0..trials {
        let chosen = spells::choose(
            formulas.monster_spell_choice,
            &mut rng,
            &prospects,
            case.distance,
        )
        .and_then(|i| castable.get(i))
        .map(|&(value, _)| value);
        *tally.entry(chosen).or_default() += 1;
    }
    Ok(tally)
}

/// The spell numbered `spell` in `monster`'s book, as the engine ports it.
fn damaging_spell(
    bundle: &Bundle,
    monster: u16,
    spell: u16,
) -> Result<spells::DamagingSpell<'_>, SpellCaseError> {
    let not_ported = SpellCaseError::NotPorted { monster, spell };
    let caster = bundle
        .monster(monster)
        .ok_or(SpellCaseError::NoMonster(monster))?;
    let known = bundle
        .spells
        .get(&spell)
        .ok_or(SpellCaseError::NoSpell(spell))?;
    let MonsterSpells::Book { spells: book } = &caster.spells else {
        return Err(not_ported);
    };
    if !book.iter().any(|slot| slot.spell == known.id) {
        return Err(not_ported);
    }
    spells::damaging(bundle.formulas.monster_spell_beam, known).ok_or(not_ported)
}
