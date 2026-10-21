//! Crawl's formulas for the player attacking a monster, ported closely (ADR 0002) behind the
//! formula variants a Game Knowledge bundle names: a melee swing with a weapon or unarmed, a shot
//! from a launcher, a thrown missile, and a beam from a spell or a wand, with how long each takes;
//! the power a wand is zapped with; and a monster's hit points as it is made.
//!
//! Line numbers are crawl 0.34.1's (`crawl-ref/source/` at tag 0.34.1). Only the paths a plain
//! attack takes are ported: the player has no statuses, form, mutations or slaying, wears nothing
//! that slows their attacks, sees the monster and isn't stabbing it, and the monster has no
//! statuses, armour or shield beyond its kind's, and stands in no halo or umbra. The Fact Sheet
//! lists what that leaves out.

use copilot_gk::{
    EnumId, MeleeWeapon, MissileWeapon, Monster, MonsterHitPoints, PlayerAttack, PlayerBeam, Spell,
    WandPower, Zap, ZapDamage, ZapToHit,
};

use super::melee::{AUTOMATIC_HIT, MIN_HIT_MISS_PERCENTAGE};
use super::resists::{self, MonsterBeamResist, MonsterResistances};
use super::rng::Rng;
use super::spells::{self, ArmourRule};
use crate::crawl_ids::{brand_types, missile_types, skill_types};

/// `FASTEST_PLAYER_THROWING_SPEED` (`player.h:100`): no throw takes less than this.
const FASTEST_THROWING_DELAY: i32 = 7;

/// `MAX_SKILL_LEVEL` (`defines.h`).
const MAX_SKILL_LEVEL: i32 = 27;

/// The base damage every form, and none, gives an unarmed attack (`Form::get_base_unarmed_damage`,
/// `transform.cc:329`).
const UNARMED_BASE_DAMAGE: i32 = 3;

/// The spells crawl 0.34's `_do_cast` (`spl-cast.cc:2443`) hands straight to `zapping` as one
/// zap that deals damage, which the player learns or a wand zaps: no special case in its switch.
/// Crawl's abilities and evocables that fire zaps of their own aren't spells the player knows.
const PLAYER_ZAPS_0_34: &[&str] = &[
    "SPELL_MAGIC_DART",
    "SPELL_THROW_FLAME",
    "SPELL_SHOCK",
    "SPELL_SANDBLAST",
    "SPELL_STONE_ARROW",
    "SPELL_BOMBARD",
    "SPELL_LEHUDIBS_CRYSTAL_SPEAR",
    "SPELL_FIREBALL",
    "SPELL_KINETIC_GRAPNEL",
    "SPELL_CORROSIVE_BOLT",
    "SPELL_BOLT_OF_LIGHT",
    "SPELL_MERCURY_ARROW",
    "SPELL_QUICKSILVER_BOLT",
    "SPELL_STICKY_FLAME",
    "SPELL_ICEBLAST",
];

/// The crossbows crawl 0.34's `is_crossbow` (`item-prop.cc:2158`) names, which skill never makes
/// faster than a standard-speed action (`weapon_min_delay`, `fight.cc:1259`).
const CROSSBOWS_0_34: &[&str] = &["WPN_HAND_CANNON", "WPN_ARBALEST", "WPN_TRIPLE_CROSSBOW"];

/// The skills whose weapons crawl 0.34's damage takes Dexterity for rather than Strength
/// (`weapon_uses_strength`, `fight.cc:1680`).
const DEXTEROUS_SKILLS_0_34: &[&str] = &["SK_SHORT_BLADES", "SK_LONG_BLADES", "SK_RANGED_WEAPONS"];

/// The skill whose weapons crawl 0.34 lets skill make as fast as an unarmed attack
/// (`weapon_min_delay`, `fight.cc:1259`).
const SHORT_BLADES_0_34: &str = "SK_SHORT_BLADES";

/// A spell the player casts, or a wand zaps, as one zap that deals damage: one the engine ports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PlayerZap<'a> {
    /// The spell.
    pub(crate) spell: &'a Spell,
    /// The zap it fires.
    pub(crate) zap: &'a Zap,
    damage: ZapDamage,
    to_hit: ZapToHit,
}

