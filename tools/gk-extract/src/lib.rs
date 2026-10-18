//! Extracts a Game Knowledge bundle from crawl source at a release tag: monsters and species from
//! `dat/` YAML, monster descriptions from the `dat/descript` text database, and item and spell
//! names and every enum's values from the C++ headers and tables.
//!
//! The extractor reads the source as a webtiles server's build of that release compiles it, so
//! the enum values match what webtiles sends.

mod cpp;
mod gear;
mod image;
mod spells;
mod tiles;
mod yaml;

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::process::Command;

use copilot_gk::{
    Bundle, CrawlSource, EnumId, Formulas, ItemClass, ItemSubType, LightingSigns, MonsterSpell,
    Spell,
};

use crate::cpp::{Source, Token};
pub use crate::tiles::ShippedTiles;

/// Crawl's copyright and license, recorded in every bundle (ADR 0002).
const CRAWL_LICENSE: &str = "Dungeon Crawl Stone Soup, copyright the DCSS devteam and \
contributors (https://github.com/crawl/crawl), licensed under the GNU General Public License, \
version 2 or later.";

/// What is ported from each crawl release series, by series: a release tag such as `0.34.1` is
/// in series `0.34`. A release in no series here, such as a newer stable, a beta or trunk, fails
/// to extract rather than being given another release's; the crawl oracle flags ported code that
/// changed within a series.
const SERIES: &[Series] = &[Series {
    name: "0.34",
    formulas: Formulas::CRAWL_0_34,
    gear: &gear::GEAR_RULES_0_34,
    test_monsters: &["MONS_TEST_SPAWNER", "MONS_TEST_STATUE", "MONS_TEST_BLOB"],
    innate_umbras: &["MONS_DEATH_KNIGHT"],
    lighting: &LIGHTING_0_34,
}];

/// What crawl 0.34 shows that tells the player's Lighting: `player::backlit` and
/// `player::umbra` (`player.cc:7850`, `player.cc:7864`) and `player::halo_radius`
/// (`areas.cc:564`).
const LIGHTING_0_34: LightingRules = LightingRules {
    backlighting_lights: &[
        Shown::quoted("Corona", "duration-data.h"),
        // Liquid fire, lit hotter as it burns hotter (`fill_status_info`, `status.cc:623`).
        Shown::quoted("Fire", "duration-data.h"),
        Shown::quoted("Fire+", "status.cc"),
        Shown::quoted("Fire++", "status.cc"),
        Shown::quoted("Quad", "duration-data.h"),
        // Lit only once contamination is harmful (`_describe_glow`, `status.cc:1132`).
        Shown::quoted("Contam", "status.cc"),
    ],
    halo_gods: &[Shown::quoted("the Shining One", "religion.cc")],
    // Wu Jian's heavenly storm.
    halo_statuses: &[Shown::quoted("in a heavenly storm", "duration-data.h")],
    halo_gear: &[
        // The morningstar "Eos".
        Shown {
            text: "\"eos\"",
            file: "art-data.txt",
            evidence: Some("\"Eos\""),
        },
        // The crown of Vainglory.
        Shown {
            text: "vainglory",
            file: "art-data.txt",
            evidence: Some("crown of vainglory"),
        },
        // Armour of light, named for its ego (`SPARM_LIGHT`).
        Shown {
            text: "of light",
            file: "item-name.cc",
            evidence: Some("case SPARM_LIGHT: return \"light\";"),
        },
    ],
};

/// What a crawl release series shows that tells the player's Lighting, each text checked
/// against crawl's source.
struct LightingRules {
    backlighting_lights: &'static [Shown],
    halo_gods: &'static [Shown],
    halo_statuses: &'static [Shown],
    halo_gear: &'static [Shown],
}

/// Text the Advisor matches what the game shows against, ported by hand, with what crawl's
/// source must hold for it: a renamed one fails the extraction rather than going unmatched.
struct Shown {
    /// What the Advisor matches.
    text: &'static str,
    /// The source file that must hold `evidence`.
    file: &'static str,
    /// What that file must hold, whatever its spacing, when it isn't `text` as a string
    /// literal.
    evidence: Option<&'static str>,
}

impl Shown {
    /// `text`, which `file` must hold as a string literal.
    const fn quoted(text: &'static str, file: &'static str) -> Self {
        Self {
            text,
            file,
            evidence: None,
        }
    }

