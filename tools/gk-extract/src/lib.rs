//! Extracts a Game Knowledge bundle from crawl source at a release tag: monsters and species from
//! `dat/` YAML, monster descriptions from the `dat/descript` text database, and item and spell
//! names and every enum's values from the C++ headers and tables.
//!
//! The extractor reads the source as a webtiles server's build of that release compiles it, so
//! the enum values match what webtiles sends.

mod branches;
mod character;
mod cpp;
mod features;
mod gear;
mod image;
mod level;
mod speed;
mod spells;
mod tiles;
mod ways_out;
mod yaml;

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::process::Command;

use copilot_gk::{
    Bundle, CrawlSource, DangerSigns, EnumId, Form, FormHoliness, Formulas, HolinessSigns,
    IdentificationSigns, ItemClass, ItemSubType, LightingSigns, MonsterSpell, Species, Spell,
    SpellCloud, Undeadness, WeaponBrand,
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
    own_actions: NamedKinds {
        file: "mon-act.cc",
        functions: &["handle_monster_move", "_pre_monster_move"],
        kinds: &[
            "MONS_FULMINANT_PRISM",
            "MONS_SHADOW_PRISM",
            "MONS_SPLINTERFROST_BARRICADE",
            "MONS_BLAZEHEART_CORE",
            "MONS_BLAZEHEART_GOLEM",
            "MONS_RENDING_BLADE",
            "MONS_BURSTSHROOM",
            "MONS_SIXFIRHY",
            "MONS_JIANGSHI",
            "MONS_BOULDER",
            "MONS_HELLFIRE_MORTAR",
            "MONS_DIAMOND_SAWBLADE",
            "MONS_LIGHTNING_SPIRE",
            "MONS_SOLAR_EMBER",
            "MONS_BOUNDLESS_TESSERACT",
        ],
    },
    steps_from_beside: NamedKinds {
        file: "mon-act.cc",
        functions: &["_fungal_move_check"],
        kinds: &["MONS_WANDERING_MUSHROOM", "MONS_DEATHCAP"],
    },
    steps_by_chance: NamedKinds {
        file: "mon-act.cc",
        functions: &["_fungal_move_check"],
        kinds: &["MONS_LURKING_HORROR"],
    },
    torments_as_it_dies: NamedKinds {
        file: "mon-explode.cc",
        functions: &["explode_monster"],
        kinds: &["MONS_LURKING_HORROR"],
    },
    lighting: &LIGHTING_0_34,
    holiness: &HOLINESS_0_34,
    danger: &DANGER_0_34,
    place_naming: &PLACE_NAMING_0_34,
    features: &features::FEATURE_RULES_0_34,
    ways_out: &ways_out::WAY_OUT_RULES_0_34,
    speed: &speed::SPEED_RULES_0_34,
    character: &character::CHARACTER_RULES_0_34,
    level: &level::LEVEL_RULES_0_34,
    // `seen_item`, as it tells the player what an item they carry is.
    kind_learned: Shown::quoted("You learned that %s %s actually %s.", "item-prop.cc"),
    kinds_hidden: &[
        "OBJ_WANDS",
        "OBJ_SCROLLS",
        "OBJ_JEWELLERY",
        "OBJ_POTIONS",
        "OBJ_STAVES",
    ],
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

/// How crawl 0.34's webtiles names the place a level of a branch one level deep is
/// (`TilesFramework::_send_player`, `tileweb.cc:1232`): the Abyss with `The `, Pandemonium bare,
/// and a branch the dungeon doesn't lead to by stairs with `article_a` (`english.cc:381`).
const PLACE_NAMING_0_34: PlaceNamingRules = PlaceNamingRules {
    naming: branches::PlaceNaming {
        definite: &[("BRANCH_ABYSS", "The ")],
        bare: &["BRANCH_PANDEMONIUM"],
        unconnected_flag: "no_x_level_travel",
        vowels: "aeiouAEIOU",
    },
    evidence: &[
        Shown {
            text: "brdepth[place.branch] == 1",
            file: "tileweb.cc",
            evidence: Some("if (brdepth[place.branch] == 1)"),
        },
        Shown {
            text: "The ",
            file: "tileweb.cc",
            evidence: Some("if (place.branch == BRANCH_ABYSS) short_name.insert(0, \"The \");"),
        },
        Shown {
            text: "BRANCH_PANDEMONIUM",
            file: "tileweb.cc",
            evidence: Some(
                "else if (place.branch != BRANCH_PANDEMONIUM && !is_connected_branch(place.branch))",
            ),
        },
        Shown {
            text: "no_x_level_travel",
            file: "branch.cc",
            evidence: Some("return !testbits(branch->branch_flags, brflag::no_x_level_travel);"),
        },
        Shown {
            text: "aeiouAEIOU",
            file: "english.cc",
            evidence: Some(
                "case 'a': case 'e': case 'i': case 'o': case 'u': case 'A': case 'E': case 'I': case 'O': case 'U':",
            ),
        },
    ],
};

/// How a crawl release series names the place a level of a branch one level deep is, with what
/// crawl's source must hold for it.
struct PlaceNamingRules {
    naming: branches::PlaceNaming,
    evidence: &'static [Shown],
}

/// What crawl 0.34 shows of the statuses that put the player in danger: poison that will kill
/// (`_describe_poison`, `status.cc:1198`), petrification, liquid fire, which `fill_status_info`
/// lights hotter as it burns hotter (`status.cc:623`), and confusion.
const DANGER_0_34: DangerRules = DangerRules {
    lethal_poison: [
        Shown {
            text: "lethally",
            file: "status.cc",
            evidence: Some("(pois_perc >= 100) ? \"lethally\""),
        },
        Shown {
            text: " poisoned",
            file: "status.cc",
            evidence: Some("adj + \" poisoned\""),
        },
    ],
    petrification_lights: &[
        Shown::quoted("Petr", "duration-data.h"),
        Shown::quoted("Stone", "duration-data.h"),
    ],
    liquid_fire_lights: &[
        Shown::quoted("Fire", "duration-data.h"),
        Shown::quoted("Fire+", "status.cc"),
        Shown::quoted("Fire++", "status.cc"),
    ],
    confusion_lights: &[Shown::quoted("Conf", "duration-data.h")],
};

/// What a crawl release series shows of the statuses that put the player in danger, each text
/// checked against crawl's source.
struct DangerRules {
    /// The two parts of the words that describe poison that will kill the player.
    lethal_poison: [Shown; 2],
    petrification_lights: &'static [Shown],
    liquid_fire_lights: &'static [Shown],
    confusion_lights: &'static [Shown],
}

/// What crawl 0.34 decides in code about the player's holiness: `species::is_nonliving`
/// (`species.cc:313`), `player::undead_or_demonic` (`player.cc:6893`), `is_good_god`
/// (`religion.cc:517`), `player::undead_state` (`player.cc:7268`) and `player::holiness`
/// (`player.cc:6859`).
const HOLINESS_0_34: HolinessRules = HolinessRules {
    nonliving_species: &["SP_GARGOYLE", "SP_DJINNI"],
    demonic_species: &["SP_DEMONSPAWN"],
    good_gods: &[
        Shown::quoted("Zin", "religion.cc"),
        Shown::quoted("the Shining One", "religion.cc"),
        Shown::quoted("Elyvilon", "religion.cc"),
    ],
    undead_forms: &[
        ("death", Undeadness::Undead),
        ("vampire", Undeadness::SemiUndead),
        ("bat_swarm", Undeadness::SemiUndead),
    ],
    undead_adding_forms: &["vampire", "bat_swarm"],
    torment_halving_forms: &["statue", "vampire", "bat_swarm"],
    petrified_lights: &[Shown::quoted("Stone", "duration-data.h")],
};

/// What a crawl release series decides in code about the player's holiness, each species, god,
/// form and status light checked against what the extractor read or crawl's source.
struct HolinessRules {
    /// The species it makes nonliving, by `species_type` identifier.
    nonliving_species: &'static [&'static str],
    /// The species it counts as demonic, by `species_type` identifier.
    demonic_species: &'static [&'static str],
    /// The good gods, as the game names them.
    good_gods: &'static [Shown],
    /// The forms that make the player undead whatever their species, by `transformation`
    /// identifier, with how undead.
    undead_forms: &'static [(&'static str, Undeadness)],
    /// The forms that add undead to the species' holiness rather than setting their own.
    undead_adding_forms: &'static [&'static str],
    /// The forms `torment_player` (`spl-goditem.cc`) halves torment for.
    torment_halving_forms: &'static [&'static str],
    /// The status lights of being petrified.
    petrified_lights: &'static [Shown],
}

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
    /// The kinds `handle_monster_move`, or `_pre_monster_move` before it, acts or moves for with
    /// code of its own: crawl tests for each by kind.
    own_actions: NamedKinds,
    /// The kinds `_fungal_move_check` lets step toward a foe that watches them only from beside
    /// them (`WatchedStep::FromBeside`).
    steps_from_beside: NamedKinds,
    /// The kinds `_fungal_move_check` lets step toward a foe that watches them only by chance
    /// (`WatchedStep::ByChance`).
    steps_by_chance: NamedKinds,
    /// The kinds `explode_monster` blows up into torment as they die.
    torments_as_it_dies: NamedKinds,
    /// What it shows that tells the player's Lighting.
    lighting: &'static LightingRules,
    /// What it decides in code about the player's holiness.
    holiness: &'static HolinessRules,
    /// What it shows of the statuses that put the player in danger.
    danger: &'static DangerRules,
    /// How its webtiles names the place a level of a branch one level deep is.
    place_naming: &'static PlaceNamingRules,
    /// What it decides about dungeon features in code rather than in `feat_defs`.
    features: &'static features::FeatureRules,
    /// What it counts as a Way Out, and what stops each.
    ways_out: &'static ways_out::WayOutRules,
    /// What it reads of the player's speed.
    speed: &'static speed::SpeedRules,
    /// What it says as the character changes.
    character: &'static character::CharacterRules,
    /// What it says of the level the player is on.
    level: &'static level::LevelRules,
    /// The message it gives when the player learns the kind of an item they carry by seeing
    /// another of its kind identified.
    kind_learned: Shown,
    /// The item classes whose items hide their sub-type until identified: crawl's
    /// `item_type_has_ids` (`item-name.cc`), which is code rather than data.
    kinds_hidden: &'static [&'static str],
}

