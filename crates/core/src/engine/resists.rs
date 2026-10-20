//! Crawl's adjustment of the damage the player takes for their resistances, holiness and god,
//! ported closely (ADR 0002) behind the formula variant a Game Knowledge bundle names:
//! `check_your_resists` (`ouch.cc:110`) for a beam that hits them, and `resist_adjust_damage`
//! (`fight.cc:853`), which it and the brands (`brands.rs`) call.
//!
//! Line numbers are crawl 0.34.1's (`crawl-ref/source/` at tag 0.34.1).

use copilot_gk::{
    Bundle, EnumId, Form, FormHoliness, HolinessSigns, ResistDamage, Species, Torment, Undeadness,
};

use super::torment::{self, Share};

use crate::crawl_ids::beam_types;
use crate::screen::{Resistance, Resistances};
use crate::visible_state::{Religion, Status};

/// What about the player changes the damage a beam deals them, or a brand adds to a hit: their
/// resistances, as the character overview shows them, and their holiness and god.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Resists {
    /// `rFire`, from -3 to 3.
    pub(crate) fire: i32,
    /// `rCold`, from -3 to 3.
    pub(crate) cold: i32,
    /// `rNeg`, from 0 to 3.
    pub(crate) negative: i32,
    /// `rPois`: -1 to 1, or 3 for immunity.
    pub(crate) poison: i32,
    /// `rElec`: -1 to 1.
    pub(crate) electricity: i32,
    /// `rCorr`: 0 or 1.
    pub(crate) corrosion: i32,
    /// What they are, and whom they worship.
    pub(crate) holiness: Holiness,
}

/// What the player is, and whom they worship, as the damage crawl deals them tells it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Holiness {
    /// Undead in whole or part, or demonic: `player::undead_or_demonic` (`player.cc:6893`),
    /// whom holy wrath hurts and foul flame spares.
    pub(crate) undead_or_demonic: bool,
    /// Holy, or a good god's worshipper: `player::is_holy` (`player.cc:6906`), whom holy fire
    /// spares and foul flame hurts more.
    pub(crate) holy: bool,
    /// Natural or a plant (`MH_NATURAL | MH_PLANT`), as draining needs its defender to be
    /// (`attack::drain_defender`, `attack.cc:670`).
    pub(crate) drainable: bool,
    /// Nonliving or lifeless undead, whom miasma doesn't harm (`player::res_miasma`,
    /// `player.cc:6987`).
    pub(crate) miasma_immune: bool,
    /// The share of their hit points torment takes ([`torment::share`]).
    pub(crate) torment: Share,
}

impl Holiness {
    /// A living mortal who worships no good god.
    pub(crate) const MORTAL: Self = Self {
        undead_or_demonic: false,
        holy: false,
        drainable: true,
        miasma_immune: false,
        torment: Share::Full,
    };
}

/// What the player is, as crawl's holiness bits (`mon_holy_type`) have it: those the ported
/// damage reads. Being undead or demonic, which it reads otherwise, is judged apart.
#[derive(Debug, Clone, Copy)]
struct Kinds {
    natural: bool,
    nonliving: bool,
    plant: bool,
    holy: bool,
}

impl Kinds {
    /// None of them, as an undead or demonic player is.
    const NONE: Self = Self {
        natural: false,
        nonliving: false,
        plant: false,
        holy: false,
    };
    const NATURAL: Self = Self {
        natural: true,
        ..Self::NONE
    };
    const NONLIVING: Self = Self {
        nonliving: true,
        ..Self::NONE
    };

    /// What a player of `base` is in a form that gives `holiness`.
    fn in_form(base: Self, holiness: FormHoliness) -> Self {
        match holiness {
            // Adding undead takes none of the others away.
            FormHoliness::Species | FormHoliness::AddsUndead => base,
            FormHoliness::Natural => Self::NATURAL,
            FormHoliness::Nonliving => Self::NONLIVING,
            FormHoliness::Undead | FormHoliness::Demonic => Self::NONE,
            FormHoliness::Plant => Self {
                plant: true,
                ..Self::NONE
            },
            FormHoliness::Holy => Self {
                holy: true,
                ..Self::NONE
            },
        }
    }
}

