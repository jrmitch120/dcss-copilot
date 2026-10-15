//! What a Game Source adapter tells the core. Source-neutral: nothing here knows how a game was read.

use serde::Serialize;

use crate::visible_state::VisibleState;

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
