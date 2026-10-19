//! Crawl's formulas for a monster shooting or throwing at the player, ported closely (ADR 0002)
//! behind the formula variants a Game Knowledge bundle names: whether it fires this action, the
//! energy that takes, and the missile's accuracy and damage. The roll against the player's
//! evasion, the shield block and armour are melee's: crawl's `ranged_attack` is an `attack`.
//!
//! Line numbers are crawl 0.34.1's (`crawl-ref/source/` at tag 0.34.1). Only the paths a monster
//! in plain view takes are ported: it has no statuses, isn't fleeing, sees the player and has a
//! clear line to them, and its launcher is in its hand. A launcher's enchantment and brand are
//! played (`brands.rs`); a thrown missile's brand isn't. The Fact Sheet lists what that leaves
//! out.

use copilot_gk::{
    Bundle, EnumId, Formulas, MissileAttack, MissileKind, MissileWeapon, Monster, MonsterFire,
};

use super::brands::{self, Brand, Necromancy};
use super::melee::{self, Aim, RolledDamage, Swing, Target, Weapon};
use super::rng::Rng;
use crate::crawl_ids::missile_types;

/// How far a missile flies: `you.current_vision` (`ranged_attack_beam::initialise_beam`,
/// `throw.cc:478`), the player's normal sight, `LOS_DEFAULT_RANGE` (`defines.h:115`).
pub(crate) const MISSILE_RANGE: i32 = 7;

/// A launcher or throwing missile a monster may fire, as the formulas tell them apart.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Missile<'a> {
    /// Its Game Knowledge.
    pub(crate) weapon: &'a MissileWeapon,
    /// It deals no damage itself, as a dart only poisons and a net only catches
    /// (`ranged_attack::handle_phase_hit`, `ranged-attack.cc:255`).
    harmless: bool,
    /// It passes through the player's shield, as a javelin does (`is_penetrating_attack`,
    /// `throw.cc:60`, and `ranged_attack::ignores_shield`).
    pierces: bool,
    /// Its enchantment, which adds to its accuracy and its damage: a launcher's plus. Thrown
    /// missiles have none.
    pub(crate) plus: i32,
    /// Its brand: a launcher's. A thrown missile's own brands do nothing the engine ports.
    pub(crate) brand: Brand,
}

impl<'a> Missile<'a> {
    /// The missile weapon `weapon`, the sub-type `id`, with enchantment `plus` and `brand`, as
    /// `variant` tells missiles apart. A thrown missile takes neither.
    pub(crate) fn of(
        variant: MissileAttack,
        id: &EnumId,
        weapon: &'a MissileWeapon,
        plus: i32,
        brand: Brand,
    ) -> Self {
        match variant {
            MissileAttack::Crawl034 => {
                let launcher = weapon.kind == MissileKind::Launcher;
                Self {
                    weapon,
                    harmless: matches!(
                        id.0.as_str(),
                        missile_types::DART | missile_types::THROWING_NET
                    ),
                    pierces: id.0 == missile_types::JAVELIN,
                    plus: if launcher { plus } else { 0 },
                    brand: if launcher { brand } else { Brand::Plain },
                }
            }
        }
    }

    /// It as crawl's `attack` rolls with it.
    pub(crate) fn weapon(self) -> Weapon {
        Weapon {
            damage: self.weapon.damage,
            accuracy: self.weapon.accuracy,
            plus: self.plus,
            brand: self.brand,
        }
    }

    /// The missile weapon `weapon`, the sub-type `id`, +0 and unbranded: as a monster's Possible
    /// Gear is played.
    pub(crate) fn plain(variant: MissileAttack, id: &EnumId, weapon: &'a MissileWeapon) -> Self {
        Self::of(variant, id, weapon, 0, Brand::Plain)
    }
}

/// What a monster may shoot or throw: a launcher, missiles to throw, or both.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Missiles<'a> {
    /// Its launcher.
    pub(crate) launcher: Option<Missile<'a>>,
    /// Its missiles to throw.
    pub(crate) thrown: Option<Missile<'a>>,
}

impl<'a> Missiles<'a> {
    /// `missile` alone.
    pub(crate) fn of(missile: Missile<'a>) -> Self {
        if missile.weapon.kind == MissileKind::Launcher {
            Self {
                launcher: Some(missile),
                thrown: None,
            }
        } else {
            Self {
                launcher: None,
                thrown: Some(missile),
            }
        }
    }

    /// Each of them.
    pub(crate) fn each(self) -> impl Iterator<Item = Missile<'a>> {
        self.launcher.into_iter().chain(self.thrown)
    }
}

/// What a monster `distance` squares from the player shoots or throws at them this action, of
/// `missiles`, if anything: `handle_throw` (`mon-act.cc:1487`) for a monster that sees the player
/// with nothing between them. It doesn't shoot from beside the player unless it `prefers_ranged`
/// (`M_PREFER_RANGED`). With both a launcher and missiles to throw, it takes the launcher two
/// times in three (`mon-act.cc:1535`). Then it fires nine times in ten if it prefers ranged, a
/// launcher four in five, and a thrown missile one in three.
pub(crate) fn fires<'a>(
    variant: MonsterFire,
    rng: &mut Rng,
    missiles: Missiles<'a>,
    prefers_ranged: bool,
    distance: i32,
) -> Option<Missile<'a>> {
    match variant {
        MonsterFire::Crawl034 => {
            // Its tracer stops at the missile's range.
            if distance > MISSILE_RANGE {
                return None;
            }
            let adjacent = distance <= 1;
            if adjacent && !prefers_ranged {
                return None;
            }
            let missile = match (missiles.launcher, missiles.thrown) {
                (Some(launcher), Some(thrown)) => {
                    if rng.one_chance_in(3) {
                        thrown
                    } else {
                        launcher
                    }
                }
                (Some(one), None) | (None, Some(one)) => one,
                (None, None) => return None,
            };
            let fires = if prefers_ranged {
                !(rng.one_chance_in(10) && !adjacent)
            } else if missile.weapon.kind == MissileKind::Launcher {
                !rng.one_chance_in(5)
            } else {
                rng.one_chance_in(3)
            };
            fires.then_some(missile)
        }
    }
}

