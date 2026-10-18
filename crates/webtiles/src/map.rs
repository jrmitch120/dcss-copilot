//! The current level's map, merged from webtiles `map` deltas as the page's `map_knowledge.js`
//! merges them, and the monsters in view on it.

use std::collections::{BTreeMap, HashMap};

use copilot_core::{
    Attitude, Colour, CrawlThreat, Glyph, MapCell, Position, SquareLight, VisibleMonster,
};
use copilot_gk::{Bundle, EnumId};
use serde_json::Value;

/// Tile background flags that mark a cell the player can't see now, from crawl's
/// `tile-flags.h` (`TILE_FLAG_MM_UNSEEN | TILE_FLAG_UNSEEN`). The page's `enums.js` reads the same
/// bits; they have held since tiles came to webtiles.
const UNSEEN: u64 = 0x0002_0000 | 0x0004_0000;

/// The tile `halo` values that mark a square lit by a halo, and the first and last that mark one
/// in an umbra, from crawl's `tilecell.h:10` (`HALO_RANGE`, `HALO_UMBRA_FIRST` and
/// `HALO_UMBRA_LAST`); the umbra's four only vary how it is drawn. The page's `enums.js` reads
/// the same values.
const HALO_RANGE: i64 = 1;
const HALO_UMBRA: std::ops::RangeInclusive<i64> = 2..=5;

/// What a monster is called when neither the game nor Game Knowledge has named it.
const UNNAMED: &str = "unknown monster";

/// The colour the page draws a cell whose colour the game hasn't sent (`cell_renderer.js`).
const DEFAULT_COLOUR: u8 = 7;

/// The cells of the current level the game has described, and the monsters on them.
#[derive(Debug, Default)]
pub struct LevelMap {
    /// By `(y, x)`, so iterating goes top row first, left to right.
    cells: BTreeMap<(i64, i64), Cell>,
    /// Monsters with a client id, which a delta in one cell can update after they move.
    tracked: HashMap<i64, MonsterInfo>,
    /// Where the last merged cell was: a cell without coordinates follows it on the same row.
    last: (i64, i64),
    /// The game has sent a map since the game started.
    received: bool,
}

#[derive(Debug, Default)]
struct Cell {
    /// Its `dungeon_feature_type` value.
    feature: Option<i64>,
    glyph: Option<String>,
    colour: Option<u8>,
    /// The tile's background flags, once the game has sent tile data for the cell.
    tile_bg: Option<u64>,
    /// The tile's `halo` value: how a halo or umbra draws over it, 0 for neither until the game
    /// says otherwise.
    halo: i64,
    monster: Option<CellMonster>,
}

/// A monster on a cell: tracked by client id, or held by the cell when it has none.
#[derive(Debug, Clone)]
enum CellMonster {
    Tracked(i64),
    Untracked(MonsterInfo),
}

/// The monster fields merged so far.
#[derive(Debug, Clone, Default)]
struct MonsterInfo {
    name: Option<String>,
    /// Its `monster_type` value, which only the Crawl Version's Game Knowledge can decode.
    kind: Option<i64>,
    /// Its `mon_attitude_type` value.
    attitude: Option<i64>,
    /// Its `mon_threat_level_type` value.
    threat: Option<i64>,
}

impl MonsterInfo {
    fn merge(&mut self, delta: &Value) {
        if let Some(name) = delta.get("name").and_then(Value::as_str) {
            self.name = Some(name.to_owned());
        }
        if let Some(kind) = delta.get("type").and_then(Value::as_i64) {
            self.kind = Some(kind);
        }
        if let Some(attitude) = delta.get("att").and_then(Value::as_i64) {
            self.attitude = Some(attitude);
        }
        if let Some(threat) = delta.get("threat").and_then(Value::as_i64) {
            self.threat = Some(threat);
        }
    }
}

