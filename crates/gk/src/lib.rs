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

    /// Every bundle in the library, by Crawl Version.
    pub fn bundles(&self) -> impl Iterator<Item = &Bundle> {
        self.bundles.values()
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
    /// The melee weapons monsters and the player fight with, by their `weapon_type` identifier,
    /// such as `WPN_SPEAR`.
    pub melee_weapons: BTreeMap<EnumId, MeleeWeapon>,
    /// Every wand, by its `wand_type` identifier, such as `WAND_FLAME`.
    pub wands: BTreeMap<EnumId, Wand>,
    /// The brands a weapon may have, by their `brand_type` identifier, such as `SPWPN_FLAMING`,
    /// with how its name shows them. Brands crawl has retired or never names are left out.
    pub weapon_brands: BTreeMap<EnumId, WeaponBrand>,
    /// Every species this version defines, playable or not, in crawl's file order.
    pub species: Vec<Species>,
    /// Every skill, by its `skill_type` identifier, such as `SK_MACES_FLAILS`, with its name as
    /// the skills screen shows it, such as `Maces & Flails`.
    pub skills: BTreeMap<EnumId, String>,
    /// Every dungeon feature crawl defines, by its `dungeon_feature_type` enum value: a map
    /// cell's webtiles `f`.
    pub features: BTreeMap<u16, Feature>,
    /// Every monster attitude, by its `mon_attitude_type` enum value: a monster's webtiles `att`.
    pub attitudes: BTreeMap<u8, EnumId>,
    /// Crawl's own ratings of how dangerous a monster is to the player, by their
    /// `mon_threat_level_type` enum value: a monster's webtiles `threat`.
    pub monster_threats: BTreeMap<u8, EnumId>,
    /// What the game can wait for the player to do, by their `mouse_mode` enum value: webtiles'
    /// `input_mode`.
    pub mouse_modes: BTreeMap<u8, EnumId>,
    /// Every branch of the dungeon, in `branch_type` order, with the monsters crawl makes on its
    /// levels at random.
    pub branches: Vec<Branch>,
    /// What the game shows of the statuses that put the player in danger.
    pub danger: DangerSigns,
    /// What the game shows as the player learns which kind of item they carry.
    pub identification: IdentificationSigns,
    /// What the game shows that tells the player's Lighting.
    pub lighting: LightingSigns,
    /// What the game shows that tells the player's holiness and whether they worship a good god.
    pub holiness: HolinessSigns,
    /// Which of the player's items, spells and abilities are Ways Out, and the statuses that stop
    /// them.
    pub ways_out: WayOutSigns,
    /// What the game shows that tells how fast the player moves.
    pub speed: SpeedSigns,
    /// Which variant of each ported crawl formula this Crawl Version uses.
    pub formulas: Formulas,
}

/// What the game shows as the player learns which kind an item they carry is, beyond what the
/// item itself shows, as this Crawl Version shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IdentificationSigns {
    /// The message crawl gives when the player sees an identified item of a kind they carry
    /// unidentified (`seen_item`), as crawl's format for it, with `%s` for each part it fills
    /// in, such as `You learned that %s %s actually %s.`. Its last part is the carried item's
    /// name as the player now knows it, such as `a scroll of teleportation`.
    pub kind_learned: String,
}

impl IdentificationSigns {
    /// The name of the item `message` says the player has learned the kind of, such as
    /// `a scroll of teleportation`, if it is that message.
    #[must_use]
    pub fn learned_name<'m>(&self, message: &'m str) -> Option<&'m str> {
        let (before, after) = self.kind_learned.rsplit_once("%s")?;
        let rest = message.strip_suffix(after)?;
        match before.rsplit_once("%s") {
            None => rest.strip_prefix(before),
            Some((earlier, lead)) => {
                let opening = earlier
                    .split_once("%s")
                    .map_or(earlier, |(opening, _)| opening);
                // The parts before the name are names too, so the name follows the last `lead`.
                let (_, name) = rest.strip_prefix(opening)?.rsplit_once(lead)?;
                Some(name)
            }
        }
    }
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
    /// The colour the ability menu draws an ability in when crawl's checks say it can't be used
    /// now (`COL_INAPPLICABLE`), as crawl's `COLOURS` numbers it.
    pub inapplicable_colour: u8,
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
    /// The statuses that stop this ability in particular, besides those [`WayOutSigns::stops`]
    /// says stop every ability, as the status pane shows them: each one's light, such as
    /// `-Berserk`, or its words when it has none.
    pub stopped_by: Vec<String>,
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

