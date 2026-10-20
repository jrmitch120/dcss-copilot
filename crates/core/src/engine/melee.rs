//! Crawl's melee formulas for a monster attacking the player, ported closely (ADR 0002) behind
//! the formula variants a Game Knowledge bundle names.
//!
//! Each function cites the crawl source it ports. Line numbers are crawl 0.34.1's
//! (`crawl-ref/source/` at tag 0.34.1); trunk at 28a137de has the same code for all of them except
//! reach, where an attack carries its own reach (`mon_attack_def::reach`), which a later variant
//! will read. Only the paths a monster's plain melee attack on the player takes are ported: the
//! attacker has no statuses, holds a melee weapon or nothing in each hand, and the player and it
//! see each other. The Fact Sheet lists what that leaves out.

use copilot_gk::{
    Armour, EvasionRoll, Formulas, MeleeWeapon, MonsterAttack, MonsterDamage, MonsterEnergy,
    MonsterReach, MonsterToHit, MonsterWeapon, ShieldBlock,
};

use super::brands::{self, Brand, Necromancy};
use super::lighting::Lighting;
use super::resists::Resists;
use super::rng::Rng;
use crate::accumulated_state::Shield;
use crate::crawl_ids::{attack_flavours, attack_types};

/// One of a monster's melee attacks, as the formulas below tell attacks apart: classified once
/// from crawl's `attack_type` and `attack_flavour` identifiers in its Game Knowledge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Attack {
    /// Its type, as far as the formulas care.
    pub(crate) kind: Kind,
    /// What the attack does besides its damage, as far as the formulas care.
    pub(crate) flavour: Flavour,
    /// The most damage it deals before armour.
    pub(crate) damage: i32,
}

/// The attack types the ported formulas treat differently: those a monster may make with a
/// weapon (`monster::weapon`, `monster.cc:494`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    /// `AT_HIT`: made with the monster's weapon, if it has one.
    Hit,
    /// `AT_WEAP_ONLY`: made only with a melee weapon in the monster's main hand, as a hit.
    WeaponOnly,
    /// Any other type, such as a bite: never made with a weapon.
    Other,
}

/// The attack flavours the ported formulas treat differently. Every other flavour only adds an
/// effect the Threat Level doesn't model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Flavour {
    /// `AF_CRUSH`: it grabs, and deals no damage itself.
    Crush,
    /// `AF_REACH`, `AF_REACH_STING`, `AF_REACH_TONGUE` and `AF_REACH_CLEAVE_UGLY`: crawl's
    /// `flavour_has_reach` (`mon-util.cc:2209`).
    Reaching,
    /// `AF_RIFT`: it reaches through a rift.
    Rift,
    /// Any other flavour.
    Other,
}

impl Attack {
    /// The attack `attack` describes, or `None` for the empty attack slot `AT_NONE`.
    pub(crate) fn of(attack: &MonsterAttack) -> Option<Self> {
        if attack.kind.0 == attack_types::NONE {
            return None;
        }
        let kind = match attack.kind.0.as_str() {
            attack_types::HIT => Kind::Hit,
            attack_types::WEAP_ONLY => Kind::WeaponOnly,
            _ => Kind::Other,
        };
        let flavour = match attack.flavour.0.as_str() {
            attack_flavours::CRUSH => Flavour::Crush,
            attack_flavours::REACH
            | attack_flavours::REACH_STING
            | attack_flavours::REACH_TONGUE
            | attack_flavours::REACH_CLEAVE_UGLY => Flavour::Reaching,
            attack_flavours::RIFT => Flavour::Rift,
            _ => Flavour::Other,
        };
        Some(Self {
            kind,
            flavour,
            damage: i32::try_from(attack.damage).unwrap_or(i32::MAX),
        })
    }
}

/// `AUTOMATIC_HIT` (`defines.h:141`): a to-hit or a beam's accuracy this high never misses.
pub const AUTOMATIC_HIT: i32 = 1500;

/// `MIN_HIT_MISS_PERCENTAGE` (`defines.h:147`): the chance in 100 that an attack ignores
/// evasion, hitting or missing outright with equal odds.
const MIN_HIT_MISS_PERCENTAGE: i32 = 5;

/// `ENERGY_THRESHOLD` (`monster.cc:5353`): a monster acts while it has this much energy.
pub(crate) const ENERGY_THRESHOLD: i32 = 80;

/// `BASELINE_DELAY` (`defines.h:149`): the time a standard-speed player action takes.
pub(crate) const BASELINE_DELAY: i32 = 10;

/// The player as a monster's attack meets them: their defences, how the light falls on them, and
/// what they resist.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Target {
    /// Their AC, EV and SH.
    pub(crate) defences: Defences,
    /// How the light falls on them.
    pub(crate) lighting: Lighting,
    /// Their resistances, holiness and god.
    pub(crate) resists: Resists,
}

/// A monster aiming an attack at the player, as crawl's to-hit takes it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Aim {
    /// Its hit dice.
    pub(crate) hit_dice: i32,
    /// It is skilled with the attack: a fighter in melee (`M_FIGHTER`) or an archer with
    /// missiles (`M_ARCHER`).
    pub(crate) skilled: bool,
    /// What its weapon adds to its accuracy ([`Weapon::to_hit`]), 0 for none.
    pub(crate) accuracy: i32,
    /// It sees through an umbra (`monster::nightvision`, `monster.cc:6545`).
    pub(crate) nightvision: bool,
}

