//! Visible State: what the game currently reports to the player, as a Game Source decodes it.

use copilot_gk::{EnumId, ItemKind};
use serde::Serialize;

use crate::view_model::{Colour, Glyph};

/// What the game currently reports to the player. A value is `None` until the game has reported it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VisibleState {
    /// The game's turn count.
    pub turn: Option<u32>,
    /// The character's numbers, as the status pane shows them.
    pub stats: Stats,
    /// Which god the character worships, if any.
    pub religion: Option<Religion>,
    /// The level the character is on.
    pub place: Option<Place>,
    /// Where the character stands on the level's map.
    pub position: Option<Position>,
    /// The statuses the status pane lists, in its order.
    pub statuses: Option<Vec<Status>>,
    /// The monsters the player can see, in map order: top row first, left to right. `None` until
    /// the game has sent its map, which is not the same as there being none.
    pub monsters: Option<Vec<VisibleMonster>>,
    /// The items the player carries, by inventory letter. `None` until the game has sent the
    /// inventory.
    pub inventory: Option<Vec<InventoryItem>>,
}

/// The character's numbers from the status pane. Each is `None` until the game reports it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Stats {
    /// Current hit points.
    pub hp: Option<i32>,
    /// Maximum hit points.
    pub hp_max: Option<i32>,
    /// Current magic points.
    pub mp: Option<i32>,
    /// Maximum magic points.
    pub mp_max: Option<i32>,
    /// Armour class, including temporary changes.
    pub ac: Option<i32>,
    /// Evasion, including temporary changes.
    pub ev: Option<i32>,
    /// Shield class, including temporary changes.
    pub sh: Option<i32>,
    /// Strength.
    pub strength: Option<i32>,
    /// Intelligence.
    pub intelligence: Option<i32>,
    /// Dexterity.
    pub dexterity: Option<i32>,
    /// Experience level.
    pub xl: Option<u32>,
    /// Gold carried.
    pub gold: Option<u32>,
}

/// The character's god, or none.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(
    tag = "worships",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Religion {
    /// The character worships no god.
    NoGod,
    /// The character worships a god.
    God {
        /// The god's name, as the game shows it, such as `Trog`.
        name: String,
        /// Piety as the stars the status pane shows, from 0 to 6, once reported.
        piety_rank: Option<u32>,
        /// The god is punishing the character.
        penance: bool,
    },
}

/// A level of the dungeon, such as `D:3`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Place {
    /// The branch, as the game names it, such as `Dungeon` or `a Bazaar`.
    pub branch: String,
    /// The depth in the branch; `None` for a branch only one level deep.
    pub depth: Option<u32>,
}

/// A square of the current level's map. The Game Source fixes where `(0, 0)` is; `x` grows to
/// the right and `y` grows down.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub struct Position {
    /// Squares to the right.
    pub x: i32,
    /// Squares down.
    pub y: i32,
}

/// One status the status pane lists, such as `Pois`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    /// The short label the status lights show, such as `Pois`, if it has one.
    pub light: Option<String>,
    /// The words it is described with, such as `poisoned`, if it has them.
    pub text: Option<String>,
    /// The colour it is shown in, which tells how severe it is.
    pub colour: Option<Colour>,
}

/// An item the player carries.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InventoryItem {
    /// Its inventory letter.
    pub letter: char,
    /// Its name, as the game shows it, such as `2 potions of curing`.
    pub name: String,
    /// How many there are in the stack.
    pub quantity: u32,
    /// What it is, when the bundle for the game's Crawl Version decodes its class and sub-type.
    pub kind: Option<ItemKind>,
}

/// A monster the player can see.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VisibleMonster {
    /// Its name, as the game names it, such as `jackal` or `Sigmund`.
    pub name: String,
    /// The glyph the game draws it with on the map.
    pub glyph: Glyph,
    /// Its Game Knowledge identity, when the bundle for the game's Crawl Version decodes its type.
    pub id: Option<EnumId>,
}
