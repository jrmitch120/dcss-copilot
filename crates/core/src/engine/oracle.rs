//! The crawl oracle's seam (`CODING_STANDARDS.md`, Testing): the engine's ported melee, spell,
//! missile and wand formulas, the gear Game Knowledge says monsters are made with, and the
//! Threat Level's play of a monster's turns before the player's next action, run on the inputs a
//! crawl harness reports, tallied as the harness tallies crawl's own rolls, so
//! `tools/crawl-oracle` can diff the two.

use std::collections::BTreeMap;

use copilot_gk::{Bundle, EnumId, Formulas, Monster, MonsterSpells};

use crate::accumulated_state::Shield;

use super::melee::{self, Attack, Defences, Flavour, Swing};
use super::missiles::{self, Missile, Shooter};
use super::rng::Rng;
use super::spells::{self, DamagingSpell};
use super::threat::{self, Attacker, Draw};
use super::wands;

pub use super::melee::AUTOMATIC_HIT;

/// The player's AC, EV and SH as a harness dresses them: whole points, exactly as displayed, with
/// no fraction the display drops.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
pub struct ExactDefences {
    /// Armour class.
    pub ac: i32,
    /// Evasion.
    pub ev: i32,
    /// Shield class.
    pub sh: i32,
}

impl From<ExactDefences> for Defences {
    fn from(defences: ExactDefences) -> Self {
        Self::exact(defences.ac, defences.ev, defences.sh)
    }
}

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
    /// The player's defences.
    pub defences: ExactDefences,
}

/// What many runs of one melee attack or missile came to.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AttackTally {
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
pub fn melee(formulas: Formulas, case: &MeleeCase, seed: (u64, u64), trials: u32) -> AttackTally {
    let mut rng = Rng::new(seed.0, seed.1);
    let attack = Attack {
        flavour: if case.crushes {
            Flavour::Crush
        } else {
            Flavour::Other
        },
        damage: case.damage,
    };
    let defences = Defences::from(case.defences);
    let mut tally = AttackTally::default();
    for _ in 0..trials {
        tally.add(melee::swing(
            formulas,
            &mut rng,
            case.hit_dice,
            case.fighter,
            attack,
            defences,
            // The harness resets crawl's shield blocks before every attack.
            true,
        ));
    }
    tally
}

impl AttackTally {
    fn add(&mut self, swing: Swing) {
        *self.to_hit.entry(swing.to_hit).or_default() += 1;
        self.hits += u32::from(swing.hits);
        self.blocks += u32::from(swing.blocked);
        if swing.landed() {
            *self.damage.entry(swing.damage).or_default() += 1;
        }
    }
}

/// Why a case can't be run through the engine.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CaseError {
    /// The bundle has no monster with this `monster_type` value.
    #[error("the Game Knowledge has no monster numbered {0}")]
    NoMonster(u16),
    /// The bundle has no spell with this `spell_type` value.
    #[error("the Game Knowledge has no spell numbered {0}")]
    NoSpell(u16),
    /// A spell in the monster's book isn't in the Game Knowledge.
    #[error("the Game Knowledge has no spell {0}, which a monster's book names")]
    UnknownSpell(String),
    /// The bundle has no launcher or missile with this sub-type identifier.
    #[error("the Game Knowledge has no launcher or missile {0}")]
    NoMissileWeapon(String),
    /// The armour isn't a shield.
    #[error("{0} isn't a shield")]
    NotAShield(String),
    /// The engine doesn't port the spell a wand zaps.
    #[error("the engine doesn't port spell {0}, zapped from a wand")]
    WandNotPorted(u16),
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
    /// The player's defences.
    pub defences: ExactDefences,
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
) -> Result<SpellTally, CaseError> {
    let formulas = bundle.formulas;
    let cast = damaging_spell(bundle, case.monster, case.spell)?;
    let power = spells::power(formulas.monster_spell_power, &cast.spell.id, case.hit_dice);
    let defences = Defences::from(case.defences);
    Ok(beams(formulas, &cast, power, defences, seed, trials))
}

