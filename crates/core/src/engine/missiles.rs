//! Crawl's formulas for a monster shooting or throwing at the player, ported closely (ADR 0002)
//! behind the formula variants a Game Knowledge bundle names: whether it fires this action, the
//! energy that takes, and the missile's accuracy and damage. The roll against the player's
//! evasion, the shield block and armour are melee's: crawl's `ranged_attack` is an `attack`.
//!
//! Line numbers are crawl 0.34.1's (`crawl-ref/source/` at tag 0.34.1). Only the paths a monster
//! in plain view takes are ported: it has no statuses, isn't fleeing, sees the player and has a
//! clear line to them, and its launcher or missile is +0 and unbranded. The Fact Sheet lists what
//! that leaves out.

use copilot_gk::{
    EnumId, Formulas, MissileAttack, MissileKind, MissileWeapon, Monster, MonsterFire,
};

use super::melee::{self, Aim, RolledDamage, Swing, Target};
use super::rng::Rng;

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
}

impl<'a> Missile<'a> {
    /// The missile weapon `weapon`, the sub-type `id`, as `variant` tells missiles apart.
    pub(crate) fn of(variant: MissileAttack, id: &EnumId, weapon: &'a MissileWeapon) -> Self {
        match variant {
            MissileAttack::Crawl034 => Self {
                weapon,
                harmless: matches!(id.0.as_str(), "MI_DART" | "MI_THROWING_NET"),
                pierces: id.0 == "MI_JAVELIN",
            },
        }
    }
}

/// Whether a monster `distance` squares from the player shoots or throws `missile` at them this
/// action: `handle_throw` (`mon-act.cc:1487`) for a monster that sees the player with nothing
/// between them. It doesn't shoot from beside the player unless it `prefers_ranged`
/// (`M_PREFER_RANGED`); then it fires nine times in ten, a launcher four in five, and a thrown
/// missile one in three.
pub(crate) fn fires(
    variant: MonsterFire,
    rng: &mut Rng,
    missile: Missile<'_>,
    prefers_ranged: bool,
    distance: i32,
) -> bool {
    match variant {
        MonsterFire::Crawl034 => {
            // Its tracer stops at the missile's range.
            if distance > MISSILE_RANGE {
                return false;
            }
            let adjacent = distance <= 1;
            if adjacent && !prefers_ranged {
                return false;
            }
            if prefers_ranged {
                !(rng.one_chance_in(10) && !adjacent)
            } else if missile.weapon.kind == MissileKind::Launcher {
                !rng.one_chance_in(5)
            } else {
                rng.one_chance_in(3)
            }
        }
    }
}

/// The energy firing takes a monster whose missile action costs `missile_energy`: `mons_throw`
/// (`throw.cc:820`), its cost times its attack delay over 10, where an unbranded launcher or a
/// thrown missile has a delay of 10 (`monster::attack_delay`, `monster.cc:439`).
pub(crate) fn fire_energy(variant: MonsterFire, rng: &mut Rng, missile_energy: i32) -> i32 {
    match variant {
        MonsterFire::Crawl034 => {
            let delay = 10;
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
}

impl Shooter {
    /// `monster` firing with `hit_dice`.
    pub(crate) fn of(monster: &Monster, hit_dice: i32) -> Self {
        Self {
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
/// rolls it: accuracy, evasion, the shield, then damage and armour for a missile that lands.
/// `can_block` is whether the shield has a block left before the player's next action.
pub(crate) fn shoot(
    formulas: Formulas,
    rng: &mut Rng,
    shooter: Shooter,
    missile: Missile<'_>,
    target: Target,
    can_block: bool,
) -> Swing {
    let Target { defences, lighting } = target;
    match formulas.missile_attack {
        MissileAttack::Crawl034 => {
            // `ranged_attack::calc_mon_to_hit_base` (`ranged-attack.cc:363`) counts an archer as
            // skilled, and a thrown missile adds no accuracy (`property`, `item-prop.cc:2818`).
            let aim = Aim {
                hit_dice: shooter.hit_dice,
                skilled: shooter.archer,
                accuracy: missile.weapon.accuracy,
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
            if swing.landed() && !missile.harmless {
                let rolled = missile_damage(rng, shooter, missile);
                swing.damage = melee::apply_ac(formulas.armour, rng, rolled, defences);
            }
            swing
        }
    }
}

/// A missile's damage before armour: the monster branch of `attack::calc_damage`
/// (`attack.cc:918`) with a +0 unbranded weapon, `random2` of its damage plus the monster's own
/// `1 + random2(first_attack)`, and an archer's bonus `random2avg(hd * 4 / 3, 2)`
/// (`ranged_attack::apply_damage_modifiers`, `ranged-attack.cc:369`, and
/// `archer_bonus_damage`, `fight.cc:1671`).
fn missile_damage(rng: &mut Rng, shooter: Shooter, missile: Missile<'_>) -> RolledDamage {
    let weapon = missile.weapon.damage;
    // A +0 weapon and no slaying add `random2(1)`, nothing (`_core_apply_slaying`).
    let mut damage = rng.random2(weapon);
    damage += 1 + rng.random2(shooter.first_attack);
    if shooter.archer {
        damage += rng.random2avg(shooter.hit_dice * 4 / 3, 2);
    }
    RolledDamage {
        damage,
        max: weapon + shooter.first_attack,
    }
}
