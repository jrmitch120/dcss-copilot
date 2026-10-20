//! Monsters, species and forms from crawl's `dat/` YAML, and monster descriptions from its text
//! database.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;

use copilot_gk::{
    Energy, EnumId, Form, FormHoliness, Habitat, Intelligence, ItemUse, Monster, MonsterAttack,
    MonsterSpell, MonsterSpells, Species, Undeadness, WatchedStep, Will,
};
use serde::Deserialize;

use crate::gear::{Gear, GearOf};
use crate::tiles::ShippedTiles;
use crate::{ExtractError, in_range, read};

/// A `dat/mons/*.yaml` file: the fields the bundle keeps. `util/mon-gen.py` defines the format.
#[derive(Deserialize)]
struct MonsterFile {
    name: String,
    #[serde(rename = "enum")]
    id: Option<String>,
    glyph: Glyph,
    hd: u32,
    hp_10x: u32,
    ac: i32,
    ev: i32,
    will: Option<WillField>,
    will_per_hd: Option<u32>,
    speed: Option<u32>,
    #[serde(default)]
    attacks: Vec<AttackField>,
    #[serde(default)]
    flags: Vec<String>,
    /// What it is, such as `natural` or `undead`, as `mon_holy_type` names it without `MH_`.
    #[serde(default)]
    holiness: Vec<String>,
    /// The god it serves, by name.
    god: Option<String>,
    energy: Option<EnergyField>,
    /// Its spellbook, named as `mon-spell.h` names it without the `MST_` prefix.
    spells: Option<String>,
    /// The monster it is a kind of, by name: its `species` unless set.
    genus: Option<String>,
    /// Its species, by name: itself unless set.
    species: Option<String>,
    /// The terrain it can stand on, as `habitat_type` names it without the `HT_` prefix.
    habitat: Option<String>,
    /// Which items it can use, as `mon_itemuse_type` names it without the `MONUSE_` prefix.
    uses: Option<String>,
    /// How clever it is, as `mon_intel_type` names it without the `I_` prefix.
    intelligence: String,
    /// The tile it is drawn with, when not the one named after its enum.
    tile: Option<String>,
    #[serde(rename = "TAG_MAJOR_VERSION")]
    tag_major: Option<i64>,
}

/// One entry of a monster's `attacks`. `parse_attacks` in `util/mon-gen.py` refuses any other
/// field, so a crawl that adds one (as later trunk adds `reach`) fails here rather than being
/// read wrongly.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AttackField {
    #[serde(rename = "type")]
    kind: String,
    damage: u32,
    flavour: Option<String>,
}

/// A monster's `energy`, as `parse_energy` in `util/mon-gen.py` reads it: `move` sets both
/// `walk` and `swim`, and anything not given costs the default.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EnergyField {
    #[serde(rename = "move")]
    moving: Option<u32>,
    walk: Option<u32>,
    swim: Option<u32>,
    attack: Option<u32>,
    missile: Option<u32>,
    spell: Option<u32>,
}

impl EnergyField {
    fn energy(&self) -> Energy {
        let or_default = |cost: Option<u32>| cost.unwrap_or(DEFAULT_ENERGY);
        Energy {
            walk: or_default(self.walk.or(self.moving)),
            swim: or_default(self.swim.or(self.moving)),
            attack: or_default(self.attack),
            missile: or_default(self.missile),
            spell: or_default(self.spell),
        }
    }
}

