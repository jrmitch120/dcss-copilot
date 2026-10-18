//! Crawl's weapon brands, ported closely (ADR 0002) behind the formula variant a Game Knowledge
//! bundle names: the damage a brand adds to a hit, and how it changes the weapon's own damage and
//! the time an attack with it takes. A monster's launcher is played with them; so can its melee
//! weapon be (#98).
//!
//! Line numbers are crawl 0.34.1's (`crawl-ref/source/` at tag 0.34.1). The player is taken as
//! one who resists nothing, as spells take them, and as a living mortal who worships no good god,
//! whom holy wrath doesn't hurt and foul flame hurts as anyone. Brands' side effects, such as
//! draining the player's maximum hit points, are left out with the other attack flavours.

use copilot_gk::{Bundle, DamageBrand, EnumId, Intelligence, Monster, MonsterSpells};

use super::rng::Rng;

/// The school whose spells make a monster a necromancer, as `spschool` names it.
const NECROMANCY: &str = "necromancy";

/// A weapon's brand, as the formulas tell brands apart.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Brand {
    /// No brand.
    Plain,
    /// `SPWPN_FLAMING`.
    Flaming,
    /// `SPWPN_FREEZING`.
    Freezing,
    /// `SPWPN_ELECTROCUTION`.
    Electrocution,
    /// `SPWPN_HOLY_WRATH`.
    HolyWrath,
    /// `SPWPN_FOUL_FLAME`.
    FoulFlame,
    /// `SPWPN_DRAINING`.
    Draining,
    /// `SPWPN_PAIN`.
    Pain,
    /// `SPWPN_HEAVY`: more damage, more slowly.
    Heavy,
    /// `SPWPN_SPEED`: the same damage, more quickly.
    Speed,
    /// Any other brand, such as venom or chaos: what it does isn't ported, and it is played as
    /// no brand.
    Other,
}

impl Brand {
    /// The brand whose `brand_type` identifier is `id`, as `variant` tells brands apart.
    pub(crate) fn of(variant: DamageBrand, id: &EnumId) -> Self {
        match variant {
            DamageBrand::Crawl034 => match id.0.as_str() {
                "SPWPN_NORMAL" => Self::Plain,
                "SPWPN_FLAMING" => Self::Flaming,
                "SPWPN_FREEZING" => Self::Freezing,
                "SPWPN_ELECTROCUTION" => Self::Electrocution,
                "SPWPN_HOLY_WRATH" => Self::HolyWrath,
                "SPWPN_FOUL_FLAME" => Self::FoulFlame,
                "SPWPN_DRAINING" => Self::Draining,
                "SPWPN_PAIN" => Self::Pain,
                "SPWPN_HEAVY" => Self::Heavy,
                "SPWPN_SPEED" => Self::Speed,
                _ => Self::Other,
            },
        }
    }
}

/// A monster's necromancy skill, which the pain brand rolls against, in 256ths: crawl's
/// `monster::skill(SK_NECROMANCY, 256)` (`monster.cc:4085`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct Necromancy(i32);

impl Necromancy {
    /// `monster`'s, at `hit_dice`, with the spells of its book looked up in `bundle`: none unless
    /// it is as clever as a person, twice its hit dice with a necromancy spell in its book, and
    /// half of them otherwise. A kind given its spells as it is made is taken to have none. An
    /// avatar, such as a spectral weapon, which has the skill whatever its intelligence, carries
    /// no launcher.
    pub(crate) fn of(bundle: &Bundle, monster: &Monster, hit_dice: i32) -> Self {
        if monster.intelligence < Intelligence::Human {
            return Self(0);
        }
        let necromancer = match &monster.spells {
            MonsterSpells::Book { spells } => spells.iter().any(|slot| {
                bundle
                    .spell_by_id(&slot.spell)
                    .is_some_and(|spell| spell.schools.iter().any(|school| school == NECROMANCY))
            }),
            MonsterSpells::PerMonster => false,
        };
        let scaled = 256 * hit_dice;
        Self(if necromancer { scaled * 2 } else { scaled / 2 })
    }