/// Fires `spell`'s beam at `power` `trials` times through the ported formulas, from a generator
/// seeded with `seed`, with a fresh shield block for every beam.
fn beams(
    formulas: Formulas,
    spell: &DamagingSpell<'_>,
    power: i32,
    defences: Defences,
    seed: (u64, u64),
    trials: u32,
) -> SpellTally {
    let range = spell.spell.range.map_or(0, |range| {
        spells::range(formulas.monster_spell_power, range, power)
    });
    let mut rng = Rng::new(seed.0, seed.1);
    let mut tally = SpellTally {
        power,
        range,
        ..SpellTally::default()
    };
    for _ in 0..trials {
        let bolt = spells::bolt(formulas.beam_damage, &mut rng, spell, power);
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
    tally
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
) -> Result<BTreeMap<Option<u16>, u32>, CaseError> {
    let formulas = bundle.formulas;
    let monster = bundle
        .monster(case.monster)
        .ok_or(CaseError::NoMonster(case.monster))?;
    let MonsterSpells::Book { spells: book } = &monster.spells else {
        return Ok(BTreeMap::new());
    };
    let mut castable = Vec::new();
    for slot in book {
        let (value, spell) = bundle
            .spells
            .iter()
            .find(|(_, spell)| spell.id == slot.spell)
            .ok_or_else(|| CaseError::UnknownSpell(slot.spell.0.clone()))?;
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
) -> Result<spells::DamagingSpell<'_>, CaseError> {
    let not_ported = CaseError::NotPorted { monster, spell };
    let caster = bundle
        .monster(monster)
        .ok_or(CaseError::NoMonster(monster))?;
    let known = bundle.spells.get(&spell).ok_or(CaseError::NoSpell(spell))?;
    let MonsterSpells::Book { spells: book } = &caster.spells else {
        return Err(not_ported);
    };
    if !book.iter().any(|slot| slot.spell == known.id) {
        return Err(not_ported);
    }
    spells::damaging(bundle.formulas.monster_spell_beam, known).ok_or(not_ported)
}

/// How often a monster of a kind was made with each launcher or missile and each wand, by
/// sub-type identifier, `None` for none.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GearTally {
    /// Its launcher or throwing missile.
    pub missile: BTreeMap<Option<EnumId>, u32>,
    /// The wand it will zap.
    pub wand: BTreeMap<Option<EnumId>, u32>,
}

/// Draws the gear of a monster of the kind numbered `monster` `trials` times, by the odds its
/// Game Knowledge gives, as the engine draws it for each trial of the Threat Level.
///
/// # Errors
/// If the bundle has no monster numbered `monster`.
pub fn gear(
    bundle: &Bundle,
    monster: u16,
    seed: (u64, u64),
    trials: u32,
) -> Result<GearTally, CaseError> {
    let known = bundle
        .monster(monster)
        .ok_or(CaseError::NoMonster(monster))?;
    let missiles = Draw::new(&known.gear.missile, Some);
    let wands = Draw::new(&known.gear.wand, Some);
    let mut rng = Rng::new(seed.0, seed.1);
    let mut tally = GearTally::default();
    for _ in 0..trials {
        *tally
            .missile
            .entry(missiles.draw(&mut rng).cloned())
            .or_default() += 1;
        *tally.wand.entry(wands.draw(&mut rng).cloned()).or_default() += 1;
    }
    Ok(tally)
}

/// One monster firing one launcher or throwing missile at the player, in the terms the ported
/// formulas take.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MissileCase {
    /// The monster's `monster_type` value.
    pub monster: u16,
    /// The launcher or missile, by sub-type identifier, +0 and unbranded.
    pub weapon: EnumId,
    /// The monster's hit dice.
    pub hit_dice: i32,
    /// The player's defences.
    pub defences: ExactDefences,
}

/// Fires `case`'s missile `trials` times through the ported formulas, from a generator seeded
/// with `seed`, with a fresh shield block for every missile, as the harness resets crawl's.
///
/// # Errors
/// If the bundle lacks the monster or the missile weapon.
pub fn missile(
    bundle: &Bundle,
    case: &MissileCase,
    seed: (u64, u64),
    trials: u32,
) -> Result<AttackTally, CaseError> {
    let (monster, missile) = shooter(bundle, case.monster, &case.weapon)?;
    let shooter = Shooter::of(monster, case.hit_dice);
    let defences = Defences::from(case.defences);
    let mut rng = Rng::new(seed.0, seed.1);
    let mut tally = AttackTally::default();
    for _ in 0..trials {
        tally.add(missiles::shoot(
            bundle.formulas,
            &mut rng,
            shooter,
            missile,
            defences,
            true,
        ));
    }
    Ok(tally)
}

/// A monster deciding whether to fire its launcher or throwing missile at a player some squares
/// away.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FireCase {
    /// The monster's `monster_type` value.
    pub monster: u16,
    /// The launcher or missile, by sub-type identifier.
    pub weapon: EnumId,
    /// Squares between it and the player.
    pub distance: i32,
}

