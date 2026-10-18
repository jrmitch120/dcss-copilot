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
    /// The launchers and throwing missiles monsters shoot or throw, by their sub-type
    /// identifier, such as `WPN_SHORTBOW` or `MI_JAVELIN`.
    pub missile_weapons: BTreeMap<EnumId, MissileWeapon>,
    /// Every wand, by its `wand_type` identifier, such as `WAND_FLAME`.
    pub wands: BTreeMap<EnumId, Wand>,
    /// The brands a weapon may have, by their `brand_type` identifier, such as `SPWPN_FLAMING`,
    /// with how its name shows them. Brands crawl has retired or never names are left out.
    pub weapon_brands: BTreeMap<EnumId, WeaponBrand>,
    /// Every species this version defines, playable or not, in crawl's file order.
    pub species: Vec<Species>,
    /// Every dungeon feature crawl defines, by its `dungeon_feature_type` enum value: a map
    /// cell's webtiles `f`.
    pub features: BTreeMap<u16, Feature>,
    /// Every monster attitude, by its `mon_attitude_type` enum value: a monster's webtiles `att`.
    pub attitudes: BTreeMap<u8, EnumId>,
    /// Crawl's own ratings of how dangerous a monster is to the player, by their
    /// `mon_threat_level_type` enum value: a monster's webtiles `threat`.
    pub monster_threats: BTreeMap<u8, EnumId>,
    /// What the game shows that tells the player's Lighting.
    pub lighting: LightingSigns,
    /// Which of the player's items, spells and abilities are Ways Out, and the statuses that stop
    /// them.
    pub ways_out: WayOutSigns,
    /// Which variant of each ported crawl formula this Crawl Version uses.
    pub formulas: Formulas,
}

/// Which of the things the player carries, casts and invokes are Ways Out, and what the game
/// shows that stops them being used, as this Crawl Version has them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WayOutSigns {
    /// The kinds of item that are Ways Out, such as the scroll of teleportation.
    pub items: Vec<EscapeItem>,
    /// The spells that are Ways Out, such as Blink.
    pub spells: Vec<EscapeSpell>,
    /// The abilities that are Ways Out, such as Trog's Berserk.
    pub abilities: Vec<EscapeAbility>,
    /// The statuses that stop the player using things.
    pub stops: Vec<Stop>,
    /// The message crawl gives instead of the spell list when the player knows no spells.
    pub no_spells: String,
    /// The message crawl gives instead of the ability menu when the player has no abilities.
    pub no_abilities: String,
}

/// A kind of item that is a Way Out.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EscapeItem {
    /// Its class, such as `OBJ_SCROLLS`.
    pub class: EnumId,
    /// Its sub-type, such as `SCR_TELEPORTATION`.
    pub sub_type: EnumId,
    /// What it does for the player.
    pub effect: Relief,
    /// It moves the player by magic, which a dimension anchor stops.
    pub translocation: bool,
}

/// A spell that is a Way Out.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EscapeSpell {
    /// Its `spell_type` identifier, such as `SPELL_BLINK`.
    pub spell: EnumId,
    /// What it does for the player.
    pub effect: Relief,
    /// It moves the player by magic, which a dimension anchor stops.
    pub translocation: bool,
}

/// An ability that is a Way Out.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EscapeAbility {
    /// Its name, as the ability menu (`a`) lists it, such as `Berserk`.
    pub name: String,
    /// What it does for the player.
    pub effect: Relief,
    /// It is a god's power, invoked aloud, which silence stops (`talent::is_invocation`).
    pub invocation: bool,
    /// It moves the player by magic, which a dimension anchor stops.
    pub translocation: bool,
}

/// What a Way Out does for the player in trouble.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Relief {
    /// Restores hit points.
    Heals,
    /// Takes them away from danger at once, such as a blink.
    Escapes,
    /// Takes them away after a delay, such as a scroll of teleportation.
    EscapesLater,
    /// Keeps monsters from reaching or seeing them, such as fog or fear.
    Hinders,
    /// Makes them faster.
    Hastens,
    /// Makes them hit harder and take more, such as berserking.
    Strengthens,
}

/// A status that stops the player using some things while it lasts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Stop {
    /// The light or the words the status pane shows for it, such as `Conf` or `silenced`.
    pub status: String,
    /// What it stops.
    pub stops: Vec<Use>,
}