/// What the game shows that tells the player's holiness, as crawl's `player::holiness`,
/// `player::undead_state` and `player::undead_or_demonic` decide it, and whether they worship a
/// good god (`is_good_god`), as this Crawl Version has them. A species' own undeadness is on its
/// [`Species`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HolinessSigns {
    /// The species crawl makes nonliving, by `species_type` identifier, such as `SP_GARGOYLE`
    /// (`species::is_nonliving`).
    pub nonliving_species: Vec<EnumId>,
    /// The species crawl counts as demonic, which holy wrath hurts and foul flame spares, by
    /// `species_type` identifier, such as `SP_DEMONSPAWN` (`player::undead_or_demonic`).
    pub demonic_species: Vec<EnumId>,
    /// The good gods, as the game names them, such as `the Shining One`.
    pub good_gods: Vec<String>,
    /// Every form the player can take, by the status light the status pane shows it with.
    pub forms: Vec<Form>,
    /// The status lights of being petrified, which makes the player nonliving, such as `Stone`.
    pub petrified_lights: Vec<String>,
}

/// A form the player can take, from `dat/forms/*.yaml`, with what it makes of their holiness.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Form {
    /// Its `transformation` identifier, such as `death`.
    pub id: String,
    /// The status light the status pane shows while the player is in it, its `short_name`, such
    /// as `Death`.
    pub light: String,
    /// The holiness it gives the player.
    pub holiness: FormHoliness,
    /// How undead it makes the player, whatever their species: crawl's `player::undead_state`,
    /// which names its forms in code.
    pub undead: Undeadness,
    /// Its resistance to negative energy, `resists: neg` in `dat/forms`, from 0 to 3.
    pub negative: i32,
    /// Crawl halves the torment a player in it takes, as the form leaves them only partly
    /// petrified or undead: `torment_player`, which names its forms in code.
    pub halves_torment: bool,
    /// The body it gives the player, as their moving goes.
    pub body: FormBody,
}

/// The body a form gives the player, or the one they have in none, as `player_movement_speed`
/// reads it, from `dat/forms`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FormBody {
    /// The time a move takes in it before anything else changes it, where 10 is a standard move:
    /// its `move_speed`, 10 unless set (`Form::base_move_speed`).
    pub move_speed: i32,
    /// The size of body it gives, its `size`: `None` for the species' own (`SIZE_CHARACTER`).
    pub size: Option<BodySize>,
    /// Whether it lets the player swim, its `can_swim`: `None` to leave it to the species.
    pub swims: Option<bool>,
    /// It changes the body so much that mutations of its anatomy stop working, its
    /// `changes_anatomy` (`form_changes_anatomy`, which `mut_is_compatible` reads).
    pub changes_anatomy: bool,
}

/// The size of a body: crawl's `size_type`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BodySize {
    /// `SIZE_TINY`.
    Tiny,
    /// `SIZE_LITTLE`.
    Little,
    /// `SIZE_SMALL`.
    Small,
    /// `SIZE_MEDIUM`.
    Medium,
    /// `SIZE_LARGE`.
    Large,
    /// `SIZE_BIG`.
    Big,
    /// `SIZE_GIANT`.
    Giant,
}

/// A branch of the dungeon, from `branches` in `branch-data.h`, with the monsters crawl makes at
/// random on its levels, from `population` in `mon-pick-data.h`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Branch {
    /// Its `branch_type` identifier, such as `BRANCH_DUNGEON`.
    pub id: EnumId,
    /// Its short name, such as `Dungeon` or `Lair`, as the game names the place it is.
    pub name: String,
    /// The shallowest of its levels crawl makes each monster on at random, by `monster_type`
    /// identifier, from 1 for its first level. Monsters crawl only places, such as uniques, and
    /// those it makes at random only in other branches, aren't here.
    pub shallowest: BTreeMap<EnumId, u32>,
}

