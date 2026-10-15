//! The current level's map, merged from webtiles `map` deltas as the page's `map_knowledge.js`
//! merges them, and the monsters in view on it.

use std::collections::{BTreeMap, HashMap};

use copilot_core::{Colour, Glyph, VisibleMonster};
use copilot_gk::Bundle;
use serde_json::Value;

/// Tile background flags that mark a cell the player can't see now, from crawl's
/// `tile-flags.h` (`TILE_FLAG_MM_UNSEEN | TILE_FLAG_UNSEEN`). The page's `enums.js` reads the same
/// bits; they have held since tiles came to webtiles.
const UNSEEN: u64 = 0x0002_0000 | 0x0004_0000;

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
    glyph: Option<String>,
    colour: Option<u8>,
    /// The tile's background flags, once the game has sent tile data for the cell.
    tile_bg: Option<u64>,
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
}

impl MonsterInfo {
    fn merge(&mut self, delta: &Value) {
        if let Some(name) = delta.get("name").and_then(Value::as_str) {
            self.name = Some(name.to_owned());
        }
        if let Some(kind) = delta.get("type").and_then(Value::as_i64) {
            self.kind = Some(kind);
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
            .values()
            .filter(|cell| cell.tile_bg.is_some_and(|bg| bg & UNSEEN == 0))
            .filter_map(|cell| {
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
                    glyph: Glyph {
                        text: cell.glyph.clone().unwrap_or_default(),
                        colour: Colour::from_crawl(cell.colour.unwrap_or(DEFAULT_COLOUR))
                            .unwrap_or(Colour::LightGrey),
                    },
                    id: known.map(|monster| monster.id.clone()),
                })
            })
            .collect();
        Some(monsters)
    }
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