/// A way the player uses a Way Out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Use {
    /// Reading a scroll.
    Reading,
    /// Drinking a potion.
    Drinking,
    /// Casting a spell.
    Casting,
    /// Using an ability.
    UsingAbilities,
    /// Invoking a god's power aloud.
    Invoking,
    /// Being moved by magic, as a blink or a teleport moves them.
    Translocating,
}

/// What the game shows that tells how the light falls on the player, the Lighting crawl's
/// `player::backlit(false)` and `player::umbra()` decide, as this Crawl Version shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LightingSigns {
    /// The status lights of what backlights the player whatever halo or umbra they stand in,
    /// such as `Corona`: a corona, liquid fire at each heat, quad damage and harmful
    /// contamination.
    pub backlighting_lights: Vec<String>,
    /// The gods whose followers have a halo of their own while not under penance, as the game
    /// names them, such as `the Shining One`.
    pub halo_gods: Vec<String>,
    /// The status texts of a halo of the player's own, such as `in a heavenly storm`.
    pub halo_statuses: Vec<String>,
    /// What the name of an item that gives its wearer or wielder a halo holds, in lower case,
    /// such as `"eos"`.
    pub halo_gear: Vec<String>,
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

    /// The spell with this `spell_type` identifier, such as `SPELL_MAGIC_DART`, if this version
    /// defines one.
    #[must_use]
    pub fn spell_by_id(&self, id: &EnumId) -> Option<&Spell> {
        self.spells.values().find(|spell| spell.id == *id)
    }

    /// The dungeon feature whose `dungeon_feature_type` enum value is `value`.
    #[must_use]
    pub fn feature(&self, value: u16) -> Option<&Feature> {
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
    /// Which spell a monster picks to cast: `_choose_spell_to_cast` and `_find_spell_prospect`.
    pub monster_spell_choice: MonsterSpellChoice,
    /// The power a monster casts a spell with, and so its range: `mons_power_for_hd` and
    /// `calc_spell_range`.
    pub monster_spell_power: MonsterSpellPower,
    /// Which of a monster's spells it fires as one plain zap, set up from the zap's data:
    /// `mons_spell_beam`, `zappy` and `mons_cast`.
    pub monster_spell_beam: MonsterSpellBeam,
    /// Whether a beam gets past the player's shield and evasion: `bolt::misses_player`.
    pub beam_hit: BeamHit,
    /// A beam's damage after armour: `zap_damage`, `dice_def::roll` and `bolt::apply_AC`.
    pub beam_damage: BeamDamage,
    /// Whether a monster shoots or throws at the player this action, and the energy it takes:
    /// `handle_throw` and `mons_throw`.
    pub monster_fire: MonsterFire,
    /// A monster's missile's accuracy and damage before armour: `ranged_attack`,
    /// `attack::calc_to_hit` and `attack::calc_damage`.
    pub missile_attack: MissileAttack,
    /// Whether a monster zaps its wand this action, with what power, and the energy it takes:
    /// `_handle_wand` and `mons_wand_power`.
    pub monster_wand: MonsterWand,
    /// What a weapon's brand adds to a hit, and how it changes the weapon's damage and delay:
    /// `attack::apply_damage_brand`, `brand_adjust_weapon_damage` and `weapon_adjust_delay`.
    pub damage_brand: DamageBrand,
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
    /// As in crawl 0.34: a monster reaches two squares when one of its attacks' flavours does,
    /// such as `AF_REACH`, and three with `AF_RIFT`; from there it makes every attack whose
    /// flavour reaches.
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

/// The variants of a monster's choice of spell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MonsterSpellChoice {
    /// As in crawl 0.34.
    #[serde(rename = "crawl-0.34")]
    Crawl034,
}

/// The variants of a monster's spell power and range.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MonsterSpellPower {
    /// As in crawl 0.34.
    #[serde(rename = "crawl-0.34")]
    Crawl034,
}

/// The variants of which spells a monster fires as one plain zap.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MonsterSpellBeam {
    /// As in crawl 0.34.
    #[serde(rename = "crawl-0.34")]
    Crawl034,
}

/// The variants of a beam's roll against the player's shield and evasion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BeamHit {
    /// As in crawl 0.34.
    #[serde(rename = "crawl-0.34")]
    Crawl034,
}

