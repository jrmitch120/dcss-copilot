//! What the Advisor remembers of the levels of one game as the player explores them: each
//! level's map as they left it, its Landmarks, the items seen on its floor and in its shops, and
//! their Visits, with the uniques met and killed.

use std::collections::{BTreeMap, HashMap};

use copilot_gk::{Bundle, EnumId, Feature, Landmark, StairDirection};
use serde::{Deserialize, Serialize};

use crate::crawl_ids::dungeon_features;
use crate::engine::paths::{Level, Walker};
use crate::screen::ScreenRead;
use crate::told::read_line;
use crate::visible_state::{
    GameMessage, InventoryItem, LevelMap, LevelName, MapCell, MapSnapshot, Place, Position,
    VisibleState,
};

/// What the Advisor remembers of the levels of one game as the player explores them, and the
/// uniques they met.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Explored {
    /// Each level the player has been on, by its name.
    #[serde(default)]
    pub levels: BTreeMap<LevelName, LevelRecord>,
    /// The level the player was on at the last Settled Update the Advisor saw, by its name.
    #[serde(default)]
    pub on_level: Option<LevelName>,
    /// Each unique the player has met, by the name Game Knowledge gives it, such as `Terence` or
    /// `Royal Jelly`.
    #[serde(default)]
    pub uniques: BTreeMap<String, UniqueMet>,
    /// What the game's messages told of the levels since the last Settled Update, to take in at
    /// the next.
    #[serde(skip)]
    news: Vec<News>,
    /// The square crawl has said lists the items on the one the player came to, in the message
    /// that follows: `Things that are here:`.
    #[serde(skip)]
    listing_next: Option<Square>,
    /// Where the player stood at the last Settled Update, and the map then.
    #[serde(skip)]
    settled: Option<Settled>,
}

/// What the Advisor remembers of one level of a game: a Level Record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LevelRecord {
    /// Which level it is.
    pub place: Place,
    /// Its map as the game showed it when the player last left it, or when the game's stream
    /// last stopped with them on it: `None` until either.
    pub map: Option<MapSnapshot>,
    /// Its Landmarks, top row first, as the game last showed the level.
    pub landmarks: Vec<LandmarkSeen>,
    /// The items seen on its floor and in its shops, in the order first seen.
    pub items: Vec<ItemSeen>,
    /// Each stay of the player on the level, oldest first.
    pub visits: Vec<Visit>,
}

/// A Landmark the player has seen on a level: a feature they remember it by.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LandmarkSeen {
    /// Where it is.
    pub position: Position,
    /// What kind of Landmark it is.
    pub kind: LandmarkKind,
    /// What the game calls it, such as `stone staircase leading down` or `sacred altar of Trog`.
    pub name: String,
}

/// What kind of Landmark a feature is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum LandmarkKind {
    /// A staircase, or another way to the level above or below, such as an escape hatch.
    Stairs {
        /// Which way it goes.
        direction: StairDirection,
    },
    /// An altar to a god.
    Altar,
    /// A shop.
    Shop,
    /// The way into a branch the dungeon leads to by stairs, such as the Lair.
    BranchEntrance,
    /// The way into a branch it doesn't, such as a Bazaar or the Abyss.
    Portal,
}

impl LandmarkKind {
    /// What kind of Landmark `feature` is, if it is one: what it is to a player finding their way
    /// first, such as a branch entrance, drawn as a staircase, and then any staircase.
    fn of(feature: &Feature) -> Option<Self> {
        match feature.landmark {
            Some(Landmark::Altar) => Some(Self::Altar),
            Some(Landmark::Shop) => Some(Self::Shop),
            Some(Landmark::BranchEntrance) => Some(Self::BranchEntrance),
            Some(Landmark::Portal) => Some(Self::Portal),
            None => feature.stairs.map(|direction| Self::Stairs { direction }),
        }
    }
}

