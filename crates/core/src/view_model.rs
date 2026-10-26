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
    /// How much the Advisor offers beyond Information, as the player set it.
    pub assistance: AssistanceLevel,
    /// Why the Game Knowledge the app ships couldn't be read, when it couldn't. Games are then
    /// followed without it: monsters and items aren't decoded and no Threat Level is judged.
    pub unreadable_game_knowledge: Option<String>,
    /// Why the game history couldn't be read or saved, the last time it couldn't. What the
    /// Advisor remembers of a game it couldn't save lasts only until the app closes.
    pub history_unsaved: Option<String>,
    /// The panel for the game being followed, once one is identified.
    pub game: Option<GameView>,
}

impl ViewModel {
    /// The view before any browser has connected.
    #[must_use]
    pub fn initial(app_version: &str, appearance: Appearance, assistance: AssistanceLevel) -> Self {
        Self {
            app_version: app_version.to_owned(),
            connection: Connection::NoBrowser,
            appearance,
            assistance,
            unreadable_game_knowledge: None,
            history_unsaved: None,
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

/// How much the Advisor offers beyond Information, as the player sets it: the Assistance Level.
/// Every detector runs at every level; the level only decides what the panel shows.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum AssistanceLevel {
    /// Information only: the facts, and no LLM.
    Info,
    /// Adds the Suggested Action at DANGEROUS and LETHAL moments, and a one-line LLM nudge at
    /// each Decision Point.
    #[default]
    Hints,
    /// Adds the LLM's judgment of Tactical picks, and full Strategic Advice with Run Plan
    /// changes.
    Coach,
}

impl AssistanceLevel {
    /// Whether the panel shows the Suggested Action.
    #[must_use]
    pub fn suggests(self) -> bool {
        match self {
            Self::Info => false,
            Self::Hints | Self::Coach => true,
        }
    }
}

/// The panel for the followed game, top to bottom: header, now pill, focus card,
/// Run Plan rail and ask box. The footer belongs to the app, not the game.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct GameView {
    /// Who is playing, shown in the header.
    pub character: Character,
    /// How dangerous this moment is, at a glance.
    pub now: NowPill,
    /// What the Advisor's own rules suggest doing, at a DANGEROUS or LETHAL moment, when the
    /// player's Assistance Level shows it.
    pub suggested_action: Option<SuggestedActionCard>,
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
    /// The game seen before that this game's process picked up again on a later turn, until the
    /// player says whether it is that game: the panel asks before it shows anything else.
    pub picked_up: Option<PickedUp>,
    /// Why what was saved of this game couldn't be read, if it couldn't: the Advisor started the
    /// game over, remembering nothing from before. The Change Log still holds what was saved.
    pub accumulated_unreadable: Option<String>,
    /// What the Toolbar Badge shows while the panel is closed.
    pub badge: ToolbarBadge,
    /// Every Gap in what the Advisor knows of the game, in the header.
    pub gaps: GapsChip,
}

/// The Gaps chip in the panel's header: every Gap in what the Advisor knows of the game, each with
/// why it matters and the keys that fill it.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct GapsChip {
    /// One row a Gap, those the same in words as one.
    pub gaps: Vec<GapRow>,
    /// What the character dump (`?` then `#`) fills of them at once, each as a sentence names
    /// it, such as `the spell list`: none when it fills none, or the player is watching a game
    /// not theirs.
    pub dump_fills: Vec<String>,
}

/// One Gap as the Gaps chip lists it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct GapRow {
    /// What is missing or out of date, such as `Spell list` or `~Skills as of turn 12`.
    pub what: String,
    /// Why it matters, in one sentence.
    pub why: String,
    /// The keys to press in the game that fill it, such as `I` then `!`: none when no screen
    /// does, or the player is watching a game not theirs.
    pub keys: Vec<String>,
}

/// A game seen before that a game process picked up again on a later turn, as last seen. Nobody
/// watched the turns between, so the process may be playing another game of the same character.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PickedUp {
    /// Its turn then, if it had reported one.
    pub turn: Option<u32>,
    /// When, in Unix milliseconds: exact as a TypeScript number for millennia yet.
    #[specta(type = specta_typescript::Number)]
    pub last_seen_ms: u64,
}

/// The panel's stand-in while it is closed: the extension's toolbar button, badged with the
/// Decision Points that wait, or with a call to open the panel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ToolbarBadge {
    /// How many Decision Points wait for the player. Nothing raises Decision Points yet, so
    /// none wait.
    pub decisions_waiting: u32,
    /// This moment is LETHAL and no Way Out is trusted: the only moment the badge calls the
    /// player to open the panel.
    pub calls_for_panel: bool,
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

/// A value the panel shows, marked in place when a Gap leaves it unknown or out of date: drawn
/// with a dotted underline, and a tooltip saying what leaves it so and which screen fills it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Value {
    /// The value, such as `spell · ~4% fail as of turn 12`.
    pub text: String,
    /// What leaves it unknown or out of date, and how to fill it where the player can, one
    /// sentence a Gap, such as `~Spell list as of turn 12: open it (I) to bring it up to date.`:
    /// `None` while no Gap touches it.
    pub gap: Option<String>,
}