/// A monster's accuracy for one attack on a player `lighting` falls on: the `to_hit` that
/// `attack::calc_to_hit` (`attack.cc:288`) rolls for a monster.
///
/// The pre-roll value is `mon_to_hit_base` (`fight.cc:232`), via
/// `melee_attack::calc_mon_to_hit_base` (`melee-attack.cc:5087`) or
/// `ranged_attack::calc_mon_to_hit_base` (`ranged-attack.cc:363`), plus the weapon's in the
/// monster branch of `attack::calc_pre_roll_to_hit` (`attack.cc:226`), with no slaying. A
/// monster adds the "post-roll" modifiers (`attack.cc:244`) before it rolls: for a visible,
/// unconfused monster without inaccuracy only the light on the player, so its roll is
/// `random2(mhit + modifiers + 1)` (`attack.cc:300`).
pub(crate) fn monster_to_hit(
    variant: MonsterToHit,
    rng: &mut Rng,
    aim: Aim,
    lighting: Lighting,
) -> i32 {
    match variant {
        MonsterToHit::Crawl034 => {
            let hd_mult = if aim.skilled { 5 } else { 3 };
            let mhit = 18 + aim.hit_dice * hd_mult / 2 + aim.accuracy;
            let mhit = mhit + lighting.to_hit_modifier(aim.nightvision);
            rng.random2(mhit + 1)
        }
    }
}

/// Whether an attack with this `to_land` gets past the player's evasion: the margin
/// `attack::test_hit` (`attack.cc:984`) returns, which hits when it is at least 0, against the
/// evasion `melee_attack::attack` reads (`melee-attack.cc:1897`). A monster's attack randomises
/// the player's evasion (`melee-attack.cc:1898` passes `randomise_ev`).
pub(crate) fn evasion_margin(
    variant: EvasionRoll,
    rng: &mut Rng,
    to_land: i32,
    defences: Defences,
) -> i32 {
    match variant {
        EvasionRoll::Crawl034 => {
            let ev = defences.evasion(rng);
            let ev = rng.random2avg(2 * ev, 2);
            if to_land >= AUTOMATIC_HIT {
                return AUTOMATIC_HIT;
            }
            if rng.x_chance_in_y(MIN_HIT_MISS_PERCENTAGE, 100) {
                if rng.random2(2) == 1 {
                    AUTOMATIC_HIT
                } else {
                    -AUTOMATIC_HIT
                }
            } else {
                to_land - ev
            }
        }
    }
}

/// Whether the player's shield blocks an attack: `attack::attack_shield_blocked`
/// (`attack.cc:1040`) for a monster attacking a player who can see it and isn't incapacitated.
///
/// The monster's bypass is `random2(mon_shield_bypass(hd))` (`fight.cc:237`, via
/// `monster::shield_bypass_ability`, `monster.cc:3096`); the player's block is
/// `player::shield_bonus` (`player.cc:6106`).
pub(crate) fn shield_blocks(
    variant: ShieldBlock,
    rng: &mut Rng,
    to_hit: i32,
    hit_dice: i32,
    defences: Defences,
) -> bool {
    match variant {
        ShieldBlock::Crawl034 => {
            if to_hit >= AUTOMATIC_HIT {
                return false;
            }
            let con_block = rng.random2(15 + hit_dice * 2 / 3);
            shield_bonus(rng, defences) >= con_block
        }
    }
}

/// `player::shield_bonus` (`player.cc:6106`): how well the player's shield blocks one attack or
/// beam.
pub(crate) fn shield_bonus(rng: &mut Rng, defences: Defences) -> i32 {
    let shield_class = defences.shield_class(rng);
    if shield_class <= 0 {
        -100
    } else {
        rng.random2avg(shield_class * 2, 2) / 3 - 1
    }
}

/// How many attacks the player's shield can block before their next action:
/// `actor::shield_block_limit` (`actor.cc:389`), one without a shield, else the shield's
/// `shield_block_limit` (`item-prop.cc:3135`).
pub(crate) fn shield_block_limit(variant: ShieldBlock, shield: Option<Shield>) -> u32 {
    match variant {
        ShieldBlock::Crawl034 => match shield {
            None => 1,
            Some(Shield::Buckler) => 2,
            Some(Shield::KiteShield) => 3,
            Some(Shield::TowerShield) => 4,
        },
    }
}

/// A monster attack's damage before armour, and the most it could have dealt, which sets the
/// player's guaranteed damage reduction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RolledDamage {
    /// The damage rolled.
    pub(crate) damage: i32,
    /// `damage_max`: the most this attack can deal.
    pub(crate) max: i32,
}

/// A weapon as crawl's `attack` rolls with it: a melee weapon in a monster's hand, or a launcher
/// or throwing missile it fires.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Weapon {
    /// Its damage: `PWPN_DAMAGE`, or a throwing missile's own.
    pub(crate) damage: i32,
    /// The accuracy it adds: `PWPN_HIT`, and none for a throwing missile.
    pub(crate) accuracy: i32,
    /// Its enchantment, which adds to its accuracy and its damage.
    pub(crate) plus: i32,
    /// Its brand.
    pub(crate) brand: Brand,
}

