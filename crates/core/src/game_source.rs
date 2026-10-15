//! What a Game Source adapter tells the core. Source-neutral: nothing here knows how a game was read.

use copilot_gk::{Bundle, Library};
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::screen::ScreenRead;
use crate::visible_state::VisibleState;

/// One thing a Game Source learned about the game it follows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "event", rename_all = "camelCase")]
pub enum SourceEvent {
    /// The game being followed, or what is known about it, changed.
    Identified(GameIdentity),
    /// The game now reports this Visible State.
    Visible(Box<VisibleState>),
    /// The player opened a screen, or the open screen changed, and this is what it shows. The
    /// Visible State that follows holds it too, while it stays open.
    ScreenRead(ScreenRead),
    /// Something happened to the source's view of the game, rather than in the game.
    Session(SessionEvent),
}

/// Something that happened to a Game Source's view of the game it follows, rather than in the
/// game: a Session Event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "session", rename_all = "camelCase")]
pub enum SessionEvent {
    /// The source began following a game it hadn't seen before, from its start or part way
    /// through.
    Joined,
    /// The source was sent the state of a game it had already seen afresh, picking it up where it
    /// was: after the player reloaded the page, or played on from a save.
    Resynced,
    /// The game's stream stopped without the game ending, such as when the player saved, the
    /// connection to the server dropped, or the game crashed. It may be picked up again.
    ConnectionLost,
    /// The game is over.
    GameEnded {
        /// How it ended.
        how: GameEnd,
    },
}

/// How a game ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum GameEnd {
    /// The character escaped with the Orb of Zot.
    Won,
    /// The character died.
    Died,
    /// The player quit the game.
    Quit,
    /// The character left the dungeon without the Orb.
    LeftTheDungeon,
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

impl GameIdentity {
    /// The Game Knowledge in `library` for this game's Crawl Version, once the game has reported
    /// its version and if the library has a bundle for it.
    #[must_use]
    pub fn knowledge<'a>(&self, library: &'a Library) -> Option<&'a Bundle> {
        library.for_version(&self.crawl_version.as_ref()?.0)
    }
}

/// The exact build of DCSS a game runs on, such as `0.34.1` or a trunk build like `0.35-a0-123-gabc1234`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrawlVersion(pub String);

impl fmt::Display for CrawlVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// How the player relates to the game being followed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Role {
    /// The player is playing.
    Playing,
    /// The player is spectating someone else's game.
    Spectating,
}