    /// What its file must hold.
    fn evidence(&self) -> String {
        self.evidence
            .map_or_else(|| format!("\"{}\"", self.text), str::to_owned)
    }
}

/// What is ported from one crawl release series.
struct Series {
    /// The series, such as `0.34`.
    name: &'static str,
    /// The variants of the ported formulas it uses.
    formulas: Formulas,
    /// Its code that gives monsters missiles and wands.
    gear: &'static gear::GearRules,
    /// Its test monsters, which are never firewood: `mons_class_is_test`.
    test_monsters: &'static [&'static str],
    /// The monsters with an umbra of their own that aren't undead, which `monster::umbra_radius`
    /// gives them by kind in code rather than data.
    innate_umbras: &'static [&'static str],
    /// What it shows that tells the player's Lighting.
    lighting: &'static LightingRules,
}

/// `ids` as a set, each of which must be in `monster_type`, so a monster crawl renames fails the
/// extraction rather than silently dropping out of a rule ported by hand.
fn known_monsters<'a>(
    ids: impl IntoIterator<Item = &'a str>,
    monster_type: &[(String, i64)],
) -> Result<BTreeSet<String>, ExtractError> {
    ids.into_iter()
        .map(|id| {
            if monster_type.iter().any(|(known, _)| known == id) {
                Ok(id.to_owned())
            } else {
                Err(ExtractError::Missing {
                    file: "monster-type.h".to_owned(),
                    what: id.to_owned(),
                })
            }
        })
        .collect()
}

/// Where crawl's game source lives inside a crawl checkout.
const SOURCE_DIR: &str = "crawl-ref/source";

/// The tables of `mon-tentacle.cc` naming crawl's tentacles and their segments, with how many
/// leading fields of each row name something else: `_head_child_segment` starts each row with
/// the monster the tentacle grows from.
const TENTACLE_TABLES: &[(&str, usize)] =
    &[("_head_child_segment", 1), ("_solo_tentacle_to_segment", 0)];

/// Where crawl's tiles live inside its game source.
const RLTILES_DIR: &str = "rltiles";

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

impl CrawlRelease {
    /// The release checked out at `crawl`, a git checkout of crawl: `tag` must point at `commit`,
    /// which must be the commit checked out, with no changes to tracked files.
    ///
    /// # Errors
    /// If git can't read the checkout, the tag or commit isn't what is checked out, or tracked
    /// files have been changed.
    pub fn checked_out(crawl: &Path, tag: &str, commit: &str) -> Result<Self, ExtractError> {
        let head = git(crawl, &["rev-parse", "--verify", "HEAD^{commit}"])?;
        let tagged = git(
            crawl,
            &[
                "rev-parse",
                "--verify",
                &format!("refs/tags/{tag}^{{commit}}"),
            ],
        );
        if tagged.as_deref().ok() != Some(head.as_str()) || commit != head {
            return Err(ExtractError::NotCheckedOut {
                tag: tag.to_owned(),
                commit: commit.to_owned(),
                head,
            });
        }
        let changed = git(crawl, &["status", "--porcelain", "--untracked-files=no"])?;
        if !changed.is_empty() {
            return Err(ExtractError::Changed { files: changed });
        }
        Ok(Self {
            tag: tag.to_owned(),
            commit: head,
        })
    }

    /// What is ported from this release's series.
    fn series(&self) -> Result<&'static Series, ExtractError> {
        let series = self
            .tag
            .splitn(3, '.')
            .take(2)
            .collect::<Vec<_>>()
            .join(".");
        SERIES
            .iter()
            .find(|known| known.name == series)
            .ok_or_else(|| ExtractError::UnknownSeries {
                tag: self.tag.clone(),
            })
    }
}