impl Weapon {
    /// What it adds to its wielder's to-hit: its accuracy and its enchantment, as the monster
    /// branch of `attack::calc_pre_roll_to_hit` (`attack.cc:231`) adds them.
    pub(crate) fn to_hit(self) -> i32 {
        self.accuracy + self.plus
    }
}

/// A melee weapon in a monster's hand.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Wielded {
    /// It, as an attack rolls with it.
    pub(crate) weapon: Weapon,
    /// How many squares away it strikes from: `weapon_reach` (`item-prop.cc:2323`).
    pub(crate) reach: i32,
}

impl Wielded {
    /// The melee weapon `known`, with enchantment `plus` and `brand`.
    pub(crate) fn of(known: &MeleeWeapon, plus: i32, brand: Brand) -> Self {
        Self {
            weapon: Weapon {
                damage: known.damage,
                accuracy: known.accuracy,
                plus,
                brand,
            },
            reach: known.reach,
        }
    }
}

/// The melee weapons a monster holds as it fights, once it has wielded the one it fights with
/// (`monster::wield_melee_weapon`, `monster.cc:1083`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct Hands {
    /// In its main hand (`MSLOT_WEAPON`).
    pub(crate) main: Option<Wielded>,
    /// In its off hand (`MSLOT_ALT_WEAPON`), which only a monster that wields two weapons
    /// (`mons_wields_two_weapons`) fights with: none for any other.
    pub(crate) off: Option<Wielded>,
}

/// A monster fighting the player in melee, as the formulas take it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Fighter {
    /// Its hit dice.
    pub(crate) hit_dice: i32,
    /// It has crawl's `fighter` flag, which makes its attacks more accurate.
    pub(crate) skilled: bool,
    /// It sees through an umbra.
    pub(crate) nightvision: bool,
    /// Its necromancy skill, which a weapon of pain rolls against.
    pub(crate) necromancy: Necromancy,
    /// It is a draugr, which fights with its weapon only in its first attack.
    pub(crate) draugr: bool,
    /// The melee weapons it holds, when they are known; when they aren't, it fights as if it
    /// wields nothing, but makes the attacks only a weapon makes.
    pub(crate) hands: Option<Hands>,
}

impl Fighter {
    /// The weapon it makes `attack`, its attack numbered `slot` from 0, with:
    /// `monster::weapon` (`monster.cc:494`). Only a hit or an attack only a weapon makes uses
    /// one, a draugr's only in its first attack. A monster that wields two weapons makes its odd
    /// attacks with its off hand's, or its main hand's if its off hand is empty, and its even
    /// ones with its main hand's.
    pub(crate) fn weapon(
        &self,
        variant: MonsterWeapon,
        slot: usize,
        attack: Attack,
    ) -> Option<Wielded> {
        match variant {
            MonsterWeapon::Crawl034 => {
                let hands = self.hands?;
                if attack.kind == Kind::Other || self.draugr && slot != 0 {
                    return None;
                }
                if slot % 2 == 1 {
                    hands.off.or(hands.main)
                } else {
                    hands.main
                }
            }
        }
    }

    /// Whether it makes `attack` at all: an attack only a weapon makes is made as a hit with a
    /// melee weapon in its main hand, and not at all without one (`attack::init_attack`,
    /// `attack.cc:413`). When its weapons aren't known, it is made.
    pub(crate) fn makes(&self, variant: MonsterWeapon, attack: Attack) -> bool {
        match variant {
            MonsterWeapon::Crawl034 => {
                attack.kind != Kind::WeaponOnly
                    || self.hands.is_none_or(|hands| hands.main.is_some())
            }
        }
    }

    /// The delay of one of its melee actions, whose first attack is `first`, if it has one:
    /// `monster::attack_delay` (`monster.cc:439`), 10 as the brand of the weapon
    /// `monster::weapon()` gives changes it (`weapon_adjust_delay`, `fight.cc:1294`). That is
    /// the weapon of its first attack, but a monster that wields two weapons takes its off hand's
    /// one time in two, or always with its main hand empty, and a draugr none.
    pub(crate) fn attack_delay(
        &self,
        formulas: Formulas,
        rng: &mut Rng,
        first: Option<Attack>,
    ) -> i32 {
        const BASE: i32 = 10;
        match formulas.monster_weapon {
            MonsterWeapon::Crawl034 => {
                let Some(hands) = self.hands else {
                    return BASE;
                };
                // `monster::weapon(-1)` reads the first attack's type, and is never the first.
                if first.is_none_or(|attack| attack.kind == Kind::Other) || self.draugr {
                    return BASE;
                }
                let weapon = match (hands.main, hands.off) {
                    (None, off) => off,
                    (Some(main), Some(off)) => Some(if rng.random2(2) == 1 { off } else { main }),
                    (main, None) => main,
                };
                weapon.map_or(BASE, |wielded| {
                    brands::delay(formulas.damage_brand, rng, wielded.weapon.brand, BASE)
                })
            }
        }
    }
}

