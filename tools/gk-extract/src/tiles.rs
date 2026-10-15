//! Monster tiles from crawl's `rltiles`: which image file crawl's tile build draws each monster
//! tile from, read from the tile lists as `rltiles/tool/tile_list_processor.cc` reads them, and
//! the tiles a bundle names, written out for the app to ship.
//!
//! Only a tile drawn straight from one image file is named. A tile the build recolours, mirrors,
//! composes onto a background or turns into a corpse has no file of its own, so it is left out.
//! A tile the build outlines in black (`%rim 1`) ships outlined, as the build outlines it.

use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use crate::image::Image;
use crate::{ExtractError, read};

/// The tile list crawl's monster tiles are in, which includes the rest.
const PLAYER_TILES: &str = "dc-player.txt";

/// The prefix the player tile list gives its enums, as its `%prefix` line says.
const PLAYER_PREFIX: &str = "TILEP";

/// The tile lists' directives that don't change which file a tile is drawn from.
const IGNORED_DIRECTIVES: &[&str] = &[
    "name",
    "startvalue",
    "weight",
    "parts_ctg",
    "end_ctg",
    "variation",
    "enchant_variation",
    "repeat_ctg",
];

/// The tiles a bundle names, which the app ships under `tiles/`, gathered as the bundle is
/// extracted.
#[derive(Debug)]
pub struct ShippedTiles {
    files: TileFiles,
    /// Whether each file shipped is outlined, by the file.
    shipped: BTreeMap<String, bool>,
}

impl ShippedTiles {
    /// No tiles yet, to be picked from the tile lists in `rltiles`.
    ///
    /// # Errors
    /// If a list can't be read, or uses a directive the extractor doesn't understand.
    pub(crate) fn new(rltiles: &Path) -> Result<Self, ExtractError> {
        Ok(Self {
            files: TileFiles::read(rltiles)?,
            shipped: BTreeMap::new(),
        })
    }

    /// Ships the tile enum such as `TILEP_MONS_JACKAL`, returning the tile the bundle names it
    /// by: its file's path in `rltiles` without the extension, such as `mon/animals/jackal`.
    /// `None` for a tile with no file of its own.
    ///
    /// # Errors
    /// If the file is already shipped for another tile enum, but drawn with the outline where
    /// this one isn't or the other way round: the app ships one image per file.
    pub(crate) fn ship(&mut self, tile: &str) -> Result<Option<String>, ExtractError> {
        let Some(Tile { file, outlined }) = self.files.tile(tile) else {
            return Ok(None);
        };
        let shipped = *self.shipped.entry(file.clone()).or_insert(*outlined);
        if shipped != *outlined {
            return Err(ExtractError::Unreadable {
                file: PLAYER_TILES.to_owned(),
                what: format!("{tile}: {file} is drawn both with and without its outline"),
            });
        }
        Ok(Some(file.clone()))
    }

    /// Writes every tile shipped into `out`, at the path the bundle names it by with `.png`
    /// added, from the crawl checkout at `crawl`, and returns how many it wrote. A tile the
    /// build outlines is written outlined; the rest are copied unchanged.
    ///
    /// # Errors
    /// If a tile can't be read, decoded or written.
    pub fn write(&self, crawl: &Path, out: &Path) -> Result<usize, ExtractError> {
        let rltiles = crawl.join(crate::SOURCE_DIR).join(crate::RLTILES_DIR);
        for (file, outlined) in &self.shipped {
            let from = rltiles.join(format!("{file}.png"));
            let to = out.join(format!("{file}.png"));
            let io = |source| ExtractError::Io {
                path: to.clone(),
                source,
            };
            if let Some(dir) = to.parent() {
                std::fs::create_dir_all(dir).map_err(io)?;
            }
            if *outlined {
                let bytes = std::fs::read(&from).map_err(|source| ExtractError::Io {
                    path: from.clone(),
                    source,
                })?;
                let mut image =
                    Image::decode(&bytes).map_err(|reason| ExtractError::Unreadable {
                        file: format!("{file}.png"),
                        what: format!("the image: {reason}"),
                    })?;
                image.outline();
                let encoded = image.encode().map_err(|reason| ExtractError::Unreadable {
                    file: format!("{file}.png"),
                    what: format!("the outlined image: {reason}"),
                })?;
                std::fs::write(&to, encoded).map_err(io)?;
            } else {
                std::fs::copy(&from, &to).map_err(io)?;
            }
        }
        Ok(self.shipped.len())
    }
}

/// A tile crawl's build draws straight from one image file.
#[derive(Debug, Clone)]
struct Tile {
    /// The file's path in `rltiles` without the extension, such as `mon/animals/jackal`.
    file: String,
    /// The build outlines it in black (`%rim 1`).
    outlined: bool,
}

/// The tile of each enum in crawl's player tile list, by its enum without the prefix.
#[derive(Debug, Default)]
struct TileFiles {
    /// `None` for a tile with no file of its own.
    files: HashMap<String, Option<Tile>>,
}

impl TileFiles {
    /// Reads the tile lists in `rltiles`.
    fn read(rltiles: &Path) -> Result<Self, ExtractError> {
        let mut list = TileList {
            rltiles,
            prefix: String::new(),
            sdir: String::new(),
            rim: false,
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

    /// The tile a tile enum such as `TILEP_MONS_JACKAL` names.
    fn tile(&self, tile: &str) -> Option<&Tile> {
        let name = tile.strip_prefix(PLAYER_PREFIX)?.strip_prefix('_')?;
        self.files.get(name)?.as_ref()
    }
}

/// Reading the tile lists, line by line, with the state their directives set.
struct TileList<'a> {
    rltiles: &'a Path,
    prefix: String,
    sdir: String,
    /// Tiles from here on are outlined (`%rim`).
    rim: bool,
    recoloured: bool,
    mirrored: bool,
    corpse: bool,
    backed: bool,
    textured: bool,
    /// The last tile added, for `%syn`.
    last: Option<Option<Tile>>,
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
            "rim" => self.rim = first != "0",
            "hue" | "desat" | "lum" | "pal" | "alpha" => self.recoloured = true,
            "resetcol" => self.recoloured = false,
            "mirror_horizontal" => self.mirrored = true,
            "reset_mirror" => self.mirrored = false,
            "corpse" => self.corpse = first != "0",
            "back" => self.backed = first != "none",
            "texture" => self.textured = first != "none",
            "syn" => {
                let tile = self.last.clone().unwrap_or_default();
                for name in args {
                    self.files
                        .files
                        .entry((*name).to_owned())
                        .or_insert(tile.clone());
                }
            }
            "repeat" => {
                // A copy of an earlier tile, outlined or not as it was, with the colours and
                // mirroring now in effect.
                let source = self.files.files.get(first).cloned().flatten();
                let tile = source.filter(|_| !self.transformed());
                self.add(tile, args.get(1..).unwrap_or_default());
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
        let tile = if self.transformed() || self.backed {
            None
        } else {
            self.image_file(image).map(|file| Tile {
                file,
                outlined: self.rim,
            })
        };
        self.add(tile, names);
    }

    fn add(&mut self, tile: Option<Tile>, names: &[&str]) {
        for name in names {
            self.files
                .files
                .entry((*name).to_owned())
                .or_insert(tile.clone());
        }
        self.last = Some(tile);
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