/// The variants of a beam's damage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BeamDamage {
    /// As in crawl 0.34.
    #[serde(rename = "crawl-0.34")]
    Crawl034,
}

/// The variants of whether a monster shoots or throws.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MonsterFire {
    /// As in crawl 0.34.
    #[serde(rename = "crawl-0.34")]
    Crawl034,
}

/// The variants of a monster's missile attack.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MissileAttack {
    /// As in crawl 0.34.
    #[serde(rename = "crawl-0.34")]
    Crawl034,
}

/// The variants of a monster zapping a wand.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MonsterWand {
    /// As in crawl 0.34.
    #[serde(rename = "crawl-0.34")]
    Crawl034,
}

/// The variants of a weapon's brand.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DamageBrand {
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
        monster_spell_choice: MonsterSpellChoice::Crawl034,
        monster_spell_power: MonsterSpellPower::Crawl034,
        monster_spell_beam: MonsterSpellBeam::Crawl034,
        beam_hit: BeamHit::Crawl034,
        beam_damage: BeamDamage::Crawl034,
        monster_fire: MonsterFire::Crawl034,
        missile_attack: MissileAttack::Crawl034,
        monster_wand: MonsterWand::Crawl034,
        damage_brand: DamageBrand::Crawl034,
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
    /// It has crawl's `stationary` flag: it never moves.
    pub stationary: bool,
    /// It is crawl's firewood (`mons_class_is_firewood`): a harmless monster that never moves
    /// and gives no experience, such as a fungus or a plant, which the player sees as scenery.
    pub firewood: bool,
    /// It sees through an umbra, so the player's umbra doesn't spoil its aim: crawl's
    /// `monster::nightvision` for one of its kind carrying nothing. It is undead, serves
    /// Yredelemnul, or has an umbra of its own. One made with its own ghost, such as a pandemonium
    /// lord, may have an umbra its kind doesn't.
    pub nightvision: bool,
    /// It has crawl's `archer` flag, which makes its missiles more accurate and harder hitting.
    pub archer: bool,
    /// It has crawl's `prefer_ranged` flag: it shoots more often, even from beside its target.
    pub prefers_ranged: bool,
    /// It has crawl's `unique` flag: there is only one of it, such as Sigmund.
    pub unique: bool,
    /// How clever it is: crawl's `mon_intel_type`.
    pub intelligence: Intelligence,
    /// Its genus, as a `monster_type` identifier such as `MONS_DRACONIAN`: the kind crawl
    /// counts it as for rules that apply to a whole family (`mons_genus`).
    pub genus: EnumId,
    /// Which items it can use: crawl's `mon_itemuse_type`.
    pub item_use: ItemUse,
    /// The spells it casts. Monster abilities such as breath are spells here.
    pub spells: MonsterSpells,
    /// What it may be made with to shoot, throw or zap.
    pub gear: MonsterGear,
    /// How much energy each kind of action costs it, where 10 is a normal action.
    pub energy: Energy,
    /// The tile webtiles draws it with, as its image's path in crawl's `rltiles` without the
    /// extension, such as `mon/animals/jackal`: shipped with the app under `tiles/`. `None` when
    /// crawl's tile build makes the image rather than drawing it from one file.
    pub tile: Option<String>,
    /// Crawl's description text, verbatim, when it has one.
    pub description: Option<String>,
}

/// How clever a kind of monster is: crawl's `mon_intel_type`, as `dat/mons` sets `intelligence`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Intelligence {
    /// `I_BRAINLESS`.
    Brainless,
    /// `I_ANIMAL`.
    Animal,
    /// `I_HUMAN`: as clever as a person, which it needs to use a skill such as necromancy.
    Human,
}

/// Which items a kind of monster can use: crawl's `mon_itemuse_type`, as `dat/mons` sets `uses`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ItemUse {
    /// `MONUSE_NOTHING`: none.
    Nothing,
    /// `MONUSE_OPEN_DOORS`: none, though it opens doors.
    OpenDoors,
    /// `MONUSE_STARTING_EQUIPMENT`: what it is given when it is made, such as a weapon,
    /// missiles or a wand.
    StartingEquipment,
    /// `MONUSE_WEAPONS_ARMOUR`: what it is given, and weapons and armour it picks up.
    WeaponsArmour,
}