impl Value {
    /// `text`, which no Gap touches.
    pub(crate) fn known(text: String) -> Self {
        Self { text, gap: None }
    }

    /// `text`, marked with the `gaps` that leave it unknown or out of date, one sentence each:
    /// unmarked when there are none.
    pub(crate) fn marked(text: String, gaps: impl IntoIterator<Item = String>) -> Self {
        let gaps: Vec<String> = gaps.into_iter().collect();
        Self {
            text,
            gap: (!gaps.is_empty()).then(|| gaps.join(" ")),
        }
    }
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
    /// A short fact beside the name, such as `7 steps` or `spell · 4% fail`, marked when the
    /// menu or list it was read off is out of date.
    pub detail: Option<Value>,
    /// Why it can't be used, when it can't, such as `Centaur is faster`, marked when it rests on
    /// a menu out of date.
    pub why_not: Option<Value>,
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
        /// when anything can attack, marked when they were judged around a Gap, such as
        /// resistances unread.
        detail: Option<Value>,
    },
}

/// The Suggested Action, under the now pill: what the Advisor's own rules pick, and the rule that
/// picked it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SuggestedActionCard {
    /// The Threat Level the rule is for, LETHAL or DANGEROUS. The pick holds steady, so the
    /// moment may have eased since.
    pub level: ThreatLevel,
    /// The rule that picked it, such as `Leave by stairs`.
    pub rule: String,
    /// What it picked, in one line, such as `Stone staircase leading down · 4 steps` or `Melee:
    /// ~95% win, ~20% HP lost`, marked when a Gap leaves its facts unknown or out of date.
    pub action: Value,
    /// At fight carefully, the Way Out to take should the moment turn LETHAL, such as `Blink ·
    /// spell · 4% fail`, or that none is left, marked as `action` is.
    pub if_lethal: Option<Value>,
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
        /// The fight in one line: the best Attack Plan's odds, such as `Best plan (Fireball, then
        /// melee): ~80% win, ~45% HP lost`, or that the Advisor can't judge it yet, marked with
        /// the Gaps that keep it from being judged or may hide a better plan.
        line: Value,
        /// The other plans, best first, one line each, such as `Melee: ~60% win, ~70% HP lost`.
        others: Vec<String>,
        /// What the fight leaves out, as one line, such as `Not modelled: summons, allies, hexes`.
        /// `None` when the fight wasn't judged.
        not_modelled: Option<String>,
        /// What keeps the Advisor from judging the fight, or may hide a better plan, one sentence
        /// each, such as a screen to open.
        gaps: Vec<String>,
    },
    /// The Advisor joined the game part way through, and nothing hostile is in view: the join
    /// card.
    Join {
        /// What the card shows.
        join: JoinCard,
    },
}

/// The join card: what the panel asks of a player whose game the Advisor joined part way
/// through, and what it read once they opened the screen it asked for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(
    tag = "step",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum JoinCard {
    /// It asks the player to open the character dump, or the dungeon overview for the levels
    /// alone.
    Asking {
        /// The game's turn when the Advisor joined it, if the game had reported one.
        turn: Option<u32>,
    },
    /// The character dump is open, and this is what the Advisor read off it.
    Reading {
        /// One row for each part of the dump the Advisor reads.
        checklist: Vec<ChecklistRow>,
    },
    /// The player closed the character dump: the Advisor is caught up on what it showed.
    CaughtUpFromDump {
        /// The parts of the dump the Advisor read off it, as a sentence names them, such as
        /// `skills` or `the dungeon overview`.
        current: Vec<String>,
        /// The parts of the dump the Advisor reads that it didn't show and that the Advisor
        /// doesn't know otherwise, each with the keys that open the screen showing it.
        not_in_dump: Vec<UnreadPart>,
    },
    /// The player opened the dungeon overview instead of the dump: the Advisor knows the levels,
    /// but not yet the character.
    CaughtUpFromOverview {
        /// What it read of the levels, such as `3 branches, 1 altar, 2 shops`.
        levels: String,
    },
}

/// A part of the character dump the Advisor reads that the dump didn't show, such as the spells,
/// and how to show it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct UnreadPart {
    /// What it is, as a sentence names it, such as `spells` or `the dungeon overview`.
    pub name: String,
    /// The keys to press in the game that open the screen showing it, such as `I`, or `Ctrl`
    /// then `O`.
    pub keys: Vec<String>,
}

/// One part of the character dump on the join card's checklist, such as the skills.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ChecklistRow {
    /// The Advisor read it off the dump: drawn ✓, else ○.
    pub read: bool,
    /// What it read, such as `Skills (12, with decimals)`, or that the dump didn't show it.
    pub line: String,
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
    /// The Advisor joined the game part way through and hasn't read the character dump, so it
    /// makes no Run Plan yet: the rail is greyed out.
    AwaitingDump,
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
        let json = serde_json::to_value(ViewModel::initial(
            "1.2.3",
            Appearance::Tiles,
            AssistanceLevel::Hints,
        ))
        .unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "appVersion": "1.2.3",
                "connection": { "state": "noBrowser" },
                "appearance": "tiles",
                "assistance": "hints",
                "unreadableGameKnowledge": null,
                "historyUnsaved": null,
                "game": null
            })
        );
    }
}
