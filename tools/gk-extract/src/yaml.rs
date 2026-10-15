//! Monsters and species from crawl's `dat/` YAML, and monster descriptions from its text database.

use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use copilot_gk::{EnumId, Monster, Species, Will};
use serde::Deserialize;

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
    #[serde(rename = "TAG_MAJOR_VERSION")]
    tag_major: Option<i64>,
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

/// Every monster defined in `dat/mons` for this build, by its value in `monster_type`.
pub fn monsters(
    source: &Path,
    monster_type: &[(String, i64)],
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
        monsters.insert(
            in_range(&id, value)?,
            Monster {
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
            },
        );
    }
    Ok(monsters)
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