/// An item the player has seen on a level, on its floor or in one of its shops.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemSeen {
    /// Its name, as the game named it then, such as `a +0 dagger`.
    pub name: String,
    /// The square it lies on, or the shop's.
    pub position: Position,
    /// The shop selling it, with its price; `None` for an item on the floor.
    pub offered: Option<ShopOffer>,
    /// The game's turn when the player last saw it there, if the game had reported one.
    pub seen: Option<u32>,
    /// Whether it is still there, as far as the Advisor knows.
    pub status: ItemStatus,
}

/// A shop selling an item, and for how much.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShopOffer {
    /// The shop's name, such as `Otaer's Distillery`.
    pub shop: String,
    /// Its price, in gold.
    pub price: u32,
}

/// Whether an item seen is still where the player saw it, as far as the Advisor knows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum ItemStatus {
    /// It was there the last time the player saw its square or shop, and they haven't taken it:
    /// Spotted.
    Spotted,
    /// It was gone the next time the game told what lies there, taken by the player or anyone
    /// else.
    Gone {
        /// The game's turn when it was seen gone, if the game had reported one.
        turn: Option<u32>,
    },
}

/// One stay of the player on a level, as the Advisor saw it: a Visit.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Visit {
    /// The game's turn when the Advisor first saw the player there, if the game had reported
    /// one.
    pub arrived: Option<u32>,
    /// The game's turn when the Advisor first saw the player on another level since: `None`
    /// while they stay.
    pub left: Option<u32>,
    /// The game's turn when the player cleared the floor in this Visit, if they did.
    pub cleared: Option<u32>,
}

/// A unique the player has met: where and when it was last seen, and killed.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UniqueMet {
    /// Where and when the Advisor last saw it in view.
    pub seen: Option<OnLevel>,
    /// Where and when the game said it died, if it did.
    pub killed: Option<OnLevel>,
}

/// A level and a turn of the game.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OnLevel {
    /// The level.
    pub level: LevelName,
    /// The game's turn, if the game had reported one.
    pub turn: Option<u32>,
}

/// A square of a level, by the level's name.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Square {
    level: LevelName,
    position: Position,
}

/// What a message told of the level the player is on.
#[derive(Debug, Clone, PartialEq, Eq)]
enum News {
    /// What lies on the square the player came to: these items, named, or more than crawl
    /// names, `None`.
    Items {
        square: Square,
        named: Option<Vec<String>>,
        turn: Option<u32>,
    },
    /// Exploring found nothing more to go to on the level.
    DoneExploring { level: LevelName },
    /// A unique died, by the name Game Knowledge gives it.
    UniqueDied { name: String, at: OnLevel },
}

/// Where the player stood at a Settled Update.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Settled {
    level: LevelName,
    position: Option<Position>,
    turn: Option<u32>,
    map: Option<LevelMap>,
    /// What the player carried, once the game had sent it.
    carried: Option<Vec<InventoryItem>>,
}

impl Explored {
    /// Takes in what the game's new `messages` told of the level, as `now` shows the game,
    /// through `bundle`'s words for it: kept until the next Settled Update.
    pub(crate) fn hear(&mut self, messages: &[GameMessage], now: &VisibleState, bundle: &Bundle) {
        let signs = &bundle.level;
        let level = now.place.as_ref().map(Place::level_name);
        // Crawl stops travel and exploring on a square it names the items of, so the player
        // stands there as the message comes, as every one in the corpus shows.
        let square = level
            .clone()
            .zip(now.position)
            .map(|(level, position)| Square { level, position });
        for message in messages {
            let turn = message.turn.or(now.turn);
            if let Some(square) = self.listing_next.take() {
                let named = message
                    .text
                    .split(signs.items_apart.as_str())
                    .map(str::to_owned)
                    .collect();
                self.news.push(News::Items {
                    square,
                    named: Some(named),
                    turn,
                });
                continue;
            }
            for said in read_line(&message.text, |said| level_news(said, bundle)) {
                let news = match said {
                    Said::Item(name) => square.clone().map(|square| News::Items {
                        square,
                        named: Some(vec![name]),
                        turn,
                    }),
                    Said::ItemsFollow => {
                        self.listing_next.clone_from(&square);
                        None
                    }
                    Said::ItemsUnnamed => square.clone().map(|square| News::Items {
                        square,
                        named: None,
                        turn,
                    }),
                    Said::DoneExploring => level.clone().map(|level| News::DoneExploring { level }),
                    Said::UniqueDied(name) => level.clone().map(|level| News::UniqueDied {
                        name,
                        at: OnLevel { level, turn },
                    }),
                };
                self.news.extend(news);
            }
        }
    }

