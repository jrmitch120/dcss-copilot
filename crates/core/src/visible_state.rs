//! Visible State: what the game currently reports to the player, as a Game Source decodes it.

use std::collections::BTreeMap;

use copilot_gk::{EnumId, ItemKind};
use serde::Serialize;

use crate::screen::ScreenRead;
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
    /// What the player holds in their weapon hand. `None` until the game has said.
    pub wielded: Option<Wielding>,
    /// The current level as the game shows it: the squares in view and those remembered. `None`
    /// until the game has sent its map.
    pub map: Option<LevelMap>,
    /// The most recent messages, oldest first, up to the last [`RECENT_MESSAGES`].
    pub messages: Vec<GameMessage>,
    /// The screen the player has open over the map, if any: the top one, when one opened another.
    pub screen: Option<ScreenRead>,
    /// The square the player's look or aiming cursor is on, while one is shown.
    pub look_cursor: Option<Position>,
}

impl VisibleState {
    /// Whether a halo lights the square the character stands on or an umbra darkens it, once
    /// the game has said where they stand and sent its map.
    #[must_use]
    pub fn light_at_player(&self) -> Option<SquareLight> {
        let position = self.position?;
        let map = self.map.as_ref()?;
        Some(
            map.cell(position)
                .map_or(SquareLight::Plain, |cell| cell.light),
        )
    }
}

/// How many of the latest messages Visible State keeps: more than one turn's worth, as the
/// message pane shows. Older messages are history, not what the game shows now.
pub const RECENT_MESSAGES: usize = 20;

/// One line of the message log.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameMessage {
    /// Its words, as plain text without colours.
    pub text: String,
    /// The turn it was given on.
    pub turn: Option<u32>,
}

/// The current level's map: every square the game shows, in view or remembered from earlier.
///
/// It serializes as text, one string per row as the console would draw it, so a snapshot of it
/// reads as a map.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LevelMap {
    /// By `(y, x)`, so iterating goes top row first, left to right.
    cells: BTreeMap<(i32, i32), MapCell>,
}

/// One square of the map.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MapCell {
    /// What the game draws there: the feature, or an item, cloud or monster on it.
    pub glyph: Glyph,
    /// The dungeon feature, such as `DNGN_FLOOR`, when the bundle for the game's Crawl Version
    /// decodes it.
    pub feature: Option<EnumId>,
    /// The player can see the square now. Otherwise the game shows it as remembered, which may
    /// be out of date.
    pub in_view: bool,
    /// Whether a halo lights it or an umbra darkens it.
    pub light: SquareLight,
}

/// Whether a halo lights a square or an umbra darkens it, as the game draws it. A square in both
/// is drawn as neither, which is how crawl's light treats it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SquareLight {
    /// Neither.
    #[default]
    Plain,
    /// A halo lights it, such as an angel's or the player's own.
    Halo,
    /// An umbra darkens it, such as a profane servitor's or the player's own.
    Umbra,
}

impl LevelMap {
    /// A map of these squares.
    pub fn new(cells: impl IntoIterator<Item = (Position, MapCell)>) -> Self {
        Self {
            cells: cells
                .into_iter()
                .map(|(position, cell)| ((position.y, position.x), cell))
                .collect(),
        }
    }

    /// The square at `position`, if the game shows one there.
    #[must_use]
    pub fn cell(&self, position: Position) -> Option<&MapCell> {
        self.cells.get(&(position.y, position.x))
    }

    /// Every square, top row first, left to right.
    pub fn cells(&self) -> impl Iterator<Item = (Position, &MapCell)> {
        self.cells
            .iter()
            .map(|(&(y, x), cell)| (Position { x, y }, cell))
    }

    /// The top-left corner of the smallest rectangle holding every square, and its bottom-right.
    fn bounds(&self) -> Option<(Position, Position)> {
        let (&(top, _), _) = self.cells.first_key_value()?;
        let (&(bottom, _), _) = self.cells.last_key_value()?;
        let left = self.cells.keys().map(|&(_, x)| x).min()?;
        let right = self.cells.keys().map(|&(_, x)| x).max()?;
        Some((
            Position { x: left, y: top },
            Position {
                x: right,
                y: bottom,
            },
        ))
    }

    /// The map's rows as text, drawing each square with `draw` and a space where there is none.
    fn rows(&self, draw: impl Fn(&MapCell) -> String) -> Vec<String> {
        let Some((top_left, bottom_right)) = self.bounds() else {
            return Vec::new();
        };
        (top_left.y..=bottom_right.y)
            .map(|y| {
                (top_left.x..=bottom_right.x)
                    .map(|x| {
                        self.cells
                            .get(&(y, x))
                            .map_or_else(|| " ".to_owned(), &draw)
                    })
                    .collect::<String>()
                    .trim_end()
                    .to_owned()
            })
            .collect()
    }
}

impl Serialize for LevelMap {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut map = serializer.serialize_struct("LevelMap", 3)?;
        map.serialize_field("topLeft", &self.bounds().map(|(top_left, _)| top_left))?;
        map.serialize_field("rows", &self.rows(|cell| cell.glyph.text.clone()))?;
        map.serialize_field(
            "inView",
            &self.rows(|cell| if cell.in_view { "x" } else { "." }.to_owned()),
        )?;
        map.end()
    }
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
    /// Crawl judges it of no use to the character right now, as the inventory greys it out:
    /// `is_useless_item` with temporary conditions, such as a scroll while confused.
    pub useless: bool,
}

/// What the player holds in their weapon hand.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "holds", content = "letter", rename_all = "camelCase")]
pub enum Wielding {
    /// Nothing: they fight unarmed.
    Nothing,
    /// The item they carry under this inventory letter.
    Item(char),
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
    /// The square it is on.
    pub position: Position,
    /// Whose side it is on, when the bundle for the game's Crawl Version decodes it.
    pub attitude: Option<Attitude>,
    /// How dangerous crawl itself rates it for this character, when the bundle for the game's
    /// Crawl Version decodes it. Crawl's own rating, not the Advisor's Threat Level.
    pub crawl_threat: Option<CrawlThreat>,
    /// This stretch of it in view, when the Game Source tells it apart from other monsters.
    pub sighting: Option<Sighting>,
}

/// One stretch of a monster in view: what tells it apart from other monsters while it stays in
/// view. A monster that leaves view and comes back is a new Sighting, as crawl gives it a new
/// client id on purpose (`reset_client_id`, `player-notices.cc:553`), so the player can't be sure
/// it is the one they saw before.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub struct Sighting(pub u64);

/// Whose side a monster is on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Attitude {
    /// It fights the player.
    Hostile,
    /// It fights everyone, but isn't hunting the player.
    Neutral,
    /// It won't attack the player or the player's allies.
    GoodNeutral,
    /// It won't attack the player, but may attack the player's allies.
    StrictNeutral,
    /// It is the player's ally.
    Friendly,
    /// It is an ally for now, under a spell that will end.
    Marionette,
}

/// Crawl's rating of how dangerous a monster is to the character, which webtiles shows as the
/// colour of its name in the monster list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum CrawlThreat {
    /// No danger.
    Trivial,
    /// Little danger.
    Easy,
    /// A real fight.
    Tough,
    /// Dangerous.
    Nasty,
}
