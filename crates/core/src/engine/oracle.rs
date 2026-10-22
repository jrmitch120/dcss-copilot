//! The crawl oracle's seam (`CODING_STANDARDS.md`, Testing): the engine's ported melee, spell,
//! missile and wand formulas, the gear Game Knowledge says monsters are made with, the Threat
//! Level's play of a monster's turns before the player's next action, and the time the player's
//! actions and moves take, run on the inputs a crawl harness reports, tallied as the harness
//! tallies crawl's own rolls, so `tools/crawl-oracle` can diff the two.

use std::collections::BTreeMap;

use copilot_gk::{Bundle, EnumId, Formulas, Monster, MonsterSpells};

use crate::accumulated_state::{MutationsRead, Shield};

use crate::crawl_ids::monster_types;

use super::brands::{Brand, Necromancy};
use super::melee::{self, Attack, Defences, Fighter, Hands, Swing, Target, Wielded};
use super::missiles::{self, Missile, Missiles, Shooter};
use super::player_attacks::{self, Defence, DrawnSkill, Hand, Heft, PlayerZap, Shot, Striker};
use super::player_move::{self, Unread};
use super::resists::{self, Resists, Seen};
use super::rng::Rng;
use super::spells::{self, DamagingSpell, Firing};
use super::step::Offset;
use super::threat::{self, Attacker, Draw};
use super::{torment, wands};
use crate::screen::{Resistance, Resistances, Screen, ScreenRead};
use crate::visible_state::{Religion, Status};

pub use super::lighting::Lighting;
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

/// The player's resistances as a harness's character overview shows them, with their species
/// and god as the game names them.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
pub struct PlayerResists {
    /// `rFire`.
    pub fire: i32,
    /// `rCold`.
    pub cold: i32,
    /// `rNeg`.
    pub negative: i32,
    /// `rPois`, 3 for immunity.
    pub poison: i32,
    /// `rElec`.
    pub electricity: i32,
    /// `rCorr`.
    pub corrosion: i32,
    /// The species, such as `Mummy`.
    pub species: String,
    /// The god, such as `the Shining One`, or `No God`.
    pub god: String,
}

/// What crawl's `god_name` calls no god.
const NO_GOD: &str = "No God";

impl PlayerResists {
    /// The resistances as the character overview draws them (`_resist_composer`): immunity to
    /// poison as `∞`, every other level as pips.
    fn overview(&self) -> Resistances {
        let pips = |level: i32| Some(Resistance::Pips(i8::try_from(level).unwrap_or(i8::MAX)));
        Resistances {
            fire: pips(self.fire),
            cold: pips(self.cold),
            negative: pips(self.negative),
            poison: if self.poison == 3 {
                Some(Resistance::Immune)
            } else {
                pips(self.poison)
            },
            electricity: pips(self.electricity),
            corrosion: pips(self.corrosion),
            see_invisible: None,
            willpower: None,
        }
    }

    /// The resistances, holiness and god the engine plays for the player these are, with
    /// `statuses` lit, read through `bundle` as the Advisor reads the character overview and the
    /// status pane.
    fn read(&self, bundle: &Bundle, statuses: &[Status]) -> Result<Resists, CaseError> {
        let overview = self.overview();
        let religion = self.religion();
        let seen = Seen {
            resistances: &overview,
            species: Some(&self.species),
            statuses: Some(statuses),
            religion: Some(&religion),
        };
        resists::read(bundle, &seen)
            .filter(|read| read.holiness_known)
            .map(|read| read.resists)
            .ok_or_else(|| CaseError::NoSpecies(self.species.clone()))
    }

    /// The god as the status pane reports it.
    fn religion(&self) -> Religion {
        if self.god == NO_GOD {
            Religion::NoGod
        } else {
            Religion::God {
                name: self.god.clone(),
                piety_rank: None,
                penance: false,
            }
        }
    }
}

/// The player a case's monster attacks: exactly these defences, in this light, resisting as
/// `resists` says, with the holiness their species and god give them, read through `bundle` as
/// the Advisor reads the character overview, with no statuses lit.
fn target(
    bundle: &Bundle,
    defences: ExactDefences,
    lighting: Lighting,
    resists: &PlayerResists,
) -> Result<Target, CaseError> {
    Ok(Target {
        defences: defences.into(),
        lighting,
        resists: resists.read(bundle, &[])?,
    })
}