impl<'a> PlayerZap<'a> {
    /// `spell` as the player casts it, if `variant` ports it: a zap of damage and accuracy the
    /// player fires, of a flavour the monster resist port knows.
    pub(crate) fn of(variant: PlayerBeam, spell: &'a Spell) -> Option<Self> {
        let zaps = match variant {
            PlayerBeam::Crawl034 => PLAYER_ZAPS_0_34,
        };
        if !zaps.contains(&spell.id.0.as_str()) {
            return None;
        }
        let zap = spell.zap.as_ref()?;
        resists::monster_beam(variant, &zap.flavour)?;
        Some(Self {
            spell,
            zap,
            damage: zap.player_damage?,
            to_hit: zap.player_to_hit?,
        })
    }

    /// The most power it takes, past which it grows no stronger: `spell_power_cap`.
    pub(crate) fn power_cap(self) -> Option<i32> {
        self.spell.range.map(|range| range.power_cap)
    }

    /// Its accuracy at `power`, `None` when it never misses.
    pub(crate) fn accuracy(self, power: i32) -> Option<i32> {
        spells::accuracy(self.to_hit, power)
    }
}

/// One of the player's skills at the level the skills screen shows, in tenths.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct Skill {
    tenths: i32,
}

impl Skill {
    /// A skill the screen shows at `tenths` tenths of a level.
    pub(crate) fn shown(tenths: u32) -> Self {
        Self {
            tenths: i32::try_from(tenths).unwrap_or(i32::MAX),
        }
    }

    /// The skill as one trial plays it: crawl rolls with finer fractions of a level than the
    /// tenths it shows (`player::skill`, `player.cc:6212`), so where it lies within its tenth is
    /// drawn, anywhere as likely as anywhere else. A skill shown at 0 is untrained, and exactly 0.
    pub(crate) fn drawn(self, rng: &mut Rng) -> DrawnSkill {
        let tenth = DrawnSkill::SCALE / 10;
        let fine = if self.tenths > 0 {
            self.tenths * tenth + rng.random2(tenth)
        } else {
            0
        };
        DrawnSkill { fine }
    }
}

/// A skill as one trial plays it, in [`DrawnSkill::SCALE`]ths of a level.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct DrawnSkill {
    fine: i32,
}

impl DrawnSkill {
    /// The fraction of a level a skill is kept to: every scale crawl reads a skill at (7, 10, 100
    /// and 256) divides it, so reading it at any of them rounds as crawl does.
    pub(crate) const SCALE: i32 = 179_200;

    /// A skill of exactly `fine` [`DrawnSkill::SCALE`]ths of a level, as a crawl oracle harness
    /// reads it: `you.skill(sk, 179200)`.
    pub(crate) fn exact(fine: i32) -> Self {
        Self { fine }
    }

    /// `you.skill(sk, scale)`: the level times `scale`, rounded down.
    fn scaled(self, scale: i32) -> i32 {
        i32::try_from(i64::from(self.fine) * i64::from(scale) / i64::from(Self::SCALE))
            .unwrap_or(i32::MAX)
    }

    /// `actor::skill_rdiv` (`actor.cc:92`): the level, rounded at random.
    fn rounded(self, rng: &mut Rng) -> i32 {
        rng.div_rand_round(self.scaled(256), 256)
    }
}

/// The player as their attacks meet a monster in one trial: their Strength and Dexterity as the
/// status pane shows them, and their Fighting skill.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Striker {
    /// Strength.
    pub(crate) strength: i32,
    /// Dexterity.
    pub(crate) dexterity: i32,
    /// Fighting.
    pub(crate) fighting: DrawnSkill,
}

/// A weapon brand as the player's attacks feel it: those that change a weapon's damage or how
/// long a swing or shot takes (`brand_adjust_weapon_damage` and `player::attack_delay_with`).
/// Every other brand adds an effect on a hit, which isn't ported for the player.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Heft {
    /// No such brand.
    Plain,
    /// `SPWPN_HEAVY`: nine fifths the damage, half as long again.
    Heavy,
    /// `SPWPN_SPEED`: two thirds as long.
    Speed,
}

impl Heft {
    /// The heft of a weapon branded `brand`, a `brand_type` identifier, as `variant` plays it:
    /// `None` for a brand whose effect on a hit the port leaves out, such as flaming.
    pub(crate) fn of(variant: PlayerAttack, brand: &EnumId) -> Option<Self> {
        match variant {
            PlayerAttack::Crawl034 => match brand.0.as_str() {
                brand_types::NORMAL => Some(Self::Plain),
                brand_types::HEAVY => Some(Self::Heavy),
                brand_types::SPEED => Some(Self::Speed),
                _ => None,
            },
        }
    }
}