/// How much energy `case`'s monster spent firing in each of `trials` actions, from a generator
/// seeded with `seed`: 0 for an action it didn't fire in.
///
/// # Errors
/// If the bundle lacks the monster or the missile weapon.
pub fn fire(
    bundle: &Bundle,
    case: &FireCase,
    seed: (u64, u64),
    trials: u32,
) -> Result<BTreeMap<i32, u32>, CaseError> {
    let (monster, missile) = shooter(bundle, case.monster, &case.weapon)?;
    let variant = bundle.formulas.monster_fire;
    let cost = i32::try_from(monster.energy.missile).unwrap_or(i32::MAX);
    let mut rng = Rng::new(seed.0, seed.1);
    let mut tally = BTreeMap::new();
    for _ in 0..trials {
        let energy = if missiles::fires(
            variant,
            &mut rng,
            missile,
            monster.prefers_ranged,
            case.distance,
        ) {
            missiles::fire_energy(variant, &mut rng, cost)
        } else {
            0
        };
        *tally.entry(energy).or_default() += 1;
    }
    Ok(tally)
}

/// A monster zapping a wand of a spell at the player, in the terms the ported formulas take.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WandCase {
    /// The spell's `spell_type` value.
    pub spell: u16,
    /// The monster's hit dice.
    pub hit_dice: i32,
    /// The player's defences.
    pub defences: ExactDefences,
}

/// Zaps `case`'s wand `trials` times through the ported formulas, from a generator seeded with
/// `seed`, with a fresh shield block for every beam, as the harness resets crawl's.
///
/// # Errors
/// If the bundle lacks the spell or the engine doesn't port it.
pub fn wand(
    bundle: &Bundle,
    case: &WandCase,
    seed: (u64, u64),
    trials: u32,
) -> Result<SpellTally, CaseError> {
    let spell = wand_spell(bundle, case.spell)?;
    let formulas = bundle.formulas;
    let power = wands::power(
        formulas.monster_wand,
        formulas.monster_spell_power,
        &spell.spell.id,
        case.hit_dice,
    );
    let defences = Defences::from(case.defences);
    Ok(beams(formulas, &spell, power, defences, seed, trials))
}

/// A monster deciding whether to zap a wand of a spell at a player some squares away.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ZapCase {
    /// The spell's `spell_type` value.
    pub spell: u16,
    /// The monster's hit dice.
    pub hit_dice: i32,
    /// Squares between it and the player.
    pub distance: i32,
}

/// How much energy `case`'s monster spent zapping in each of `trials` actions, from a generator
/// seeded with `seed`: 0 for an action it didn't zap in.
///
/// # Errors
/// If the bundle lacks the spell or the engine doesn't port it.
pub fn zap(
    bundle: &Bundle,
    case: &ZapCase,
    seed: (u64, u64),
    trials: u32,
) -> Result<BTreeMap<i32, u32>, CaseError> {
    let spell = wand_spell(bundle, case.spell)?;
    let formulas = bundle.formulas;
    let power = wands::power(
        formulas.monster_wand,
        formulas.monster_spell_power,
        &spell.spell.id,
        case.hit_dice,
    );
    let reach = spell.spell.range.map_or(0, |range| {
        let range = spells::range(formulas.monster_spell_power, range, power);
        spells::reach(formulas.monster_spell_beam, &spell, range).casts
    });
    let mut rng = Rng::new(seed.0, seed.1);
    let mut tally = BTreeMap::new();
    for _ in 0..trials {
        let zaps = wands::zaps(formulas.monster_wand, &mut rng) && reach >= case.distance;
        let energy = if zaps {
            wands::energy(formulas.monster_wand)
        } else {
            0
        };
        *tally.entry(energy).or_default() += 1;
    }
    Ok(tally)
}

/// The monster numbered `monster` and its missile weapon `weapon`.
fn shooter<'b>(
    bundle: &'b Bundle,
    monster: u16,
    weapon: &EnumId,
) -> Result<(&'b Monster, Missile<'b>), CaseError> {
    let known = bundle
        .monster(monster)
        .ok_or(CaseError::NoMonster(monster))?;
    let missile = bundle
        .missile_weapons
        .get(weapon)
        .ok_or_else(|| CaseError::NoMissileWeapon(weapon.0.clone()))?;
    Ok((
        known,
        Missile::of(bundle.formulas.missile_attack, weapon, missile),
    ))
}

