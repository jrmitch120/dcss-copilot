use serde::{Deserialize, Serialize};
use specta::Type;

use crate::fact_sheet::ThreatLevel;

/// Everything the panel renders, pushed by the core to the webview.
///
/// The webview holds no game logic: it only draws this. TypeScript types are
/// generated from it, so the two sides can't drift apart.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ViewModel {
    /// The running app's version, shown in the panel footer and About.
    pub app_version: String,
    /// How the app is connected to the browser.
    pub connection: Connection,
    /// Whether monsters and items are drawn as DCSS tiles or glyphs.
    pub appearance: Appearance,
    /// Why the Game Knowledge the app ships couldn't be read, when it couldn't. Games are then
    /// followed without it: monsters and items aren't decoded and no Threat Level is judged.
    pub unreadable_game_knowledge: Option<String>,
    /// The panel for the game being followed, once one is identified.
    pub game: Option<GameView>,
}

impl ViewModel {
    /// The view before any browser has connected.
    #[must_use]
    pub fn initial(app_version: &str, appearance: Appearance) -> Self {
        Self {
            app_version: app_version.to_owned(),
            connection: Connection::NoBrowser,
            appearance,
            unreadable_game_knowledge: None,
            game: None,
        }
    }
}

/// How the app is connected to the browser extension.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(
    tag = "state",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Connection {
    /// No browser is connected.
    NoBrowser,
    /// A browser asked to connect for the first time: the player is asked "Allow this browser?".
    AllowBrowser {
        /// The browser extension's origin, such as `chrome-extension://…`.
        browser: String,
    },
    /// A browser is connected.
    Connected {
        /// A tab's buffer overflowed before the app connected, so its game is missing its start:
        /// the player should reload the game page, which sends no game input.
        buffer_overflowed: bool,
    },
    /// The app can't listen for browsers, so none can connect.
    RelayUnavailable {
        /// Why, for the player.
        reason: String,
    },
}

/// The player's setting for how monsters and items are drawn.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum Appearance {
    /// DCSS tiles, as webtiles draws them. Anything without a tile falls back to its glyph.
    #[default]
    Tiles,
    /// DCSS glyphs in their game colours, as the console game draws them.
    Glyphs,
}

/// The docked panel for the followed game, top to bottom: header, now pill, focus card,
/// Run Plan rail and ask box. The footer belongs to the app, not the game.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct GameView {
    /// Who is playing, shown in the header.
    pub character: Character,
    /// How dangerous this moment is, at a glance.
    pub now: NowPill,
    /// The one thing that deserves the player's attention now.
    pub focus: FocusCard,
    /// A fact card for each kind of unique or Out of Depth monster in view, in map order, shown
    /// in the focus card.
    pub fact_cards: Vec<FactCard>,
    /// The player's Ways Out, under the focus card while anything hostile is in view.
    pub ways_out: Option<WaysOutList>,
    /// The Run Plan, summarised along the bottom of the panel.
    pub run_plan: RunPlanRail,
    /// Where the player asks the Advisor questions.
    pub ask: AskBox,
    /// The game's stream is flowing: it hasn't stopped or ended since the Advisor last joined or
    /// resynced it.
    pub live: bool,
    /// What the Collapsed Pill shows besides the now pill's label and line.
    pub pill: CollapsedPill,
}

/// The panel collapsed out of the way, always on top: the now pill's label and line, a badge,
/// and whether this moment calls the panel back.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CollapsedPill {
    /// How many Decision Points wait for the player, counted on the badge. Nothing raises
    /// Decision Points yet, so none wait.
    pub decisions_waiting: u32,
    /// This moment is LETHAL and no Way Out is trusted, so the pill expands itself into the
    /// panel: the only moment it does.
    pub expands: bool,
}