/// One monster attack on the player, in the terms the ported formulas take.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeleeCase {
    /// The attacker's `monster_type` value, whose Game Knowledge gives its attacks and says
    /// whether it sees through an umbra.
    pub monster: u16,
    /// The attacker's hit dice.
    pub hit_dice: i32,
    /// Whether the attacker is a fighter (`M_FIGHTER`), which raises its accuracy.
    pub fighter: bool,
    /// The attack's slot among the attacker's, numbered from 0.
    pub slot: usize,
    /// The attack's damage before armour.
    pub damage: i32,
    /// The melee weapons the attacker holds.
    pub hands: HandsCase,
    /// The player's defences.
    pub defences: ExactDefences,
    /// What the player resists, and their species and god.
    pub resists: PlayerResists,
    /// How the light falls on the player.
    pub lighting: Lighting,
}

/// The melee weapons a harness's monster holds.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HandsCase {
    /// In its main hand.
    pub main: Option<WeaponCase>,
    /// In its off hand: for a monster that wields two weapons.
    pub off: Option<WeaponCase>,
}

/// A melee weapon a harness's monster holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WeaponCase {
    /// The weapon, by `weapon_type` identifier.
    pub weapon: EnumId,
    /// Its enchantment.
    pub plus: i32,
    /// Its brand, by `brand_type` identifier.
    pub brand: EnumId,
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
    /// How often each amount of damage, after armour and with what a brand adds, was taken from
    /// an attack that hit and wasn't blocked.
    pub damage: BTreeMap<i32, u32>,
}

/// What many runs of one melee attack came to, with the delay of a melee action by its attacker.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MeleeTally {
    /// The attack's rolls.
    pub attack: AttackTally,
    /// How often each delay came up for a melee action, as `monster::attack_delay` rolls it.
    pub delay: BTreeMap<i32, u32>,
}

/// Runs `case` `trials` times through the ported formulas, from a generator seeded with
/// `seed`, with a fresh shield block for every attack, as the harness resets crawl's, and rolls
/// its attacker's melee delay as often.
///
/// # Errors
/// If the bundle has no monster numbered as the attacker, it has no attack in the case's slot,
/// or the bundle lacks one of its weapons or the player's species.
pub fn melee(
    bundle: &Bundle,
    case: &MeleeCase,
    seed: (u64, u64),
    trials: u32,
) -> Result<MeleeTally, CaseError> {
    let formulas = bundle.formulas;
    let monster = bundle
        .monster(case.monster)
        .ok_or(CaseError::NoMonster(case.monster))?;
    let no_attack = CaseError::NoAttack {
        monster: case.monster,
        slot: case.slot,
    };
    let attack = Attack {
        damage: case.damage,
        ..monster
            .attacks
            .get(case.slot)
            .and_then(Attack::of)
            .ok_or(no_attack)?
    };
    let fighter = Fighter {
        hit_dice: case.hit_dice,
        skilled: case.fighter,
        nightvision: monster.nightvision,
        necromancy: Necromancy::of(bundle, monster, case.hit_dice),
        draugr: monster.id.0 == monster_types::DRAUGR,
        hands: Some(hands(bundle, &case.hands)?),
    };
    let weapon = fighter
        .weapon(formulas.monster_weapon, case.slot, attack)
        .map(|wielded| wielded.weapon);
    let first = monster.attacks.first().and_then(Attack::of);
    let mut rng = Rng::new(seed.0, seed.1);
    let target = target(bundle, case.defences, case.lighting, &case.resists)?;
    let mut tally = MeleeTally::default();
    for _ in 0..trials {
        tally.attack.add(melee::swing(
            formulas, &mut rng, &fighter, attack, weapon, target,
            // The harness resets crawl's shield blocks before every attack.
            true,
        ));
        let delay = fighter.attack_delay(formulas, &mut rng, first);
        *tally.delay.entry(delay).or_default() += 1;
    }
    Ok(tally)
}