impl Holiness {
    /// A player of `species`, in `form` if they are in one, petrified or not, who worships a good
    /// god or not, as `variant` judges them through `signs`: `player::holiness`
    /// (`player.cc:6859`), with the form's own holiness taking the species' place and
    /// petrification both's, `player::undead_state` (`player.cc:7268`), `undead_or_demonic`,
    /// `is_holy` and `res_miasma`, but for a resistance to miasma of a form, a mutation or an
    /// artefact; and the share of their hit points torment takes, as `torment_variant` judges it.
    pub(crate) fn of(
        variant: ResistDamage,
        torment_variant: Torment,
        signs: &HolinessSigns,
        species: &Species,
        form: Option<&Form>,
        petrified: bool,
        good_god: bool,
    ) -> Self {
        match variant {
            ResistDamage::Crawl034 => {
                let undead = match form {
                    Some(form) if form.undead != Undeadness::Alive => form.undead,
                    _ => species.undead,
                };
                let base = if species.undead == Undeadness::Undead {
                    Kinds::NONE
                } else if signs.nonliving_species.contains(&species.id) {
                    Kinds::NONLIVING
                } else {
                    Kinds::NATURAL
                };
                let kinds = match form {
                    _ if petrified => Kinds::NONLIVING,
                    Some(form) => Kinds::in_form(base, form.holiness),
                    None => base,
                };
                Self {
                    undead_or_demonic: undead != Undeadness::Alive
                        || signs.demonic_species.contains(&species.id),
                    holy: kinds.holy || good_god,
                    drainable: kinds.natural || kinds.plant,
                    miasma_immune: kinds.nonliving || undead == Undeadness::Undead,
                    torment: torment::share(torment_variant, form, petrified, kinds.plant),
                }
            }
        }
    }
}

impl Resists {
    /// A living mortal who resists nothing and worships no good god: how the player is taken when
    /// the Advisor hasn't read their resistances.
    pub(crate) const NONE: Self = Self {
        fire: 0,
        cold: 0,
        negative: 0,
        poison: 0,
        electricity: 0,
        corrosion: 0,
        holiness: Holiness::MORTAL,
    };

    /// Their resistance to holy fire: `player::res_holy_energy` (`player.cc:7008`), -1 for the
    /// undead or demonic and 3, immunity, for the holy.
    pub(crate) fn holy_energy(self) -> i32 {
        if self.holiness.undead_or_demonic {
            -1
        } else if self.holiness.holy {
            3
        } else {
            0
        }
    }

    /// Their resistance to foul flame: `player::res_foul_flame` (`player.cc:7019`), 1 for the
    /// undead or demonic and -1 for the holy.
    pub(crate) fn foul_flame(self) -> i32 {
        if self.holiness.undead_or_demonic {
            1
        } else if self.holiness.holy {
            -1
        } else {
            0
        }
    }

    /// Their resistance to steam: `player_res_steam` (`player.cc:1429`) for a player with no
    /// resistance to steam of their own, from steam dragon scales or a mutation, which the
    /// character overview doesn't show: twice their `rFire`, up to 2.
    fn steam(self) -> i32 {
        (self.fire * 2).min(2)
    }
}

/// What the Advisor has seen of the player that changes the damage they take.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Seen<'a> {
    /// Their resistances, as the character overview or dump showed them.
    pub(crate) resistances: &'a Resistances,
    /// Their species, as the game names it, once it has.
    pub(crate) species: Option<&'a str>,
    /// The statuses the status pane lists, once the game has reported them.
    pub(crate) statuses: Option<&'a [Status]>,
    /// Their god, once the game has reported it.
    pub(crate) religion: Option<&'a Religion>,
}

/// The player's resistances, holiness and god as the engine plays what the Advisor has seen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Read {
    /// As the engine plays them.
    pub(crate) resists: Resists,
    /// Whether their holiness and god are known, and played. Otherwise they are played as a
    /// living mortal of no good god.
    pub(crate) holiness_known: bool,
}

/// The player as `seen`, judged through `bundle`: `None` unless their resistances show each of
/// those that cut damage. Their holiness, from their species, the form and petrification the
/// status pane lights and whether they worship a good god, is known when the bundle knows the
/// species and the game has reported the statuses and the god.
pub(crate) fn read(bundle: &Bundle, seen: &Seen<'_>) -> Option<Read> {
    // Immunity is drawn only for a resistance of 3 on a one-pip scale (`_resist_composer`).
    let level = |resistance: Option<Resistance>| match resistance? {
        Resistance::Pips(pips) => Some(i32::from(pips)),
        Resistance::Immune => Some(3),
    };
    let read = seen.resistances;
    let holiness = holiness(bundle, seen);
    Some(Read {
        resists: Resists {
            fire: level(read.fire)?,
            cold: level(read.cold)?,
            negative: level(read.negative)?,
            poison: level(read.poison)?,
            electricity: level(read.electricity)?,
            corrosion: level(read.corrosion)?,
            holiness: holiness.unwrap_or(Holiness::MORTAL),
        },
        holiness_known: holiness.is_some(),
    })
}

