//! Crawl's weapon brands, ported closely (ADR 0002) behind the formula variant a Game Knowledge
//! bundle names: the damage a brand adds to a hit, and how it changes the weapon's own damage and
//! the time an attack with it takes. A monster's launcher and its melee weapons are played with
//! them.
//!
//! Line numbers are crawl 0.34.1's (`crawl-ref/source/` at tag 0.34.1). The player's resistances,
//! holiness and god change what a brand adds as `resists.rs` ports them. Brands' side effects,
//! such as draining the player's maximum hit points, are left out with the other attack flavours.

use copilot_gk::{Bundle, DamageBrand, EnumId, Formulas, Intelligence, Monster, MonsterSpells};

use super::resists::{self, Element, Needs, Resists};
use super::rng::Rng;
use crate::crawl_ids::brand_types;

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
    /// What adjusting the damage it adds for the player's resistances depends on, as
    /// [`damage`] adjusts it.
    pub(crate) fn needs(self) -> Needs {
        let (reading, holiness) = match self {
            Self::Flaming | Self::Freezing | Self::Electrocution | Self::Pain => (true, false),
            Self::Draining => (true, true),
            Self::HolyWrath | Self::FoulFlame => (false, true),
            Self::Plain | Self::Heavy | Self::Speed | Self::Other => (false, false),
        };
        Needs {
            reading,
            holiness,
            ..Needs::NOTHING
        }
    }

    /// The brand whose `brand_type` identifier is `id`, as `variant` tells brands apart.
    pub(crate) fn of(variant: DamageBrand, id: &EnumId) -> Self {
        match variant {
            DamageBrand::Crawl034 => match id.0.as_str() {
                brand_types::NORMAL => Self::Plain,
                brand_types::FLAMING => Self::Flaming,
                brand_types::FREEZING => Self::Freezing,
                brand_types::ELECTROCUTION => Self::Electrocution,
                brand_types::HOLY_WRATH => Self::HolyWrath,
                brand_types::FOUL_FLAME => Self::FoulFlame,
                brand_types::DRAINING => Self::Draining,
                brand_types::PAIN => Self::Pain,
                brand_types::HEAVY => Self::Heavy,
                brand_types::SPEED => Self::Speed,
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
    /// no launcher, and its weapon, an animated object's, isn't played.
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

/// The damage `brand` adds to a hit on a player who `resists` that dealt them `dealt` after armour, from a
/// monster with `necromancy`: `attack::apply_damage_brand` (`attack.cc:1103`).
///
/// Flaming and freezing add `random2(dealt) / 2 + 1` (`attack::calc_elemental_brand_damage`,
/// `attack.cc:1402`), cut by `rFire` or `rCold`; foul flame `1 + random2(dealt)`, divided by one
/// more than the player's resistance to it, or half again for the holy; and holy wrath
/// `1 + random2(dealt * 15) / 10` to the undead or demonic, and nothing to anyone else. Those four
/// add nothing to a hit that dealt none. Electrocution adds `8 + random2(13)` one time in four,
/// whatever the hit dealt, unless the player has `rElec`; draining adds `(1 + random2(dealt)) /
/// 2`, cut by `rNeg`, to a player who is natural or a plant (`attack::drain_defender`,
/// `attack.cc:639`); and pain, unless the monster's skill fails it, `random2(1 + skill)`, cut by
/// `rNeg` (`attack::pain_affects_defender`, `attack.cc:521`). The brand lands even on a player the
/// hit killed, which only adds to the worst-case hit.
pub(crate) fn damage(
    formulas: Formulas,
    rng: &mut Rng,
    brand: Brand,
    necromancy: Necromancy,
    dealt: i32,
    resists: Resists,
) -> i32 {
    let adjust =
        |element, damage| resists::adjust(formulas.resist_damage, element, resists, damage);
    match formulas.damage_brand {
        DamageBrand::Crawl034 => {
            let needs_damage = matches!(
                brand,
                Brand::Flaming | Brand::Freezing | Brand::HolyWrath | Brand::FoulFlame
            );
            if needs_damage && dealt == 0 {
                return 0;
            }
            match brand {
                Brand::Flaming => adjust(Element::Fire, rng.random2(dealt) / 2 + 1),
                Brand::Freezing => adjust(Element::Cold, rng.random2(dealt) / 2 + 1),
                Brand::FoulFlame => {
                    let res = resists.foul_flame();
                    if res < 0 {
                        // `1 + random2(dealt) * 1.5`, truncated.
                        1 + rng.random2(dealt) * 3 / 2
                    } else if res < 3 {
                        1 + rng.random2(dealt) / (res + 1)
                    } else {
                        0
                    }
                }
                // `actor::holy_wrath_susceptible` (`actor.cc:238`).
                Brand::HolyWrath if resists.holy_energy() < 0 => 1 + rng.random2(dealt * 15) / 10,
                Brand::Electrocution => {
                    if resists.electricity <= 0 && rng.one_chance_in(4) {
                        8 + rng.random2(13)
                    } else {
                        0
                    }
                }
                Brand::Draining if resists.holiness.drainable => {
                    adjust(Element::Negative, (1 + rng.random2(dealt)) / 2)
                }
                Brand::Pain => {
                    let skill = necromancy.roll(rng);
                    if rng.one_chance_in(skill + 1) {
                        0
                    } else {
                        let skill = necromancy.roll(rng);
                        adjust(Element::Negative, rng.random2(1 + skill))
                    }
                }
                Brand::Plain
                | Brand::HolyWrath
                | Brand::Draining
                | Brand::Heavy
                | Brand::Speed
                | Brand::Other => 0,
            }
        }
    }
}
