//! The crawl identifiers `core` compares Game Knowledge against, grouped by what they name.
//!
//! A typo here, or crawl renaming one, would compile and quietly never match, so every
//! identifier `core` compares against is named in this module, and a test checks each against
//! every shipped Game Knowledge bundle. A list tied to one Formula Variant, such as the spells
//! crawl 0.34 fires as plain zaps, stays with its variant's code and is checked there.

/// Declares a group of identifiers as constants, and lists them all in `ALL` for the test.
macro_rules! identifiers {
    ($(#[$group_doc:meta])* $group:ident { $($(#[$doc:meta])* $name:ident = $id:literal;)+ }) => {
        $(#[$group_doc])*
        pub(crate) mod $group {
            $($(#[$doc])* pub(crate) const $name: &str = $id;)+

            /// Every identifier in the group.
            #[cfg(test)]
            pub(crate) const ALL: &[&str] = &[$($name),+];
        }
    };
}

identifiers! {
    /// `monster_type` identifiers.
    monster_types {
        /// Made with a tier and facets its Game Knowledge entry doesn't carry:
        /// `_mutant_beast_facet_attack` and `init_mutant_beast` in `mon-util.cc`.
        MUTANT_BEAST = "MONS_MUTANT_BEAST";
    }
}

identifiers! {
    /// `monster_type` identifiers a monster's kind names as its genus.
    genera {
        /// Draconians, which breathe by their colour, a breath outside their book: `drac_breath`
        /// in `mon-util.cc`.
        DRACONIAN = "MONS_DRACONIAN";
    }
}

identifiers! {
    /// `attack_type` identifiers.
    attack_types {
        /// The empty slot crawl pads a monster's attacks with.
        NONE = "AT_NONE";
    }
}

identifiers! {
    /// `attack_flavour` identifiers.
    attack_flavours {
        /// It grabs, and deals no damage itself.
        CRUSH = "AF_CRUSH";
        /// The flavours that reach: crawl's `flavour_has_reach` (`mon-util.cc:2209`).
        REACH = "AF_REACH";
        REACH_STING = "AF_REACH_STING";
        REACH_TONGUE = "AF_REACH_TONGUE";
        REACH_CLEAVE_UGLY = "AF_REACH_CLEAVE_UGLY";
        /// It reaches through a rift.
        RIFT = "AF_RIFT";
        /// As a first attack, the monster swoops at or flanks the foe: `_handle_swoop_or_flank`
        /// in `mon-act.cc`.
        SWOOP = "AF_SWOOP";
        FLANK = "AF_FLANK";
        /// Its hit tramples the defender back or drags them along:
        /// `melee_attack::mons_attack_effects` in `melee-attack.cc`.
        TRAMPLE = "AF_TRAMPLE";
        DRAG = "AF_DRAG";
        /// Its hit may blink the attacker away, or away with the defender:
        /// `melee_attack::mons_apply_attack_flavour` in `melee-attack.cc`.
        BLINK = "AF_BLINK";
        BLINK_WITH = "AF_BLINK_WITH";
    }
}

identifiers! {
    /// `spell_type` identifiers the engine treats on their own.
    spell_types {
        STICKY_FLAME = "SPELL_STICKY_FLAME";
        ORB_OF_ELECTRICITY = "SPELL_ORB_OF_ELECTRICITY";
        MIASMA_BREATH = "SPELL_MIASMA_BREATH";
        FREEZING_GUST = "SPELL_FREEZING_GUST";
        BLINKBOLT = "SPELL_BLINKBOLT";
    }
}

identifiers! {
    /// `mon_spell_slot_flag` identifiers.
    spell_slot_flags {
        BREATH = "MON_SPELL_BREATH";
        INSTANT = "MON_SPELL_INSTANT";
        EMERGENCY = "MON_SPELL_EMERGENCY";
        SHORT_RANGE = "MON_SPELL_SHORT_RANGE";
        LONG_RANGE = "MON_SPELL_LONG_RANGE";
    }
}

identifiers! {
    /// `beam_type` identifiers: the flavour of a spell's zap.
    beam_types {
        DAMNATION = "BEAM_DAMNATION";
        ELECTRICITY = "BEAM_ELECTRICITY";
        THUNDER = "BEAM_THUNDER";
        STUN_BOLT = "BEAM_STUN_BOLT";
        FRAG = "BEAM_FRAG";
    }
}

identifiers! {
    /// `brand_type` identifiers.
    brand_types {
        /// No brand, as the crawl oracle names a thrown missile's.
        NORMAL = "SPWPN_NORMAL";
        FLAMING = "SPWPN_FLAMING";
        FREEZING = "SPWPN_FREEZING";
        ELECTROCUTION = "SPWPN_ELECTROCUTION";
        HOLY_WRATH = "SPWPN_HOLY_WRATH";
        FOUL_FLAME = "SPWPN_FOUL_FLAME";
        DRAINING = "SPWPN_DRAINING";
        PAIN = "SPWPN_PAIN";
        HEAVY = "SPWPN_HEAVY";
        SPEED = "SPWPN_SPEED";
    }
}

identifiers! {
    /// `object_class_type` identifiers.
    object_classes {
        WEAPONS = "OBJ_WEAPONS";
        MISSILES = "OBJ_MISSILES";
        ARMOUR = "OBJ_ARMOUR";
        WANDS = "OBJ_WANDS";
        SCROLLS = "OBJ_SCROLLS";
        POTIONS = "OBJ_POTIONS";
    }
}

identifiers! {
    /// `missile_type` identifiers.
    missile_types {
        DART = "MI_DART";
        THROWING_NET = "MI_THROWING_NET";
        JAVELIN = "MI_JAVELIN";
        /// Crawl doesn't list it among a monster's gear unless it is branded.
        LARGE_ROCK = "MI_LARGE_ROCK";
    }
}

identifiers! {
    /// `armour_type` identifiers.
    armour_types {
        BUCKLER = "ARM_BUCKLER";
        KITE_SHIELD = "ARM_KITE_SHIELD";
        TOWER_SHIELD = "ARM_TOWER_SHIELD";
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use copilot_gk::{Bundle, Library, MonsterSpells};

    use super::*;

    /// Identifiers Game Knowledge doesn't carry, with why `core` names them anyway.
    const NOT_IN_GAME_KNOWLEDGE: [(&str, &str); 2] = [
        (
            attack_types::NONE,
            "crawl pads its attack array with it; the extractor reads the attacks a kind has",
        ),
        (
            brand_types::NORMAL,
            "Game Knowledge leaves out brands crawl never names; the crawl oracle sends it",
        ),
    ];

    /// The identifiers of each kind a bundle carries.
    fn known(bundle: &Bundle) -> [(&'static str, &'static [&'static str], BTreeSet<&str>); 11] {
        let monsters = || bundle.monsters.values();
        let attacks = || monsters().flat_map(|monster| &monster.attacks);
        let class_sub_types = |class: &str| {
            bundle
                .item_classes
                .values()
                .filter(move |item_class| item_class.id.0 == class)
                .flat_map(|item_class| item_class.sub_types.values())
                .map(|sub_type| sub_type.id.0.as_str())
                .collect()
        };
        [
            (
                "monsters",
                monster_types::ALL,
                monsters().map(|monster| monster.id.0.as_str()).collect(),
            ),
            (
                "genera",
                genera::ALL,
                monsters().map(|monster| monster.genus.0.as_str()).collect(),
            ),
            (
                "attack types",
                attack_types::ALL,
                attacks().map(|attack| attack.kind.0.as_str()).collect(),
            ),
            (
                "attack flavours",
                attack_flavours::ALL,
                attacks().map(|attack| attack.flavour.0.as_str()).collect(),
            ),
            (
                "spells",
                spell_types::ALL,
                bundle
                    .spells
                    .values()
                    .map(|spell| spell.id.0.as_str())
                    .collect(),
            ),
            (
                "spell slot flags",
                spell_slot_flags::ALL,
                monsters()
                    .filter_map(|monster| match &monster.spells {
                        MonsterSpells::Book { spells } => Some(spells),
                        _ => None,
                    })
                    .flatten()
                    .flat_map(|slot| &slot.flags)
                    .map(|flag| flag.0.as_str())
                    .collect(),
            ),
            (
                "beam flavours",
                beam_types::ALL,
                bundle
                    .spells
                    .values()
                    .filter_map(|spell| spell.zap.as_ref())
                    .map(|zap| zap.flavour.0.as_str())
                    .collect(),
            ),
            (
                "brands",
                brand_types::ALL,
                bundle
                    .weapon_brands
                    .keys()
                    .map(|id| id.0.as_str())
                    .collect(),
            ),
            (
                "item classes",
                object_classes::ALL,
                bundle
                    .item_classes
                    .values()
                    .map(|item_class| item_class.id.0.as_str())
                    .collect(),
            ),
            (
                "missiles",
                missile_types::ALL,
                class_sub_types(object_classes::MISSILES),
            ),
            (
                "armour",
                armour_types::ALL,
                class_sub_types(object_classes::ARMOUR),
            ),
        ]
    }

    #[test]
    fn every_identifier_core_names_is_in_every_shipped_bundle() {
        let library = Library::shipped().unwrap();
        let exempt: BTreeSet<&str> = NOT_IN_GAME_KNOWLEDGE.iter().map(|&(id, _)| id).collect();
        let mut bundles = 0;
        for bundle in library.bundles() {
            bundles += 1;
            for (kind, named, known) in known(bundle) {
                for id in named {
                    assert!(
                        known.contains(id) || exempt.contains(id),
                        "{} has no {kind} identifier {id}",
                        bundle.crawl.tag
                    );
                }
            }
        }
        assert!(bundles > 0, "no shipped bundles");
    }

    #[test]
    fn every_identifier_not_in_game_knowledge_is_still_missing_from_it() {
        let library = Library::shipped().unwrap();
        for bundle in library.bundles() {
            for (kind, named, known) in known(bundle) {
                for (id, why) in NOT_IN_GAME_KNOWLEDGE {
                    assert!(
                        !(named.contains(&id) && known.contains(id)),
                        "{} now has the {kind} identifier {id}, exempt as {why}",
                        bundle.crawl.tag
                    );
                }
            }
        }
    }
}