impl LevelMap {
    /// Merges a `map` message.
    pub fn merge(&mut self, msg: &Value) {
        self.received = true;
        if msg.get("clear").and_then(Value::as_bool) == Some(true) {
            self.cells.clear();
            self.tracked.clear();
        }
        for cell in msg
            .get("cells")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            self.merge_cell(cell);
        }
        // A monster no cell shows any more is gone, as the page's monster table forgets it.
        let shown: Vec<i64> = self
            .cells
            .values()
            .filter_map(|cell| match cell.monster {
                Some(CellMonster::Tracked(id)) => Some(id),
                _ => None,
            })
            .collect();
        self.tracked.retain(|id, _| shown.contains(id));
    }

    fn merge_cell(&mut self, delta: &Value) {
        let x = delta
            .get("x")
            .and_then(Value::as_i64)
            .unwrap_or(self.last.0 + 1);
        let y = delta
            .get("y")
            .and_then(Value::as_i64)
            .unwrap_or(self.last.1);
        self.last = (x, y);

        let cell = self.cells.entry((y, x)).or_default();
        if let Some(feature) = delta.get("f").and_then(Value::as_i64) {
            cell.feature = Some(feature);
        }
        if let Some(glyph) = delta.get("g").and_then(Value::as_str) {
            cell.glyph = Some(glyph.to_owned());
        }
        if let Some(colour) = delta.get("col").and_then(Value::as_u64) {
            // The low four bits are the colour; the rest is highlighting.
            cell.colour = u8::try_from(colour & 0xF).ok();
        }
        if let Some(tile) = delta.get("t") {
            let bg = tile.get("bg").map(low_word);
            cell.tile_bg = Some(bg.or(cell.tile_bg).unwrap_or(0));
            if let Some(halo) = tile.get("halo").and_then(Value::as_i64) {
                cell.halo = halo;
            }
        }
        if let Some(monster) = delta.get("mon") {
            let old = cell.monster.take();
            let merged = merge_monster(&mut self.tracked, old, monster);
            if let Some(cell) = self.cells.get_mut(&(y, x)) {
                cell.monster = merged;
            }
        }
    }

    /// The monsters on cells the player can see now, top row first, named by the game and
    /// identified through `knowledge` when there is a bundle for the game's Crawl Version.
    /// `None` until the game has sent a map.
    pub fn monsters_in_view(&self, knowledge: Option<&Bundle>) -> Option<Vec<VisibleMonster>> {
        if !self.received {
            return None;
        }
        let monsters = self
            .cells
            .iter()
            .filter(|(_, cell)| cell.in_view())
            .filter_map(|(&key, cell)| {
                let info = match cell.monster.as_ref()? {
                    CellMonster::Tracked(id) => self.tracked.get(id)?,
                    CellMonster::Untracked(info) => info,
                };
                let known = info
                    .kind
                    .and_then(|kind| u16::try_from(kind).ok())
                    .and_then(|kind| knowledge?.monster(kind));
                Some(VisibleMonster {
                    // The game names every monster it first shows; the fallbacks are for a
                    // stream that missed its start.
                    name: info
                        .name
                        .clone()
                        .or_else(|| known.map(|monster| monster.name.clone()))
                        .unwrap_or_else(|| UNNAMED.to_owned()),
                    glyph: cell.glyph(),
                    id: known.map(|monster| monster.id.clone()),
                    position: position(key)?,
                    attitude: decode(info.attitude, knowledge, Bundle::attitude).and_then(attitude),
                    crawl_threat: decode(info.threat, knowledge, Bundle::monster_threat)
                        .and_then(crawl_threat),
                })
            })
            .collect();
        Some(monsters)
    }

    /// The level as the game shows it, with each square's feature decoded through `knowledge`
    /// when there is a bundle for the game's Crawl Version. `None` until the game has sent a map.
    pub fn level(&self, knowledge: Option<&Bundle>) -> Option<copilot_core::LevelMap> {
        if !self.received {
            return None;
        }
        Some(copilot_core::LevelMap::new(self.cells.iter().filter_map(
            |(&key, cell)| {
                // A square the game blanks out is one it doesn't show.
                cell.glyph
                    .as_deref()
                    .filter(|glyph| !glyph.trim().is_empty())?;
                let cell = MapCell {
                    glyph: cell.glyph(),
                    feature: decode(cell.feature, knowledge, Bundle::feature).cloned(),
                    in_view: cell.in_view(),
                    light: cell.light(),
                };
                Some((position(key)?, cell))
            },
        )))
    }
}

