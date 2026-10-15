//! Game Knowledge bundle schema, shared by the extractor and the core.
//!
//! A [`Bundle`] holds the static facts about DCSS for one Crawl Version, extracted from crawl's
//! source at a release tag. Webtiles sends monster and item types as crawl's C++ enum values,
//! which are renumbered between versions, so a bundle keys them by the values of its own version.
//! The [`Library`] holds the bundles the app ships.

use std::collections::BTreeMap;
use std::sync::{Arc, OnceLock};

use serde::{Deserialize, Serialize};

/// The bundles built into the app, as `(crawl tag, JSON)`. `docs/game-knowledge.md` says how to
/// build one.
const SHIPPED: &[(&str, &str)] = &[("0.34.1", include_str!("../bundles/0.34.1.json"))];

/// The Game Knowledge bundles the Advisor has, one per Crawl Version. Cheap to clone.
#[derive(Debug, Clone, Default)]
pub struct Library {
    bundles: Arc<BTreeMap<String, Bundle>>,
}

/// A shipped bundle that couldn't be read.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("the Game Knowledge bundle for crawl {tag} couldn't be read: {reason}")]
pub struct BundleError {
    /// The bundle's crawl tag.
    pub tag: String,
    /// Why it couldn't be read.
    pub reason: String,
}

impl Library {
    /// A library of these bundles.
    #[must_use]
    pub fn new(bundles: impl IntoIterator<Item = Bundle>) -> Self {
        Self {
            bundles: Arc::new(
                bundles
                    .into_iter()
                    .map(|bundle| (bundle.crawl.tag.clone(), bundle))
                    .collect(),
            ),
        }
    }

    /// The bundles built into the app, read once.
    ///
    /// # Errors
    /// If a shipped bundle doesn't read, which its tests rule out for a released build.
    pub fn shipped() -> Result<Self, BundleError> {
        static SHIPPED_LIBRARY: OnceLock<Result<Library, BundleError>> = OnceLock::new();
        SHIPPED_LIBRARY
            .get_or_init(|| {
                let bundles = SHIPPED
                    .iter()
                    .map(|&(tag, json)| {
                        serde_json::from_str(json).map_err(|error| BundleError {
                            tag: tag.to_owned(),
                            reason: error.to_string(),
                        })
                    })
                    .collect::<Result<Vec<Bundle>, _>>()?;
                Ok(Self::new(bundles))
            })
            .clone()
    }

    /// The bundle for exactly this Crawl Version, such as `0.34.1`, if the library has one.
    #[must_use]
    pub fn for_version(&self, crawl_version: &str) -> Option<&Bundle> {
        self.bundles.get(crawl_version)
    }
}

/// Game Knowledge for one Crawl Version.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Bundle {
    /// The crawl source this bundle was extracted from.
    pub crawl: CrawlSource,
    /// Every monster with a definition, by its `monster_type` enum value.
    pub monsters: BTreeMap<u16, Monster>,
    /// Every item class, by its `object_class_type` enum value: an item's webtiles `base_type`.
    pub item_classes: BTreeMap<u8, ItemClass>,
    /// Every spell with a definition, by its `spell_type` enum value.
    pub spells: BTreeMap<u16, Spell>,
    /// Every species this version defines, playable or not, in crawl's file order.
    pub species: Vec<Species>,
    /// Every dungeon feature, by its `dungeon_feature_type` enum value: a map cell's webtiles `f`.
    pub features: BTreeMap<u16, EnumId>,
    /// Every monster attitude, by its `mon_attitude_type` enum value: a monster's webtiles `att`.
    pub attitudes: BTreeMap<u8, EnumId>,
    /// Crawl's own ratings of how dangerous a monster is to the player, by their
    /// `mon_threat_level_type` enum value: a monster's webtiles `threat`.
    pub monster_threats: BTreeMap<u8, EnumId>,
    /// Which variant of each ported crawl formula this Crawl Version uses.
    pub formulas: Formulas,
}

impl Bundle {
    /// The monster whose `monster_type` enum value is `value`, if this version defines one.
    #[must_use]
    pub fn monster(&self, value: u16) -> Option<&Monster> {
        self.monsters.get(&value)
    }

    /// The monster with this `monster_type` identifier, such as `MONS_GOBLIN`, if this version
    /// defines one.
    #[must_use]
    pub fn monster_by_id(&self, id: &EnumId) -> Option<&Monster> {
        self.monsters.values().find(|monster| monster.id == *id)
    }

