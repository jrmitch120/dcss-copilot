use serde::{Deserialize, Serialize};

/// One webtiles message as the extension relayed it. See `docs/relay-protocol.md`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Envelope {
    /// The game server's page origin.
    pub origin: String,
    /// The browser tab the game runs in.
    pub tab: i64,
    /// When the page received the message, in Unix milliseconds.
    pub ts: u64,
    /// The text of the latest webtiles `version` message, verbatim.
    pub game_version: Option<String>,
    /// The webtiles message, unchanged.
    pub msg: serde_json::Value,
}