/// What the player attacks with in melee.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Hand<'a> {
    /// Nothing: unarmed, trained by Unarmed Combat.
    Unarmed {
        /// Unarmed Combat.
        skill: DrawnSkill,
    },
    /// A melee weapon.
    Weapon {
        /// Its kind.
        weapon: &'a MeleeWeapon,
        /// Its enchantment.
        plus: i32,
        /// Its brand, as far as damage and delay go.
        heft: Heft,
        /// The skill it trains.
        skill: DrawnSkill,
    },
}

/// What the player shoots or throws.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Shot<'a> {
    /// The launcher, or the missile thrown.
    pub(crate) weapon: &'a MissileWeapon,
    /// Its kind's identifier, such as `MI_JAVELIN`.
    pub(crate) id: &'a EnumId,
    /// A launcher's enchantment; a thrown missile adds none.
    pub(crate) plus: i32,
    /// A launcher's brand, as far as damage and delay go.
    pub(crate) heft: Heft,
    /// Ranged Weapons for a launcher, Throwing for a thrown missile.
    pub(crate) skill: DrawnSkill,
}

impl Shot<'_> {
    /// Whether it is thrown by hand rather than shot from a launcher.
    fn thrown(self) -> bool {
        self.weapon.delay.is_none()
    }
}

/// How one of the player's attacks on a monster went.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Blow {
    /// Its rolled accuracy, [`AUTOMATIC_HIT`] for one that can't miss.
    pub(crate) to_hit: i32,
    /// It got past the monster's evasion.
    pub(crate) hits: bool,
    /// What the monster took: 0 unless it hit.
    pub(crate) damage: i32,
    /// A thrown missile broke as it landed, and is gone.
    pub(crate) broke: bool,
}

/// A monster as the player's attacks meet it: its kind's AC, EV and resistances, with nothing
/// worn and no statuses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Defence {
    /// `monster::armour_class`.
    pub(crate) ac: i32,
    /// `monster::evasion`.
    pub(crate) ev: i32,
    /// What it resists of a beam's damage.
    pub(crate) resists: MonsterResistances,
}

impl Defence {
    /// A monster of `monster`'s kind, as `variant` reads its resistances.
    pub(crate) fn of(variant: PlayerBeam, monster: &Monster) -> Self {
        Self {
            ac: monster.ac,
            ev: monster.ev,
            resists: MonsterResistances::of(variant, monster),
        }
    }
}

/// The player's melee to-hit for one attack: `attack::calc_to_hit` (`attack.cc:288`) for the
/// player, with the pre-roll value of `attack::calc_pre_roll_to_hit` (`attack.cc:160`) and none of
/// the post-roll modifiers a plain attack on a visible monster has.
fn melee_to_hit(variant: PlayerAttack, rng: &mut Rng, striker: Striker, hand: Hand<'_>) -> i32 {
    match variant {
        PlayerAttack::Crawl034 => {
            let (skill, bonus) = match hand {
                // Unarmed Combat gets six to make up for having no enchantment.
                Hand::Unarmed { skill } => (skill, 6),
                Hand::Weapon {
                    weapon,
                    plus,
                    skill,
                    ..
                } => (skill, plus + weapon.accuracy),
            };
            pre_rolled_to_hit(rng, striker, skill, bonus)
        }
    }
}

/// `15 + Dex/2`, Fighting and the attack's skill each rounded at random, and `bonus`, rolled as
/// `random2` (`attack.cc:160` to `attack.cc:300`, for the player).
fn pre_rolled_to_hit(rng: &mut Rng, striker: Striker, skill: DrawnSkill, bonus: i32) -> i32 {
    let mut mhit = 15 + striker.dexterity / 2;
    mhit += random2_div(rng, striker.fighting.scaled(100), 100);
    mhit += random2_div(rng, skill.scaled(100), 100);
    mhit += bonus;
    maybe_random2(rng, mhit)
}

/// `maybe_random2_div(nom, denom, true)` (`random.cc:246`).
fn random2_div(rng: &mut Rng, nom: i32, denom: i32) -> i32 {
    if nom <= 0 {
        0
    } else {
        rng.random2(nom + denom) / denom
    }
}