/// What the game shows of the statuses that put the player in danger, as this Crawl Version
/// shows them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DangerSigns {
    /// The words the status pane starts the description of poison that will kill the player
    /// with, such as `lethally poisoned` (`_describe_poison`).
    pub lethal_poison: String,
    /// The status lights of turning to stone or being stone, such as `Petr` and `Stone`.
    pub petrification_lights: Vec<String>,
    /// The status lights of burning in liquid fire, the sticky flame, such as `Fire`.
    pub liquid_fire_lights: Vec<String>,
    /// The status lights of confusion, which may stumble the player into deep water or lava,
    /// such as `Conf`.
    pub confusion_lights: Vec<String>,
}

/// What the game shows that tells how fast the player moves, as `player_speed` and
/// `player_movement_speed` decide it, as this Crawl Version has them. A species' own size,
/// swimming and mutations are on its [`Species`], and a form's body on its [`Form`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpeedSigns {
    /// Every status light the status pane may show, with what it tells of the player's speed:
    /// those it lights for a status that changes it, and every other it may show, which tell
    /// nothing. Forms are among [`HolinessSigns::forms`] instead. A light crawl builds as it
    /// draws it, such as `Torch (3)`, is listed by the words before what changes, `Torch`.
    pub lights: Vec<SpeedLight>,
    /// The body the player has in no form: the form `none`.
    pub unformed: FormBody,
    /// The mutations that change how fast the player moves.
    pub mutations: Vec<SpeedMutation>,
    /// The banes that change how fast the player moves.
    pub banes: Vec<SpeedBane>,
    /// The gods whose followers move slowly for their piety and are never hastened by a
    /// berserker rage, as the game names them, such as `Cheibriados` (`passive_t::slowed` and
    /// `passive_t::no_haste`).
    pub slowing_gods: Vec<String>,
    /// The gods whose followers walk on water once their piety reaches a rank, so it doesn't
    /// slow them (`passive_t::water_walk`).
    pub water_walking_gods: Vec<GodRank>,
    /// What the name of armour that slows its wearer holds, in lower case: `"ponderousness"`
    /// (`SPARM_PONDEROUSNESS`).
    pub ponderous_gear: Vec<String>,
    /// What the name of armour that quickens its wearer holds, in lower case: the lightning
    /// scales (`UNRAND_LIGHTNING_SCALES`).
    pub quickening_gear: Vec<String>,
}

/// A status light, and what it tells of the player's speed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpeedLight {
    /// The light, such as `Fast`.
    pub light: String,
    /// What the status it lights does to the player's speed, `None` for nothing.
    pub status: Option<SpeedStatus>,
}

/// A status that changes how fast the player moves, as the status pane lights it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SpeedStatus {
    /// Hasted (`DUR_HASTE`).
    Hasted,
    /// Slowed, or a stat at zero (`DUR_SLOW`, `have_stat_zero`).
    Slowed,
    /// Both hasted and slowed.
    HastedAndSlowed,
    /// In a berserker rage (`DUR_BERSERK`).
    Berserk,
    /// Swift (`DUR_SWIFTNESS` with `ATTR_SWIFTNESS` above zero).
    Swift,
    /// Sluggish as swiftness wears off (`DUR_SWIFTNESS` with `ATTR_SWIFTNESS` below zero).
    Unswift,
    /// Partly encased in ice (`DUR_FROZEN`).
    Frozen,
    /// Turning to stone (`DUR_PETRIFYING`).
    Petrifying,
    /// On liquefied ground, or liquefying it (`STATUS_LIQUEFIED`).
    SlowedMovement,
    /// Flying, so water doesn't slow them (`player::airborne`).
    Flying,
    /// Unable to act, asleep, paralysed or petrified (`player::helpless`).
    Helpless,
}

/// A god, from the rank of piety the status pane shows as stars.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GodRank {
    /// The god, as the game names them, such as `Beogh`.
    pub god: String,
    /// The rank, from 1 to 6 stars.
    pub piety_rank: u32,
}

/// A mutation that changes how fast the player moves, from `mut_data` in `mutation-data.h`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpeedMutation {
    /// Its `mutation_type` identifier, such as `MUT_FAST`.
    pub id: EnumId,
    /// How the mutations screen (`A`) describes it at each of its levels, first level first
    /// (`have`).
    pub shown: Vec<String>,
    /// It is a mutation of the player's anatomy (`mutflag::anatomy`), which stops working in a
    /// form that changes it.
    pub anatomy: bool,
    /// The messages the game gives as the player gains or loses a level of it (`gain` and
    /// `lose`).
    pub messages: Vec<String>,
}