/// The player's holiness as `seen`, judged through `bundle`: `None` when the bundle doesn't know
/// the species, or the game hasn't reported the statuses or the god.
fn holiness(bundle: &Bundle, seen: &Seen<'_>) -> Option<Holiness> {
    let species = bundle.species_named(seen.species?)?;
    let signs = &bundle.holiness;
    let lights: Vec<&str> = seen
        .statuses?
        .iter()
        .filter_map(|status| status.light.as_deref())
        .collect();
    let form = signs
        .forms
        .iter()
        .find(|form| lights.contains(&form.light.as_str()));
    let petrified = signs
        .petrified_lights
        .iter()
        .any(|light| lights.contains(&light.as_str()));
    let good_god = match seen.religion? {
        Religion::NoGod => false,
        Religion::God { name, .. } => signs.good_gods.contains(name),
    };
    Some(Holiness::of(
        bundle.formulas.resist_damage,
        bundle.formulas.torment,
        signs,
        species,
        form,
        petrified,
        good_god,
    ))
}

/// A kind of damage, as `resist_adjust_damage` tells them apart for the player.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Element {
    /// `BEAM_FIRE`, against `rFire`.
    Fire,
    /// `BEAM_LAVA`: half of it against `rFire`.
    Lava,
    /// `BEAM_COLD`, against `rCold`.
    Cold,
    /// `BEAM_ICE`: half of it against `rCold`.
    Ice,
    /// `BEAM_ELECTRICITY` or `BEAM_STUN_BOLT`, against `rElec`.
    Electricity,
    /// `BEAM_THUNDER`: half of it against `rElec`.
    Thunder,
    /// `BEAM_NEG`, against `rNeg`.
    Negative,
    /// `BEAM_POISON`, against `rPois`.
    Poison,
    /// `BEAM_POISON_ARROW`: seven tenths of it against `rPois`.
    PoisonArrow,
    /// `BEAM_ACID`, against `rCorr`.
    Acid,
    /// `BEAM_HOLY`, against the player's holiness.
    Holy,
    /// `BEAM_STEAM`, against `rSteam`.
    Steam,
}

/// What `check_your_resists` does to the damage of a beam that hits the player.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BeamResist {
    /// Nothing the player resists changes it.
    Unresisted,
    /// `resist_adjust_damage` adjusts it for this element.
    Adjusted(Element),
    /// `BEAM_MIASMA`: none at all to a player who resists miasma.
    Miasma,
}

impl BeamResist {
    /// What its adjustment of a beam's damage depends on.
    pub(crate) fn needs(self) -> Needs {
        match self {
            Self::Unresisted => Needs::NOTHING,
            Self::Adjusted(Element::Holy) => Needs {
                holiness: true,
                ..Needs::NOTHING
            },
            Self::Adjusted(Element::Steam) => Needs {
                reading: true,
                unshown: true,
                ..Needs::NOTHING
            },
            Self::Adjusted(_) => Needs {
                reading: true,
                ..Needs::NOTHING
            },
            Self::Miasma => Needs {
                unshown: true,
                holiness: true,
                ..Needs::NOTHING
            },
        }
    }
}

/// What adjusting some damage the player may take for their resistances depends on.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct Needs {
    /// Their resistances as the character overview shows them.
    pub(crate) reading: bool,
    /// A resistance the character overview doesn't show, such as `rSteam`.
    pub(crate) unshown: bool,
    /// Their holiness or god.
    pub(crate) holiness: bool,
}

impl Needs {
    /// Nothing: the damage is the same whatever the player resists.
    pub(crate) const NOTHING: Self = Self {
        reading: false,
        unshown: false,
        holiness: false,
    };

    /// What either depends on.
    #[must_use]
    pub(crate) fn and(self, other: Self) -> Self {
        Self {
            reading: self.reading || other.reading,
            unshown: self.unshown || other.unshown,
            holiness: self.holiness || other.holiness,
        }
    }

    /// Whether it depends on anything.
    pub(crate) fn any(self) -> bool {
        self != Self::NOTHING
    }
}