impl Cell {
    /// The player can see the cell now.
    fn in_view(&self) -> bool {
        self.tile_bg.is_some_and(|bg| bg & UNSEEN == 0)
    }

    /// Whether a halo lights the cell or an umbra darkens it.
    fn light(&self) -> SquareLight {
        match self.halo {
            HALO_RANGE => SquareLight::Halo,
            halo if HALO_UMBRA.contains(&halo) => SquareLight::Umbra,
            _ => SquareLight::Plain,
        }
    }

    fn glyph(&self) -> Glyph {
        Glyph {
            text: self.glyph.clone().unwrap_or_default(),
            colour: Colour::from_crawl(self.colour.unwrap_or(DEFAULT_COLOUR))
                .unwrap_or(Colour::LightGrey),
        }
    }
}

/// The square at a `(y, x)` cell key.
fn position((y, x): (i64, i64)) -> Option<Position> {
    Some(Position {
        x: i32::try_from(x).ok()?,
        y: i32::try_from(y).ok()?,
    })
}

/// Merges a cell's `mon` delta as `map_knowledge.js` does: onto the monster with the same client
/// id wherever it was last seen, else onto the cell's previous monster. `null` clears the cell.
fn merge_monster(
    tracked: &mut HashMap<i64, MonsterInfo>,
    old: Option<CellMonster>,
    delta: &Value,
) -> Option<CellMonster> {
    if delta.is_null() {
        return None;
    }
    let previous = |old: Option<CellMonster>, tracked: &HashMap<i64, MonsterInfo>| match old {
        Some(CellMonster::Tracked(id)) => tracked.get(&id).cloned().unwrap_or_default(),
        Some(CellMonster::Untracked(info)) => info,
        None => MonsterInfo::default(),
    };
    match delta
        .get("id")
        .and_then(Value::as_i64)
        .filter(|&id| id != 0)
    {
        Some(id) => {
            if !tracked.contains_key(&id) {
                let base = previous(old, tracked);
                tracked.insert(id, base);
            }
            tracked.entry(id).or_default().merge(delta);
            Some(CellMonster::Tracked(id))
        }
        None => {
            let mut info = previous(old, tracked);
            info.merge(delta);
            Some(CellMonster::Untracked(info))
        }
    }
}

/// A tile index's low 32 bits, where the flags that matter live. Webtiles sends an index too big
/// for a JavaScript number as `[low, high]`.
fn low_word(index: &Value) -> u64 {
    index
        .as_u64()
        .or_else(|| index.get(0).and_then(Value::as_u64))
        .unwrap_or(0)
}

/// The identifier `knowledge` gives an enum value, through one of its tables.
fn decode<'a, T: TryFrom<i64>>(
    value: Option<i64>,
    knowledge: Option<&'a Bundle>,
    table: fn(&'a Bundle, T) -> Option<&'a EnumId>,
) -> Option<&'a EnumId> {
    table(knowledge?, T::try_from(value?).ok()?)
}

/// The attitude a `mon_attitude_type` identifier names.
fn attitude(id: &EnumId) -> Option<Attitude> {
    Some(match id.0.as_str() {
        "ATT_HOSTILE" => Attitude::Hostile,
        "ATT_NEUTRAL" => Attitude::Neutral,
        "ATT_GOOD_NEUTRAL" => Attitude::GoodNeutral,
        "ATT_STRICT_NEUTRAL" | "ATT_OLD_STRICT_NEUTRAL" => Attitude::StrictNeutral,
        "ATT_FRIENDLY" => Attitude::Friendly,
        "ATT_MARIONETTE" => Attitude::Marionette,
        _ => return None,
    })
}

/// The rating a `mon_threat_level_type` identifier names.
fn crawl_threat(id: &EnumId) -> Option<CrawlThreat> {
    Some(match id.0.as_str() {
        "MTHRT_TRIVIAL" => CrawlThreat::Trivial,
        "MTHRT_EASY" => CrawlThreat::Easy,
        "MTHRT_TOUGH" => CrawlThreat::Tough,
        "MTHRT_NASTY" => CrawlThreat::Nasty,
        _ => return None,
    })
}