/// The damage of a monster attack with `attack_damage` of its own, made with `weapon` if any,
/// before armour: the monster branch of `attack::calc_damage` (`attack.cc:918`) before its
/// modifiers. A weapon adds `random2` of its damage as its brand adjusts it
/// (`attack::adjusted_weapon_damage`), and its enchantment as `_core_apply_slaying`
/// (`attack.cc:829`) adds it, with no slaying; the attack adds `1 + random2(attk_damage)`.
pub(crate) fn monster_damage(
    formulas: Formulas,
    rng: &mut Rng,
    weapon: Option<Weapon>,
    attack_damage: i32,
) -> RolledDamage {
    match formulas.monster_damage {
        MonsterDamage::Crawl034 => {
            let (mut damage, mut max) = (0, 0);
            if let Some(weapon) = weapon {
                max =
                    brands::weapon_damage(formulas.damage_brand, rng, weapon.brand, weapon.damage);
                damage = rng.random2(max);
                // A +0 weapon adds `random2(1)`, nothing.
                damage = if weapon.plus >= 0 {
                    damage + rng.random2(1 + weapon.plus)
                } else {
                    damage - rng.random2(1 - weapon.plus)
                };
            }
            RolledDamage {
                damage: damage + 1 + rng.random2(attack_damage),
                max: max + attack_damage,
            }
        }
    }
}

/// The damage of one monster melee attack that hit, before armour: `melee_attack::calc_damage`
/// (`melee-attack.cc:5156`), where a crushing attack only grabs, and deals nothing itself, and
/// any other deals [`monster_damage`].
///
/// `melee_attack::apply_damage_modifiers` (`melee-attack.cc:5098`) changes nothing for a monster
/// without statuses attacking an awake player.
pub(crate) fn melee_damage(
    formulas: Formulas,
    rng: &mut Rng,
    attack: Attack,
    weapon: Option<Weapon>,
) -> RolledDamage {
    if attack.flavour == Flavour::Crush {
        return RolledDamage { damage: 0, max: 0 };
    }
    monster_damage(formulas, rng, weapon, attack.damage)
}

/// What the player takes from `damage` after armour: `actor::apply_ac` (`actor.cc:343`) under the
/// normal rule, with guaranteed damage reduction from `player::gdr_perc` (`player.cc:6620`),
/// which rounds the player's AC again for itself.
pub(crate) fn apply_ac(
    variant: Armour,
    rng: &mut Rng,
    damage: RolledDamage,
    defences: Defences,
) -> i32 {
    match variant {
        Armour::Crawl034 => {
            let ac = defences.armour_class(rng).max(0);
            let gdr = gdr_percent(defences.armour_class(rng));
            let saved = rng.random2(1 + ac);
            let saved = saved.max((gdr * damage.max / 100).min(rng.div_rand_round(ac, 2)));
            (damage.damage - saved).max(0)
        }
    }
}

/// `player::gdr_perc` (`player.cc:6620`): `16 * AC^(1/4)`, truncated, as a percentage.
fn gdr_percent(ac: i32) -> i32 {
    // Truncation is crawl's `(int)` cast.
    #[allow(clippy::cast_possible_truncation)]
    let gdr = (16.0 * f64::from(ac.max(0)).sqrt().sqrt()) as i32;
    gdr
}

/// The player's AC, EV and SH as the game displays them: whole points, with the fraction crawl
/// keeps dropped (`armour_class_scaled(1)`, `evasion_scaled(1)` and
/// `player_displayed_shield_class()`, all truncated).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DisplayedDefences {
    /// Armour class.
    pub(crate) ac: i32,
    /// Evasion.
    pub(crate) ev: i32,
    /// Shield class.
    pub(crate) sh: i32,
}

impl DisplayedDefences {
    /// Whether any of them hides a fraction that [`Defences::drawn_behind`] guesses.
    pub(crate) fn any_hides_a_fraction(self) -> bool {
        [self.ac, self.ev, self.sh]
            .into_iter()
            .any(hides_a_fraction)
    }
}

/// Whether a defence displayed as `displayed` hides a fraction: one shown above 0. SH 0 is almost
/// always no shield at all, which must never block, and below zero the display truncates towards
/// zero.
fn hides_a_fraction(displayed: i32) -> bool {
    displayed > 0
}

/// The player's AC, EV and SH in hundredths of a point, as crawl keeps them. Crawl rounds each
/// at random every time it rolls with it, so the formulas ask for them through these methods.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Defences {
    /// `player::armour_class_scaled(100)`.
    ac: i32,
    /// `player::evasion_scaled(100)`.
    ev: i32,
    /// `player_shield_class(100)`: twice the SH the game displays.
    sh: i32,
}

impl Defences {
    /// Defences of exactly these whole points of AC, EV and SH, as the game displays them.
    pub(crate) fn exact(ac: i32, ev: i32, sh: i32) -> Self {
        Self {
            ac: ac * 100,
            ev: ev * 100,
            sh: sh * 200,
        }
    }