/// `maybe_random2(x, true)` (`random.cc:235`).
fn maybe_random2(rng: &mut Rng, x: i32) -> i32 {
    if x <= 1 { 0 } else { rng.random2(x) }
}

/// Whether an attack by the player with `to_hit` gets past a monster's evasion `ev`:
/// `attack::test_hit` (`attack.cc:984`), which doesn't randomise the evasion of a monster the
/// player attacks.
fn hits(rng: &mut Rng, to_hit: i32, ev: i32) -> bool {
    if to_hit >= AUTOMATIC_HIT {
        return true;
    }
    if rng.x_chance_in_y(MIN_HIT_MISS_PERCENTAGE, 100) {
        return rng.random2(2) == 1;
    }
    to_hit - ev >= 0
}

/// One melee attack by the player on a monster, rolled as `melee_attack::attack`
/// (`melee-attack.cc:1739`) rolls it: accuracy, the monster's evasion, then for a hit, damage and
/// armour (`attack::calc_damage`, `attack.cc:918`).
pub(crate) fn swing(
    variant: PlayerAttack,
    rng: &mut Rng,
    striker: Striker,
    hand: Hand<'_>,
    defence: Defence,
) -> Blow {
    let dexterous_skills = match variant {
        PlayerAttack::Crawl034 => DEXTEROUS_SKILLS_0_34,
    };
    let to_hit = melee_to_hit(variant, rng, striker, hand);
    if !hits(rng, to_hit, defence.ev) {
        return Blow::missed(to_hit);
    }
    let damage = match hand {
        Hand::Unarmed { skill } => {
            // `attack::calc_base_unarmed_damage` (`attack.cc:899`), with Unarmed Combat rounded
            // at random (`unarmed_base_damage_bonus`, `fight.cc:1774`).
            let potential = UNARMED_BASE_DAMAGE + skill.rounded(rng);
            let damage = rolled(rng, potential.max(0), true, striker);
            let damage = with_fighting(rng, damage, striker);
            with_slaying(rng, damage, 0)
        }
        Hand::Weapon {
            weapon,
            plus,
            heft,
            skill,
        } => {
            let potential = hefted_damage(rng, weapon.damage, heft);
            let uses_strength = !dexterous_skills.contains(&weapon.skill.0.as_str());
            let damage = rolled(rng, potential, uses_strength, striker);
            let damage = with_weapon_skill(rng, damage, skill);
            let damage = with_fighting(rng, damage, striker);
            with_slaying(rng, damage, plus)
        }
    };
    Blow {
        to_hit,
        hits: true,
        damage: monster_armour(rng, ArmourRule::Normal, defence.ac, damage),
        broke: false,
    }
}

impl Blow {
    /// An attack with `to_hit` that missed.
    fn missed(to_hit: i32) -> Self {
        Self {
            to_hit,
            hits: false,
            damage: 0,
            broke: false,
        }
    }
}

/// A weapon's damage as its brand leaves it: `brand_adjust_weapon_damage` (`fight.cc:1812`),
/// nine fifths of it, rounded at random, for a heavy weapon.
fn hefted_damage(rng: &mut Rng, damage: i32, heft: Heft) -> i32 {
    match heft {
        Heft::Heavy => rng.div_rand_round(damage * 9, 5),
        Heft::Plain | Heft::Speed => damage,
    }
}

/// The damage rolled from `potential`, after the player's Strength, or Dexterity when not
/// `uses_strength`: `stat_modify_damage` (`fight.cc:1695`) on a hundred times it, then
/// `div_round_near(random2(potential + 1), 100)` (`attack.cc:957`).
fn rolled(rng: &mut Rng, potential: i32, uses_strength: bool, striker: Striker) -> i32 {
    let attribute = if uses_strength {
        striker.strength
    } else {
        striker.dexterity
    };
    // Crawl multiplies an int by a double and truncates.
    let factor = (75.0 + 2.5 * f64::from(attribute)).max(1.0);
    #[allow(clippy::cast_possible_truncation)]
    let modified = (f64::from(potential * 100) * factor) as i32 / 100;
    div_round_near(rng.random2(modified + 1), 100)
}

/// `div_round_near` (`random.cc:330`).
fn div_round_near(num: i32, den: i32) -> i32 {
    num / den + i32::from(num % den >= den / 2)
}

