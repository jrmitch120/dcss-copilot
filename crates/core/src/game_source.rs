//! What a Game Source adapter tells the core. Source-neutral: nothing here knows how a game was read.

use copilot_gk::{EnumId, ItemKind};
use serde::Serialize;

use crate::view_model::Glyph;

/// One thing a Game Source learned about the game it follows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "event", rename_all = "camelCase")]
pub enum SourceEvent {
    /// The game being followed, or what is known about it, changed.
    Identified(GameIdentity),
    /// The game now reports this Visible State.
    Visible(VisibleState),
}

/// Which game this is: what tells one character's run apart from every other.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameIdentity {
    /// Where the game is played, such as a webtiles server's origin.
    pub server: String,
    /// The account playing the game, when the source has seen it.
    pub account: Option<String>,
    /// The character's name.
    pub character: String,
    /// The character's species, as the game names it.
    pub species: String,
    /// When the source first saw this game, in Unix milliseconds.
    pub first_seen_ms: u64,
    /// The Crawl Version, once the game has reported it.
    pub crawl_version: Option<CrawlVersion>,
    /// Whether this is a wizard-mode game.
    pub wizard: bool,
    /// Whether the player is playing this game or watching someone else's, once known.
    pub role: Option<Role>,
}

/// The exact build of DCSS a game runs on, such as `0.34.1` or a trunk build like `0.35-a0-123-gabc1234`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CrawlVersion(pub String);

/// How the player relates to the game being followed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Role {
    /// The player is playing.
    Playing,
    /// The player is spectating someone else's game.
    Spectating,
}

/// What the game currently reports to the player. A value is `None` until the game has reported it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VisibleState {
    /// Current hit points.
    pub hp: Option<i32>,
    /// Maximum hit points.
    pub hp_max: Option<i32>,
    /// The game's turn count.
    pub turn: Option<u32>,
    /// The monsters the player can see, in map order: top row first, left to right. `None` until
    /// the game has sent its map, which is not the same as there being none.
    pub monsters: Option<Vec<VisibleMonster>>,
    /// The items the player carries, by inventory letter. `None` until the game has sent the
    /// inventory.
    pub inventory: Option<Vec<InventoryItem>>,
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