/// A bane that changes how fast the player moves, from `bane_data` in `bane-data.h`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpeedBane {
    /// Its `bane_type` identifier, such as `BANE_LETHARGY`.
    pub id: EnumId,
    /// How the mutations screen (`A`) describes it (`description`).
    pub shown: String,
    /// The messages the game gives as the player gains it, it grows stronger, or it is lifted
    /// (`add_bane` and `remove_bane`).
    pub messages: Vec<String>,
}

/// The holiness a form gives the player: its `holiness` in `dat/forms`, or, for a form
/// `player::holiness` names in code, what that gives.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FormHoliness {
    /// None of its own (`MH_NONE`): the species' holiness stands.
    Species,
    /// `MH_NATURAL`.
    Natural,
    /// `MH_UNDEAD`.
    Undead,
    /// `MH_DEMONIC`.
    Demonic,
    /// `MH_NONLIVING`.
    Nonliving,
    /// `MH_PLANT`.
    Plant,
    /// `MH_HOLY`.
    Holy,
    /// The species' holiness with undead added, as crawl gives a vampire or a bat swarm.
    AddsUndead,
}

/// How undead a species or form makes the player: crawl's `undead_state_type`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Undeadness {
    /// `US_ALIVE`.
    #[default]
    Alive,
    /// `US_SEMI_UNDEAD`: undead only in part, as a vampire is.
    SemiUndead,
    /// `US_UNDEAD`: lifeless, as a mummy is.
    Undead,
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

    /// The species webtiles names `name`, such as `Mummy`, if this version defines one.
    #[must_use]
    pub fn species_named(&self, name: &str) -> Option<&Species> {
        self.species.iter().find(|species| species.name == name)
    }

    /// The dungeon feature with this `dungeon_feature_type` identifier, such as `DNGN_LAVA`, if
    /// this version defines one.
    #[must_use]
    pub fn feature_by_id(&self, id: &EnumId) -> Option<&Feature> {
        self.features.values().find(|feature| feature.id == *id)
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

    /// What the game waits for the player to do when webtiles' `input_mode` is `value`.
    #[must_use]
    pub fn mouse_mode(&self, value: u8) -> Option<&EnumId> {
        self.mouse_modes.get(&value)
    }

    /// The branch the game names `place`, as the status pane names where the player is: its
    /// short name, which crawl gives an article for a branch one level deep that the dungeon
    /// doesn't lead to by stairs, such as `a Bazaar` or `The Abyss` (`tileweb.cc`).
    #[must_use]
    pub fn branch_named(&self, place: &str) -> Option<&Branch> {
        let name = ["a ", "an ", "The "]
            .iter()
            .find_map(|article| place.strip_prefix(article))
            .filter(|name| self.branches.iter().any(|branch| branch.name == *name))
            .unwrap_or(place);
        self.branches.iter().find(|branch| branch.name == name)
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
    /// How far a monster's melee attack reaches: `monster::reach_range` and
    /// `melee_attack::can_reach`.
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
    /// Which of a monster's melee attacks it makes with which of its weapons, whether it makes
    /// one only a weapon makes, and the weapon that sets how long a melee action takes:
    /// `monster::weapon`, `attack::init_attack` and `monster::attack_delay`.
    pub monster_weapon: MonsterWeapon,
    /// How the player's resistances, holiness and god change the damage a beam deals them, and
    /// what an elemental or draining brand adds: `check_your_resists`, `resist_adjust_damage`
    /// and the player's `res_` functions.
    pub resist_damage: ResistDamage,
    /// Whether a monster its foe watches steps toward them this action: `_fungal_move_check`.
    pub monster_step: MonsterStep,
    /// Where a monster that keeps its range steps, rather than into its foe:
    /// `_set_firing_pos` and `monster::threat_range`.
    pub firing_position: FiringPosition,
    /// How a batty monster flits after it attacks or casts, and for how long, before it hunts
    /// its foe again: `_handle_battiness`, `set_random_target`, `_find_best_step` and
    /// `BATTY_TURNS_KEY` in `_post_monster_move`.
    pub batty_flight: BattyFlight,
    /// What the clouds a monster's own casts leave change before the player's next action:
    /// whether it sees the player through opaque ones (`opacity_default`, `monster::near_foe`),
    /// and whether a cloud spell's tracer finds a player its cloud must spread to
    /// (`targeter_cloud::set_aim`).
    pub monster_clouds: MonsterClouds,
    /// The hit points torment takes from the player: `torment_player` and `player::res_torment`.
    pub torment: Torment,
    /// The player's melee, launcher and thrown attacks on a monster: their accuracy, damage
    /// after the monster's armour and how long each takes: `attack::calc_to_hit`,
    /// `attack::calc_damage` and `player::attack_delay`.
    pub player_attack: PlayerAttack,
    /// A beam the player fires at a monster, from a spell or a wand: its dice and accuracy at a
    /// power, the roll against the monster's evasion, its armour and its resistances:
    /// `zappy`, `bolt::affect_monster` and `mons_adjust_flavoured`.
    pub player_beam: PlayerBeam,
    /// The power the player zaps a wand with: `wand_power`.
    pub wand_power: WandPower,
    /// A monster's hit points as it is made: `hit_points`.
    pub monster_hit_points: MonsterHitPoints,
    /// The time a move takes the player: `player_speed`, `player_movement_speed` and the delay
    /// `move_player_action` gives a move.
    pub player_move: PlayerMove,
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
    /// such as `AF_REACH`, and three with `AF_RIFT`, or as far as the weapon of its first attack
    /// reaches; from there it makes every attack whose flavour reaches, and every hit made with
    /// a weapon that reaches that far.
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

/// The variants of how the player's resistances change the damage they take.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResistDamage {
    /// As in crawl 0.34.
    #[serde(rename = "crawl-0.34")]
    Crawl034,
}

/// The variants of whether a watched monster steps toward its foe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MonsterStep {
    /// As in crawl 0.34.
    #[serde(rename = "crawl-0.34")]
    Crawl034,
}

