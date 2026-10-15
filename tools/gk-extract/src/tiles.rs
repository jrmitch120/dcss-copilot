//! Monster tiles from crawl's `rltiles`: which image file crawl's tile build draws each monster
//! tile from, read from the tile lists as `rltiles/tool/tile_list_processor.cc` reads them.
//!
//! Only a tile drawn straight from one image file is named. A tile the build recolours, mirrors,
//! composes onto a background or turns into a corpse has no file of its own, so it is left out,
//! as is a tile whose file is on the list of crawl tiles under an unknown license. The black rim
//! the build draws around some tiles (`%rim`) isn't in the file, so those tiles ship without it.

use std::collections::{BTreeSet, HashMap};
use std::path::Path;

use crate::{ExtractError, read};

/// The tile list crawl's monster tiles are in, which includes the rest.
const PLAYER_TILES: &str = "dc-player.txt";

/// The prefix the player tile list gives its enums, as its `%prefix` line says.
const PLAYER_PREFIX: &str = "TILEP";

/// Crawl tiles whose license is unknown, by file name, which crawl's tile project leaves out of
/// any release of its tiles under CC0: `TILES_UNDER_UNKNOWN_LICENSE.md` at
/// <https://github.com/crawl/tiles> commit a6ea1655db5c044829d9eea19a232fa6fcac87b0.
const UNKNOWN_LICENSE: &str = include_str!("../tiles-under-unknown-license.txt");

/// The tile lists' directives that don't change which file a tile is drawn from.
const IGNORED_DIRECTIVES: &[&str] = &[
    "name",
    "startvalue",
    "rim",
    "weight",
    "parts_ctg",
    "end_ctg",
    "variation",
    "enchant_variation",
    "repeat_ctg",
];

/// The image file of each tile in crawl's player tile list, by its enum without the prefix.
#[derive(Debug, Default)]
pub struct TileFiles {
    /// `None` for a tile with no file of its own.
    files: HashMap<String, Option<String>>,
}

impl TileFiles {
    /// Reads the tile lists in `rltiles`.
    ///
    /// # Errors
    /// If a list can't be read, or uses a directive the extractor doesn't understand.
    pub fn read(rltiles: &Path) -> Result<Self, ExtractError> {
        let unknown_license: BTreeSet<&str> = UNKNOWN_LICENSE
            .lines()
            .filter(|line| !line.starts_with('#') && !line.trim().is_empty())
            .map(str::trim)
            .collect();
        let mut list = TileList {
            rltiles,
            unknown_license: &unknown_license,
            prefix: String::new(),
            sdir: String::new(),
            recoloured: false,
            mirrored: false,
            corpse: false,
            backed: false,
            textured: false,
            last: None,
            files: TileFiles::default(),
        };
        list.process(PLAYER_TILES)?;
        if list.prefix != PLAYER_PREFIX {
            return Err(ExtractError::Missing {
                file: PLAYER_TILES.to_owned(),
                what: format!("%prefix {PLAYER_PREFIX}"),
            });
        }
        Ok(list.files)
    }

    /// The file a tile enum such as `TILEP_MONS_JACKAL` is drawn from, as its path in `rltiles`
    /// without the extension, such as `mon/animals/jackal`.
    pub fn file(&self, tile: &str) -> Option<&str> {
        let name = tile.strip_prefix(PLAYER_PREFIX)?.strip_prefix('_')?;
        self.files.get(name)?.as_deref()
    }
}

/// Reading the tile lists, line by line, with the state their directives set.
struct TileList<'a> {
    rltiles: &'a Path,
    unknown_license: &'a BTreeSet<&'a str>,
    prefix: String,
    sdir: String,
    recoloured: bool,
    mirrored: bool,
    corpse: bool,
    backed: bool,
    textured: bool,
    /// The file of the last tile added, for `%syn`.
    last: Option<Option<String>>,
    files: TileFiles,
}

impl TileList<'_> {
    fn process(&mut self, list: &str) -> Result<(), ExtractError> {
        let text = read(&self.rltiles.join(list))?;
        for line in text.lines() {
            let line = without_comments(line);
            let args: Vec<&str> = line.split_whitespace().collect();
            let Some((&first, rest)) = args.split_first() else {
                continue;
            };
            if first.starts_with('#') {
                continue;
            }
            match first.strip_prefix('%') {
                Some(directive) => self.directive(list, directive, rest)?,
                None => self.tile(first, rest),
            }
        }
        Ok(())
    }

    fn directive(
        &mut self,
        list: &str,
        directive: &str,
        args: &[&str],
    ) -> Result<(), ExtractError> {
        let first = args.first().copied().unwrap_or_default();
        match directive {
            "include" => self.process(first)?,
            "prefix" => first.clone_into(&mut self.prefix),
            "sdir" => first.clone_into(&mut self.sdir),
            "hue" | "desat" | "lum" | "pal" | "alpha" => self.recoloured = true,
            "resetcol" => self.recoloured = false,
            "mirror_horizontal" => self.mirrored = true,
            "reset_mirror" => self.mirrored = false,
            "corpse" => self.corpse = first != "0",
            "back" => self.backed = first != "none",
            "texture" => self.textured = first != "none",
            "syn" => {
                let file = self.last.clone().unwrap_or_default();
                for name in args {
                    self.files
                        .files
                        .entry((*name).to_owned())
                        .or_insert(file.clone());
                }
            }
            "repeat" => {
                // A copy of an earlier tile, with the colours and mirroring now in effect.
                let source = self.files.files.get(first).cloned().flatten();
                let file = source.filter(|_| !self.transformed());
                self.add(file, args.get(1..).unwrap_or_default());
            }
            _ if IGNORED_DIRECTIVES.contains(&directive) => {}
            _ => {
                return Err(ExtractError::Unreadable {
                    file: list.to_owned(),
                    what: format!("the directive %{directive}"),
                });
            }
        }
        Ok(())
    }

    /// A tile line: an image file, then the enums that name it.
    fn tile(&mut self, image: &str, names: &[&str]) {
        let file = if self.transformed() || self.backed {
            None
        } else {
            self.image_file(image)
                .filter(|file| !self.under_unknown_license(file))
        };
        self.add(file, names);
    }

    fn add(&mut self, file: Option<String>, names: &[&str]) {
        for name in names {
            self.files
                .files
                .entry((*name).to_owned())
                .or_insert(file.clone());
        }
        self.last = Some(file);
    }

    fn transformed(&self) -> bool {
        self.recoloured || self.mirrored || self.corpse || self.textured
    }

    /// Where the build finds `image`: in the `%sdir` first, else from the top of `rltiles`
    /// (`tile_list_processor::load_image`). `None` for an image made from another tile.
    fn image_file(&self, image: &str) -> Option<String> {
        let in_sdir = (!self.sdir.is_empty()).then(|| format!("{}/{image}", self.sdir));
        in_sdir
            .into_iter()
            .chain([image.to_owned()])
            .find(|file| self.rltiles.join(format!("{file}.png")).is_file())
    }

    fn under_unknown_license(&self, file: &str) -> bool {
        let name = file.rsplit('/').next().unwrap_or(file);
        self.unknown_license
            .contains(format!("{name}.png").as_str())
    }
}

/// `line` without its `/* … */` comments.
fn without_comments(line: &str) -> String {
    let mut rest = line;
    let mut kept = String::new();
    while let Some(start) = rest.find("/*") {
        kept.push_str(&rest[..start]);
        rest = rest[start..]
            .find("*/")
            .map_or("", |end| &rest[start + end + 2..]);
    }
    kept.push_str(rest);
    kept
}