/// What a kind of monster may be made with to shoot, throw or zap: crawl's `give_item`
/// (`mon-gear.cc`), as it gives items to a monster made on D:3 or deeper, outside the Orcish
/// Mines and not in a band.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MonsterGear {
    /// The launcher or throwing missile it is made with, from [`Bundle::missile_weapons`]: crawl
    /// gives a monster at most one.
    pub missile: Odds,
    /// The wand it is made with, from [`Bundle::wands`], when it will zap it
    /// (`monster::likes_wand`).
    pub wand: Odds,
}

impl MonsterGear {
    /// Whether a monster of this kind may be made with a launcher, a throwing missile or a wand.
    #[must_use]
    pub fn may_shoot_or_zap(&self) -> bool {
        self.missile.is_possible() || self.wand.is_possible()
    }
}

/// Which of some items a monster is made with, by chance: each item `weight` times in `out_of`,
/// and none of them the rest of the time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Odds {
    /// The chances' common denominator.
    pub out_of: u32,
    /// The items, each with its share of `out_of`. Their weights sum to at most `out_of`.
    pub items: Vec<Weighted>,
}

impl Odds {
    /// Never any of them.
    #[must_use]
    pub fn none() -> Self {
        Self {
            out_of: 1,
            items: Vec::new(),
        }
    }

    /// Whether any item has a chance.
    #[must_use]
    pub fn is_possible(&self) -> bool {
        self.items.iter().any(|item| item.weight > 0)
    }
}

/// One item of [`Odds`] and its share.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Weighted {
    /// The item, by its sub-type identifier, such as `WPN_SHORTBOW`.
    pub item: EnumId,
    /// How many times in [`Odds::out_of`] it is the one.
    pub weight: u32,
}

/// A launcher or a throwing missile, as `Weapon_prop` and `Missile_prop` (`item-prop.cc`) define
/// it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MissileWeapon {
    /// Crawl's name for it, such as `shortbow`.
    pub name: String,
    /// Whether it shoots or is thrown.
    pub kind: MissileKind,
    /// The damage it adds to a monster's own: its `PWPN_DAMAGE`.
    pub damage: i32,
    /// The accuracy it adds: a launcher's `PWPN_HIT`, and none for a thrown missile.
    pub accuracy: i32,
}

/// How a [`MissileWeapon`] is used.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MissileKind {
    /// A launcher, which needs no ammunition: a weapon trained by `SK_RANGED_WEAPONS`.
    Launcher,
    /// A missile thrown by hand: an `OBJ_MISSILES` item.
    Thrown,
}

/// A wand, as crawl names it and the spell it zaps (`_wand_spells` in `spl-book.cc`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Wand {
    /// Crawl's name for it, such as `flame`.
    pub name: String,
    /// The spell it zaps, by its `spell_type` identifier.
    pub spell: EnumId,
}

/// How a weapon's name shows its brand, as `weapon_brand_desc` (`item-name.cc`) names a weapon
/// whose brand is known: `+0 shortbow of flaming`, or `+0 heavy shortbow` for a brand named
/// before the weapon.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WeaponBrand {
    /// The brand's words in a weapon's name, such as `flaming` or `heavy`.
    pub name: String,
    /// The words come before the weapon's name rather than after `of`.
    pub before: bool,
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

/// The spells a kind of monster casts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum MonsterSpells {
    /// The spells of its kind's spellbook (`mspell_list` in `mon-spell.h`), in crawl's order:
    /// empty for a monster with none.
    Book {
        /// The book's slots.
        spells: Vec<MonsterSpell>,
    },
    /// Each monster of its kind is given its own spells when it is made (`MST_GHOST`), such as a
    /// player ghost or an orc apostle, so the Game Knowledge doesn't know them.
    PerMonster,
}