    /// Defences that would show as `displayed`, with the fraction the display drops drawn as if
    /// any were as likely: 0 to 99 hundredths of AC and EV, and 0 to 199 of the shield class,
    /// which the display truncates and then halves. That assumption is an approximation: a real
    /// character's fraction comes from their skills, Dexterity and gear.
    ///
    /// A displayed value that hides no fraction ([`hides_a_fraction`]) is taken as exact.
    pub(crate) fn drawn_behind(displayed: DisplayedDefences, rng: &mut Rng) -> Self {
        let mut defences = Self::exact(displayed.ac, displayed.ev, displayed.sh);
        if hides_a_fraction(displayed.ac) {
            defences.ac += rng.random2(100);
        }
        if hides_a_fraction(displayed.ev) {
            defences.ev += rng.random2(100);
        }
        if hides_a_fraction(displayed.sh) {
            defences.sh += rng.random2(200);
        }
        defences
    }

    /// `player::armour_class` (`player.cc:6533`): AC, rounded at random.
    pub(crate) fn armour_class(self, rng: &mut Rng) -> i32 {
        rng.div_rand_round(self.ac, 100)
    }

    /// `player::evasion` (`player.cc:6635`): EV, rounded at random.
    pub(crate) fn evasion(self, rng: &mut Rng) -> i32 {
        rng.div_rand_round(self.ev, 100)
    }

    /// `player_shield_class()` (`player.cc:2281`): twice the displayed SH, rounded at random.
    pub(crate) fn shield_class(self, rng: &mut Rng) -> i32 {
        rng.div_rand_round(self.sh, 100)
    }
}

/// How one monster attack on the player went.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Swing {
    /// The attack's rolled accuracy.
    pub(crate) to_hit: i32,
    /// Whether it got past the player's evasion.
    pub(crate) hits: bool,
    /// Whether the player's shield blocked it, which it does whether or not it would have hit.
    pub(crate) blocked: bool,
    /// What the player took: 0 unless it landed.
    pub(crate) damage: i32,
}

impl Swing {
    /// Whether the attack landed: it hit and the shield didn't block it.
    pub(crate) fn landed(self) -> bool {
        self.hits && !self.blocked
    }
}

/// One monster attack on the player, made with `weapon` if any, rolled in the order
/// `melee_attack::attack` (`melee-attack.cc:1898`) rolls it: accuracy, evasion, the shield, then
/// for an attack that lands, damage, armour, and what the weapon's brand adds
/// (`melee_attack::handle_phase_hit`, `melee-attack.cc:927`). `can_block` is whether the shield
/// has a block left before the player's next action (`actor::shield_exhausted`, `actor.cc:397`).
pub(crate) fn swing(
    formulas: Formulas,
    rng: &mut Rng,
    fighter: &Fighter,
    attack: Attack,
    weapon: Option<Weapon>,
    target: Target,
    can_block: bool,
) -> Swing {
    let Target {
        defences, lighting, ..
    } = target;
    let aim = Aim {
        hit_dice: fighter.hit_dice,
        skilled: fighter.skilled,
        accuracy: weapon.map_or(0, Weapon::to_hit),
        nightvision: fighter.nightvision,
    };
    let to_hit = monster_to_hit(formulas.monster_to_hit, rng, aim, lighting);
    let hits = evasion_margin(formulas.evasion_roll, rng, to_hit, defences) >= 0;
    let blocked =
        shield_blocks(formulas.shield_block, rng, to_hit, aim.hit_dice, defences) && can_block;
    let mut swing = Swing {
        to_hit,
        hits,
        blocked,
        damage: 0,
    };
    if swing.landed() {
        let rolled = melee_damage(formulas, rng, attack, weapon);
        let dealt = apply_ac(formulas.armour, rng, rolled, defences);
        // Without a weapon, the brand is `SPWPN_NORMAL` (`monster::damage_brand`).
        let brand = weapon.map_or(Brand::Plain, |weapon| weapon.brand);
        swing.damage = dealt
            + brands::damage(
                formulas,
                rng,
                brand,
                fighter.necromancy,
                dealt,
                target.resists,
            );
    }
    swing
}

/// How many squares away this attack lets a monster strike from by its flavour: two when its
/// flavour reaches (`flavour_has_reach`, `mon-util.cc:2209`) and three for a rift, as
/// `monster::reach_range` (`monster.cc:6224`) counts the attacks that deal damage.
pub(crate) fn attack_reach(variant: MonsterReach, attack: Attack) -> i32 {
    match variant {
        MonsterReach::Crawl034 => match attack.flavour {
            Flavour::Rift => 3,
            Flavour::Reaching => 2,
            Flavour::Crush | Flavour::Other => 1,
        },
    }
}

/// How far a monster whose attacks reach `by_flavour` squares by their flavours reaches with
/// `primary`, the weapon of its first attack, if any: `monster::reach_range` (`monster.cc:6224`)
/// takes the farther.
pub(crate) fn reach_with(variant: MonsterReach, by_flavour: i32, primary: Option<Wielded>) -> i32 {
    match variant {
        MonsterReach::Crawl034 => primary.map_or(by_flavour, |weapon| by_flavour.max(weapon.reach)),
    }
}