    /// The skill in whole levels, rounded at random: `actor::skill_rdiv` (`actor.cc:92`).
    fn roll(self, rng: &mut Rng) -> i32 {
        rng.div_rand_round(self.0, 256)
    }
}

/// The damage a weapon of `damage` (its `PWPN_DAMAGE`) deals with `brand`, rolled:
/// `brand_adjust_weapon_damage` (`fight.cc:1738`), which a heavy weapon raises by four fifths.
pub(crate) fn weapon_damage(variant: DamageBrand, rng: &mut Rng, brand: Brand, damage: i32) -> i32 {
    match variant {
        DamageBrand::Crawl034 => {
            if brand == Brand::Heavy {
                rng.div_rand_round(damage * 9, 5)
            } else {
                damage
            }
        }
    }
}

/// The delay of an attack with a weapon of `brand` that would take `base` with an unbranded one:
/// `weapon_adjust_delay` (`fight.cc:1294`), half again for a heavy weapon and two thirds for a
/// fast one.
pub(crate) fn delay(variant: DamageBrand, rng: &mut Rng, brand: Brand, base: i32) -> i32 {
    match variant {
        DamageBrand::Crawl034 => match brand {
            Brand::Heavy => rng.div_rand_round(base * 3, 2),
            Brand::Speed => rng.div_rand_round(base * 2, 3),
            _ => base,
        },
    }
}

/// The damage `brand` adds to a hit on the player that dealt them `dealt` after armour, from a
/// monster with `necromancy`: `attack::apply_damage_brand` (`attack.cc:1103`) for a player who
/// resists nothing (`resist_adjust_damage`, `fight.cc:853`, leaves the damage as it is).
///
/// Flaming and freezing add `random2(dealt) / 2 + 1` (`attack::calc_elemental_brand_damage`,
/// `attack.cc:1402`), and foul flame `1 + random2(dealt)`, but only to a hit that dealt some;
/// electrocution adds `8 + random2(13)` one time in four, whatever the hit dealt; draining adds
/// `(1 + random2(dealt)) / 2` (`attack::drain_defender`, `attack.cc:639`); and pain, unless the
/// monster's skill fails it, `random2(1 + skill)` (`attack::pain_affects_defender`,
/// `attack.cc:521`). Holy wrath doesn't hurt a living mortal. The brand lands even on a player the
/// hit killed, which only adds to the worst-case hit.
pub(crate) fn damage(
    variant: DamageBrand,
    rng: &mut Rng,
    brand: Brand,
    necromancy: Necromancy,
    dealt: i32,
) -> i32 {
    match variant {
        DamageBrand::Crawl034 => {
            let needs_damage = matches!(
                brand,
                Brand::Flaming | Brand::Freezing | Brand::HolyWrath | Brand::FoulFlame
            );
            if needs_damage && dealt == 0 {
                return 0;
            }
            match brand {
                Brand::Flaming | Brand::Freezing => rng.random2(dealt) / 2 + 1,
                // The player's `res_foul_flame` (`player.cc:7019`) is 0 for a mortal of no good
                // god, which divides by one.
                Brand::FoulFlame => 1 + rng.random2(dealt),
                Brand::Electrocution => {
                    if rng.one_chance_in(4) {
                        8 + rng.random2(13)
                    } else {
                        0
                    }
                }
                Brand::Draining => (1 + rng.random2(dealt)) / 2,
                Brand::Pain => {
                    let skill = necromancy.roll(rng);
                    if rng.one_chance_in(skill + 1) {
                        0
                    } else {
                        let skill = necromancy.roll(rng);
                        rng.random2(1 + skill)
                    }
                }
                Brand::Plain | Brand::HolyWrath | Brand::Heavy | Brand::Speed | Brand::Other => 0,
            }
        }
    }
}
