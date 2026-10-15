//! Extracts a Game Knowledge bundle from crawl source at a release tag: monsters and species from
//! `dat/` YAML, monster descriptions from the `dat/descript` text database, and item and spell
//! names and every enum's values from the C++ headers and tables.
//!
//! The extractor reads the source as a webtiles server's build of that release compiles it, so
//! the enum values match what webtiles sends.

mod cpp;
mod yaml;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use copilot_gk::{Bundle, CrawlSource, EnumId, ItemClass, ItemSubType, Spell};

use crate::cpp::Source;

/// Crawl's copyright and license, recorded in every bundle (ADR 0002).
const CRAWL_LICENSE: &str = "Dungeon Crawl Stone Soup, copyright the DCSS devteam and \
contributors (https://github.com/crawl/crawl), licensed under the GNU General Public License, \
version 2 or later.";

/// Where crawl's game source lives inside a crawl checkout.
const SOURCE_DIR: &str = "crawl-ref/source";

/// Each item class with sub-types, and the enum its sub-types are values of.
const ITEM_SUB_TYPE_ENUMS: &[(&str, &str, &str)] = &[
    ("OBJ_WEAPONS", "item-prop-enum.h", "weapon_type"),
    ("OBJ_MISSILES", "item-prop-enum.h", "missile_type"),
    ("OBJ_ARMOUR", "item-prop-enum.h", "armour_type"),
    ("OBJ_WANDS", "item-prop-enum.h", "wand_type"),
    ("OBJ_SCROLLS", "item-prop-enum.h", "scroll_type"),
    ("OBJ_JEWELLERY", "item-prop-enum.h", "jewellery_type"),
    ("OBJ_POTIONS", "potion-type.h", "potion_type"),
    ("OBJ_BOOKS", "book-type.h", "book_type"),
    ("OBJ_STAVES", "item-prop-enum.h", "stave_type"),
    ("OBJ_MISCELLANY", "item-prop-enum.h", "misc_item_type"),
    ("OBJ_RUNES", "item-prop-enum.h", "rune_type"),
    ("OBJ_TALISMANS", "item-prop-enum.h", "talisman_type"),
];

/// Where crawl names item sub-types, file by file.
const ITEM_NAMES: &[ItemNames] = &[
    ItemNames {
        file: "item-prop.cc",
        tables: &["Weapon_prop", "Armour_prop", "Missile_prop", "Staff_prop"],
        functions: &["talisman_type_name"],
    },
    ItemNames {
        file: "item-name.cc",
        tables: &[],
        functions: &[
            "_wand_type_name",
            "potion_type_name",
            "scroll_type_name",
            "jewellery_effect_name",
            "rune_type_name",
            "_book_type_name",
            "misc_type_name",
        ],
    },
];

/// The places in one file that name item sub-types.
struct ItemNames {
    file: &'static str,
    /// Tables whose entries start `{ ID, "name"`.
    tables: &'static [&'static str],
    /// Functions that `switch` on the sub-type and return its name.
    functions: &'static [&'static str],
}

/// The crawl release a bundle is extracted from, recorded in it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrawlRelease {
    /// The release tag, such as `0.34.1`.
    pub tag: String,
    /// The commit the tag points at.
    pub commit: String,
}

/// Why a bundle couldn't be extracted.
#[derive(Debug, thiserror::Error)]
pub enum ExtractError {
    /// A source file couldn't be read.
    #[error("couldn't read {path}: {source}")]
    Io {
        /// The file.
        path: PathBuf,
        /// What went wrong.
        source: std::io::Error,
    },
    /// Something the extractor needs isn't in the source.
    #[error("{file} has no {what}")]
    Missing {
        /// The file it should be in.
        file: String,
        /// What is missing.
        what: String,
    },
    /// Something in the source isn't in a form the extractor understands.
    #[error("couldn't read {what} in {file}")]
    Unreadable {
        /// The file it is in.
        file: String,
        /// What couldn't be read.
        what: String,
    },
    /// An enum value doesn't fit the bundle's type for it.
    #[error("{id} has the value {value}, out of range")]
    OutOfRange {
        /// The enumerator.
        id: String,
        /// Its value.
        value: i64,
    },
}