    /// Takes in what a shop's menu the player opened shows, as `visible` shows the game, through
    /// `bundle`: the items it sells, Spotted, and those it sold before and no longer does, Gone.
    /// Read only standing on a shop, which a shop's menu shows the stock of.
    pub(crate) fn read_shop(&mut self, read: &ScreenRead, visible: &VisibleState, bundle: &Bundle) {
        let Some(shown) = read.shop(&bundle.level) else {
            return;
        };
        let (Some(place), Some(position), Some(map)) =
            (&visible.place, visible.position, &visible.map)
        else {
            return;
        };
        let on_a_shop = map
            .cell(position)
            .and_then(|cell| cell.feature.as_ref())
            .and_then(|id| bundle.feature_by_id(id))
            .is_some_and(|feature| feature.landmark == Some(Landmark::Shop));
        if !on_a_shop {
            return;
        }
        let record = self.record(place);
        let offered = |price| {
            Some(ShopOffer {
                shop: shown.name.clone(),
                price,
            })
        };
        let listed: Vec<(String, Option<ShopOffer>)> = shown
            .items
            .iter()
            .map(|(name, price)| (name.clone(), offered(*price)))
            .collect();
        record.listed_at(position, true, &listed, visible.turn);
    }

    /// Takes in what the game shows at a Settled Update, `visible`, and what its messages told
    /// since the last, through `knowledge`: the Visits, Landmarks and items of the level the
    /// player is on, the uniques in view and those that died, and whether the player cleared the
    /// floor, which needs no hostile in view, as `hostile_in_view` tells.
    pub(crate) fn settle(
        &mut self,
        visible: &VisibleState,
        knowledge: Option<&Bundle>,
        hostile_in_view: bool,
    ) {
        let news = std::mem::take(&mut self.news);
        let Some(place) = &visible.place else {
            return;
        };
        let level = place.level_name();
        self.arrive(place, visible.turn);
        // Only a square the Advisor saw the player come to: not where they stood as it joined or
        // picked the game up again, which crawl doesn't name the items of again.
        let came_to_their_square = self
            .settled
            .as_ref()
            .is_some_and(|settled| settled.level != level || settled.position != visible.position);
        let mut named_their_square = false;
        let mut done_exploring = false;
        for said in news {
            match said {
                News::Items {
                    square,
                    named,
                    turn,
                } => {
                    named_their_square |=
                        square.level == level && Some(square.position) == visible.position;
                    let Some(named) = named else { continue };
                    if let Some(record) = self.levels.get_mut(&square.level) {
                        let listed: Vec<(String, Option<ShopOffer>)> =
                            named.into_iter().map(|name| (name, None)).collect();
                        record.listed_at(square.position, false, &listed, turn);
                    }
                }
                News::DoneExploring { level: explored } => done_exploring |= explored == level,
                News::UniqueDied { name, at } => {
                    self.uniques.entry(name).or_default().killed = Some(at);
                }
            }
        }
        if let Some(bundle) = knowledge {
            let uniques = visible
                .monsters
                .iter()
                .flatten()
                .filter_map(|monster| bundle.monster_by_id(monster.id.as_ref()?))
                .filter(|kind| kind.unique);
            for unique in uniques {
                self.uniques.entry(unique.name.clone()).or_default().seen = Some(OnLevel {
                    level: level.clone(),
                    turn: visible.turn,
                });
            }
        }
        let gained = gained(
            self.settled
                .as_ref()
                .and_then(|settled| settled.carried.as_deref()),
            visible.inventory.as_deref(),
        );
        let record = self.record(place);
        if let Some(position) = visible.position
            && !named_their_square
        {
            if came_to_their_square {
                // Crawl names what lies on every square the player comes to: nothing named,
                // nothing lies there.
                record.listed_at(position, false, &[], visible.turn);
            } else {
                // Nor does it name what is left as the player picks up the last of it.
                record.taken_from(position, &gained, visible.turn);
            }
        }
        if let Some(bundle) = knowledge {
            if let Some(map) = &visible.map {
                record.landmarks = landmarks(map, bundle);
            }
            // Once a Visit, and judged from the map only until then, as that costs a walk.
            if let Some(visit) = record.visits.last_mut()
                && visit.cleared.is_none()
                && !hostile_in_view
                && (done_exploring || explored_out(visible, bundle))
            {
                visit.cleared = visible.turn;
            }
        }
        self.settled = Some(Settled {
            level,
            position: visible.position,
            turn: visible.turn,
            map: visible.map.clone(),
            carried: visible.inventory.clone(),
        });
    }