/// What git prints for `args` in the checkout at `crawl`, trimmed.
fn git(crawl: &Path, args: &[&str]) -> Result<String, ExtractError> {
    let failed = |reason: String| ExtractError::Git {
        command: args.join(" "),
        reason,
    };
    let out = Command::new("git")
        .arg("-C")
        .arg(crawl)
        .args(args)
        .output()
        .map_err(|error| failed(error.to_string()))?;
    if !out.status.success() {
        return Err(failed(
            String::from_utf8_lossy(&out.stderr).trim().to_owned(),
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_owned())
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
    /// Git couldn't read the crawl checkout.
    #[error("git {command} failed: {reason}")]
    Git {
        /// The git command, without `git -C <checkout>`.
        command: String,
        /// What git said.
        reason: String,
    },
    /// The release asked for isn't what the crawl checkout has checked out.
    #[error("crawl {tag} at {commit} isn't what is checked out: HEAD is {head}")]
    NotCheckedOut {
        /// The tag asked for.
        tag: String,
        /// The commit asked for.
        commit: String,
        /// The commit checked out.
        head: String,
    },
    /// Tracked files in the crawl checkout have been changed, so it isn't the release.
    #[error("the crawl checkout has changes, so it isn't the release:\n{files}")]
    Changed {
        /// The changed files, as `git status --porcelain` lists them.
        files: String,
    },
    /// The extractor knows no formula variants or gear rules for the release's series, so it
    /// can't say which the engine should use.
    #[error("no formula variants or gear rules are ported for crawl {tag}'s release series")]
    UnknownSeries {
        /// The release tag.
        tag: String,
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

/// What the extractor makes of a crawl release.
#[derive(Debug)]
pub struct Extraction {
    /// The Game Knowledge bundle.
    pub bundle: Bundle,
    /// The tiles the bundle names, for the app to ship.
    pub tiles: ShippedTiles,
}

/// Extracts the bundle for `release`, and the tiles it names, from the crawl checkout at `crawl`.
///
/// # Errors
/// If the extractor knows no formula variants for the release, or a file can't be read or isn't
/// in a form the extractor understands.
pub fn extract(crawl: &Path, release: &CrawlRelease) -> Result<Extraction, ExtractError> {
    let series = release.series()?;
    let crawl = Crawl::open(crawl)?;
    let item_names = crawl.item_names()?;
    let item_prop = crawl.parse("item-prop.cc")?;
    let missile_weapons = gear::missile_weapons(&item_prop, "item-prop.cc")?;
    let gear = gear::Gear::read(
        series.gear,
        &crawl.parse("mon-gear.cc")?,
        &item_prop,
        missile_weapons.clone(),
    )?;
    let mut tiles = ShippedTiles::new(&crawl.source.join(RLTILES_DIR))?;
    let monster_type = crawl.enumeration("monster-type.h", "monster_type")?;
    let exceptions = yaml::KindExceptions {
        never_firewood: crawl.never_firewood(series, &monster_type)?,
        innate_umbras: known_monsters(series.innate_umbras.iter().copied(), &monster_type)?,
    };
    let bundle = Bundle {
        crawl: CrawlSource {
            tag: release.tag.clone(),
            commit: release.commit.clone(),
            license: CRAWL_LICENSE.to_owned(),
        },
        monsters: yaml::monsters(
            &crawl.source,
            &monster_type,
            &crawl.spellbooks()?,
            &gear,
            &exceptions,
            &mut tiles,
            crawl.tag_major,
        )?,
        item_classes: crawl.item_classes(&item_names)?,
        spells: crawl.spells()?,
        missile_weapons,
        wands: gear::wands(&crawl.parse("spl-book.cc")?, "spl-book.cc", &item_names)?,
        species: yaml::species(&crawl.source, crawl.tag_major)?,
        features: crawl.values(
            "dungeon-feature-type.h",
            "dungeon_feature_type",
            "NUM_FEATURES",
        )?,
        attitudes: crawl.values("mon-attitude-type.h", "mon_attitude_type", "")?,
        monster_threats: crawl.values("mon-util.h", "mon_threat_level_type", "MTHRT_UNDEF")?,
        lighting: crawl.lighting(series.lighting)?,
        formulas: series.formulas,
    };
    Ok(Extraction { bundle, tiles })
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

    /// The enumerators of `enum <name>` before `end` (or all of them, when `end` is empty), by
    /// value. An enumerator that aliases an earlier one's value keeps the earlier one's name.
    fn values<T: TryFrom<i64> + Ord>(
        &self,
        file: &str,
        name: &str,
        end: &str,
    ) -> Result<BTreeMap<T, EnumId>, ExtractError> {
        let mut values = BTreeMap::new();
        for (id, value) in self
            .enumeration(file, name)?
            .into_iter()
            .take_while(|(id, _)| id != end)
        {
            values.entry(in_range(&id, value)?).or_insert(EnumId(id));
        }
        Ok(values)
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

    /// The integer constants crawl's tables use, such as `LOS_RADIUS` and `AUTOMATIC_HIT`.
    fn constants(&self) -> Result<HashMap<String, i64>, ExtractError> {
        Ok(cpp::integer_defines(&read(&self.source.join("defines.h"))?))
    }

    /// Every spell in `spelldata`, by its `spell_type` value, with the zap it fires.
    fn spells(&self) -> Result<BTreeMap<u16, Spell>, ExtractError> {
        let values: BTreeMap<String, i64> = self
            .enumeration("spell-type.h", "spell_type")?
            .into_iter()
            .collect();
        let constants = self.constants()?;
        let zaps = spells::zaps(&self.parse("zap-data.h")?, "zap-data.h", &constants)?;
        let spell_zaps = spells::spell_zaps(&self.parse("spl-zap.cc")?, "spl-zap.cc")?;
        let mut spells = BTreeMap::new();
        let entries = spells::spell_entries(&self.parse("spl-data.h")?, "spl-data.h", &constants)?;
        for entry in entries {
            let value = *values.get(&entry.id).ok_or_else(|| ExtractError::Missing {
                file: "spell-type.h".to_owned(),
                what: entry.id.clone(),
            })?;
            let zap = spell_zaps
                .get(&entry.id)
                .map(|zap| {
                    zaps.get(zap).cloned().ok_or_else(|| ExtractError::Missing {
                        file: "zap-data.h".to_owned(),
                        what: format!("{zap}, which spl-zap.cc gives {}", entry.id),
                    })
                })
                .transpose()?;
            spells.insert(
                in_range(&entry.id, value)?,
                Spell {
                    id: EnumId(entry.id),
                    name: entry.name,
                    range: entry.range,
                    zap,
                },
            );
        }
        Ok(spells)
    }

    /// The monsters crawl never counts as firewood whatever their flags
    /// (`mons_class_is_firewood`): the series' test monsters, and every tentacle and tentacle
    /// segment (`mons_is_tentacle_or_tentacle_segment`). Each must be in `monster_type`, so a
    /// renamed one fails rather than turns into firewood.
    fn never_firewood(
        &self,
        series: &Series,
        monster_type: &[(String, i64)],
    ) -> Result<BTreeSet<String>, ExtractError> {
        let file = "mon-tentacle.cc";
        let source = self.parse(file)?;
        let mut never = BTreeSet::new();
        for &(table, skipped) in TENTACLE_TABLES {
            for row in source.entries(table)? {
                for field in row.iter().skip(skipped) {
                    let [Token::Ident(id)] = field.as_slice() else {
                        return Err(ExtractError::Unreadable {
                            file: file.to_owned(),
                            what: format!("a row of {table}"),
                        });
                    };
                    never.insert(id.clone());
                }
            }
        }
        never.extend(series.test_monsters.iter().map(|&id| id.to_owned()));
        known_monsters(never.iter().map(String::as_str), monster_type)
    }

    /// The texts of `rules`, each found in crawl's source as its rule says.
    fn lighting(&self, rules: &LightingRules) -> Result<LightingSigns, ExtractError> {
        let texts = |shown: &[Shown]| -> Result<Vec<String>, ExtractError> {
            shown
                .iter()
                .map(|shown| {
                    let evidence = shown.evidence();
                    let unspaced = |text: &str| text.split_whitespace().collect::<String>();
                    if unspaced(&read(&self.source.join(shown.file))?)
                        .contains(&unspaced(&evidence))
                    {
                        Ok(shown.text.to_owned())
                    } else {
                        Err(ExtractError::Missing {
                            file: shown.file.to_owned(),
                            what: evidence,
                        })
                    }
                })
                .collect()
        };
        Ok(LightingSigns {
            backlighting_lights: texts(rules.backlighting_lights)?,
            halo_gods: texts(rules.halo_gods)?,
            halo_statuses: texts(rules.halo_statuses)?,
            halo_gear: texts(rules.halo_gear)?,
        })
    }

    /// Every monster spellbook, by its `mon_spellbook_type` identifier.
    fn spellbooks(&self) -> Result<BTreeMap<String, Vec<MonsterSpell>>, ExtractError> {
        spells::spellbooks(&self.parse("mon-spell.h")?, "mon-spell.h")
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