/// The variants of where a monster that keeps its range steps.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FiringPosition {
    /// As in crawl 0.34.
    #[serde(rename = "crawl-0.34")]
    Crawl034,
}

/// The variants of how a batty monster flits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BattyFlight {
    /// As in crawl 0.34.
    #[serde(rename = "crawl-0.34")]
    Crawl034,
}

/// The variants of what the clouds a monster's own casts leave change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MonsterClouds {
    /// As in crawl 0.34.
    #[serde(rename = "crawl-0.34")]
    Crawl034,
}

/// The variants of the hit points torment takes from the player.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Torment {
    /// As in crawl 0.34.
    #[serde(rename = "crawl-0.34")]
    Crawl034,
}

/// The variants of which weapon a monster attacks with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MonsterWeapon {
    /// As in crawl 0.34.
    #[serde(rename = "crawl-0.34")]
    Crawl034,
}

/// The variants of the player's melee, launcher and thrown attacks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlayerAttack {
    /// As in crawl 0.34.
    #[serde(rename = "crawl-0.34")]
    Crawl034,
}

/// The variants of a beam the player fires at a monster.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlayerBeam {
    /// As in crawl 0.34.
    #[serde(rename = "crawl-0.34")]
    Crawl034,
}

/// The variants of the power the player zaps a wand with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WandPower {
    /// As in crawl 0.34.
    #[serde(rename = "crawl-0.34")]
    Crawl034,
}

/// The variants of a monster's hit points as it is made.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MonsterHitPoints {
    /// As in crawl 0.34.
    #[serde(rename = "crawl-0.34")]
    Crawl034,
}

/// The variants of the time a move takes the player.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlayerMove {
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
        monster_weapon: MonsterWeapon::Crawl034,
        resist_damage: ResistDamage::Crawl034,
        monster_step: MonsterStep::Crawl034,
        firing_position: FiringPosition::Crawl034,
        batty_flight: BattyFlight::Crawl034,
        monster_clouds: MonsterClouds::Crawl034,
        torment: Torment::Crawl034,
        player_attack: PlayerAttack::Crawl034,
        player_beam: PlayerBeam::Crawl034,
        wand_power: WandPower::Crawl034,
        monster_hit_points: MonsterHitPoints::Crawl034,
        player_move: PlayerMove::Crawl034,
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