/// What `check_your_resists` does to a beam of `flavour`, a `beam_type` identifier, as `variant`
/// does it: `None` for a flavour the port doesn't know, which no spell the engine ports fires.
pub(crate) fn beam(variant: ResistDamage, flavour: &EnumId) -> Option<BeamResist> {
    match variant {
        ResistDamage::Crawl034 => {
            let element = match flavour.0.as_str() {
                beam_types::FIRE => Element::Fire,
                beam_types::LAVA => Element::Lava,
                beam_types::COLD => Element::Cold,
                beam_types::ICE => Element::Ice,
                beam_types::ELECTRICITY | beam_types::STUN_BOLT => Element::Electricity,
                beam_types::THUNDER => Element::Thunder,
                beam_types::NEG => Element::Negative,
                beam_types::POISON => Element::Poison,
                beam_types::POISON_ARROW => Element::PoisonArrow,
                beam_types::ACID => Element::Acid,
                beam_types::HOLY => Element::Holy,
                beam_types::STEAM => Element::Steam,
                beam_types::MIASMA => return Some(BeamResist::Miasma),
                // Damnation, which `check_your_resists` lets through whatever the player
                // resists, and flavours its switch has no case for, or none that changes damage.
                beam_types::DAMNATION
                | beam_types::MISSILE
                | beam_types::MMISSILE
                | beam_types::FRAG
                | beam_types::DEVASTATION
                | beam_types::BOLAS
                | beam_types::CRYSTALLISING
                | beam_types::LIGHT => return Some(BeamResist::Unresisted),
                _ => return None,
            };
            Some(BeamResist::Adjusted(element))
        }
    }
}

/// The damage a beam that `resist` adjusts deals a player who `resists`, `damage` after armour:
/// `check_your_resists` (`ouch.cc:110`).
pub(crate) fn beam_damage(
    variant: ResistDamage,
    resist: BeamResist,
    resists: Resists,
    damage: i32,
) -> i32 {
    match variant {
        ResistDamage::Crawl034 => match resist {
            BeamResist::Adjusted(element) => adjust(variant, element, resists, damage),
            BeamResist::Miasma if resists.holiness.miasma_immune => 0,
            BeamResist::Unresisted | BeamResist::Miasma => damage,
        },
    }
}

/// `damage` of `element` as it hurts a player who `resists`: `resist_adjust_damage`
/// (`fight.cc:853`). A resistance cuts the part of the damage it resists to a half, a third or a
/// fifth at one, two or three pips, or a third at one pip of `rElec` or `rPois`, and `rNeg` to a
/// half, a quarter or none. Three pips of `rNeg`, `rPois` or holy energy are immunity. Each pip
/// of vulnerability raises it by half.
pub(crate) fn adjust(
    variant: ResistDamage,
    element: Element,
    resists: Resists,
    damage: i32,
) -> i32 {
    match variant {
        ResistDamage::Crawl034 => {
            // The resistance, the percentage of the damage it can resist
            // (`_get_resistible_fraction`), whether it claims to be all or nothing
            // (`_is_boolean_resist`) and whether three pips are immunity.
            let (res, fraction, boolean, immune_at_3) = match element {
                Element::Fire => (resists.fire, 100, false, false),
                Element::Lava => (resists.fire, 50, false, false),
                Element::Cold => (resists.cold, 100, false, false),
                Element::Ice => (resists.cold, 50, false, false),
                Element::Electricity => (resists.electricity, 100, true, false),
                Element::Thunder => (resists.electricity, 50, true, false),
                Element::Negative => (resists.negative, 100, false, true),
                Element::Poison => (resists.poison, 100, true, true),
                Element::PoisonArrow => (resists.poison, 70, true, true),
                Element::Acid => (resists.corrosion, 100, false, false),
                Element::Holy => (resists.holy_energy(), 100, false, true),
                Element::Steam => (resists.steam(), 100, false, false),
            };
            if res == 0 {
                return damage;
            }
            let mut resistible = damage * fraction / 100;
            let irresistible = damage - resistible;
            if res > 0 {
                if immune_at_3 && res >= 3 || res > 3 {
                    resistible = 0;
                } else if element == Element::Negative {
                    resistible /= res * 2;
                } else {
                    resistible /= (3 * res + 1) / 2 + i32::from(boolean);
                }
            } else {
                resistible = resistible * 15 / 10;
            }
            (resistible + irresistible).max(0)
        }
    }
}

#[cfg(test)]
mod tests {
    use copilot_gk::{Bundle, Library};

    use super::*;
    use crate::engine::spells;