/// Kinds of monster crawl's code singles out by name, ported by hand, with where it does so.
struct NamedKinds {
    /// The source file of that code.
    file: &'static str,
    /// The functions, one of which names each kind.
    functions: &'static [&'static str],
    /// The kinds, by `monster_type` identifier.
    kinds: &'static [&'static str],
}

/// The monsters crawl never counts as firewood whatever their flags
/// (`mons_class_is_firewood`): the series' test monsters, and every one of `tentacles`. Each
/// must be in `monster_type`, so a renamed one fails rather than turns into firewood.
fn never_firewood(
    series: &Series,
    tentacles: &BTreeSet<String>,
    monster_type: &[(String, i64)],
) -> Result<BTreeSet<String>, ExtractError> {
    let never = tentacles
        .iter()
        .map(String::as_str)
        .chain(series.test_monsters.iter().copied());
    known_monsters(never, monster_type)
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

/// Marks `form` as undead, or as adding undead to the species' holiness, where `rules` says
/// crawl's code does.
fn mark_undead(form: &mut Form, rules: &HolinessRules) {
    if let Some(&(_, undead)) = rules.undead_forms.iter().find(|(id, _)| *id == form.id) {
        form.undead = undead;
    }
    if rules.undead_adding_forms.contains(&form.id.as_str()) {
        form.holiness = FormHoliness::AddsUndead;
    }
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

/// The enumerator that ends `skill_type`'s skills.
const SKILLS_END: &str = "NUM_SKILLS";

/// The enumerator that ends `object_class_type`'s item classes.
const OBJECT_CLASSES_END: &str = "NUM_OBJECT_CLASSES";

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

/// What `weapon_brands_verbose` names a brand crawl has retired, kept only so old saves load.
const RETIRED_BRAND: &str = "obsolescence";

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
    // What the code names that isn't checked as it is read, each against the enum it is from.
    known_monsters(series.gear.monsters(), &monster_type)?;
    crawl.require(
        "item-prop-enum.h",
        &["weapon_type", "missile_type"],
        series
            .gear
            .missile_weapons()
            .into_iter()
            .chain([gear::NO_WEAPON]),
    )?;
    crawl.require("item-prop-enum.h", &["wand_type"], series.gear.wands())?;
    crawl.require(
        "skill-type.h",
        &["skill_type"],
        [
            gear::LAUNCHER_SKILL,
            gear::THROWING_SKILL,
            gear::POLEARM_SKILL,
        ],
    )?;
    crawl.require("object-class-type.h", &["object_class_type"], [gear::WANDS])?;
    crawl.require(
        "mon-holy-type.h",
        &["mon_holy_type_flags"],
        [yaml::DEFAULT_HOLINESS],
    )?;
    crawl.require("feature.h", &["feature_flag_type"], [features::SOLID_FLAG])?;
    crawl.require("map-feature.h", &["map_feature"], features::MAP_FEATURES)?;
    crawl.require(
        "dungeon-char-type.h",
        &["dungeon_char_type"],
        features::STAIR_CHARS,
    )?;
    // `mon-mst.h`, which defines `mon_spellbook_type`, is made by crawl's build, not checked in.
    crawl.require_written("util/gen-mst.pl", [yaml::PER_MONSTER_SPELLBOOK])?;
    let tentacles = crawl.tentacles(&monster_type)?;
    let exceptions = yaml::KindExceptions {
        never_firewood: never_firewood(series, &tentacles, &monster_type)?,
        innate_umbras: known_monsters(series.innate_umbras.iter().copied(), &monster_type)?,
        own_actions: crawl.named_kinds(&series.own_actions, &monster_type)?,
        steps_from_beside: crawl.named_kinds(&series.steps_from_beside, &monster_type)?,
        steps_by_chance: crawl.named_kinds(&series.steps_by_chance, &monster_type)?,
        torments_as_it_dies: crawl.named_kinds(&series.torments_as_it_dies, &monster_type)?,
        special_abilities: crawl.special_abilities(&monster_type)?,
        tentacles,
    };
    let item_classes = crawl.item_classes(&item_names, series.kinds_hidden)?;
    let spells = crawl.spells()?;
    let ways_out = crawl.ways_out(series.ways_out, &item_classes, &spells)?;
    let species = yaml::species(&crawl.source, crawl.tag_major)?;
    let mutations = crawl.mutations()?;
    let banes = crawl.banes(series.character)?;
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
        item_classes,
        spells,
        missile_weapons,
        melee_weapons: gear::melee_weapons(&item_prop, "item-prop.cc")?,
        wands: gear::wands(&crawl.parse("spl-book.cc")?, "spl-book.cc", &item_names)?,
        weapon_brands: crawl.weapon_brands()?,
        holiness: crawl.holiness(series.holiness, &species)?,
        species,
        skills: crawl.skills()?,
        features: features::features(
            &crawl.parse("feature-data.h")?,
            "feature-data.h",
            &crawl.enumeration("dungeon-feature-type.h", "dungeon_feature_type")?,
            &crawl.parse("terrain.cc")?,
            &branches::entrances(
                &crawl.parse("branch-data.h")?,
                series.place_naming.naming.unconnected_flag,
            )?,
            series.features,
        )?,
        attitudes: crawl.values("mon-attitude-type.h", "mon_attitude_type", "")?,
        monster_threats: crawl.values("mon-util.h", "mon_threat_level_type", "MTHRT_UNDEF")?,
        mouse_modes: crawl.declared_values("defines.h", "mouse_mode", "MOUSE_MODE_MAX")?,
        branches: branches::branches(
            &crawl.parse("branch-type.h")?,
            &crawl.parse("branch-data.h")?,
            &crawl.parse("mon-pick-data.h")?,
            &monster_type,
            crawl.place_naming(series.place_naming)?,
        )?,
        danger: crawl.danger(series.danger)?,
        identification: IdentificationSigns {
            kind_learned: crawl.shown(&series.kind_learned)?,
        },
        lighting: crawl.lighting(series.lighting)?,
        ways_out,
        speed: crawl.speed(
            series.speed,
            yaml::unformed(&crawl.source, crawl.tag_major)?,
            &mutations,
            &banes,
        )?,
        character: crawl.character(series.character, &mutations, banes)?,
        level: crawl.level(series.level)?,
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

    /// Fails unless each of `ids` is an enumerator of one of the enums `names` in `file`: a
    /// value the extractor's code names fails the extraction once crawl renames it, rather than
    /// quietly never matching.
    fn require<'a>(
        &self,
        file: &str,
        names: &[&str],
        ids: impl IntoIterator<Item = &'a str>,
    ) -> Result<(), ExtractError> {
        let source = self.parse(file)?;
        let mut enumerators = Vec::new();
        for name in names {
            enumerators.extend(source.enumerator_names(name)?);
        }
        match ids.into_iter().find(|id| !enumerators.contains(id)) {
            Some(missing) => Err(ExtractError::Missing {
                file: file.to_owned(),
                what: missing.to_owned(),
            }),
            None => Ok(()),
        }
    }

    /// Fails unless the script `file`, which crawl's build runs to write a header, names each of
    /// `ids` as a whole word: for an identifier only a generated header defines, as
    /// `util/gen-mst.pl` writes `MST_GHOST` into `mon-mst.h`.
    fn require_written<'a>(
        &self,
        file: &str,
        ids: impl IntoIterator<Item = &'a str>,
    ) -> Result<(), ExtractError> {
        let script = read(&self.source.join(file))?;
        let words: BTreeSet<&str> = script
            .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
            .collect();
        match ids.into_iter().find(|id| !words.contains(id)) {
            Some(missing) => Err(ExtractError::Missing {
                file: file.to_owned(),
                what: missing.to_owned(),
            }),
            None => Ok(()),
        }
    }

    /// The enumerators of `enum <name>` before `end` (or all of them, when `end` is empty), by
    /// value. An enumerator that aliases an earlier one's value keeps the earlier one's name.
    fn values<T: TryFrom<i64> + Ord>(
        &self,
        file: &str,
        name: &str,
        end: &str,
    ) -> Result<BTreeMap<T, EnumId>, ExtractError> {
        values_of(&self.parse(file)?, file, name, end)
    }

    /// As [`Crawl::values`], for an enum read from its own declaration in `file`.
    fn declared_values<T: TryFrom<i64> + Ord>(
        &self,
        file: &str,
        name: &str,
        end: &str,
    ) -> Result<BTreeMap<T, EnumId>, ExtractError> {
        values_of(&self.declared_enum(file, name)?, file, name, end)
    }

    /// `enum <name>` alone, parsed from its own declaration in `file`: for a header whose other
    /// conditionals the extractor can't read, as `defines.h`'s test the build's CPU.
    fn declared_enum(&self, file: &str, name: &str) -> Result<Source, ExtractError> {
        let text = read(&self.source.join(file))?;
        let declaration = text
            .find(&format!("enum {name}"))
            .and_then(|start| {
                let end = start + text[start..].find("};")? + 2;
                text.get(start..end)
            })
            .ok_or_else(|| ExtractError::Missing {
                file: file.to_owned(),
                what: format!("enum {name}"),
            })?;
        Source::parse(file, declaration, self.tag_major)
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

    /// Every item class before [`OBJECT_CLASSES_END`], with the sub-types crawl names.
    fn item_classes(
        &self,
        names: &BTreeMap<String, String>,
        kinds_hidden: &[&str],
    ) -> Result<BTreeMap<u8, ItemClass>, ExtractError> {
        let (file, name) = ("object-class-type.h", "object_class_type");
        let classes_with_sub_types = ITEM_SUB_TYPE_ENUMS.iter().map(|&(class, ..)| class);
        self.require(
            file,
            &[name],
            kinds_hidden
                .iter()
                .copied()
                .chain(classes_with_sub_types)
                .chain([OBJECT_CLASSES_END]),
        )?;
        let mut classes = BTreeMap::new();
        for (id, value) in self
            .enumeration(file, name)?
            .into_iter()
            .take_while(|(id, _)| id != OBJECT_CLASSES_END)
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
                    kinds_hidden: kinds_hidden.contains(&id.as_str()),
                    id: EnumId(id),
                    sub_types,
                },
            );
        }
        Ok(classes)
    }

    /// Every skill before [`SKILLS_END`], by its `skill_type` identifier, named by the first
    /// column of its row of `skill_titles` in `skills.cc`, which lists them in the enum's order.
    fn skills(&self) -> Result<BTreeMap<EnumId, String>, ExtractError> {
        let file = "skills.cc";
        let unreadable = |what: &str| ExtractError::Unreadable {
            file: file.to_owned(),
            what: what.to_owned(),
        };
        let names = self
            .parse(file)?
            .entries("skill_titles")?
            .into_iter()
            .map(|row| match row.first().map(Vec::as_slice) {
                Some([Token::Str(name)]) => Ok(name.clone()),
                _ => Err(unreadable("a row of skill_titles")),
            })
            .collect::<Result<Vec<String>, ExtractError>>()?;
        let skills: BTreeMap<i64, EnumId> =
            self.values("skill-type.h", "skill_type", SKILLS_END)?;
        if skills.len() != names.len() {
            return Err(unreadable("as many rows of skill_titles as skills"));
        }
        Ok(skills.into_values().zip(names).collect())
    }

    /// The brands a weapon may have, by `brand_type` identifier, with how a weapon's name shows
    /// each (`weapon_brand_desc` in `item-name.cc`): its words from `weapon_brands_adj` when it is
    /// in `brand_prefers_adj` and named before the weapon, from `weapon_brands_verbose` otherwise.
    /// Both tables run in `brand_type` order. Brands crawl has retired, which it names
    /// `obsolescence`, and the brand of none are left out.
    fn weapon_brands(&self) -> Result<BTreeMap<EnumId, WeaponBrand>, ExtractError> {
        let file = "item-name.cc";
        let source = self.parse(file)?;
        let unreadable = |what: &str| ExtractError::Unreadable {
            file: file.to_owned(),
            what: what.to_owned(),
        };
        let words = |table: &str| -> Result<Vec<String>, ExtractError> {
            cpp::split_top_level(source.initializer(table)?)
                .into_iter()
                .map(|entry| match entry.as_slice() {
                    [Token::Str(words)] => Ok(words.clone()),
                    _ => Err(unreadable(&format!("an entry of {table}"))),
                })
                .collect()
        };
        let verbose = words("weapon_brands_verbose")?;
        let adjectives = words("weapon_brands_adj")?;
        let before = cpp::split_top_level(source.initializer("brand_prefers_adj")?)
            .into_iter()
            .map(|entry| match entry.as_slice() {
                [Token::Ident(id)] => Ok(id.clone()),
                _ => Err(unreadable("an entry of brand_prefers_adj")),
            })
            .collect::<Result<BTreeSet<String>, ExtractError>>()?;
        let mut brands = BTreeMap::new();
        for (id, value) in self.enumeration("item-prop-enum.h", "brand_type")? {
            // `NUM_` and `MAX_` enumerators count brands rather than name one.
            let Some(index) = usize::try_from(value)
                .ok()
                .filter(|_| id.starts_with("SPWPN_"))
            else {
                continue;
            };
            let (Some(name), Some(adjective)) = (verbose.get(index), adjectives.get(index)) else {
                return Err(unreadable(&format!("the name of {id}")));
            };
            if name.is_empty() || name == RETIRED_BRAND {
                continue;
            }
            let before = before.contains(&id);
            brands.insert(
                EnumId(id),
                WeaponBrand {
                    name: if before { adjective } else { name }.clone(),
                    before,
                },
            );
        }
        Ok(brands)
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
        let beam = self.parse("beam.cc")?;
        let cloud_types = spells::cloud_types(&beam, "beam.cc")?;
        let trails = spells::Trails::read(&beam, "beam.cc")?;
        let clouds = self.enumeration("cloud-type.h", "cloud_type")?;
        let opaque = spells::opaque_clouds(&self.parse("cloud.cc")?, "cloud.cc", &clouds)?;
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
            let cloud = cloud_types
                .get(&entry.id)
                .map(|kind| {
                    if !clouds.iter().any(|(name, _)| name == kind) {
                        return Err(ExtractError::Missing {
                            file: "cloud-type.h".to_owned(),
                            what: format!("{kind}, which beam.cc gives {}", entry.id),
                        });
                    }
                    let flavour = zap.as_ref().map(|zap| zap.flavour.0.as_str());
                    Ok(SpellCloud {
                        kind: EnumId(kind.clone()),
                        opaque: opaque.contains(kind),
                        trail: trails.leaves(&entry.id, flavour, kind),
                    })
                })
                .transpose()?;
            spells.insert(
                in_range(&entry.id, value)?,
                Spell {
                    id: EnumId(entry.id),
                    name: entry.name,
                    schools: entry.schools,
                    range: entry.range,
                    zap,
                    cloud,
                },
            );
        }
        Ok(spells)
    }

    /// Every tentacle and tentacle segment (`mons_is_tentacle_or_tentacle_segment`), from the
    /// tables in `mon-tentacle.cc`. Each must be in `monster_type`.
    fn tentacles(&self, monster_type: &[(String, i64)]) -> Result<BTreeSet<String>, ExtractError> {
        let file = "mon-tentacle.cc";
        let source = self.parse(file)?;
        let mut tentacles = BTreeSet::new();
        for &(table, skipped) in TENTACLE_TABLES {
            for row in source.entries(table)? {
                for field in row.iter().skip(skipped) {
                    let [Token::Ident(id)] = field.as_slice() else {
                        return Err(ExtractError::Unreadable {
                            file: file.to_owned(),
                            what: format!("a row of {table}"),
                        });
                    };
                    tentacles.insert(id.clone());
                }
            }
        }
        known_monsters(tentacles.iter().map(String::as_str), monster_type)
    }

    /// The kinds of `named`, each checked against `monster_type` and against crawl's code, which
    /// must name it in one of the functions `named` gives, so a kind crawl stops singling out
    /// fails the extraction rather than keeping a rule it no longer has.
    fn named_kinds(
        &self,
        named: &NamedKinds,
        monster_type: &[(String, i64)],
    ) -> Result<BTreeSet<String>, ExtractError> {
        let source = self.parse(named.file)?;
        let bodies = named
            .functions
            .iter()
            .map(|function| source.function_body(function))
            .collect::<Result<Vec<_>, _>>()?;
        for kind in named.kinds {
            let ident = Token::Ident((*kind).to_owned());
            if !bodies.iter().any(|body| body.contains(&ident)) {
                return Err(ExtractError::Missing {
                    file: named.file.to_owned(),
                    what: format!("{kind}, in {}", named.functions.join(" or ")),
                });
            }
        }
        known_monsters(named.kinds.iter().copied(), monster_type)
    }

    /// The kinds with a special ability of their own: those `mon_special_ability`
    /// (`mon-abil.cc`) has a case for. Each must be in `monster_type`.
    fn special_abilities(
        &self,
        monster_type: &[(String, i64)],
    ) -> Result<BTreeSet<String>, ExtractError> {
        let cases = self
            .parse("mon-abil.cc")?
            .switch_cases("mon_special_ability")?;
        known_monsters(cases.iter().map(String::as_str), monster_type)
    }

    /// The text of `shown`, once found in crawl's source as it says.
    fn shown(&self, shown: &Shown) -> Result<String, ExtractError> {
        let evidence = shown.evidence();
        let unspaced = |text: &str| text.split_whitespace().collect::<String>();
        if unspaced(&read(&self.source.join(shown.file))?).contains(&unspaced(&evidence)) {
            Ok(shown.text.to_owned())
        } else {
            Err(ExtractError::Missing {
                file: shown.file.to_owned(),
                what: evidence,
            })
        }
    }

    /// The texts of `rules`, each found in crawl's source as its rule says.
    fn lighting(&self, rules: &LightingRules) -> Result<LightingSigns, ExtractError> {
        let texts = |shown: &[Shown]| -> Result<Vec<String>, ExtractError> {
            shown.iter().map(|shown| self.shown(shown)).collect()
        };
        Ok(LightingSigns {
            backlighting_lights: texts(rules.backlighting_lights)?,
            halo_gods: texts(rules.halo_gods)?,
            halo_statuses: texts(rules.halo_statuses)?,
            halo_gear: texts(rules.halo_gear)?,
        })
    }

    /// How `rules` names places, once crawl's source holds what they say it does.
    fn place_naming<'r>(
        &self,
        rules: &'r PlaceNamingRules,
    ) -> Result<&'r branches::PlaceNaming, ExtractError> {
        for shown in rules.evidence {
            self.shown(shown)?;
        }
        Ok(&rules.naming)
    }

    /// The texts of `rules`, each found in crawl's source as its rule says.
    fn danger(&self, rules: &DangerRules) -> Result<DangerSigns, ExtractError> {
        let texts = |shown: &[Shown]| -> Result<Vec<String>, ExtractError> {
            shown.iter().map(|shown| self.shown(shown)).collect()
        };
        Ok(DangerSigns {
            lethal_poison: texts(&rules.lethal_poison)?.concat(),
            petrification_lights: texts(rules.petrification_lights)?,
            liquid_fire_lights: texts(rules.liquid_fire_lights)?,
            confusion_lights: texts(rules.confusion_lights)?,
        })
    }

    /// The player's holiness signs: the forms in `dat/forms` marked as `rules` says crawl's code
    /// treats them, and its species, gods and status lights, each found among `species` or in
    /// crawl's source.
    fn holiness(
        &self,
        rules: &HolinessRules,
        species: &[Species],
    ) -> Result<HolinessSigns, ExtractError> {
        let known_species = |ids: &[&str]| -> Result<Vec<EnumId>, ExtractError> {
            ids.iter()
                .map(|&id| {
                    if species.iter().any(|known| known.id.0 == id) {
                        Ok(EnumId(id.to_owned()))
                    } else {
                        Err(ExtractError::Missing {
                            file: "dat/species".to_owned(),
                            what: id.to_owned(),
                        })
                    }
                })
                .collect()
        };
        let mut forms = yaml::forms(&self.source, self.tag_major)?;
        let named = rules
            .undead_forms
            .iter()
            .map(|(id, _)| id)
            .chain(rules.undead_adding_forms)
            .chain(rules.torment_halving_forms);
        for id in named {
            if !forms.iter().any(|form| form.id == *id) {
                return Err(ExtractError::Missing {
                    file: "dat/forms".to_owned(),
                    what: (*id).to_owned(),
                });
            }
        }
        let source = self.parse("spl-goditem.cc")?;
        let torment = source.function_body("torment_player")?;
        for id in rules.torment_halving_forms {
            if !torment.contains(&Token::Ident((*id).to_owned())) {
                return Err(ExtractError::Missing {
                    file: "spl-goditem.cc".to_owned(),
                    what: format!("{id}, in torment_player"),
                });
            }
        }
        for form in &mut forms {
            mark_undead(form, rules);
            form.halves_torment = rules.torment_halving_forms.contains(&form.id.as_str());
        }
        Ok(HolinessSigns {
            nonliving_species: known_species(rules.nonliving_species)?,
            demonic_species: known_species(rules.demonic_species)?,
            good_gods: rules
                .good_gods
                .iter()
                .map(|shown| self.shown(shown))
                .collect::<Result<_, _>>()?,
            forms,
            petrified_lights: rules
                .petrified_lights
                .iter()
                .map(|shown| self.shown(shown))
                .collect::<Result<_, _>>()?,
        })
    }

    /// Every monster spellbook, by its `mon_spellbook_type` identifier.
    fn spellbooks(&self) -> Result<BTreeMap<String, Vec<MonsterSpell>>, ExtractError> {
        spells::spellbooks(&self.parse("mon-spell.h")?, "mon-spell.h")
    }
}

/// The enumerators of `enum <name>` in `source`, read from `file`, before `end` (or all of
/// them, when `end` is empty), by value. An enumerator that aliases an earlier one's value keeps
/// the earlier one's name.
fn values_of<T: TryFrom<i64> + Ord>(
    source: &Source,
    file: &str,
    name: &str,
    end: &str,
) -> Result<BTreeMap<T, EnumId>, ExtractError> {
    let mut enumeration = source.enumeration(name)?;
    if !end.is_empty() {
        let at = enumeration
            .iter()
            .position(|(id, _)| id == end)
            .ok_or_else(|| ExtractError::Missing {
                file: file.to_owned(),
                what: end.to_owned(),
            })?;
        enumeration.truncate(at);
    }
    let mut values = BTreeMap::new();
    for (id, value) in enumeration {
        values.entry(in_range(&id, value)?).or_insert(EnumId(id));
    }
    Ok(values)
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