/// Declares a group of crawl identifiers a crate compares Game Knowledge against, as a module of
/// constants, and lists them all in the module's `ALL` for that crate's tests to check against
/// every shipped bundle: a typo, or crawl renaming one, would otherwise compile and quietly never
/// match.
///
/// ```
/// copilot_gk::crawl_identifiers! {
///     /// `monster_type` identifiers.
///     monster_types {
///         GOBLIN = "MONS_GOBLIN";
///     }
/// }
/// assert_eq!(monster_types::GOBLIN, "MONS_GOBLIN");
/// ```
#[macro_export]
macro_rules! crawl_identifiers {
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
    /// What it is, by its `mon_holy_type` identifiers, such as `MH_UNDEAD`: `MH_NATURAL` unless
    /// `dat/mons` says otherwise.
    pub holiness: Vec<EnumId>,
    /// Its resistances as `dat/mons` gives them, before what its holiness adds.
    pub resists: MonsterResists,
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
    /// It has crawl's `two_weapons` flag: it wields a second melee weapon in its off hand.
    pub two_weapons: bool,
    /// It has crawl's `unique` flag: there is only one of it, such as Sigmund.
    pub unique: bool,
    /// It has crawl's `batty` flag: it flits away after it attacks or casts (`_handle_battiness`).
    pub batty: bool,
    /// It has crawl's `maintain_range` flag: it holds a firing position away from its foe
    /// rather than close in (`_decide_monster_firing_position`).
    pub keeps_range: bool,
    /// It has crawl's `flies` flag, so it crosses deep water and lava.
    pub flies: bool,
    /// The terrain it can stand on, as `dat/mons` sets `habitat`, before its flying is added.
    pub habitat: Habitat,
    /// It is a tentacle or a tentacle segment, which moves with its parent
    /// (`mons_is_tentacle_or_tentacle_segment`), such as the snaplasher vine.
    pub tentacle: bool,
    /// `handle_monster_move` acts or moves for it with code of its own rather than by walking,
    /// fighting and casting, such as a prism or a hellfire mortar; or it is a projectile, such as
    /// an orb of destruction, which flies by its own code (`iood_act`).
    pub own_actions: bool,
    /// When it steps toward a foe that watches it (`_fungal_move_check`).
    pub watched_step: WatchedStep,
    /// It blows itself up as it dies, tormenting everyone in its sight (`explode_monster`), and
    /// so as it attacks, as one that blows up does (`mons_self_destructs`): the lurking horror.
    pub torments_as_it_dies: bool,
    /// It has a special ability of its own, outside its spells (`mon_special_ability`), such as
    /// a slime creature's splitting or a foxfire's burst.
    pub special_ability: bool,
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

impl Monster {
    /// Whether one of its kind can stand on the dungeon's floor: crawl's
    /// `monster_habitable_feat(type, DNGN_FLOOR)`, which a habitat on dry land or flying allows.
    #[must_use]
    pub fn stands_on_floor(&self) -> bool {
        self.flies || self.habitat.dry_land()
    }
}

/// When a kind of monster steps toward a foe that watches it: crawl's `_fungal_move_check` holds
/// back the kinds it names. A monster attacks in melee by stepping into its foe, so one held back
/// beside them doesn't attack either.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WatchedStep {
    /// Whether watched or not.
    Always,
    /// Only into its foe, from beside them: the wandering mushroom and the deathcap.
    FromBeside,
    /// Only when a roll on how far the player sees allows, more often the nearer it is: the
    /// lurking horror.
    ByChance,
}

/// The terrain a kind of monster can stand on: crawl's `habitat_type`, as `dat/mons` sets
/// `habitat`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Habitat {
    /// `HT_LAND`: dry land and shallow water.
    Land,
    /// `HT_AMPHIBIOUS`: land and deep water.
    Amphibious,
    /// `HT_WATER`: shallow and deep water only.
    Water,
    /// `HT_LAVA`: lava only.
    Lava,
    /// `HT_AMPHIBIOUS_LAVA`: land and lava.
    AmphibiousLava,
    /// `HT_DEEP_WATER`: deep water only.
    DeepWater,
    /// `HT_ELDRITCH_TENTACLE`: land, deep water and malign gateways.
    EldritchTentacle,
    /// `HT_WALL`: land and walls.
    Wall,
    /// `HT_WALLS_ONLY`: walls only.
    WallsOnly,
}