/// Extracts the bundle for `release` from the crawl checkout at `crawl`.
///
/// # Errors
/// If a file can't be read or isn't in a form the extractor understands.
pub fn extract(crawl: &Path, release: &CrawlRelease) -> Result<Bundle, ExtractError> {
    let crawl = Crawl::open(crawl)?;
    let item_names = crawl.item_names()?;
    Ok(Bundle {
        crawl: CrawlSource {
            tag: release.tag.clone(),
            commit: release.commit.clone(),
            license: CRAWL_LICENSE.to_owned(),
        },
        monsters: yaml::monsters(
            &crawl.source,
            &crawl.enumeration("monster-type.h", "monster_type")?,
            crawl.tag_major,
        )?,
        item_classes: crawl.item_classes(&item_names)?,
        spells: crawl.spells()?,
        species: yaml::species(&crawl.source, crawl.tag_major)?,
    })
}

/// A crawl checkout, read as one release's webtiles build compiles it.
struct Crawl {
    source: PathBuf,
    tag_major: i64,
}

impl Crawl {
    fn open(crawl: &Path) -> Result<Self, ExtractError> {
        let source = crawl.join(SOURCE_DIR);
        let file = "tag-version.h";
        let text = read(&source.join(file))?;
        let tag_major = text
            .lines()
            .find_map(|line| line.trim().strip_prefix("#define TAG_MAJOR_VERSION "))
            .and_then(|value| value.trim().parse().ok())
            .ok_or_else(|| ExtractError::Missing {
                file: file.to_owned(),
                what: "TAG_MAJOR_VERSION".to_owned(),
            })?;
        Ok(Self { source, tag_major })
    }

    fn parse(&self, file: &str) -> Result<Source, ExtractError> {
        Source::parse(file, &read(&self.source.join(file))?, self.tag_major)
    }

    fn enumeration(&self, file: &str, name: &str) -> Result<Vec<(String, i64)>, ExtractError> {
        self.parse(file)?.enumeration(name)
    }

    /// Every item sub-type identifier crawl names, with its name. An identifier's first name wins.
    fn item_names(&self) -> Result<BTreeMap<String, String>, ExtractError> {
        let mut names = BTreeMap::new();
        for places in ITEM_NAMES {
            let source = self.parse(places.file)?;
            let tables = places.tables.iter().map(|table| source.table(table));
            let functions = places
                .functions
                .iter()
                .map(|function| source.switch_names(function));
            for entries in tables.chain(functions) {
                for (id, name) in entries? {
                    names.entry(id).or_insert(name);
                }
            }
        }
        Ok(names)
    }

    /// Every item class before `NUM_OBJECT_CLASSES`, with the sub-types crawl names.
    fn item_classes(
        &self,
        names: &BTreeMap<String, String>,
    ) -> Result<BTreeMap<u8, ItemClass>, ExtractError> {
        let mut classes = BTreeMap::new();
        let all = self.enumeration("object-class-type.h", "object_class_type")?;
        for (id, value) in all
            .into_iter()
            .take_while(|(id, _)| id != "NUM_OBJECT_CLASSES")
        {
            let mut sub_types = BTreeMap::new();
            if let Some(&(_, file, name)) =
                ITEM_SUB_TYPE_ENUMS.iter().find(|(class, ..)| *class == id)
            {
                for (sub_id, sub_value) in self.enumeration(file, name)? {
                    if let Some(name) = names.get(&sub_id) {
                        sub_types.insert(
                            in_range(&sub_id, sub_value)?,
                            ItemSubType {
                                id: EnumId(sub_id),
                                name: name.clone(),
                            },
                        );
                    }
                }
            }
            classes.insert(
                in_range(&id, value)?,
                ItemClass {
                    id: EnumId(id),
                    sub_types,
                },
            );
        }
        Ok(classes)
    }

    /// Every spell in `spelldata`, by its `spell_type` value.
    fn spells(&self) -> Result<BTreeMap<u16, Spell>, ExtractError> {
        let values: BTreeMap<String, i64> = self
            .enumeration("spell-type.h", "spell_type")?
            .into_iter()
            .collect();
        let mut spells = BTreeMap::new();
        for (id, name) in self.parse("spl-data.h")?.table("spelldata")? {
            let value = *values.get(&id).ok_or_else(|| ExtractError::Missing {
                file: "spell-type.h".to_owned(),
                what: id.clone(),
            })?;
            spells.insert(
                in_range(&id, value)?,
                Spell {
                    id: EnumId(id),
                    name,
                },
            );
        }
        Ok(spells)
    }
}

/// An enum value as the bundle's integer type for it.
fn in_range<T: TryFrom<i64>>(id: &str, value: i64) -> Result<T, ExtractError> {
    T::try_from(value).map_err(|_| ExtractError::OutOfRange {
        id: id.to_owned(),
        value,
    })
}

fn read(path: &Path) -> Result<String, ExtractError> {
    std::fs::read_to_string(path).map_err(|source| ExtractError::Io {
        path: path.to_owned(),
        source,
    })
}
