//! The character, merged from webtiles `player` messages, which send a field only when it changed.

use copilot_core::{Colour, Place, Position, Religion, Stats, Status, Wielding};
use copilot_gk::Bundle;
use serde_json::Value;

use crate::inventory::Inventory;
use crate::json::{flag, int, string};

/// The `player` message fields decoded so far, merged across deltas.
#[derive(Debug, Default)]
pub struct Player {
    pub name: Option<String>,
    pub species: Option<String>,
    pub wizard: bool,
    pub turn: Option<u32>,
    pub stats: Stats,
    /// The god's name, empty for none.
    god: Option<String>,
    piety_rank: Option<u32>,
    penance: bool,
    branch: Option<String>,
    /// Webtiles sends 0 for a branch one level deep.
    depth: Option<u32>,
    pub position: Option<Position>,
    pub statuses: Option<Vec<Status>>,
    /// `None` until a `player` message has carried the inventory.
    pub inventory: Option<Inventory>,
    /// The inventory slot of the item in the weapon hand, -1 for none: `weapon_index`.
    weapon_index: Option<i64>,
}

impl Player {
    /// Merges a `player` delta, learning through `knowledge` the kinds of item it shows
    /// identified.
    pub fn merge(&mut self, msg: &Value, knowledge: Option<&Bundle>) {
        if let Some(inv) = msg.get("inv") {
            self.inventory.get_or_insert_default().merge(inv, knowledge);
        }
        set(&mut self.name, string(msg, "name"));
        set(&mut self.species, string(msg, "species"));
        if let Some(wizard) = msg.get("wizard") {
            self.wizard = flag(wizard);
        }
        set(&mut self.turn, int(msg, "turn"));

        let stats = &mut self.stats;
        set(&mut stats.hp, int(msg, "hp"));
        set(&mut stats.hp_max, int(msg, "hp_max"));
        set(&mut stats.mp, int(msg, "mp"));
        set(&mut stats.mp_max, int(msg, "mp_max"));
        set(&mut stats.ac, int(msg, "ac"));
        set(&mut stats.ev, int(msg, "ev"));
        set(&mut stats.sh, int(msg, "sh"));
        set(&mut stats.strength, int(msg, "str"));
        set(&mut stats.intelligence, int(msg, "int"));
        set(&mut stats.dexterity, int(msg, "dex"));
        set(&mut stats.xl, int(msg, "xl"));
        set(&mut stats.gold, int(msg, "gold"));

        set(&mut self.god, string(msg, "god"));
        set(&mut self.piety_rank, int(msg, "piety_rank"));
        if let Some(penance) = msg.get("penance") {
            self.penance = flag(penance);
        }
        set(&mut self.branch, string(msg, "place"));
        set(&mut self.depth, int(msg, "depth"));
        if let Some(pos) = msg.get("pos") {
            set(
                &mut self.position,
                int(pos, "x")
                    .zip(int(pos, "y"))
                    .map(|(x, y)| Position { x, y }),
            );
        }
        set(&mut self.weapon_index, int(msg, "weapon_index"));
        if let Some(statuses) = msg.get("status").and_then(Value::as_array) {
            // The status list is always sent whole.
            self.statuses = Some(statuses.iter().map(status).collect());
        }
    }

    /// The character's god, once the game has said.
    pub fn religion(&self) -> Option<Religion> {
        let god = self.god.as_ref()?;
        Some(if god.is_empty() {
            Religion::NoGod
        } else {
            Religion::God {
                name: god.clone(),
                piety_rank: self.piety_rank,
                penance: self.penance,
            }
        })
    }

    /// What the character holds in their weapon hand, once the game has said and, when it is an
    /// item, sent the inventory slot it is in.
    pub fn wielded(&self) -> Option<Wielding> {
        let index = self.weapon_index?;
        let Ok(slot) = u16::try_from(index) else {
            return Some(Wielding::Nothing);
        };
        self.inventory.as_ref()?.letter_of(slot).map(Wielding::Item)
    }

    /// The level the character is on, once the game has said.
    pub fn place(&self) -> Option<Place> {
        Some(Place {
            branch: self.branch.clone()?,
            depth: self.depth.filter(|&depth| depth > 0),
        })
    }
}

fn status(status: &Value) -> Status {
    Status {
        light: string(status, "light"),
        text: string(status, "text"),
        colour: int::<u8>(status, "col").and_then(|colour| Colour::from_crawl(colour & 0xF)),
    }
}

/// Keeps the old value when a delta doesn't carry the field.
fn set<T>(field: &mut Option<T>, value: Option<T>) {
    if value.is_some() {
        *field = value;
    }
}