impl Habitat {
    /// Whether it includes dry land: crawl's `HT_DRY_LAND` bit.
    #[must_use]
    pub fn dry_land(self) -> bool {
        !matches!(
            self,
            Self::Water | Self::Lava | Self::DeepWater | Self::WallsOnly
        )
    }
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
    /// How long a launcher takes to fire with no skill, in crawl's time units where a
    /// standard-speed action takes 10: its `PWPN_SPEED`. `None` for a thrown missile, whose
    /// delay comes from its damage.
    pub delay: Option<i32>,
    /// One time in how many a thrown missile breaks as it lands: its `mulch_rate`, unbranded.
    /// `None` for a launcher.
    pub mulch: Option<i32>,
}

/// A melee weapon, as `Weapon_prop` (`item-prop.cc`) defines it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MeleeWeapon {
    /// Crawl's name for it, such as `spear`.
    pub name: String,
    /// The damage it adds to a monster's own: its `PWPN_DAMAGE`.
    pub damage: i32,
    /// The accuracy it adds: its `PWPN_HIT`.
    pub accuracy: i32,
    /// How many squares away it strikes from: crawl's `weapon_reach` for one that isn't an
    /// artefact, two for a polearm and one for any other.
    pub reach: i32,
    /// The skill it trains and fights with, by its `skill_type` identifier, such as
    /// `SK_POLEARMS`: its `PWPN_SKILL`.
    pub skill: EnumId,
    /// How long a swing takes with no skill, in crawl's time units where a standard-speed action
    /// takes 10: its `PWPN_SPEED`.
    pub delay: i32,
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

/// A kind of monster's resistances, as its `dat/mons` file gives them: pips from -1 to 4, 0
/// when not given. What its holiness adds, such as an undead monster's resistance to poison, is
/// left to the engine (`get_mons_class_resists`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MonsterResists {
    /// `fire`.
    pub fire: i32,
    /// `cold`.
    pub cold: i32,
    /// `elec`.
    pub elec: i32,
    /// `poison`.
    pub poison: i32,
    /// `neg`: negative energy.
    pub neg: i32,
    /// `corr`: corrosion, which acid meets.
    pub corr: i32,
    /// `steam`.
    pub steam: i32,
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
    /// The cloud its beam leaves, when it leaves one.
    pub cloud: Option<SpellCloud>,
}

/// The cloud a spell's beam leaves: round where it ends, as crawl's `bolt::affect_endpoint` and
/// `bolt::get_cloud_type` in `beam.cc` place it, and along the way when it leaves a trail.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpellCloud {
    /// Its `cloud_type` identifier, such as `CLOUD_HOLY`.
    pub kind: EnumId,
    /// Two of its squares or more block sight, as `clouds` in `cloud.cc` marks it opaque.
    pub opaque: bool,
    /// It is left on every square the beam passes, too (`bolt::affect_place_clouds`).
    pub trail: bool,
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
    /// The damage it deals when the player fires it, by power; `None` when it deals none.
    pub player_damage: Option<ZapDamage>,
    /// Its accuracy when the player fires it, by power; `None` when it has none.
    pub player_to_hit: Option<ZapToHit>,
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
    /// It is water, shallow or deep, which slows a player wading through it (`feat_is_water`).
    pub water: bool,
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
    /// How undead it is: its `undead_type`.
    pub undead: Undeadness,
    /// The size of its body, its `size`, medium unless set (`species::size` of `PSIZE_BODY`).
    pub size: BodySize,
    /// It swims, its `can_swim`: its habitat is water (`species::can_swim`).
    pub swims: bool,
    /// The mutations it has from the start or grows as it gains experience levels, its
    /// `mutations`, by experience level, then by identifier.
    pub mutations: Vec<InnateMutation>,
}

/// Levels of a mutation a species has from an experience level on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InnateMutation {
    /// The experience level it comes at: 1 for one the species starts with.
    pub xl: u32,
    /// Its `mutation_type` identifier, such as `MUT_CONSTRICTING_TAIL`.
    pub mutation: EnumId,
    /// The levels of it that come then.
    pub levels: u32,
}