    /// The dungeon feature whose `dungeon_feature_type` enum value is `value`.
    #[must_use]
    pub fn feature(&self, value: u16) -> Option<&EnumId> {
        self.features.get(&value)
    }

    /// The monster attitude whose `mon_attitude_type` enum value is `value`.
    #[must_use]
    pub fn attitude(&self, value: u8) -> Option<&EnumId> {
        self.attitudes.get(&value)
    }

    /// The monster threat rating whose `mon_threat_level_type` enum value is `value`.
    #[must_use]
    pub fn monster_threat(&self, value: u8) -> Option<&EnumId> {
        self.monster_threats.get(&value)
    }

    /// What an item with this `base_type` and `sub_type` is, if this version has the class. The
    /// kind has no sub-type when the bundle doesn't know the class's sub-types or this value.
    #[must_use]
    pub fn item(&self, base_type: u8, sub_type: Option<u16>) -> Option<ItemKind> {
        let class = self.item_classes.get(&base_type)?;
        Some(ItemKind {
            class: class.id.clone(),
            sub_type: sub_type
                .and_then(|value| class.sub_types.get(&value))
                .map(|sub_type| sub_type.id.clone()),
        })
    }
}

/// Which variant of each ported crawl formula a Crawl Version uses. The engine picks its code by
/// these names and never compares version strings. A variant is named after the first crawl
/// release whose source it was ported from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Formulas {
    /// A monster's melee accuracy: `mon_to_hit_base` and `attack::calc_to_hit`.
    pub monster_to_hit: MonsterToHit,
    /// Whether an attack beats the defender's evasion: `attack::test_hit`.
    pub evasion_roll: EvasionRoll,
    /// A monster's melee damage before armour: `attack::calc_damage`.
    pub monster_damage: MonsterDamage,
    /// What the player's armour saves: `actor::apply_ac` and `player::gdr_perc`.
    pub armour: Armour,
    /// Whether the player's shield blocks a monster's attack: `attack::attack_shield_blocked`.
    pub shield_block: ShieldBlock,
    /// How far a monster's melee attack reaches: `melee_attack::can_reach`.
    pub monster_reach: MonsterReach,
    /// How often a monster acts in one player turn: `_monster_add_energy` and the energy
    /// threshold in `monster::has_action_energy`.
    pub monster_energy: MonsterEnergy,
}

/// The variants of a monster's melee accuracy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MonsterToHit {
    /// As in crawl 0.34.
    #[serde(rename = "crawl-0.34")]
    Crawl034,
}

/// The variants of the roll against evasion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EvasionRoll {
    /// As in crawl 0.34.
    #[serde(rename = "crawl-0.34")]
    Crawl034,
}

/// The variants of a monster's melee damage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MonsterDamage {
    /// As in crawl 0.34.
    #[serde(rename = "crawl-0.34")]
    Crawl034,
}

/// The variants of what armour saves.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Armour {
    /// As in crawl 0.34.
    #[serde(rename = "crawl-0.34")]
    Crawl034,
}

/// The variants of the shield block.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShieldBlock {
    /// As in crawl 0.34.
    #[serde(rename = "crawl-0.34")]
    Crawl034,
}

/// The variants of a monster's melee reach.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MonsterReach {
    /// As in crawl 0.34: an attack reaches two squares when its flavour does, such as
    /// `AF_REACH`, and `AF_RIFT` reaches three.
    #[serde(rename = "crawl-0.34")]
    Crawl034,
}

/// The variants of monster energy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MonsterEnergy {
    /// As in crawl 0.34.
    #[serde(rename = "crawl-0.34")]
    Crawl034,
}

impl Formulas {
    /// The variants crawl 0.34 uses.
    pub const CRAWL_0_34: Self = Self {
        monster_to_hit: MonsterToHit::Crawl034,
        evasion_roll: EvasionRoll::Crawl034,
        monster_damage: MonsterDamage::Crawl034,
        armour: Armour::Crawl034,
        shield_block: ShieldBlock::Crawl034,
        monster_reach: MonsterReach::Crawl034,
        monster_energy: MonsterEnergy::Crawl034,
    };
}

/// Where a bundle came from. Its corresponding source is the extractor plus this crawl tag.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CrawlSource {
    /// The crawl release tag, such as `0.34.1`, which is also the Crawl Version it describes.
    pub tag: String,
    /// The crawl commit the tag points at.
    pub commit: String,
    /// Crawl's copyright and license, which the extracted data and text are under.
    pub license: String,
}