    /// Notes the player on the level `place` names on `turn`: if they were on another at the
    /// last Settled Update, they left it, its map as it was then, and began a Visit here.
    fn arrive(&mut self, place: &Place, turn: Option<u32>) {
        let level = place.level_name();
        if self.on_level.as_ref() == Some(&level) {
            return;
        }
        self.keep_map();
        if let Some(left) = self.on_level.take()
            && let Some(visit) = self
                .levels
                .get_mut(&left)
                .and_then(|record| record.visits.last_mut())
        {
            visit.left = turn;
        }
        self.record(place).visits.push(Visit {
            arrived: turn,
            ..Visit::default()
        });
        self.on_level = Some(level);
    }

    /// Keeps the map of the level the player was on at the last Settled Update as it was then:
    /// as they leave it, or as the game's stream stops, after which the Advisor may not see them
    /// leave.
    pub(crate) fn keep_map(&mut self) {
        let Some(settled) = &self.settled else { return };
        if let Some(record) = self.levels.get_mut(&settled.level)
            && let Some(map) = settled
                .map
                .as_ref()
                .and_then(|map| map.snapshot(settled.turn))
        {
            record.map = Some(map);
        }
    }

    /// The record of the level `place` names, made empty if there is none yet.
    fn record(&mut self, place: &Place) -> &mut LevelRecord {
        self.levels
            .entry(place.level_name())
            .or_insert_with(|| LevelRecord {
                place: place.clone(),
                map: None,
                landmarks: Vec::new(),
                items: Vec::new(),
                visits: Vec::new(),
            })
    }

    /// Forgets where the player stood at the last Settled Update the Advisor saw: a game picked
    /// up again is a new game process, which sends its map afresh.
    pub(crate) fn pick_up(&mut self) {
        self.settled = None;
        self.listing_next = None;
        self.news.clear();
    }
}

impl LevelRecord {
    /// Takes in what the game said lies at `position` on `turn`, in a shop if `in_shop`: each of
    /// `listed`, by its name and its shop's offer, is Spotted there, and each item seen there
    /// before that it doesn't list is Gone.
    fn listed_at(
        &mut self,
        position: Position,
        in_shop: bool,
        listed: &[(String, Option<ShopOffer>)],
        turn: Option<u32>,
    ) {
        let here = |item: &ItemSeen| item.position == position && item.offered.is_some() == in_shop;
        for item in self.items.iter_mut().filter(|item| here(item)) {
            if item.status == ItemStatus::Spotted
                && !listed.iter().any(|(name, _)| *name == item.name)
            {
                item.status = ItemStatus::Gone { turn };
            }
        }
        for (name, offered) in listed {
            match self
                .items
                .iter_mut()
                .find(|item| here(item) && item.name == *name)
            {
                Some(item) => {
                    item.status = ItemStatus::Spotted;
                    item.seen = turn;
                    item.offered.clone_from(offered);
                }
                None => self.items.push(ItemSeen {
                    name: name.clone(),
                    position,
                    offered: offered.clone(),
                    seen: turn,
                    status: ItemStatus::Spotted,
                }),
            }
        }
    }
}