/// Whether a monster striking from `distance` squares, within its reach, makes this attack with
/// `weapon`: `melee_attack::can_reach` (`melee-attack.cc:97`). An attack is made from beside the
/// player; a hit, which an attack only a weapon makes is by then, from as far as its weapon
/// reaches; and an attack whose flavour reaches from anywhere the monster reaches, so a
/// starflower's `AF_REACH` lash follows its rift three squares away.
pub(crate) fn reaches_from(
    variant: MonsterReach,
    attack: Attack,
    weapon: Option<Wielded>,
    distance: i32,
) -> bool {
    match variant {
        MonsterReach::Crawl034 => {
            distance <= 1
                || matches!(attack.flavour, Flavour::Reaching | Flavour::Rift)
                || attack.kind != Kind::Other
                    && weapon.is_some_and(|weapon| weapon.reach >= distance)
        }
    }
}

/// The energy one melee action whose actions cost `cost` takes `fighter`, whose first attack is
/// `first`: its cost times the action's delay over 10 (`fight_melee`, `fight.cc:562`), with the
/// delay [`Fighter::attack_delay`].
pub(crate) fn melee_energy(
    formulas: Formulas,
    rng: &mut Rng,
    fighter: &Fighter,
    first: Option<Attack>,
    cost: i32,
) -> i32 {
    match formulas.monster_energy {
        MonsterEnergy::Crawl034 => {
            let delay = fighter.attack_delay(formulas, rng, first);
            rng.div_rand_round(cost * delay, 10)
        }
    }
}

/// A monster's energy as the player starts an action: just below the threshold, as crawl leaves
/// a monster that has used its action (`monster::drain_action_energy`, `monster.cc:5361`).
pub(crate) fn starting_energy(variant: MonsterEnergy, rng: &mut Rng) -> i32 {
    match variant {
        MonsterEnergy::Crawl034 => ENERGY_THRESHOLD - rng.roll_dice(1, 10),
    }
}

/// The energy a monster of this speed gains while the player takes an action lasting `time`, in
/// crawl's units where a standard-speed action takes [`BASELINE_DELAY`]: `_monster_add_energy`
/// (`mon-act.cc:1653`) with `you.time_taken` at `time`.
pub(crate) fn energy_gained(variant: MonsterEnergy, rng: &mut Rng, speed: i32, time: i32) -> i32 {
    match variant {
        MonsterEnergy::Crawl034 => {
            if speed > 0 {
                rng.div_rand_round(speed * time, 10)
            } else {
                0
            }
        }
    }
}