/// The spell numbered `spell`, as the engine ports it, which a wand zaps.
fn wand_spell(bundle: &Bundle, spell: u16) -> Result<DamagingSpell<'_>, CaseError> {
    let known = bundle.spells.get(&spell).ok_or(CaseError::NoSpell(spell))?;
    spells::damaging(bundle.formulas.monster_spell_beam, known)
        .ok_or(CaseError::WandNotPorted(spell))
}

/// The most attacks a shield blocks before the player's next action, by the `armour_type`
/// identifier of the shield worn, `None` for none.
///
/// # Errors
/// If the armour isn't a shield.
pub fn shield_block_limit(bundle: &Bundle, shield: Option<&EnumId>) -> Result<u32, CaseError> {
    let shield = shield
        .map(|id| Shield::of_armour(id).ok_or_else(|| CaseError::NotAShield(id.0.clone())))
        .transpose()?;
    Ok(threat::shield_blocks_per_action(bundle.formulas, shield))
}

/// How far a monster's melee reaches, and which of its attacks it makes from each distance
/// within that.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MeleeReach {
    /// The most squares from which it attacks in melee: 0 when it has no damaging attack.
    pub reach: i32,
    /// From each distance up to its reach, the slots of the attacks it makes, as its Game
    /// Knowledge numbers them from 0.
    pub attacks_from: BTreeMap<i32, Vec<usize>>,
}

/// How far the monster numbered `monster` reaches in melee, and with which attacks.
///
/// # Errors
/// If the bundle has no monster numbered `monster`.
pub fn melee_reach(bundle: &Bundle, monster: u16) -> Result<MeleeReach, CaseError> {
    let formulas = bundle.formulas;
    let known = bundle
        .monster(monster)
        .ok_or(CaseError::NoMonster(monster))?;
    let reach = threat::melee_reach(formulas, known);
    let attacks_from = (1..=reach)
        .map(|distance| {
            let slots = threat::attacks_from(formulas, known, distance)
                .map(|(slot, _)| slot)
                .collect();
            (distance, slots)
        })
        .collect();
    Ok(MeleeReach {
        reach,
        attacks_from,
    })
}

/// A monster some squares from the player, carrying nothing, as the player starts a
/// standard-speed action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TurnCase {
    /// The monster's `monster_type` value.
    pub monster: u16,
    /// Its hit dice.
    pub hit_dice: u32,
    /// Its speed.
    pub speed: u32,
    /// Squares between it and the player.
    pub distance: i32,
}

/// What a monster did before the player's next standard-speed action, over many trials: how
/// many trials it did each thing in each number of times.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TurnTally {
    /// The melee actions it took against the player.
    pub melee: BTreeMap<i32, u32>,
    /// The spells it cast.
    pub casts: BTreeMap<i32, u32>,
    /// The squares it closed on the player.
    pub closed: BTreeMap<i32, u32>,
    /// Whether it struck the player in any trial.
    pub struck: bool,
    /// Whether the Threat Level counts it at all: [`threat::can_attack`].
    pub can_attack: bool,
}

/// Plays out `case`'s monster's actions before the player's next standard-speed action
/// `trials` times, as the Threat Level's trial does, from a generator seeded with `seed`.
///
/// # Errors
/// If the bundle lacks the monster.
pub fn turn(
    bundle: &Bundle,
    case: &TurnCase,
    seed: (u64, u64),
    trials: u32,
) -> Result<TurnTally, CaseError> {
    let formulas = bundle.formulas;
    let mut monster = bundle
        .monster(case.monster)
        .ok_or(CaseError::NoMonster(case.monster))?
        .clone();
    monster.hd = case.hit_dice;
    monster.speed = case.speed;
    let attacker = Attacker::new(bundle, &monster, case.distance).without_gear();
    // Undefended, so whatever strikes the player deals damage.
    let defences = Defences::exact(0, 0, 0);
    let mut rng = Rng::new(seed.0, seed.1);
    let mut tally = TurnTally {
        can_attack: threat::can_attack(formulas, &attacker),
        ..TurnTally::default()
    };
    for _ in 0..trials {
        // The undefended player has no shield to block with.
        let mut blocks_left = 0;
        let turn = threat::turn(formulas, &mut rng, &attacker, defences, &mut blocks_left);
        *tally.melee.entry(turn.melee).or_default() += 1;
        *tally.casts.entry(turn.casts).or_default() += 1;
        *tally.closed.entry(turn.closed).or_default() += 1;
        tally.struck |= turn.damage > 0;
    }
    Ok(tally)
}