impl LevelRecord {
    /// Takes in that the player, standing at `position` on `turn`, came to carry the `gained`
    /// items: each Spotted on the floor there that one of them names is Gone. An item picked up
    /// onto a stack of its kind they carry already goes unnoticed, as the stack's name differs.
    fn taken_from(&mut self, position: Position, gained: &[&InventoryItem], turn: Option<u32>) {
        let taken = |item: &ItemSeen| {
            gained
                .iter()
                .any(|carried| bare_name(&carried.name) == bare_name(&item.name))
        };
        for item in &mut self.items {
            if item.position == position
                && item.offered.is_none()
                && item.status == ItemStatus::Spotted
                && taken(item)
            {
                item.status = ItemStatus::Gone { turn };
            }
        }
    }
}

/// An item's name without what the floor and the inventory name it with apart: the article
/// the floor gives one item, as `a +0 dagger`, and the note the inventory gives one in use, as
/// `+0 mace (weapon)`.
fn bare_name(name: &str) -> &str {
    let name = ["a ", "an ", "the "]
        .iter()
        .find_map(|article| name.strip_prefix(article))
        .unwrap_or(name);
    match name.rsplit_once(" (") {
        Some((bare, note)) if note.ends_with(')') => bare,
        _ => name,
    }
}

/// The items `now` shows the player carrying that `before` didn't, by letter, name and quantity:
/// none unless the game showed both.
fn gained<'a>(
    before: Option<&[InventoryItem]>,
    now: Option<&'a [InventoryItem]>,
) -> Vec<&'a InventoryItem> {
    let (Some(before), Some(now)) = (before, now) else {
        return Vec::new();
    };
    now.iter()
        .filter(|item| {
            !before.iter().any(|was| {
                (was.letter, &was.name, was.quantity) == (item.letter, &item.name, item.quantity)
            })
        })
        .collect()
}

/// What one message tells of the level the player is on.
enum Said {
    /// The one item on the square they came to.
    Item(String),
    /// The next message names the few items on it.
    ItemsFollow,
    /// More items lie on it than crawl names.
    ItemsUnnamed,
    /// Exploring found nothing more to go to.
    DoneExploring,
    /// A unique died, by the name Game Knowledge gives it.
    UniqueDied(String),
}

/// What `said`, one message, tells of the level, through `bundle`'s words for it.
fn level_news(said: &str, bundle: &Bundle) -> Option<Said> {
    let signs = &bundle.level;
    if let Some(names) = signs.item_here.names(said) {
        return Some(Said::Item((*names.first()?).to_owned()));
    }
    if said == signs.items_here {
        return Some(Said::ItemsFollow);
    }
    if signs
        .items_unnamed
        .iter()
        .any(|template| template.names(said).is_some())
    {
        return Some(Said::ItemsUnnamed);
    }
    if said == signs.done_exploring {
        return Some(Said::DoneExploring);
    }
    signs.monster_died.iter().find_map(|template| {
        let named = *template.names(said)?.first()?;
        unique_named(named, bundle).map(|name| Said::UniqueDied(name.to_owned()))
    })
}

/// The name Game Knowledge gives the unique `named` names as crawl writes one with `the`
/// (`DESC_THE`), if it names one: most uniques' names take no article, but some take `the`, as
/// `the Royal Jelly`.
fn unique_named<'b>(named: &str, bundle: &'b Bundle) -> Option<&'b str> {
    let bare = named
        .strip_prefix("the ")
        .or_else(|| named.strip_prefix("The "))
        .unwrap_or(named);
    bundle
        .monsters
        .values()
        .filter(|monster| monster.unique)
        .find(|monster| monster.name == named || monster.name == bare)
        .map(|monster| monster.name.as_str())
}