/// One slot of a monster's spellbook, as `mon-spell.h` defines it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MonsterSpell {
    /// Its `spell_type` identifier, such as `SPELL_MAGIC_DART`.
    pub spell: EnumId,
    /// How often the monster picks it: the chance in 200 of each roll for a spell.
    pub frequency: u32,
    /// Its `mon_spell_slot_flag` identifiers, such as `MON_SPELL_WIZARD` or `MON_SPELL_BREATH`.
    pub flags: Vec<EnumId>,
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
    /// The player doesn't know which sub-type one of its items is until they identify it, as a
    /// scroll's is unknown until read (crawl's `item_type_has_ids`). Webtiles sends the
    /// sub-type all the same.
    pub kinds_hidden: bool,
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
    /// Its schools of magic, as `spschool` names them, such as `necromancy`.
    pub schools: Vec<String>,
    /// How far it reaches, or `None` for a spell without a range.
    pub range: Option<SpellRange>,
    /// The zap it fires, from `_spl_zaps` in `spl-zap.cc`, when it fires one.
    pub zap: Option<Zap>,
}

/// How far a spell reaches, from `spl-data.h`: between `min` and `max` squares, rising with
/// power up to `power_cap`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpellRange {
    /// Its range at no power.
    pub min: i32,
    /// Its range at `power_cap`.
    pub max: i32,
    /// The power past which it grows no stronger.
    pub power_cap: i32,
}

/// A zap: a bolt, beam or explosion as `zap_data` in `zap-data.h` defines it, with what it does
/// when a monster fires it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Zap {
    /// Its `zap_type` identifier, such as `ZAP_MAGIC_DART`.
    pub id: EnumId,
    /// Its `beam_type` identifier, such as `BEAM_FIRE`: what it is made of, which decides the
    /// resistance and armour that apply.
    pub flavour: EnumId,
    /// It is an enchantment, such as a hex, rather than something that deals damage.
    pub enchantment: bool,
    /// It passes through what it hits (`can_beam`).
    pub pierces: bool,
    /// It explodes where it lands (`is_explosion`).
    pub explodes: bool,
    /// The damage it deals when a monster fires it, by power; `None` when it deals none.
    pub monster_damage: Option<ZapDamage>,
    /// Its accuracy when a monster fires it, by power; `None` when it has none.
    pub monster_to_hit: Option<ZapToHit>,
}

/// A zap's damage dice at a power: crawl's `dicedef_calculator` and `calcdice_calculator`.
/// `dice` dice of `adder + power * per_power / per` sides, or, when `spread`, `dice` dice that
/// together reach that many (`calc_dice`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ZapDamage {
    /// How many dice.
    pub dice: i32,
    /// Sides, or the total reach when `spread`, at no power.
    pub adder: i32,
    /// What power adds, `per_power` for every `per`.
    pub per_power: i32,
    /// See `per_power`.
    pub per: i32,
    /// The dice share a total between them, as `calc_dice` splits it.
    pub spread: bool,
}

/// A zap's accuracy at a power: crawl's `tohit_calculator`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ZapToHit {
    /// It never misses: `AUTOMATIC_HIT`.
    Automatic,
    /// `adder + power * per_power / per`.
    ByPower {
        /// Accuracy at no power.
        adder: i32,
        /// What power adds, `per_power` for every `per`.
        per_power: i32,
        /// See `per_power`.
        per: i32,
    },
}

/// A dungeon feature, as `feat_defs` in `feature-data.h` defines it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Feature {
    /// Its `dungeon_feature_type` identifier, such as `DNGN_FLOOR`.
    pub id: EnumId,
    /// Crawl's name for it, such as `stone staircase leading down`.
    pub name: String,
    /// What it is to someone walking onto it.
    pub terrain: Terrain,
    /// Which way it takes the player to another level, when it is a staircase they can take:
    /// drawn as one, and neither sealed nor leading out of the dungeon.
    pub stairs: Option<StairDirection>,
}

/// What a dungeon feature is to someone walking onto it, from its `feat_defs` flags and
/// minimap colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Terrain {
    /// Walked onto as onto floor, such as floor, shallow water, an open door, a trap or stairs.
    Open,
    /// A closed door, which is opened and then walked through.
    ClosedDoor,
    /// Deep water, which only swimmers and fliers cross.
    DeepWater,
    /// Lava, which only fliers cross.
    Lava,
    /// Nothing walks onto it (`FFT_SOLID`), such as a wall, a tree, a statue or a sealed door.
    Solid,
}

/// Which way a staircase takes the player.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StairDirection {
    /// To a shallower level, or out of a branch: drawn `<`.
    Up,
    /// To a deeper level, or into a branch: drawn `>`.
    Down,
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