/// A crawl enum identifier, such as `MONS_GOBLIN` or `POT_CURING`. Identifiers stay the same when
/// crawl renumbers its enums, so they name a thing across Crawl Versions.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct EnumId(pub String);

/// A monster's definition, from `dat/mons/*.yaml` and `dat/descript/monsters.txt`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Monster {
    /// Its `monster_type` identifier, such as `MONS_GOBLIN`.
    pub id: EnumId,
    /// Its name, as crawl names one of its kind.
    pub name: String,
    /// The character it is drawn with on the console map.
    pub glyph: String,
    /// Hit dice.
    pub hd: u32,
    /// Ten times its average hit points.
    pub hp_10x: u32,
    /// Armour class.
    pub ac: i32,
    /// Evasion.
    pub ev: i32,
    /// Willpower.
    pub will: Will,
    /// Speed, where 10 is the player's normal speed.
    pub speed: u32,
    /// Its melee attacks, in crawl's order: everything it does in one melee action. Empty for a
    /// monster that never attacks in melee.
    pub attacks: Vec<MonsterAttack>,
    /// It has crawl's `fighter` flag, which makes its attacks more accurate.
    pub fighter: bool,
    /// How much energy each kind of action costs it, where 10 is a normal action.
    pub energy: Energy,
    /// The tile webtiles draws it with, as its image's path in crawl's `rltiles` without the
    /// extension, such as `mon/animals/jackal`: shipped with the app under `tiles/`. `None` when
    /// crawl's tile build makes the image rather than drawing it from one file.
    pub tile: Option<String>,
    /// Crawl's description text, verbatim, when it has one.
    pub description: Option<String>,
}

/// One of a monster's melee attacks, as `dat/mons` defines it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MonsterAttack {
    /// Its `attack_type` identifier, such as `AT_BITE`.
    pub kind: EnumId,
    /// Its `attack_flavour` identifier, such as `AF_POISON`, or `AF_PLAIN` for none.
    pub flavour: EnumId,
    /// The most damage it deals before armour, its "damage" in `dat/mons`.
    pub damage: u32,
}

/// What each kind of action costs a monster, in energy: crawl's `mon_energy_usage`. A monster
/// gains its speed in energy over a normal player turn and acts while it has enough.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Energy {
    /// Moving on land.
    pub walk: u32,
    /// Moving through water.
    pub swim: u32,
    /// A melee attack, with all of its attacks.
    pub attack: u32,
    /// Firing a missile.
    pub missile: u32,
    /// Casting a spell.
    pub spell: u32,
}

/// A monster's willpower: how hard it is to affect with hexes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Will {
    /// Resists hexes up to this value.
    Value(u32),
    /// This much willpower per hit die, for monsters whose hit dice vary.
    PerHd(u32),
    /// Can't be affected by hexes at all.
    Invulnerable,
}

/// An item class and the sub-types the bundle knows for it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemClass {
    /// Its `object_class_type` identifier, such as `OBJ_POTIONS`.
    pub id: EnumId,
    /// Its sub-types, by enum value: an item's webtiles `sub_type`. Empty for a class the
    /// extractor has no name table for.
    pub sub_types: BTreeMap<u16, ItemSubType>,
}

/// One kind of item within a class, such as the potion of curing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemSubType {
    /// Its identifier in the class's sub-type enum, such as `POT_CURING`.
    pub id: EnumId,
    /// Crawl's name for the sub-type, such as `curing` for a potion or `long sword` for a weapon.
    pub name: String,
}

/// What an item is, decoded through a bundle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemKind {
    /// Its class, such as `OBJ_POTIONS`.
    pub class: EnumId,
    /// Its sub-type, such as `POT_CURING`, when the bundle knows it.
    pub sub_type: Option<EnumId>,
}

/// A spell's definition, from `spl-data.h`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Spell {
    /// Its `spell_type` identifier, such as `SPELL_MAGIC_DART`.
    pub id: EnumId,
    /// Its name, such as `Magic Dart`.
    pub name: String,
}

/// A species' definition, from `dat/species/*.yaml`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Species {
    /// Its `species_type` identifier, such as `SP_HUMAN`.
    pub id: EnumId,
    /// Its name, as webtiles reports a character's species.
    pub name: String,
}