/// Each square `map` shows with the feature `bundle` decodes there, top row first.
fn features<'m, 'b>(
    map: &'m LevelMap,
    bundle: &'b Bundle,
) -> impl Iterator<Item = (Position, &'m MapCell, &'b Feature)> {
    let by_id: HashMap<&EnumId, &Feature> = bundle
        .features
        .values()
        .map(|feature| (&feature.id, feature))
        .collect();
    map.cells().filter_map(move |(position, cell)| {
        Some((position, cell, *by_id.get(cell.feature.as_ref()?)?))
    })
}

/// The Landmarks `map` shows, its features decoded by `bundle`, top row first.
fn landmarks(map: &LevelMap, bundle: &Bundle) -> Vec<LandmarkSeen> {
    features(map, bundle)
        .filter_map(|(position, _, feature)| {
            Some(LandmarkSeen {
                position,
                kind: LandmarkKind::of(feature)?,
                name: feature.name.clone(),
            })
        })
        .collect()
}

/// Whether no square the player can walk to on the level `visible` shows borders one they
/// haven't explored, its features decoded by `bundle`: so exploring has nowhere more to go. A
/// square they have never seen, which the game shows only as magic mapping or detection drew
/// it, is unexplored, as crawl's exploring goes to it; and a transporter they can walk to may
/// lead on to more, as the map doesn't show where, so a level with one is never judged so.
/// `false` until the game has sent its map and where the player stands.
fn explored_out(visible: &VisibleState, bundle: &Bundle) -> bool {
    let (Some(map), Some(player)) = (&visible.map, visible.position) else {
        return false;
    };
    let explored = features(map, bundle).filter(|(_, cell, _)| !cell.mapped_only);
    let Some(level) =
        Level::new(explored.map(|(position, _, feature)| (position, feature.terrain)))
    else {
        return false;
    };
    let walks = level.moves_from(Walker::Player { occupied: &[] }, player);
    let transporter = |square: Position| {
        map.cell(square)
            .and_then(|cell| cell.feature.as_ref())
            .is_some_and(|feature| feature.0 == dungeon_features::TRANSPORTER)
    };
    !walks.reached().any(|(square, _)| {
        transporter(square)
            || level
                .around(square)
                .any(|next| level.terrain(next).is_none())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view_model::{Colour, Glyph};
    use crate::visible_state::{InventoryItem, MapCell, SquareLight};

    /// The shipped 0.34.1 bundle.
    fn bundle() -> &'static Bundle {
        static LIBRARY: std::sync::OnceLock<copilot_gk::Library> = std::sync::OnceLock::new();
        LIBRARY
            .get_or_init(|| copilot_gk::Library::shipped().unwrap())
            .for_version("0.34.1")
            .unwrap()
    }

    /// The player at `(1, 1)` of a level the game shows as `rows`: `#` a wall, `.` floor, `T` a
    /// transporter, `m` floor only magic mapping drew, and a space a square it doesn't show.
    fn standing_in(rows: &[&str]) -> VisibleState {
        let cell = |feature: &str, glyph: char, mapped_only: bool| MapCell {
            glyph: Glyph {
                text: glyph.to_string(),
                colour: Colour::LightGrey,
            },
            feature: Some(EnumId(feature.to_owned())),
            in_view: !mapped_only,
            mapped_only,
            light: SquareLight::Plain,
        };
        let cells = rows.iter().zip(0..).flat_map(|(row, y)| {
            row.chars().zip(0..).filter_map(move |(glyph, x)| {
                let square = match glyph {
                    '#' => cell("DNGN_ROCK_WALL", glyph, false),
                    '.' => cell("DNGN_FLOOR", glyph, false),
                    'T' => cell(dungeon_features::TRANSPORTER, glyph, false),
                    'm' => cell("DNGN_FLOOR", '.', true),
                    _ => return None,
                };
                Some((Position { x, y }, square))
            })
        });
        VisibleState {
            position: Some(Position { x: 1, y: 1 }),
            map: Some(LevelMap::new(cells)),
            ..VisibleState::default()
        }
    }

    #[test]
    fn a_level_is_explored_out_once_no_square_the_player_can_walk_to_borders_an_unexplored_one() {
        let explored_out = |rows: &[&str]| explored_out(&standing_in(rows), bundle());
        assert!(explored_out(&["#####", "#...#", "#####"]));
        // A room the player can't walk to, beyond a wall, doesn't count.
        assert!(explored_out(&["#####", "#...#", "#####", "#   #"]));
        assert!(!explored_out(&["#####", "#... ", "#####"]));
        assert!(!explored_out(&["#####", "#..m#", "#####"]));
        assert!(!explored_out(&["#####", "#..T#", "#####"]));
    }

    /// The player on D:1 at `(1, 1)` on `turn`, carrying nothing, the game having just said
    /// `said`.
    fn on_d1(turn: u32, said: &[&str]) -> VisibleState {
        VisibleState {
            turn: Some(turn),
            place: Some(Place {
                branch: "Dungeon".to_owned(),
                depth: Some(1),
            }),
            messages: said
                .iter()
                .map(|text| GameMessage {
                    text: (*text).to_owned(),
                    turn: Some(turn),
                })
                .collect(),
            inventory: Some(Vec::new()),
            ..standing_in(&["###", "#.#", "###"])
        }
    }

    /// A game picked up again shows the player where they stood, and crawl doesn't name the
    /// items there again: they aren't taken for gone.
    #[test]
    fn the_items_where_the_player_stands_as_a_game_is_picked_up_again_are_still_spotted() {
        let mut explored = Explored::default();
        let seen = on_d1(5, &["You see here a +0 dagger."]);
        explored.hear(&seen.messages, &seen, bundle());
        explored.settle(&seen, Some(bundle()), false);
        explored.pick_up();
        explored.settle(&on_d1(5, &[]), Some(bundle()), false);
        let items: Vec<_> = explored.levels["Dungeon:1"]
            .items
            .iter()
            .map(|item| (item.name.as_str(), item.status))
            .collect();
        assert_eq!(items, [("a +0 dagger", ItemStatus::Spotted)]);
    }

    /// Picking up the last item on a square, crawl names nothing more there: what the player
    /// came to carry is gone from the floor, and what they didn't take stays.
    #[test]
    fn an_item_the_player_picks_up_where_they_stand_is_gone() {
        let mut explored = Explored::default();
        let came = on_d1(5, &[]);
        explored.settle(&came, Some(bundle()), false);
        let seen = on_d1(5, &["Things that are here:", "a +0 dagger; a +0 robe"]);
        explored.hear(&seen.messages, &seen, bundle());
        explored.settle(&seen, Some(bundle()), false);
        let carrying = VisibleState {
            inventory: Some(vec![InventoryItem {
                letter: 'c',
                name: "+0 dagger".to_owned(),
                quantity: 1,
                kind: None,
                useless: false,
            }]),
            ..on_d1(6, &[])
        };
        explored.settle(&carrying, Some(bundle()), false);
        let items: Vec<_> = explored.levels["Dungeon:1"]
            .items
            .iter()
            .map(|item| (item.name.as_str(), item.status))
            .collect();
        assert_eq!(
            items,
            [
                ("a +0 dagger", ItemStatus::Gone { turn: Some(6) }),
                ("a +0 robe", ItemStatus::Spotted)
            ]
        );
    }

    /// Crawl names some uniques with `the`, as `You kill the Royal Jelly!`.
    #[test]
    fn a_unique_crawl_names_with_the_is_remembered_killed() {
        for said in ["You kill the Royal Jelly!", "The Royal Jelly dies!"] {
            let mut explored = Explored::default();
            let died = on_d1(9, &[said]);
            explored.hear(&died.messages, &died, bundle());
            explored.settle(&died, Some(bundle()), false);
            assert_eq!(
                explored
                    .uniques
                    .get("Royal Jelly")
                    .and_then(|met| met.killed.clone()),
                Some(OnLevel {
                    level: on_d1(9, &[]).place.unwrap().level_name(),
                    turn: Some(9)
                }),
                "{said}"
            );
        }
    }
}