/// `apply_weapon_skill` (`fight.cc:1710`).
fn with_weapon_skill(rng: &mut Rng, damage: i32, skill: DrawnSkill) -> i32 {
    damage * (2500 + maybe_random2(rng, skill.scaled(100) + 1)) / 2500
}

/// `apply_fighting_skill` (`fight.cc:1718`) for a weapon or unarmed attack, not an auxiliary one.
fn with_fighting(rng: &mut Rng, damage: i32, striker: Striker) -> i32 {
    damage * (3000 + maybe_random2(rng, striker.fighting.scaled(100) + 1)) / 3000
}

/// `attack::player_apply_slaying_bonuses` (`attack.cc:840`) with no slaying: the weapon's
/// enchantment, by `_core_apply_slaying` (`attack.cc:829`).
fn with_slaying(rng: &mut Rng, damage: i32, plus: i32) -> i32 {
    if plus >= 0 {
        damage + rng.random2(1 + plus)
    } else {
        damage - rng.random2(1 - plus)
    }
}

/// What a monster with armour class `ac` takes of `damage`: `actor::apply_ac` (`actor.cc:343`)
/// under `rule`, with no guaranteed damage reduction, which a monster doesn't get.
fn monster_armour(rng: &mut Rng, rule: ArmourRule, ac: i32, damage: i32) -> i32 {
    (damage - rule.saved(rng, ac.max(0))).max(0)
}

/// How long one melee attack takes the player, in crawl's units where a standard-speed action
/// takes 10: `player::attack_delay_with` (`player-act.cc:252`) for what they hold, unhindered by
/// a shield or armour and at their normal speed.
pub(crate) fn melee_delay(variant: PlayerAttack, rng: &mut Rng, hand: Hand<'_>) -> i32 {
    match variant {
        PlayerAttack::Crawl034 => {
            let delay = match hand {
                Hand::Unarmed { skill } => 10 - rng.div_rand_round(skill.scaled(10), 27 * 2),
                Hand::Weapon {
                    weapon,
                    heft,
                    skill,
                    ..
                } => {
                    let min_delay = min_delay(weapon.delay, weapon.skill.0.as_str(), false);
                    skilled_delay(rng, weapon.delay, min_delay, skill, heft)
                }
            };
            // At the player's normal speed, the scaling by speed and its floor of 1 change nothing.
            delay.max(3)
        }
    }
}

/// How long one shot or throw takes the player: `player::attack_delay_with` (`player-act.cc:252`)
/// for a launcher, unhindered by a shield or armour, or for a thrown missile, `10` plus half its
/// damage less half its skill, at their normal speed.
pub(crate) fn shot_delay(variant: PlayerAttack, rng: &mut Rng, shot: Shot<'_>) -> i32 {
    match variant {
        PlayerAttack::Crawl034 => {
            let delay = match shot.weapon.delay {
                None => {
                    let delay =
                        10 + shot.weapon.damage / 2 - rng.div_rand_round(shot.skill.scaled(10), 20);
                    delay.max(FASTEST_THROWING_DELAY)
                }
                Some(speed) => {
                    let crossbow = CROSSBOWS_0_34.contains(&shot.id.0.as_str());
                    let min_delay = min_delay(speed, skill_types::RANGED_WEAPONS, crossbow);
                    skilled_delay(rng, speed, min_delay, shot.skill, shot.heft)
                }
            };
            // At the player's normal speed, the scaling by speed and its floor of 1 change nothing.
            delay.max(3)
        }
    }
}

/// A weapon's delay of `speed` with no skill, less the skill that trains it, which counts only
/// until the weapon is as fast as it gets (`min_delay`), then changed by its brand.
fn skilled_delay(rng: &mut Rng, speed: i32, min_delay: i32, skill: DrawnSkill, heft: Heft) -> i32 {
    // `weapon_min_delay_skill` (`fight.cc:1244`).
    let skill_for_min_delay = (speed - min_delay) * 2;
    let skill = skill.scaled(10).min(10 * skill_for_min_delay);
    let delay = speed - rng.div_rand_round(skill, 20);
    match heft {
        Heft::Plain => delay,
        Heft::Speed => rng.div_rand_round(delay * 2, 3),
        Heft::Heavy => rng.div_rand_round(delay * 3, 2),
    }
}

