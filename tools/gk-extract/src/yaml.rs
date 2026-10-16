//! Monsters and species from crawl's `dat/` YAML, and monster descriptions from its text database.

use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use copilot_gk::{
    Energy, EnumId, ItemUse, Monster, MonsterAttack, MonsterSpell, MonsterSpells, Species, Will,
};
use serde::Deserialize;

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
    energy: Option<EnergyField>,
    /// Its spellbook, named as `mon-spell.h` names it without the `MST_` prefix.
    spells: Option<String>,
    /// The monster it is a kind of, by name: its `species` unless set.
    genus: Option<String>,
    /// Its species, by name: itself unless set.
    species: Option<String>,
    /// Which items it can use, as `mon_itemuse_type` names it without the `MONUSE_` prefix.
    uses: Option<String>,
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
    #[serde(rename = "TAG_MAJOR_VERSION")]
    tag_major: Option<i64>,
}

/// `util/mon-gen.py`'s default speed.
const DEFAULT_SPEED: u32 = 10;

/// `util/mon-gen.py`'s default energy cost of every action, crawl's `DEFAULT_ENERGY`.
const DEFAULT_ENERGY: u32 = 10;

/// The flag `dat/mons` gives a monster that never moves: crawl's `M_STATIONARY`.
const STATIONARY_FLAG: &str = "stationary";

/// The spellbook of monsters given their own spells when they are made: crawl's `MST_GHOST`,
/// which `mspell_list` has no entry for.
const PER_MONSTER_SPELLBOOK: &str = "MST_GHOST";

/// The flag `dat/mons` gives a monster whose melee is more accurate: crawl's `M_FIGHTER`.
const FIGHTER_FLAG: &str = "fighter";

/// Every monster defined in `dat/mons` for this build, by its value in `monster_type`.
pub fn monsters(
    source: &Path,
    monster_type: &[(String, i64)],
    spellbooks: &BTreeMap<String, Vec<MonsterSpell>>,
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
                fighter: monster.flags.iter().any(|flag| flag == FIGHTER_FLAG),
                stationary: monster.flags.iter().any(|flag| flag == STATIONARY_FLAG),
                genus: EnumId(genus),
                item_use,
                spells,
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
    Ok(yaml_files::<SpeciesFile>(&source.join("dat/species"))?
        .into_iter()
        .filter(|(_, species)| species.tag_major.is_none_or(|tag| tag == tag_major))
        .map(|(_, species)| Species {
            id: EnumId(species.id),
            name: species.name,
        })
        .collect())
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