/// The Ways Out as the panel lists them: a ✓ or ✕ for each, with why not.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct WaysOutList {
    /// One staircase each way, the nearest that can be used or else the nearest, then each
    /// escape carried, ability and spell.
    pub rows: Vec<WayOutRow>,
    /// The ground the player stands on, such as `In a corridor: 2 sides open`, once the Advisor
    /// can tell.
    pub ground: Option<String>,
    /// What keeps the Advisor from judging more, such as a screen to open, one sentence each.
    pub gaps: Vec<String>,
}

/// `text` with its first letter capitalised, as a name starts a line in the panel.
pub(crate) fn capitalised(text: &str) -> String {
    let mut chars = text.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(chars).collect()
    })
}

/// One Way Out in the panel's list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct WayOutRow {
    /// It can be used now, as far as the Advisor can tell: drawn ✓, else ✕.
    pub usable: bool,
    /// What it is, such as `Stone staircase leading down` or `Teleportation`.
    pub name: String,
    /// A short fact beside the name, such as `7 steps` or `spell · 4% fail`.
    pub detail: Option<String>,
    /// Why it can't be used, when it can't, such as `Centaur is faster`.
    pub why_not: Option<String>,
}

/// The character in the game being followed. A stat is `None` until the game reports it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Character {
    /// The character's name.
    pub name: String,
    /// The character's species.
    pub species: String,
    /// Current hit points.
    pub hp: Option<i32>,
    /// Maximum hit points.
    pub hp_max: Option<i32>,
    /// The game's turn count.
    pub turn: Option<u32>,
}

/// The threat label and one line at the top of the panel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(
    tag = "state",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum NowPill {
    /// The Advisor can't judge a Threat Level yet, so it shows none rather than guess.
    Unrated {
        /// The Alert raised at the player's last action, until they act again.
        alert: Option<String>,
    },
    /// The Advisor judged how dangerous this moment is.
    Rated {
        /// The Threat Level.
        level: ThreatLevel,
        /// The Alert raised at the player's last action, until they act again, such as
        /// `Threat rose to LETHAL.`.
        alert: Option<String>,
        /// One sentence about this moment, such as which monsters can attack.
        line: String,
        /// The numbers behind the level, such as the chance of death and the worst-case hit,
        /// when anything can attack.
        detail: Option<String>,
    },
}

/// The card under the now pill: what deserves the player's attention now.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(
    tag = "card",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum FocusCard {
    /// Nothing calls for a decision: the card lists what is in view.
    InView {
        /// The monsters in view, identical ones in one row, without Firewood when Game
        /// Knowledge says which monsters are Firewood. `None` while the Advisor can't read
        /// monsters, which is not the same as there being none.
        monsters: Option<Vec<Listing>>,
    },
    /// A small fight: what is in view, and how fighting it by each Attack Plan comes out.
    Fight {
        /// The monsters in view, as [`FocusCard::InView`] lists them.
        monsters: Option<Vec<Listing>>,
        /// The fight in one line: the best Attack Plan's odds, such as `Fight on (Fireball, then
        /// melee): ~80% win, ~45% HP lost`, or that the Advisor can't judge it yet.
        line: String,
        /// The other plans, best first, one line each, such as `Melee: ~60% win, ~70% HP lost`.
        others: Vec<String>,
        /// What the fight leaves out, as one line, such as `Not modelled: summons, allies, hexes`.
        /// `None` when the fight wasn't judged.
        not_modelled: Option<String>,
        /// What keeps the Advisor from judging the fight, or may hide a better plan, one sentence
        /// each, such as a screen to open.
        gaps: Vec<String>,
    },
}

/// A unique or Out of Depth monster in view, singled out so the player recognises it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct FactCard {
    /// How it is drawn.
    pub depiction: Depiction,
    /// Its name, as the game names it, capitalised.
    pub name: String,
    /// What makes it exceptional, such as `Unique` or `Out of depth: usually met 5 levels
    /// deeper`.
    pub why: String,
    /// The Threat Level it makes on its own, when the Advisor can judge it.
    pub alone: Option<ThreatLevel>,
}