#[derive(Deserialize)]
struct Glyph {
    char: String,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum WillField {
    Value(u32),
    Word(String),
}

/// A `dat/species/*.yaml` file: the fields the bundle keeps.
#[derive(Deserialize)]
struct SpeciesFile {
    #[serde(rename = "enum")]
    id: String,
    name: String,
    /// `undead` or `semi_undead`, as `undead_state_type` names it without `US_`: alive unless
    /// set.
    undead_type: Option<String>,
    #[serde(rename = "TAG_MAJOR_VERSION")]
    tag_major: Option<i64>,
}

/// A `dat/forms/*.yaml` file: the fields the bundle keeps. `util/form-gen.py` defines the format.
#[derive(Deserialize)]
struct FormFile {
    #[serde(rename = "enum")]
    id: String,
    short_name: Option<String>,
    /// As `mon_holy_type` names it without `MH_`: none unless set.
    holiness: Option<String>,
    resists: Option<FormResists>,
    #[serde(rename = "TAG_MAJOR_VERSION")]
    tag_major: Option<i64>,
}

/// A form's `resists`: those the bundle keeps, each 0 unless set.
#[derive(Deserialize)]
struct FormResists {
    neg: Option<i32>,
}

/// `util/mon-gen.py`'s default speed.
const DEFAULT_SPEED: u32 = 10;

/// `util/mon-gen.py`'s default energy cost of every action, crawl's `DEFAULT_ENERGY`.
const DEFAULT_ENERGY: u32 = 10;

/// The flag `dat/mons` gives a monster that never moves: crawl's `M_STATIONARY`.
const STATIONARY_FLAG: &str = "stationary";

/// The flag `dat/mons` gives a monster that is no threat and gives no experience: crawl's
/// `M_NO_THREAT`.
const NO_THREAT_FLAG: &str = "no_threat";

/// The holiness `dat/mons` gives an undead monster: crawl's `MH_UNDEAD`.
const UNDEAD_HOLINESS: &str = "undead";

/// The god whose servants see through an umbra (`monster::nightvision`), as `dat/mons` names it.
const NIGHTVISION_GOD: &str = "Yredelemnul";

/// The spellbook of monsters given their own spells when they are made: crawl's `MST_GHOST`,
/// which `mspell_list` has no entry for.
pub const PER_MONSTER_SPELLBOOK: &str = "MST_GHOST";

/// The flag `dat/mons` gives a monster whose melee is more accurate: crawl's `M_FIGHTER`.
const FIGHTER_FLAG: &str = "fighter";

/// The flag `dat/mons` gives a monster whose missiles are more accurate and hit harder: crawl's
/// `M_ARCHER`.
const ARCHER_FLAG: &str = "archer";

/// The flag `dat/mons` gives a monster that shoots more readily: crawl's `M_PREFER_RANGED`.
const PREFER_RANGED_FLAG: &str = "prefer_ranged";

/// The flag `dat/mons` gives a monster that wields a melee weapon in each hand: crawl's
/// `M_TWO_WEAPONS`.
const TWO_WEAPONS_FLAG: &str = "two_weapons";

/// The flag `dat/mons` gives a monster there is only one of: crawl's `M_UNIQUE`.
const UNIQUE_FLAG: &str = "unique";

/// The flag `dat/mons` gives a monster that flits away after it attacks: crawl's `M_BATTY`.
const BATTY_FLAG: &str = "batty";

/// The flag `dat/mons` gives a monster that keeps its distance to fire: crawl's
/// `M_MAINTAIN_RANGE`.
const MAINTAIN_RANGE_FLAG: &str = "maintain_range";

/// The flag `dat/mons` gives a monster that flies: crawl's `M_FLIES`.
const FLIES_FLAG: &str = "flies";

/// The flag `dat/mons` gives a projectile, which `iood_act` flies: crawl's `M_PROJECTILE`.
const PROJECTILE_FLAG: &str = "projectile";

/// The kinds of monster crawl singles out in code rather than in `dat/mons`, by `monster_type`
/// identifier.
pub struct KindExceptions {
    /// Kinds that aren't firewood whatever their flags.
    pub never_firewood: BTreeSet<String>,
    /// Kinds that have an umbra of their own.
    pub innate_umbras: BTreeSet<String>,
    /// Tentacles and tentacle segments.
    pub tentacles: BTreeSet<String>,
    /// Kinds `handle_monster_move` acts or moves for with code of its own.
    pub own_actions: BTreeSet<String>,
    /// Kinds that step toward a foe that watches them only from beside them.
    pub steps_from_beside: BTreeSet<String>,
    /// Kinds that step toward a foe that watches them only by chance.
    pub steps_by_chance: BTreeSet<String>,
    /// Kinds that blow themselves up into torment as they die.
    pub torments_as_it_dies: BTreeSet<String>,
    /// Kinds with a special ability of their own.
    pub special_abilities: BTreeSet<String>,
}

/// Every monster defined in `dat/mons` for this build, by its value in `monster_type`, with the
/// kinds `exceptions` singles out.
pub fn monsters(
    source: &Path,
    monster_type: &[(String, i64)],
    spellbooks: &BTreeMap<String, Vec<MonsterSpell>>,
    gear: &Gear,
    exceptions: &KindExceptions,
    tiles: &mut ShippedTiles,
    tag_major: i64,
) -> Result<BTreeMap<u16, Monster>, ExtractError> {
    let values: HashMap<&str, i64> = monster_type
        .iter()
        .map(|(id, value)| (id.as_str(), *value))
        .collect();
    let descriptions = text_database(&source.join("dat/descript/monsters.txt"))?;
    let mut monsters = BTreeMap::new();
    for (file, monster) in yaml_files::<MonsterFile>(&source.join("dat/mons"))? {
        if monster.tag_major.is_some_and(|tag| tag != tag_major) {
            continue;
        }
        let id = format!(
            "MONS_{}",
            monster
                .id
                .unwrap_or_else(|| monster.name.replace(' ', "_"))
                .to_uppercase()
        );
        let value = *values
            .get(id.as_str())
            .ok_or_else(|| ExtractError::Missing {
                file: "monster-type.h".to_owned(),
                what: format!("{id}, which {file} defines"),
            })?;
        let will = match (monster.will, monster.will_per_hd) {
            (Some(WillField::Value(will)), None) => Will::Value(will),
            (Some(WillField::Word(word)), None) if word == "invuln" => Will::Invulnerable,
            (None, Some(per_hd)) => Will::PerHd(per_hd),
            _ => {
                return Err(ExtractError::Unreadable {
                    file,
                    what: "will".to_owned(),
                });
            }
        };
        let tile = tiles.ship(&tile_enum(&id, monster.tile.as_deref()))?;
        // As `util/mon-gen.py` defaults them: genus to species, species to the monster itself.
        let genus = match monster.genus.or(monster.species) {
            Some(name) => format!("MONS_{}", name.to_uppercase()),
            None => id.clone(),
        };
        let item_use =
            item_use(monster.uses.as_deref()).ok_or_else(|| ExtractError::Unreadable {
                file: file.clone(),
                what: "uses".to_owned(),
            })?;
        // As `util/mon-gen.py` names the book.
        let spells = match monster
            .spells
            .map(|book| format!("MST_{}", book.to_uppercase()))
        {
            None => MonsterSpells::Book { spells: Vec::new() },
            Some(book) if book == PER_MONSTER_SPELLBOOK => MonsterSpells::PerMonster,
            Some(book) => MonsterSpells::Book {
                spells: spellbooks
                    .get(&book)
                    .cloned()
                    .ok_or_else(|| ExtractError::Missing {
                        file: "mon-spell.h".to_owned(),
                        what: format!("{book}, which {file} names"),
                    })?,
            },
        };
        let gear = gear.of(&GearOf {
            id: &id,
            hd: monster.hd,
            item_use,
            flags: &monster.flags,
        })?;
        let intelligence =
            intelligence(&monster.intelligence).ok_or_else(|| ExtractError::Unreadable {
                file: file.clone(),
                what: "intelligence".to_owned(),
            })?;
        let flag = |name: &str| monster.flags.iter().any(|flag| flag == name);
        // As `mons_class_is_firewood` (`mon-util.cc`) judges it.
        let firewood = flag(STATIONARY_FLAG)
            && flag(NO_THREAT_FLAG)
            && !exceptions.never_firewood.contains(&id);
        // As `monster::nightvision` (`monster.cc:6545`) judges one carrying nothing; every undead
        // kind `monster::umbra_radius` (`areas.cc:764`) names is undead already.
        let nightvision = monster.holiness.iter().any(|holy| holy == UNDEAD_HOLINESS)
            || monster.god.as_deref() == Some(NIGHTVISION_GOD)
            || exceptions.innate_umbras.contains(&id);
        let tentacle = exceptions.tentacles.contains(&id);
        let own_actions = flag(PROJECTILE_FLAG) || exceptions.own_actions.contains(&id);
        let watched_step = if exceptions.steps_from_beside.contains(&id) {
            WatchedStep::FromBeside
        } else if exceptions.steps_by_chance.contains(&id) {
            WatchedStep::ByChance
        } else {
            WatchedStep::Always
        };
        let special_ability = exceptions.special_abilities.contains(&id);
        let torments_as_it_dies = exceptions.torments_as_it_dies.contains(&id);
        let habitat =
            habitat(monster.habitat.as_deref()).ok_or_else(|| ExtractError::Unreadable {
                file: file.clone(),
                what: "habitat".to_owned(),
            })?;
        monsters.insert(
            in_range(&id, value)?,
            Monster {
                tile,
                description: descriptions.get(&monster.name.to_lowercase()).cloned(),
                id: EnumId(id),
                name: monster.name,
                glyph: monster.glyph.char,
                hd: monster.hd,
                hp_10x: monster.hp_10x,
                ac: monster.ac,
                ev: monster.ev,
                will,
                speed: monster.speed.unwrap_or(DEFAULT_SPEED),
                attacks: monster
                    .attacks
                    .into_iter()
                    .map(|attack| MonsterAttack {
                        // As `parse_attacks` in `util/mon-gen.py` names them.
                        kind: EnumId(format!("AT_{}", attack.kind.to_uppercase())),
                        flavour: EnumId(format!(
                            "AF_{}",
                            attack.flavour.as_deref().unwrap_or("plain").to_uppercase()
                        )),
                        damage: attack.damage,
                    })
                    .collect(),
                fighter: flag(FIGHTER_FLAG),
                stationary: flag(STATIONARY_FLAG),
                firewood,
                nightvision,
                archer: flag(ARCHER_FLAG),
                prefers_ranged: flag(PREFER_RANGED_FLAG),
                two_weapons: flag(TWO_WEAPONS_FLAG),
                unique: flag(UNIQUE_FLAG),
                batty: flag(BATTY_FLAG),
                keeps_range: flag(MAINTAIN_RANGE_FLAG),
                flies: flag(FLIES_FLAG),
                habitat,
                tentacle,
                own_actions,
                watched_step,
                torments_as_it_dies,
                special_ability,
                intelligence,
                genus: EnumId(genus),
                item_use,
                spells,
                gear,
                energy: monster.energy.map_or(
                    Energy {
                        walk: DEFAULT_ENERGY,
                        swim: DEFAULT_ENERGY,
                        attack: DEFAULT_ENERGY,
                        missile: DEFAULT_ENERGY,
                        spell: DEFAULT_ENERGY,
                    },
                    |energy| energy.energy(),
                ),
            },
        );
    }
    Ok(monsters)
}

/// A monster's `habitat`, as `parse_habitat` in `util/mon-gen.py` reads it: `land` when unset,
/// and `None` for a value it refuses.
fn habitat(habitat: Option<&str>) -> Option<Habitat> {
    match habitat.unwrap_or("land") {
        "land" => Some(Habitat::Land),
        "amphibious" => Some(Habitat::Amphibious),
        "water" => Some(Habitat::Water),
        "lava" => Some(Habitat::Lava),
        "amphibious_lava" => Some(Habitat::AmphibiousLava),
        "deep_water" => Some(Habitat::DeepWater),
        "eldritch_tentacle" => Some(Habitat::EldritchTentacle),
        "wall" => Some(Habitat::Wall),
        "walls_only" => Some(Habitat::WallsOnly),
        _ => None,
    }
}

/// A monster's `uses`, as `parse_use` in `util/mon-gen.py` reads it: `nothing` when unset, and
/// `None` for a value it refuses.
fn item_use(uses: Option<&str>) -> Option<ItemUse> {
    match uses.unwrap_or("nothing") {
        "nothing" => Some(ItemUse::Nothing),
        "open_doors" => Some(ItemUse::OpenDoors),
        "starting_equipment" => Some(ItemUse::StartingEquipment),
        "weapons_armour" => Some(ItemUse::WeaponsArmour),
        _ => None,
    }
}

/// A monster's `intelligence`, as `parse_intelligence` in `util/mon-gen.py` reads it, which
/// requires one: `None` for a value it refuses.
fn intelligence(intelligence: &str) -> Option<Intelligence> {
    match intelligence {
        "brainless" => Some(Intelligence::Brainless),
        "animal" => Some(Intelligence::Animal),
        "human" => Some(Intelligence::Human),
        _ => None,
    }
}

/// The tile enum a monster is drawn with, as `base_tile` in `util/mon-gen.py` names it: its own
/// enum's, unless its `tile` names another.
fn tile_enum(id: &str, tile: Option<&str>) -> String {
    match tile {
        None => format!("TILEP_{id}"),
        Some(tile) if tile.starts_with("tile") => tile.to_uppercase(),
        Some(tile) => format!("TILEP_MONS_{}", tile.to_uppercase()),
    }
}

/// Every species defined in `dat/species` for this build, in file name order.
pub fn species(source: &Path, tag_major: i64) -> Result<Vec<Species>, ExtractError> {
    yaml_files::<SpeciesFile>(&source.join("dat/species"))?
        .into_iter()
        .filter(|(_, species)| species.tag_major.is_none_or(|tag| tag == tag_major))
        .map(|(file, species)| {
            let undead = match species.undead_type.as_deref() {
                None => Undeadness::Alive,
                Some("undead") => Undeadness::Undead,
                Some("semi_undead") => Undeadness::SemiUndead,
                Some(other) => {
                    return Err(ExtractError::Unreadable {
                        file,
                        what: format!("undead_type {other}"),
                    });
                }
            };
            Ok(Species {
                id: EnumId(species.id),
                name: species.name,
                undead,
            })
        })
        .collect()
}

/// Every form defined in `dat/forms` for this build, in file name order, as `util/form-gen.py`
/// reads them, but `none`, which shows no light: a form's status light is its `short_name`, or
/// its `enum` capitalised, and it gives no holiness of its own unless it sets one. None of them
/// is undead here: the series' holiness rules mark those `player::undead_state` names.
pub fn forms(source: &Path, tag_major: i64) -> Result<Vec<Form>, ExtractError> {
    yaml_files::<FormFile>(&source.join("dat/forms"))?
        .into_iter()
        .filter(|(_, form)| form.tag_major.is_none_or(|tag| tag == tag_major))
        .filter(|(_, form)| form.short_name.as_deref() != Some(""))
        .map(|(file, form)| {
            let holiness = match form.holiness.as_deref() {
                None => FormHoliness::Species,
                Some("natural") => FormHoliness::Natural,
                Some("undead") => FormHoliness::Undead,
                Some("demonic") => FormHoliness::Demonic,
                Some("nonliving") => FormHoliness::Nonliving,
                Some("plant") => FormHoliness::Plant,
                Some("holy") => FormHoliness::Holy,
                Some(other) => {
                    return Err(ExtractError::Unreadable {
                        file,
                        what: format!("holiness {other}"),
                    });
                }
            };
            Ok(Form {
                light: form.short_name.unwrap_or_else(|| capitalised(&form.id)),
                id: form.id,
                holiness,
                undead: Undeadness::Alive,
                negative: form.resists.and_then(|resists| resists.neg).unwrap_or(0),
                halves_torment: false,
            })
        })
        .collect()
}

/// `word` as Python's `str.capitalize` gives it: its first letter upper case, the rest lower.
fn capitalised(word: &str) -> String {
    let mut chars = word.chars();
    chars.next().map_or_else(String::new, |first| {
        first
            .to_uppercase()
            .chain(chars.flat_map(char::to_lowercase))
            .collect()
    })
}

/// Every `*.yaml` file in `dir`, parsed, in file name order, with its file name.
fn yaml_files<T: for<'de> Deserialize<'de>>(dir: &Path) -> Result<Vec<(String, T)>, ExtractError> {
    let io = |source| ExtractError::Io {
        path: dir.to_owned(),
        source,
    };
    let mut paths: Vec<_> = std::fs::read_dir(dir)
        .map_err(io)?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<Result<_, _>>()
        .map_err(io)?;
    paths.retain(|path| path.extension().is_some_and(|ext| ext == "yaml"));
    paths.sort();
    paths
        .into_iter()
        .map(|path| {
            let file = path.display().to_string();
            let parsed = serde_saphyr::from_str(&read(&path)?).map_err(|error| {
                ExtractError::Unreadable {
                    file: file.clone(),
                    what: error.to_string(),
                }
            })?;
            Ok((file, parsed))
        })
        .collect()
}

/// A crawl text database: entries separated by `%%%%` lines, each a key line then its text, with
/// `#` lines as comments. Keys are lowercased, as crawl looks them up; the first entry for a key
/// wins.
fn text_database(path: &Path) -> Result<HashMap<String, String>, ExtractError> {
    let text = read(path)?;
    let mut entries = HashMap::new();
    for entry in text.split("%%%%\n") {
        let mut lines = entry.lines().filter(|line| !line.starts_with('#'));
        let Some(key) = lines.next() else { continue };
        let body = lines.collect::<Vec<_>>().join("\n");
        let body = body.trim();
        if !key.is_empty() && !body.is_empty() {
            entries
                .entry(key.trim().to_lowercase())
                .or_insert_with(|| body.to_owned());
        }
    }
    Ok(entries)
}