/// The most energy a monster can have as the player starts an action.
pub(crate) fn most_starting_energy(variant: MonsterEnergy) -> i32 {
    match variant {
        MonsterEnergy::Crawl034 => ENERGY_THRESHOLD - 1,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use copilot_gk::EnumId;

    use super::*;

    const TRIALS: u32 = 200_000;

    fn rng() -> Rng {
        Rng::new(7, 11)
    }

    /// The share of `TRIALS` runs of `hits` that came out true.
    fn chance(mut hits: impl FnMut(&mut Rng) -> bool) -> f64 {
        let mut rng = rng();
        let count = (0..TRIALS).filter(|_| hits(&mut rng)).count();
        f64::from(u32::try_from(count).unwrap()) / f64::from(TRIALS)
    }

    fn attack(flavour: &str, damage: u32) -> Attack {
        Attack::of(&MonsterAttack {
            kind: EnumId("AT_HIT".to_owned()),
            flavour: EnumId(flavour.to_owned()),
            damage,
        })
        .unwrap()
    }

    /// `mon_to_hit_pct(to_land, scaled_ev)` (`fight.cc:251`): crawl's own estimate of a monster's
    /// chance to hit the player, which its monster descriptions show. Ported here as an
    /// independent check on the rolls: it sums the evasion roll as two uniform halves, where the
    /// real roll is `random2avg`, so the two agree only to within a point or two.
    fn crawl_mon_to_hit_pct(to_land: i32, ev: i32) -> f64 {
        if ev <= 0 {
            return f64::from(100 - MIN_HIT_MISS_PERCENTAGE / 2);
        }
        let to_land = to_land + 1;
        let mut hits = 0;
        for ev1 in 0..ev {
            for ev2 in 0..ev {
                hits += (to_land - (ev1 + ev2)).max(0);
            }
        }
        let hit_chance = f64::from(hits) / f64::from(to_land * ev * ev);
        let auto = f64::from(MIN_HIT_MISS_PERCENTAGE) / 200.0;
        (hit_chance * (1.0 - auto) + (1.0 - hit_chance) * auto) * 100.0
    }

    #[test]
    fn a_monsters_chance_to_hit_agrees_with_crawls_own_estimate() {
        for (hd, fighter, ev) in [
            (1, false, 10),
            (5, true, 10),
            (10, false, 25),
            (3, false, 0),
        ] {
            let pre_roll = 18 + hd * if fighter { 5 } else { 3 } / 2;
            let ported = chance(|rng| {
                let aim = Aim {
                    hit_dice: hd,
                    skilled: fighter,
                    accuracy: 0,
                    nightvision: false,
                };
                let to_hit = monster_to_hit(MonsterToHit::Crawl034, rng, aim, Lighting::Plain);
                evasion_margin(
                    EvasionRoll::Crawl034,
                    rng,
                    to_hit,
                    Defences::exact(0, ev, 0),
                ) >= 0
            }) * 100.0;
            let crawl = crawl_mon_to_hit_pct(pre_roll, ev);
            assert!(
                (ported - crawl).abs() < 2.5,
                "HD {hd}, EV {ev}: ported {ported:.1}% vs crawl's estimate {crawl:.1}%"
            );
        }
    }

    /// A hit-dice-1 monster's aim, which rolls `random2(19 + 1)` on a player in plain light.
    fn aim(nightvision: bool) -> Aim {
        Aim {
            hit_dice: 1,
            skilled: false,
            accuracy: 0,
            nightvision,
        }
    }

    /// The highest to-hit `aim` rolls on a player `lighting` falls on, over many rolls.
    fn best_to_hit(aim: Aim, lighting: Lighting) -> i32 {
        let mut rng = Rng::new(7, 8);
        (0..10_000)
            .map(|_| monster_to_hit(MonsterToHit::Crawl034, &mut rng, aim, lighting))
            .max()
            .unwrap()
    }

    #[test]
    fn a_backlit_player_is_aimed_at_with_five_more_before_the_roll() {
        assert_eq!(best_to_hit(aim(false), Lighting::Plain), 19);
        assert_eq!(best_to_hit(aim(false), Lighting::Backlit), 24);
        assert_eq!(best_to_hit(aim(true), Lighting::Backlit), 24);
    }

    #[test]
    fn an_umbra_takes_three_from_the_aim_of_a_monster_without_nightvision_only() {
        assert_eq!(best_to_hit(aim(false), Lighting::Umbra), 16);
        assert_eq!(best_to_hit(aim(true), Lighting::Umbra), 19);
    }

    #[test]
    fn one_attack_in_forty_hits_or_misses_regardless_of_evasion() {
        // Against EV 1000 only the automatic hit lands: 5% of attacks ignore evasion, half hit.
        let ported = chance(|rng| {
            let to_hit = monster_to_hit(MonsterToHit::Crawl034, rng, aim(false), Lighting::Plain);
            evasion_margin(
                EvasionRoll::Crawl034,
                rng,
                to_hit,
                Defences::exact(0, 1000, 0),
            ) >= 0
        });
        assert!((ported - 0.025).abs() < 0.002, "{ported}");
    }

    #[test]
    fn without_a_shield_nothing_is_blocked() {
        assert!((0..1000).all(|_| !shield_blocks(
            ShieldBlock::Crawl034,
            &mut rng(),
            10,
            5,
            Defences::exact(0, 0, 0)
        )));
    }

    /// `mon_beat_sh_pct(bypass, scaled_sh)` (`fight.cc:286`) for a whole shield class: crawl's
    /// own estimate of the chance in 100 that a monster's attack gets past the player's shield,
    /// shown in monster descriptions. Ported here as an independent check on the rolls. It counts
    /// `bypass - adj_sh` of the `bypass` bypass rolls as beating the shield, where the attack
    /// code beats it on only `bypass - adj_sh - 1` of them (a tie blocks, `attack.cc:1060`), so
    /// it overstates the chance by `100 / bypass` points.
    fn crawl_mon_beat_sh_pct(bypass: i32, shield_class: i32) -> f64 {
        let sh = shield_class * 2;
        let mut hits = 0;
        for sh1 in 0..sh {
            for sh2 in 0..sh {
                let adj_sh = (sh1 + sh2) / (3 * 2) - 1;
                hits += (bypass - adj_sh).max(0);
            }
        }
        f64::from(hits * 100) / f64::from(sh * sh * bypass)
    }

    #[test]
    fn a_shield_blocks_as_often_as_crawls_own_estimate_says() {
        for (hd, displayed_sh) in [(1, 3), (6, 5), (12, 15)] {
            let bypass = 15 + hd * 2 / 3;
            let estimate =
                100.0 - crawl_mon_beat_sh_pct(bypass, displayed_sh * 2) + 100.0 / f64::from(bypass);
            let ported = chance(|rng| {
                shield_blocks(
                    ShieldBlock::Crawl034,
                    rng,
                    10,
                    hd,
                    Defences::exact(0, 0, displayed_sh),
                )
            }) * 100.0;
            assert!(
                (ported - estimate).abs() < 2.5,
                "HD {hd}, SH {displayed_sh}: ported {ported:.1}% vs crawl's estimate {estimate:.1}%"
            );
        }
    }

    #[test]
    fn a_monster_attack_deals_one_to_its_damage_before_armour() {
        let mut rng = rng();
        let rolls: Vec<_> = (0..10_000)
            .map(|_| melee_damage(Formulas::CRAWL_0_34, &mut rng, attack("AF_PLAIN", 5), None))
            .collect();
        assert!(rolls.iter().all(|roll| roll.max == 5));
        assert_eq!(rolls.iter().map(|roll| roll.damage).min(), Some(1));
        assert_eq!(rolls.iter().map(|roll| roll.damage).max(), Some(5));
    }

    #[test]
    fn a_crushing_attack_deals_nothing_as_it_grabs() {
        let roll = melee_damage(
            Formulas::CRAWL_0_34,
            &mut rng(),
            attack("AF_CRUSH", 20),
            None,
        );
        assert_eq!(roll, RolledDamage { damage: 0, max: 0 });
    }

    #[test]
    fn gdr_is_sixteen_times_the_fourth_root_of_ac() {
        // Worked by hand: 16 * 0^(1/4) = 0, 16 * 1 = 16, 16 * 16^(1/4) = 32, 16 * 81^(1/4) = 48,
        // 16 * 10^(1/4) = 28.45.
        assert_eq!([0, 1, 10, 16, 81].map(gdr_percent), [0, 16, 28, 32, 48]);
    }

    #[test]
    fn armour_saves_up_to_its_ac_and_at_least_its_guaranteed_reduction() {
        // AC 16: GDR 32%, so a 20-damage hit (max 20) always saves at least min(6, AC/2 = 8) = 6,
        // and at most 16.
        let mut rng = rng();
        let taken: Vec<_> = (0..20_000)
            .map(|_| {
                apply_ac(
                    Armour::Crawl034,
                    &mut rng,
                    RolledDamage {
                        damage: 20,
                        max: 20,
                    },
                    Defences::exact(16, 0, 0),
                )
            })
            .collect();
        assert_eq!(taken.iter().max(), Some(&14));
        assert_eq!(taken.iter().min(), Some(&4));
    }

    /// The share of `TRIALS` players behind `displayed` whose `roll` came out as each value.
    fn shares(
        displayed: DisplayedDefences,
        roll: fn(Defences, &mut Rng) -> i32,
    ) -> BTreeMap<i32, f64> {
        let mut rng = rng();
        let mut counts = BTreeMap::new();
        for _ in 0..TRIALS {
            let defences = Defences::drawn_behind(displayed, &mut rng);
            *counts.entry(roll(defences, &mut rng)).or_insert(0) += 1;
        }
        counts
            .into_iter()
            .map(|(value, count)| (value, f64::from(count) / f64::from(TRIALS)))
            .collect()
    }

    fn assert_shares(actual: &BTreeMap<i32, f64>, expected: &[(i32, f64)]) {
        assert_eq!(
            actual.keys().copied().collect::<Vec<_>>(),
            expected.iter().map(|&(value, _)| value).collect::<Vec<_>>()
        );
        for &(value, share) in expected {
            assert!(
                (actual[&value] - share).abs() < 0.005,
                "{value}: {actual:?}"
            );
        }
    }

    #[test]
    fn behind_a_displayed_ac_or_ev_each_roll_is_that_or_one_more_about_half_the_time() {
        // A fraction of 0 to 99 hundredths, each as likely, rounds up 49.5 times in 100.
        let displayed = DisplayedDefences {
            ac: 7,
            ev: 12,
            sh: 0,
        };
        assert_shares(
            &shares(displayed, Defences::armour_class),
            &[(7, 0.505), (8, 0.495)],
        );
        assert_shares(
            &shares(displayed, Defences::evasion),
            &[(12, 0.505), (13, 0.495)],
        );
    }

    #[test]
    fn behind_a_displayed_sh_the_shield_class_hides_up_to_two_points() {
        // Displayed SH 5 is a shield class of 10.00 to 11.99: half the time 10 rounding up to 11
        // 49.5 times in 100, half the time 11 rounding up to 12 as often.
        let displayed = DisplayedDefences {
            ac: 0,
            ev: 0,
            sh: 5,
        };
        assert_shares(
            &shares(displayed, Defences::shield_class),
            &[(10, 0.2525), (11, 0.5), (12, 0.2475)],
        );
    }

    #[test]
    fn a_displayed_zero_or_less_is_taken_as_exact() {
        let displayed = DisplayedDefences {
            ac: 0,
            ev: -2,
            sh: 0,
        };
        assert_shares(&shares(displayed, Defences::armour_class), &[(0, 1.0)]);
        assert_shares(&shares(displayed, Defences::evasion), &[(-2, 1.0)]);
        assert_shares(&shares(displayed, Defences::shield_class), &[(0, 1.0)]);
    }

    #[test]
    fn reaching_flavours_reach_two_squares_and_a_rift_three() {
        let reach = |flavour| attack_reach(MonsterReach::Crawl034, attack(flavour, 5));
        assert_eq!(reach("AF_PLAIN"), 1);
        assert_eq!(reach("AF_REACH"), 2);
        assert_eq!(reach("AF_REACH_STING"), 2);
        assert_eq!(reach("AF_RIFT"), 3);
    }

    #[test]
    fn a_monster_of_speed_fourteen_acts_twice_in_two_turns_out_of_five() {
        // Starting at 70..=79 and gaining 14, it acts once at 84..=93, and again only from 90.
        let twice = chance(|rng| {
            let energy = starting_energy(MonsterEnergy::Crawl034, rng)
                + energy_gained(MonsterEnergy::Crawl034, rng, 14, BASELINE_DELAY);
            energy - 10 >= ENERGY_THRESHOLD
        });
        assert!((twice - 0.4).abs() < 0.005, "{twice}");
    }
}