/// The melee weapons a harness's monster holds, as the formulas take them.
fn hands(bundle: &Bundle, hands: &HandsCase) -> Result<Hands, CaseError> {
    let hold = |weapon: &WeaponCase| {
        let known = bundle
            .melee_weapons
            .get(&weapon.weapon)
            .ok_or_else(|| CaseError::NoMeleeWeapon(weapon.weapon.0.clone()))?;
        let brand = Brand::of(bundle.formulas.damage_brand, &weapon.brand);
        Ok(Wielded::of(known, weapon.plus, brand))
    };
    Ok(Hands {
        main: hands.main.as_ref().map(hold).transpose()?,
        off: hands.off.as_ref().map(hold).transpose()?,
    })
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
    /// The monster has no attack in this slot.
    #[error("monster {monster} has no attack in slot {slot}")]
    NoAttack {
        /// The monster's `monster_type` value.
        monster: u16,
        /// The attack's slot, numbered from 0.
        slot: usize,
    },
    /// The bundle has no species of this name.
    #[error("the Game Knowledge has no species {0}")]
    NoSpecies(String),
    /// The bundle has no form the status pane lights with this text.
    #[error("the Game Knowledge has no form lit {0}")]
    NoForm(String),
    /// The bundle doesn't classify a status light the status pane lit, as a form or a status that
    /// changes the player's speed or doesn't.
    #[error("the Game Knowledge doesn't classify the status light {0}")]
    UnclassifiedLight(String),
    /// The bundle has no dungeon feature with this `dungeon_feature_type` value.
    #[error("the Game Knowledge has no feature numbered {0}")]
    NoFeature(u16),
    /// The bundle has no melee weapon with this sub-type identifier.
    #[error("the Game Knowledge has no melee weapon {0}")]
    NoMeleeWeapon(String),
    /// The bundle has no launcher or missile with this sub-type identifier.
    #[error("the Game Knowledge has no launcher or missile {0}")]
    NoMissileWeapon(String),
    /// The armour isn't a shield.
    #[error("{0} isn't a shield")]
    NotAShield(String),
    /// The engine doesn't port the spell a wand zaps.
    #[error("the engine doesn't port spell {0}, zapped from a wand")]
    WandNotPorted(u16),
    /// The engine doesn't port the brand on the player's weapon, beyond its damage and delay.
    #[error("the engine doesn't port the player's brand {0}")]
    BrandNotPorted(String),
    /// The engine doesn't port the spell as the player casts it.
    #[error("the engine doesn't port spell {0} as the player casts it")]
    PlayerZapNotPorted(u16),
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpellCase {
    /// The caster's `monster_type` value.
    pub monster: u16,
    /// The spell's `spell_type` value.
    pub spell: u16,
    /// The caster's hit dice.
    pub hit_dice: i32,
    /// The player's defences.
    pub defences: ExactDefences,
    /// What the player resists, and their species and god.
    pub resists: PlayerResists,
    /// How the light falls on the player.
    pub lighting: Lighting,
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
/// If the bundle lacks the monster, the spell or the player's species, or the engine doesn't port
/// the spell as the monster casts it.
pub fn spell(
    bundle: &Bundle,
    case: &SpellCase,
    seed: (u64, u64),
    trials: u32,
) -> Result<SpellTally, CaseError> {
    let formulas = bundle.formulas;
    let (caster, cast) = damaging_spell(bundle, case.monster, case.spell)?;
    let power = spells::power(formulas.monster_spell_power, &cast.spell.id, case.hit_dice);
    let firing = Firing {
        spell: &cast,
        power,
        nightvision: caster.nightvision,
    };
    Ok(beams(
        formulas,
        firing,
        target(bundle, case.defences, case.lighting, &case.resists)?,
        seed,
        trials,
    ))
}

/// Fires `firing`'s beam at `target` `trials` times through the ported formulas, from a
/// generator seeded with `seed`, with a fresh shield block for every beam.
fn beams(
    formulas: Formulas,
    firing: Firing<'_, '_>,
    target: Target,
    seed: (u64, u64),
    trials: u32,
) -> SpellTally {
    let power = firing.power;
    let range = firing.spell.spell.range.map_or(0, |range| {
        spells::range(formulas.monster_spell_power, range, power)
    });
    let mut rng = Rng::new(seed.0, seed.1);
    let mut tally = SpellTally {
        power,
        range,
        ..SpellTally::default()
    };
    for _ in 0..trials {
        let bolt = spells::bolt(formulas, &mut rng, firing);
        tally.to_hit = bolt.to_hit;
        let shot = spells::shoot(formulas, &mut rng, bolt, target, true);
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

/// The monster numbered `monster`, and the spell numbered `spell` in its book, as the engine
/// ports it.
fn damaging_spell(
    bundle: &Bundle,
    monster: u16,
    spell: u16,
) -> Result<(&Monster, spells::DamagingSpell<'_>), CaseError> {
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
    let cast = spells::damaging(bundle.formulas.monster_spell_beam, known).ok_or(not_ported)?;
    Ok((caster, cast))
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
    /// The launcher or missile, by sub-type identifier.
    pub weapon: EnumId,
    /// A launcher's enchantment: 0 for a thrown missile.
    pub plus: i32,
    /// A launcher's brand, by `brand_type` identifier: `SPWPN_NORMAL` for a thrown missile.
    pub brand: EnumId,
    /// The monster's hit dice.
    pub hit_dice: i32,
    /// The player's defences.
    pub defences: ExactDefences,
    /// What the player resists, and their species and god.
    pub resists: PlayerResists,
    /// How the light falls on the player.
    pub lighting: Lighting,
}

/// Fires `case`'s missile `trials` times through the ported formulas, from a generator seeded
/// with `seed`, with a fresh shield block for every missile, as the harness resets crawl's.
///
/// # Errors
/// If the bundle lacks the monster, the missile weapon or the player's species.
pub fn missile(
    bundle: &Bundle,
    case: &MissileCase,
    seed: (u64, u64),
    trials: u32,
) -> Result<AttackTally, CaseError> {
    let (monster, missile) = shooter(bundle, case.monster, &case.weapon, case.plus, &case.brand)?;
    let shooter = Shooter::of(bundle, monster, case.hit_dice);
    let target = target(bundle, case.defences, case.lighting, &case.resists)?;
    let mut rng = Rng::new(seed.0, seed.1);
    let mut tally = AttackTally::default();
    for _ in 0..trials {
        tally.add(missiles::shoot(
            bundle.formulas,
            &mut rng,
            shooter,
            missile,
            target,
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
    /// A launcher's enchantment: 0 for a thrown missile.
    pub plus: i32,
    /// A launcher's brand, by `brand_type` identifier: `SPWPN_NORMAL` for a thrown missile.
    pub brand: EnumId,
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
    let (monster, missile) = shooter(bundle, case.monster, &case.weapon, case.plus, &case.brand)?;
    let variant = bundle.formulas.monster_fire;
    let cost = i32::try_from(monster.energy.missile).unwrap_or(i32::MAX);
    let mut rng = Rng::new(seed.0, seed.1);
    let mut tally = BTreeMap::new();
    for _ in 0..trials {
        let energy = missiles::fires(
            variant,
            &mut rng,
            Missiles::of(missile),
            monster.prefers_ranged,
            case.distance,
        )
        .map_or(0, |missile| {
            missiles::fire_energy(bundle.formulas, &mut rng, missile, cost)
        });
        *tally.entry(energy).or_default() += 1;
    }
    Ok(tally)
}

/// A monster zapping a wand of a spell at the player, in the terms the ported formulas take.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WandCase {
    /// The monster's `monster_type` value, whose Game Knowledge says whether it sees through an
    /// umbra.
    pub monster: u16,
    /// The spell's `spell_type` value.
    pub spell: u16,
    /// The monster's hit dice.
    pub hit_dice: i32,
    /// The player's defences.
    pub defences: ExactDefences,
    /// What the player resists, and their species and god.
    pub resists: PlayerResists,
    /// How the light falls on the player.
    pub lighting: Lighting,
}

/// Zaps `case`'s wand `trials` times through the ported formulas, from a generator seeded with
/// `seed`, with a fresh shield block for every beam, as the harness resets crawl's.
///
/// # Errors
/// If the bundle lacks the monster, the spell or the player's species, or the engine doesn't port
/// the spell.
pub fn wand(
    bundle: &Bundle,
    case: &WandCase,
    seed: (u64, u64),
    trials: u32,
) -> Result<SpellTally, CaseError> {
    let zapper = bundle
        .monster(case.monster)
        .ok_or(CaseError::NoMonster(case.monster))?;
    let spell = wand_spell(bundle, case.spell)?;
    let formulas = bundle.formulas;
    let power = wands::power(
        formulas.monster_wand,
        formulas.monster_spell_power,
        &spell.spell.id,
        case.hit_dice,
    );
    let firing = Firing {
        spell: &spell,
        power,
        nightvision: zapper.nightvision,
    };
    Ok(beams(
        formulas,
        firing,
        target(bundle, case.defences, case.lighting, &case.resists)?,
        seed,
        trials,
    ))
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
    plus: i32,
    brand: &EnumId,
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
        Missile::of(
            bundle.formulas.missile_attack,
            weapon,
            missile,
            plus,
            Brand::of(bundle.formulas.damage_brand, brand),
        ),
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

/// How far the monster numbered `monster` reaches in melee holding `hands`, and with which
/// attacks.
///
/// # Errors
/// If the bundle has no monster numbered `monster`, or lacks one of its weapons.
pub fn melee_reach(
    bundle: &Bundle,
    monster: u16,
    hands: &HandsCase,
) -> Result<MeleeReach, CaseError> {
    let formulas = bundle.formulas;
    let known = bundle
        .monster(monster)
        .ok_or(CaseError::NoMonster(monster))?;
    let fighter = Fighter {
        hands: Some(self::hands(bundle, hands)?),
        ..Attacker::new(bundle, known, Offset::east(1)).fighter
    };
    let reach = threat::melee_reach(formulas, &fighter, known);
    let attacks_from = (1..=reach)
        .map(|distance| {
            let slots = threat::attacks_from(formulas, &fighter, known, distance)
                .map(|(slot, ..)| slot)
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
    /// The `monster_type` value of the kind whose book it casts from: its own, or another's the
    /// harness gave it.
    pub book: u16,
    /// Its hit dice.
    pub hit_dice: u32,
    /// Its speed.
    pub speed: u32,
    /// Squares between it and the player.
    pub distance: i32,
    /// How the light falls on the player.
    pub lighting: Lighting,
    /// The player's hit points.
    pub hp: i32,
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
/// `trials` times, as the Threat Level's trial does, from a generator seeded with `seed`, with
/// its book cut down to the spells the engine ports, as the harness cuts it.
///
/// # Errors
/// If the bundle lacks the monster or the kind whose book it casts from.
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
    let book = bundle
        .monster(case.book)
        .ok_or(CaseError::NoMonster(case.book))?;
    monster.spells = match &book.spells {
        MonsterSpells::Book { spells } => MonsterSpells::Book {
            spells: spells
                .iter()
                .filter(|slot| {
                    bundle
                        .spell_by_id(&slot.spell)
                        .is_some_and(|spell| spells::is_ported(formulas.monster_spell_beam, spell))
                })
                .cloned()
                .collect(),
        },
        MonsterSpells::PerMonster => MonsterSpells::PerMonster,
    };
    let attacker = Attacker::new(bundle, &monster, Offset::east(case.distance)).without_gear();
    // Undefended, so whatever strikes the player deals damage.
    let target = Target {
        defences: Defences::exact(0, 0, 0),
        lighting: case.lighting,
        resists: Resists::NONE,
    };
    let mut rng = Rng::new(seed.0, seed.1);
    let mut tally = TurnTally {
        can_attack: threat::can_attack(formulas, &attacker),
        ..TurnTally::default()
    };
    for _ in 0..trials {
        // The undefended player has no shield to block with.
        let mut blocks_left = 0;
        let turn = threat::turn(
            formulas,
            &mut rng,
            &attacker,
            target,
            case.hp,
            &mut blocks_left,
        );
        *tally.melee.entry(turn.melee).or_default() += 1;
        *tally.casts.entry(turn.casts).or_default() += 1;
        *tally.closed.entry(turn.closed).or_default() += 1;
        tally.struck |= turn.damage > 0;
    }
    Ok(tally)
}

/// A player torment meets, as a harness made them: their resistances, species and god, the
/// status light of the form they are in, if any, and whether they are petrified.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TormentCase {
    /// Their resistances as the character overview shows them, with their species and god.
    pub resists: PlayerResists,
    /// The status light of their form, such as `Statue`.
    pub form: Option<String>,
    /// Whether they are petrified.
    pub petrified: bool,
}

/// The hit points torment, as a lurking horror sets it off, takes from `case`'s player on `hp`,
/// read as the Advisor reads the character overview and the status pane, with no status lit but
/// their form and petrification.
///
/// # Errors
/// If the bundle doesn't know the species, or the form by its light.
pub fn torment(bundle: &Bundle, case: &TormentCase, hp: i32) -> Result<i32, CaseError> {
    let signs = &bundle.holiness;
    if let Some(light) = &case.form
        && !signs.forms.iter().any(|form| form.light == *light)
    {
        return Err(CaseError::NoForm(light.clone()));
    }
    let petrified_light = case
        .petrified
        .then(|| signs.petrified_lights.first().cloned())
        .flatten();
    let statuses: Vec<Status> = case
        .form
        .iter()
        .cloned()
        .chain(petrified_light)
        .map(|light| Status {
            light: Some(light),
            text: None,
            colour: None,
        })
        .collect();
    let resists = case.resists.read(bundle, &statuses)?;
    Ok(torment::damage(bundle.formulas.torment, hp, resists))
}

/// The player as a harness makes them attack: their Strength and Dexterity, and Fighting exactly
/// as crawl rolls with it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
pub struct StrikerCase {
    /// Strength.
    pub strength: i32,
    /// Dexterity.
    pub dexterity: i32,
    /// Fighting, as `you.skill(SK_FIGHTING, 179200)`: fine enough that every scale crawl reads a
    /// skill at divides it.
    pub fighting: i32,
}

impl From<StrikerCase> for Striker {
    fn from(case: StrikerCase) -> Self {
        Self {
            strength: case.strength,
            dexterity: case.dexterity,
            fighting: DrawnSkill::exact(case.fighting),
        }
    }
}

/// One melee attack by the player on a monster, in the terms the ported formulas take.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerMeleeCase {
    /// The player.
    pub striker: StrikerCase,
    /// The weapon they wield, or none for an unarmed attack.
    pub weapon: Option<WeaponCase>,
    /// The skill the attack trains, the weapon's or Unarmed Combat, as `you.skill(sk, 179200)`.
    pub skill: i32,
    /// The monster's `monster_type` value, whose Game Knowledge gives its AC and EV.
    pub monster: u16,
}

/// One shot or throw by the player at a monster, in the terms the ported formulas take.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerShotCase {
    /// The player.
    pub striker: StrikerCase,
    /// The launcher they wield or the missile they throw, by sub-type identifier.
    pub weapon: EnumId,
    /// A launcher's enchantment.
    pub plus: i32,
    /// A launcher's brand, by `brand_type` identifier.
    pub brand: EnumId,
    /// Ranged Weapons or Throwing, as `you.skill(sk, 179200)`.
    pub skill: i32,
    /// The monster's `monster_type` value.
    pub monster: u16,
}

/// What many of the player's attacks on a monster came to, with how long each took.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PlayerTally {
    /// How often each to-hit roll came up.
    pub to_hit: BTreeMap<i32, u32>,
    /// Attacks that got past the monster's evasion.
    pub hits: u32,
    /// How often each amount of damage, after armour, was dealt by an attack that hit.
    pub damage: BTreeMap<i32, u32>,
    /// Thrown missiles that broke as they landed.
    pub broke: u32,
    /// How often each delay came up.
    pub delay: BTreeMap<i32, u32>,
}

impl PlayerTally {
    fn add(&mut self, blow: player_attacks::Blow, delay: i32) {
        *self.to_hit.entry(blow.to_hit).or_default() += 1;
        self.hits += u32::from(blow.hits);
        if blow.hits {
            *self.damage.entry(blow.damage).or_default() += 1;
        }
        self.broke += u32::from(blow.broke);
        *self.delay.entry(delay).or_default() += 1;
    }
}

/// The monster numbered `monster` as the player's attacks meet it.
fn defence(bundle: &Bundle, monster: u16) -> Result<Defence, CaseError> {
    let known = bundle
        .monster(monster)
        .ok_or(CaseError::NoMonster(monster))?;
    Ok(Defence::of(bundle.formulas.player_beam, known))
}

/// The heft of the player's weapon branded `brand`.
fn heft(bundle: &Bundle, brand: &EnumId) -> Result<Heft, CaseError> {
    Heft::of(bundle.formulas.player_attack, brand)
        .ok_or_else(|| CaseError::BrandNotPorted(brand.0.clone()))
}

/// Runs `case` `trials` times through the ported formulas, from a generator seeded with
/// `seed`, rolling the attack's delay as often.
///
/// # Errors
/// If the bundle lacks the monster or the weapon, or the weapon's brand isn't ported.
pub fn player_melee(
    bundle: &Bundle,
    case: &PlayerMeleeCase,
    seed: (u64, u64),
    trials: u32,
) -> Result<PlayerTally, CaseError> {
    let variant = bundle.formulas.player_attack;
    let skill = DrawnSkill::exact(case.skill);
    let hand = match &case.weapon {
        None => Hand::Unarmed { skill },
        Some(weapon) => Hand::Weapon {
            weapon: bundle
                .melee_weapons
                .get(&weapon.weapon)
                .ok_or_else(|| CaseError::NoMeleeWeapon(weapon.weapon.0.clone()))?,
            plus: weapon.plus,
            heft: heft(bundle, &weapon.brand)?,
            skill,
        },
    };
    let defence = defence(bundle, case.monster)?;
    let striker = case.striker.into();
    let mut rng = Rng::new(seed.0, seed.1);
    let mut tally = PlayerTally::default();
    for _ in 0..trials {
        let blow = player_attacks::swing(variant, &mut rng, striker, hand, defence);
        let delay = player_attacks::melee_delay(variant, &mut rng, hand);
        tally.add(blow, delay);
    }
    Ok(tally)
}

/// Runs `case` `trials` times through the ported formulas, from a generator seeded with
/// `seed`, rolling the shot's delay as often.
///
/// # Errors
/// If the bundle lacks the monster or the launcher or missile, or the launcher's brand isn't
/// ported.
pub fn player_shot(
    bundle: &Bundle,
    case: &PlayerShotCase,
    seed: (u64, u64),
    trials: u32,
) -> Result<PlayerTally, CaseError> {
    let variant = bundle.formulas.player_attack;
    let (id, weapon) = bundle
        .missile_weapons
        .get_key_value(&case.weapon)
        .ok_or_else(|| CaseError::NoMissileWeapon(case.weapon.0.clone()))?;
    let shot = Shot {
        weapon,
        id,
        plus: case.plus,
        heft: heft(bundle, &case.brand)?,
        skill: DrawnSkill::exact(case.skill),
    };
    let defence = defence(bundle, case.monster)?;
    let striker = case.striker.into();
    let mut rng = Rng::new(seed.0, seed.1);
    let mut tally = PlayerTally::default();
    for _ in 0..trials {
        let blow = player_attacks::shoot(variant, &mut rng, striker, shot, defence);
        let delay = player_attacks::shot_delay(variant, &mut rng, shot);
        tally.add(blow, delay);
    }
    Ok(tally)
}

/// A beam the player fires at a monster, from a spell or a wand.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlayerBeamCase {
    /// The spell's `spell_type` value.
    pub spell: u16,
    /// The power it is fired at.
    pub power: i32,
    /// The monster's `monster_type` value.
    pub monster: u16,
}

/// What many of the player's beams at a monster came to.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PlayerBeamTally {
    /// The beam's accuracy, `None` when it never misses.
    pub to_hit: Option<i32>,
    /// How often each amount of damage the monster took came up, 0 for a miss.
    pub damage: BTreeMap<i32, u32>,
}

/// The `spell_type` value of every spell the engine ports when the player casts it, so a crawl
/// harness can draw its cases from them.
#[must_use]
pub fn player_zaps(bundle: &Bundle) -> Vec<u16> {
    bundle
        .spells
        .iter()
        .filter(|(_, spell)| PlayerZap::of(bundle.formulas.player_beam, spell).is_some())
        .map(|(&value, _)| value)
        .collect()
}

/// The player's zap of the spell numbered `spell`.
fn player_zap(bundle: &Bundle, spell: u16) -> Result<PlayerZap<'_>, CaseError> {
    let known = bundle.spells.get(&spell).ok_or(CaseError::NoSpell(spell))?;
    PlayerZap::of(bundle.formulas.player_beam, known).ok_or(CaseError::PlayerZapNotPorted(spell))
}

/// Fires `case`'s beam `trials` times through the ported formulas, from a generator seeded with
/// `seed`.
///
/// # Errors
/// If the bundle lacks the monster or the spell, or the engine doesn't port the spell.
pub fn player_beam(
    bundle: &Bundle,
    case: &PlayerBeamCase,
    seed: (u64, u64),
    trials: u32,
) -> Result<PlayerBeamTally, CaseError> {
    let variant = bundle.formulas.player_beam;
    let zap = player_zap(bundle, case.spell)?;
    let beam = player_attacks::beam(variant, zap, case.power);
    let defence = defence(bundle, case.monster)?;
    let mut rng = Rng::new(seed.0, seed.1);
    let mut tally = PlayerBeamTally {
        to_hit: zap.accuracy(case.power),
        ..PlayerBeamTally::default()
    };
    for _ in 0..trials {
        let blow = player_attacks::fire(variant, &mut rng, beam, defence);
        *tally.damage.entry(blow.damage).or_default() += 1;
    }
    Ok(tally)
}

/// The power the player zaps a wand of the spell numbered `spell` with, at Evocations of
/// `evocations`, as `you.skill(SK_EVOCATIONS, 179200)`.
///
/// # Errors
/// If the bundle lacks the spell or the engine doesn't port it, or it has no power cap.
pub fn wand_power(bundle: &Bundle, spell: u16, evocations: i32) -> Result<i32, CaseError> {
    let cap = player_zap(bundle, spell)?
        .power_cap()
        .ok_or(CaseError::PlayerZapNotPorted(spell))?;
    Ok(player_attacks::wand_power(
        bundle.formulas.wand_power,
        DrawnSkill::exact(evocations),
        cap,
    ))
}

/// How often each number of hit points came up for a monster numbered `monster` made `trials`
/// times, from a generator seeded with `seed`.
///
/// # Errors
/// If the bundle lacks the monster.
pub fn monster_hit_points(
    bundle: &Bundle,
    monster: u16,
    seed: (u64, u64),
    trials: u32,
) -> Result<BTreeMap<i32, u32>, CaseError> {
    let known = bundle
        .monster(monster)
        .ok_or(CaseError::NoMonster(monster))?;
    let mut rng = Rng::new(seed.0, seed.1);
    let mut tally = BTreeMap::new();
    for _ in 0..trials {
        let hp = player_attacks::monster_hit_points(
            bundle.formulas.monster_hit_points,
            &mut rng,
            known.hp_10x,
        );
        *tally.entry(hp).or_default() += 1;
    }
    Ok(tally)
}

/// The player as a speed harness shows them: what the game shows of them that tells how fast
/// they move.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
pub struct MoverCase {
    /// Their species, as the game names it.
    pub species: String,
    /// Their experience level.
    pub xl: u32,
    /// The light of each status the status pane lists.
    pub lights: Vec<String>,
    /// Their god as the status pane shows them, if any.
    pub god: Option<GodShown>,
    /// The rows of the mutations screen (`A`).
    pub mutations: Vec<String>,
    /// The items they carry.
    pub carried: Vec<CarriedShown>,
    /// The `dungeon_feature_type` value of the square they stand on.
    pub feature: u16,
}

/// The player's god as the status pane shows them.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
pub struct GodShown {
    /// The god, as the game names them.
    pub name: String,
    /// The stars of piety it shows.
    pub piety_rank: u32,
    /// Whether the god is punishing them.
    pub penance: bool,
}

/// An item the player carries, by name, with whether they wear it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
pub struct CarriedShown {
    /// Its name, as the game shows it.
    pub name: String,
    /// Whether they wear it.
    pub worn: bool,
}

/// How often each time came up for the player of a speed case, tallied as the harness tallies
/// crawl's.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MoveTally {
    /// The time an action takes: `player_speed`.
    pub action: BTreeMap<i32, u32>,
    /// The delay a move onto their square adds: `player_movement_speed`.
    pub delay: BTreeMap<i32, u32>,
    /// The time a move onto their square takes.
    pub time: BTreeMap<i32, u32>,
}

/// The times `case`'s player takes, `trials` times each, from a generator seeded with `seed`,
/// read as the Advisor reads the status pane, the mutations screen, the character overview's
/// equipment and the map.
///
/// # Errors
/// If the bundle doesn't know the species, a status light or the feature underfoot.
pub fn player_move(
    bundle: &Bundle,
    case: &MoverCase,
    seed: (u64, u64),
    trials: u32,
) -> Result<MoveTally, CaseError> {
    let lights: Vec<&str> = case.lights.iter().map(String::as_str).collect();
    let religion = case.god.as_ref().map(|god| Religion::God {
        name: god.name.clone(),
        piety_rank: Some(god.piety_rank),
        penance: god.penance,
    });
    let screen = ScreenRead {
        screen: Screen::Mutations,
        title: None,
        lines: case.mutations.clone(),
    };
    let mutations = MutationsRead {
        shown: screen.mutations().unwrap_or_default(),
        xl: Some(case.xl),
    };
    let carried: Vec<(&str, Option<bool>)> = case
        .carried
        .iter()
        .map(|item| (item.name.as_str(), Some(item.worn)))
        .collect();
    let seen = player_move::Seen {
        lights: Some(&lights),
        species: Some(&case.species),
        xl: Some(case.xl),
        religion: Some(religion.as_ref().unwrap_or(&Religion::NoGod)),
        mutations: Some(&mutations),
        carried: Some(&carried),
    };
    let read = player_move::read(bundle, &seen)
        .ok_or_else(|| CaseError::NoSpecies(case.species.clone()))?;
    if let Some(Unread::Light(light)) = read
        .unread
        .iter()
        .find(|unread| matches!(unread, Unread::Light(_)))
    {
        return Err(CaseError::UnclassifiedLight(light.clone()));
    }
    let water = bundle
        .feature(case.feature)
        .ok_or(CaseError::NoFeature(case.feature))?
        .water;
    let variant = bundle.formulas.player_move;
    let mut rng = Rng::new(seed.0, seed.1);
    let mut tally = MoveTally::default();
    for _ in 0..trials {
        let action = player_move::action_time(variant, &read.mover, &mut rng);
        let delay = player_move::move_delay(variant, &read.mover, water, &mut rng);
        let time = player_move::move_time(variant, &read.mover, water, &mut rng);
        *tally.action.entry(action).or_default() += 1;
        *tally.delay.entry(delay).or_default() += 1;
        *tally.time.entry(time).or_default() += 1;
    }
    Ok(tally)
}