    /// A player of the species named `species`, in the form lit `form` if any, petrified or
    /// not, who worships a good god or not, as the shipped 0.34.1 bundle judges them.
    fn holiness(species: &str, form: Option<&str>, petrified: bool, good_god: bool) -> Holiness {
        let library = Library::shipped().unwrap();
        let bundle: &Bundle = library.for_version("0.34.1").unwrap();
        let signs = &bundle.holiness;
        let form = form.map(|light| signs.forms.iter().find(|f| f.light == light).unwrap());
        Holiness::of(
            bundle.formulas.resist_damage,
            bundle.formulas.torment,
            signs,
            bundle.species_named(species).unwrap(),
            form,
            petrified,
            good_god,
        )
    }

    /// Undead or demonic, holy, drainable and immune to miasma, in that order.
    fn kinds(holiness: Holiness) -> (bool, bool, bool, bool) {
        (
            holiness.undead_or_demonic,
            holiness.holy,
            holiness.drainable,
            holiness.miasma_immune,
        )
    }

    #[test]
    fn a_species_gives_its_holiness_as_crawl_derives_it_for_the_player() {
        for (species, expected) in [
            ("Human", (false, false, true, false)),
            ("Mummy", (true, false, false, true)),
            ("Poltergeist", (true, false, false, true)),
            ("Demonspawn", (true, false, true, false)),
            ("Gargoyle", (false, false, false, true)),
            ("Djinni", (false, false, false, true)),
        ] {
            assert_eq!(
                kinds(holiness(species, None, false, false)),
                expected,
                "{species}"
            );
        }
    }

    #[test]
    fn a_good_gods_worshipper_is_holy_unless_undead_or_demonic_comes_first() {
        assert_eq!(
            kinds(holiness("Human", None, false, true)),
            (false, true, true, false)
        );
        let mummy = holiness("Mummy", None, false, true);
        let resists = Resists {
            holiness: mummy,
            ..Resists::NONE
        };
        assert_eq!((resists.holy_energy(), resists.foul_flame()), (-1, 1));
    }

    #[test]
    fn a_form_takes_the_species_place_or_adds_undead_to_it() {
        for (species, form, expected) in [
            ("Human", "Statue", (false, false, false, true)),
            ("Human", "Tree", (false, false, true, false)),
            ("Human", "Death", (true, false, false, true)),
            ("Human", "Vampire", (true, false, true, false)),
            ("Human", "Bat Swarm", (true, false, true, false)),
            ("Human", "Slaughter", (false, false, false, false)),
            ("Human", "Dragon", (false, false, true, false)),
            ("Gargoyle", "Jelly", (false, false, true, false)),
            ("Mummy", "Jelly", (true, false, true, true)),
        ] {
            assert_eq!(
                kinds(holiness(species, Some(form), false, false)),
                expected,
                "{species} in {form}"
            );
        }
    }

    #[test]
    fn petrification_makes_any_player_nonliving() {
        assert_eq!(
            kinds(holiness("Human", Some("Tree"), true, false)),
            (false, false, false, true)
        );
    }

    fn resisting(fire: i32) -> Resists {
        Resists {
            fire,
            ..Resists::NONE
        }
    }

    #[test]
    fn fire_is_cut_to_a_half_a_third_and_a_fifth_and_raised_by_half_for_the_vulnerable() {
        // Worked by hand from `resist_adjust_damage`: (3 * res + 1) / 2 is 2, 3 and 5.
        let v = ResistDamage::Crawl034;
        let fire = |res| adjust(v, Element::Fire, resisting(res), 30);
        assert_eq!([-1, 0, 1, 2, 3].map(fire), [45, 30, 15, 10, 6]);
        // Lava is half resistible: 15 resisted to 7, and 15 not.
        assert_eq!(adjust(v, Element::Lava, resisting(1), 30), 22);
    }

    #[test]
    fn every_ported_spell_fires_a_flavour_the_resist_port_knows() {
        let library = Library::shipped().unwrap();
        for bundle in library.bundles() {
            let formulas = bundle.formulas;
            for spell in bundle.spells.values() {
                let Some(ported) = spells::damaging(formulas.monster_spell_beam, spell) else {
                    continue;
                };
                assert!(
                    beam(formulas.resist_damage, &ported.zap.flavour).is_some(),
                    "{}: {} fires {}",
                    bundle.crawl.tag,
                    spell.id.0,
                    ported.zap.flavour.0
                );
            }
        }
    }
}