/// The Run Plan rail along the bottom of the panel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum RunPlanRail {
    /// There is no Run Plan for this game yet.
    NotPlanned,
}

/// The ask box at the bottom of the panel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum AskBox {
    /// Answering needs a model, and none is set up.
    NeedsModel,
}

/// One row of a list in the panel, such as a monster in view, standing for one thing or several
/// identical ones.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Listing {
    /// How the thing is drawn.
    pub depiction: Depiction,
    /// Its name, as the game names one of it.
    pub name: String,
    /// A short fact beside the name, if any.
    pub detail: Option<String>,
    /// How many identical things the row stands for, such as 4 for a pack of jackals.
    pub count: u32,
}

impl Listing {
    /// Whether `other` lists the same kind of thing, drawn and described alike, so the two can
    /// be one row.
    fn lists_alike(&self, other: &Self) -> bool {
        self.depiction == other.depiction && self.name == other.name && self.detail == other.detail
    }

    /// The rows for `listings`, all identical ones made one row with their count, wherever they
    /// are listed, in the order each was first listed.
    #[must_use]
    pub(crate) fn grouped(listings: impl IntoIterator<Item = Self>) -> Vec<Self> {
        let mut rows: Vec<Self> = Vec::new();
        for listing in listings {
            match rows.iter_mut().find(|row| row.lists_alike(&listing)) {
                Some(row) => row.count += listing.count,
                None => rows.push(listing),
            }
        }
        rows
    }
}

/// How a monster or item is drawn: always as a glyph, and as a tile too when one is known.
/// The player's Appearance setting picks which the panel shows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Depiction {
    /// The glyph the console game draws.
    pub glyph: Glyph,
    /// The tile webtiles draws, when Game Knowledge names one the app ships.
    pub tile: Option<TileName>,
}

/// A character drawn in one of DCSS's colours, such as a yellow `o`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Glyph {
    /// The character drawn, usually one letter or symbol.
    pub text: String,
    /// Its colour.
    pub colour: Colour,
}

/// A DCSS tile, named by its path in crawl's `rltiles` tree without the extension, such as
/// `mon/animals/jackal`. The panel draws it from `tiles/<name>.png`, which the app ships for every
/// tile Game Knowledge names, and draws the glyph instead should one fail to load.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct TileName(pub String);

/// The sixteen colours DCSS draws glyphs, statuses and menu entries in, named as crawl names
/// them, in crawl's order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum Colour {
    Black,
    Blue,
    Green,
    Cyan,
    Red,
    Magenta,
    Brown,
    LightGrey,
    DarkGrey,
    LightBlue,
    LightGreen,
    LightCyan,
    LightRed,
    LightMagenta,
    Yellow,
    White,
}

impl Colour {
    /// Every colour, in crawl's `COLOURS` order: a colour's index is its number in crawl.
    const ALL: [Self; 16] = [
        Self::Black,
        Self::Blue,
        Self::Green,
        Self::Cyan,
        Self::Red,
        Self::Magenta,
        Self::Brown,
        Self::LightGrey,
        Self::DarkGrey,
        Self::LightBlue,
        Self::LightGreen,
        Self::LightCyan,
        Self::LightRed,
        Self::LightMagenta,
        Self::Yellow,
        Self::White,
    ];

    /// The colour crawl numbers `number`, from 0 for black to 15 for white.
    #[must_use]
    pub fn from_crawl(number: u8) -> Option<Self> {
        Self::ALL.get(usize::from(number)).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_with_camel_case_fields_for_the_webview() {
        let json = serde_json::to_value(ViewModel::initial("1.2.3", Appearance::Tiles)).unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "appVersion": "1.2.3",
                "connection": { "state": "noBrowser" },
                "appearance": "tiles",
                "unreadableGameKnowledge": null,
                "game": null
            })
        );
    }
}