/// The fastest a weapon of `speed` trained by `skill` gets with skill: `weapon_min_delay`
/// (`fight.cc:1259`) for a weapon that isn't an artefact, without its brand.
fn min_delay(speed: i32, skill: &str, crossbow: bool) -> i32 {
    let mut min_delay = speed / 2;
    if skill == SHORT_BLADES_0_34 && min_delay > 5 {
        min_delay = 5;
    }
    min_delay = min_delay.min(7);
    if crossbow && min_delay < 10 {
        min_delay = 10;
    }
    min_delay = min_delay.max(speed - (MAX_SKILL_LEVEL + 1) / 2);
    min_delay.max(3)
}

/// One shot or throw by the player at a monster, rolled as `ranged_attack::attack`
/// (`ranged-attack.cc:90`) rolls it: accuracy, the monster's evasion, then for a hit, damage and
/// armour, and whether a thrown missile breaks as it lands (`_thrown_object_destroyed`,
/// `throw.cc:885`), which crawl decides before it flies.
pub(crate) fn shoot(
    variant: PlayerAttack,
    rng: &mut Rng,
    striker: Striker,
    shot: Shot<'_>,
    defence: Defence,
) -> Blow {
    match variant {
        PlayerAttack::Crawl034 => {
            let thrown = shot.thrown();
            // A stone always breaks; every other thrown missile one time in its mulch rate.
            let broke = thrown
                && (shot.id.0 == missile_types::STONE
                    || shot
                        .weapon
                        .mulch
                        .is_some_and(|mulch| rng.one_chance_in(mulch)));
            // A launcher's enchantment and accuracy count; a thrown missile's don't.
            let bonus = if thrown {
                0
            } else {
                shot.plus + shot.weapon.accuracy
            };
            let to_hit = pre_rolled_to_hit(rng, striker, shot.skill, bonus);
            if !hits(rng, to_hit, defence.ev) {
                return Blow {
                    broke,
                    ..Blow::missed(to_hit)
                };
            }
            // `ranged_attack::weapon_damage` (`ranged-attack.cc:353`), with what Throwing adds
            // to a thrown missile (`throwing_base_damage_bonus`, `fight.cc:1735`).
            let mut potential = shot.weapon.damage;
            if thrown {
                let mult = shot.weapon.damage.min(4);
                let throwing = shot.skill.rounded(rng);
                potential += rng.div_rand_round(throwing * mult, 4);
            }
            let potential = hefted_damage(rng, potential, shot.heft);
            // Throwing is trained by Strength (`weapon_uses_strength`), launchers by Dexterity.
            let damage = rolled(rng, potential, thrown, striker);
            let damage = with_weapon_skill(rng, damage, shot.skill);
            let damage = with_fighting(rng, damage, striker);
            let damage = with_slaying(rng, damage, if thrown { 0 } else { shot.plus });
            let mut damage = monster_armour(rng, ArmourRule::Normal, defence.ac, damage);
            // A thrown missile that breaks hits a third harder, but a stone
            // (`ranged_attack::mulch_bonus`, `ranged-attack.cc:404`).
            if broke && shot.id.0 != missile_types::STONE && shot.weapon.damage > 0 {
                damage = rng.div_rand_round(damage * 4, 3);
            }
            Blow {
                to_hit,
                hits: true,
                damage,
                broke,
            }
        }
    }
}

/// Whether a thrown missile of `id` comes back to the player unless it breaks: `_returning`
/// (`throw.cc:432`).
pub(crate) fn returns(variant: PlayerAttack, id: &EnumId) -> bool {
    match variant {
        PlayerAttack::Crawl034 => id.0 == missile_types::BOOMERANG,
    }
}

/// A beam the player fires, set up as `zappy` (`beam.cc:470`) sets one up for the player.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Beam {
    /// Its accuracy, `None` when it never misses.
    to_hit: Option<i32>,
    damage: ZapDamage,
    power: i32,
    explodes: bool,
    armour: ArmourRule,
    resist: MonsterBeamResist,
}

/// The beam of `zap` as the player fires it at `power`.
pub(crate) fn beam(variant: PlayerBeam, zap: PlayerZap<'_>, power: i32) -> Beam {
    match variant {
        PlayerBeam::Crawl034 => Beam {
            // `zap_to_hit` (`beam.cc:385`), with no inaccuracy from the player's gear.
            to_hit: zap.accuracy(power),
            damage: zap.damage,
            power,
            explodes: zap.zap.explodes,
            armour: ArmourRule::of(&zap.zap.flavour),
            resist: resists::monster_beam(variant, &zap.zap.flavour)
                .unwrap_or(MonsterBeamResist::Unresisted),
        },
    }
}