/// The energy firing `missile` takes a monster whose missile action costs `missile_energy`:
/// `mons_throw` (`throw.cc:820`), its cost times its attack delay over 10. A thrown missile has a
/// delay of 10, and a launcher in hand 10 as its brand changes it (`monster::attack_delay`,
/// `monster.cc:439`).
pub(crate) fn fire_energy(
    formulas: Formulas,
    rng: &mut Rng,
    missile: Missile<'_>,
    missile_energy: i32,
) -> i32 {
    match formulas.monster_fire {
        MonsterFire::Crawl034 => {
            let delay = brands::delay(formulas.damage_brand, rng, missile.brand, 10);
            rng.div_rand_round(missile_energy * delay, 10)
        }
    }
}

/// The monster firing, as the missile formulas take it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Shooter {
    /// Its hit dice.
    pub(crate) hit_dice: i32,
    /// It has crawl's `archer` flag.
    pub(crate) archer: bool,
    /// The damage of its first melee attack, which a missile adds to its own
    /// (`attack::init_attack(0)`, `attack.cc:370`): 0 when it has none.
    pub(crate) first_attack: i32,
    /// It sees through an umbra.
    pub(crate) nightvision: bool,
    /// Its necromancy skill, which a launcher of pain rolls against.
    pub(crate) necromancy: Necromancy,
}

impl Shooter {
    /// `monster` firing with `hit_dice`, its skills judged by the spells of its book in `bundle`.
    pub(crate) fn of(bundle: &Bundle, monster: &Monster, hit_dice: i32) -> Self {
        Self {
            necromancy: Necromancy::of(bundle, monster, hit_dice),
            hit_dice,
            archer: monster.archer,
            first_attack: monster
                .attacks
                .first()
                .map_or(0, |attack| i32::try_from(attack.damage).unwrap_or(i32::MAX)),
            nightvision: monster.nightvision,
        }
    }
}

/// One missile fired at the player, rolled as `ranged_attack::attack` (`ranged-attack.cc:90`)
/// rolls it: accuracy, evasion, the shield, then damage and armour for a missile that lands, and
/// what its brand adds to that (`ranged_attack::handle_phase_hit`, `ranged-attack.cc:255`).
/// `can_block` is whether the shield has a block left before the player's next action.
pub(crate) fn shoot(
    formulas: Formulas,
    rng: &mut Rng,
    shooter: Shooter,
    missile: Missile<'_>,
    target: Target,
    can_block: bool,
) -> Swing {
    let Target {
        defences, lighting, ..
    } = target;
    match formulas.missile_attack {
        MissileAttack::Crawl034 => {
            // `ranged_attack::calc_mon_to_hit_base` (`ranged-attack.cc:363`) counts an archer as
            // skilled, a launcher adds its enchantment to its accuracy (`attack.cc:231`), and a
            // thrown missile adds no accuracy (`property`, `item-prop.cc:2818`).
            let aim = Aim {
                hit_dice: shooter.hit_dice,
                skilled: shooter.archer,
                accuracy: missile.weapon().to_hit(),
                nightvision: shooter.nightvision,
            };
            let to_hit = melee::monster_to_hit(formulas.monster_to_hit, rng, aim, lighting);
            let hits = melee::evasion_margin(formulas.evasion_roll, rng, to_hit, defences) >= 0;
            // The block is rolled even for a missile that ignores it (`attack_shield_blocked`).
            let blocked = melee::shield_blocks(
                formulas.shield_block,
                rng,
                to_hit,
                shooter.hit_dice,
                defences,
            ) && can_block
                && !missile.pierces;
            let mut swing = Swing {
                to_hit,
                hits,
                blocked,
                damage: 0,
            };
            if swing.landed() {
                let dealt = if missile.harmless {
                    0
                } else {
                    let rolled = missile_damage(formulas, rng, shooter, missile);
                    melee::apply_ac(formulas.armour, rng, rolled, defences)
                };
                let brand = brands::damage(
                    formulas,
                    rng,
                    missile.brand,
                    shooter.necromancy,
                    dealt,
                    target.resists,
                );
                swing.damage = dealt + brand;
            }
            swing
        }
    }
}

/// A missile's damage before armour: [`melee::monster_damage`] with the missile as the weapon
/// and the monster's first attack as its own, and an archer's bonus
/// `random2avg(hd * 4 / 3, 2)` (`ranged_attack::apply_damage_modifiers`, `ranged-attack.cc:369`,
/// and `archer_bonus_damage`, `fight.cc:1671`).
fn missile_damage(
    formulas: Formulas,
    rng: &mut Rng,
    shooter: Shooter,
    missile: Missile<'_>,
) -> RolledDamage {
    let mut rolled =
        melee::monster_damage(formulas, rng, Some(missile.weapon()), shooter.first_attack);
    if shooter.archer {
        rolled.damage += rng.random2avg(shooter.hit_dice * 4 / 3, 2);
    }
    rolled
}