/// A beam the player fires at a monster, rolled as `bolt::affect_monster` (`beam.cc:5520`) rolls
/// it: its dice, armour and the monster's resistances (`bolt::determine_damage`, `beam.cc:4659`),
/// then, unless it explodes and so engulfs the monster, the roll against its evasion
/// (`_test_beam_hit`, `beam.cc:3228`).
pub(crate) fn fire(variant: PlayerBeam, rng: &mut Rng, beam: Beam, defence: Defence) -> Blow {
    match variant {
        PlayerBeam::Crawl034 => {
            let (dice, sides) = spells::dice(rng, beam.damage, beam.power);
            let rolled = rng.roll_dice(dice, sides);
            let after_armour = monster_armour(rng, beam.armour, defence.ac, rolled);
            let damage =
                resists::monster_beam_damage(variant, beam.resist, defence.resists, after_armour);
            if !beam.explodes
                && let Some(to_hit) = beam.to_hit
            {
                let ev = rng.random2(defence.ev);
                let attack = rng.random2(to_hit);
                let dodge = rng.random2avg(ev, 2);
                if attack - dodge < 0 {
                    return Blow::missed(to_hit);
                }
            }
            Blow {
                to_hit: beam.to_hit.unwrap_or(AUTOMATIC_HIT),
                hits: true,
                damage,
                broke: false,
            }
        }
    }
}

/// The power the player zaps a wand of a spell whose power cap is `cap` with, trained as
/// `evocations`: `wand_power` (`evoke.cc:153`), drawing no magic for it.
pub(crate) fn wand_power(variant: WandPower, evocations: DrawnSkill, cap: i32) -> i32 {
    match variant {
        WandPower::Crawl034 => (15 + evocations.scaled(7) / 2).min(cap),
    }
}

/// A monster's hit points as it is made, its kind's average ten times over being `hp_10x`:
/// `hit_points` (`mon-util.cc:2251`).
pub(crate) fn monster_hit_points(variant: MonsterHitPoints, rng: &mut Rng, hp_10x: u32) -> i32 {
    match variant {
        MonsterHitPoints::Crawl034 => {
            let average = i32::try_from(hp_10x).unwrap_or(i32::MAX);
            if average == 0 {
                return 0;
            }
            let variance = rng.div_rand_round(average * 33, 100);
            let least = average - variance;
            let hp = least + rng.random2avg(variance * 2, 8);
            rng.div_rand_round(hp, 10).max(1)
        }
    }
}

#[cfg(test)]
mod tests {
    use copilot_gk::Library;

    use super::*;

    #[test]
    fn every_identifier_the_player_attack_variants_name_is_in_every_shipped_bundle() {
        let library = Library::shipped().unwrap();
        for bundle in library.bundles() {
            let (crossbows, dexterous, short_blades) = match bundle.formulas.player_attack {
                PlayerAttack::Crawl034 => {
                    (CROSSBOWS_0_34, DEXTEROUS_SKILLS_0_34, SHORT_BLADES_0_34)
                }
            };
            for &id in crossbows {
                let weapon = bundle.missile_weapons.get(&EnumId(id.to_owned()));
                assert!(
                    weapon.is_some(),
                    "{} has no launcher {id}",
                    bundle.crawl.tag
                );
            }
            for &id in dexterous.iter().chain([&short_blades]) {
                let skill = bundle.skills.get(&EnumId(id.to_owned()));
                assert!(skill.is_some(), "{} has no skill {id}", bundle.crawl.tag);
            }
        }
    }

    #[test]
    fn every_player_zap_names_a_spell_the_player_fires_in_every_shipped_bundle_of_its_variant() {
        let library = Library::shipped().unwrap();
        for bundle in library.bundles() {
            let zaps = match bundle.formulas.player_beam {
                PlayerBeam::Crawl034 => PLAYER_ZAPS_0_34,
            };
            for &id in zaps {
                let spell = bundle
                    .spell_by_id(&EnumId(id.to_owned()))
                    .unwrap_or_else(|| panic!("{} has no spell {id}", bundle.crawl.tag));
                assert!(
                    PlayerZap::of(bundle.formulas.player_beam, spell).is_some(),
                    "{} doesn't fire {id} as a zap the player's beam port plays",
                    bundle.crawl.tag
                );
            }
        }
    }
}
