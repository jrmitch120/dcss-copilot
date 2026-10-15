use std::collections::HashMap;

use copilot_core::{CrawlVersion, GameIdentity, Role, SourceEvent, VisibleState};
use copilot_gk::{Bundle, Library};
use serde_json::Value;

use crate::Envelope;
use crate::json::string;
use crate::map::LevelMap;
use crate::player::Player;

/// The webtiles Game Source: decodes relayed envelopes into source-neutral events.
///
/// Each tab's game is merged separately, because webtiles sends deltas per connection. The source
/// follows the tab whose game most recently changed and reports only that game.
///
/// Webtiles numbers monster and item types with each Crawl Version's own enums, so the source
/// decodes a game through the Game Knowledge bundle for its version, when it has one.
#[derive(Debug)]
pub struct WebtilesSource {
    knowledge: Library,
    tabs: HashMap<TabKey, TabGame>,
    followed: Option<TabKey>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct TabKey {
    origin: String,
    tab: i64,
}

/// What one tab's messages have said so far.
#[derive(Debug, Default)]
struct TabGame {
    account: Option<String>,
    role: Option<Role>,
    player: Player,
    map: LevelMap,
    first_seen_ms: Option<u64>,
    reported: Option<(GameIdentity, VisibleState)>,
}

impl WebtilesSource {
    /// A source that has seen nothing yet, decoding games through `knowledge`.
    #[must_use]
    pub fn new(knowledge: Library) -> Self {
        Self {
            knowledge,
            tabs: HashMap::new(),
            followed: None,
        }
    }

    /// Takes in one relayed envelope and returns what changed about the followed game.
    pub fn ingest(&mut self, envelope: &Envelope) -> Vec<SourceEvent> {
        let key = TabKey {
            origin: envelope.origin.clone(),
            tab: envelope.tab,
        };
        let game = self.tabs.entry(key.clone()).or_default();
        game.apply(&envelope.msg);

        let Some(identity) = game.identify(envelope) else {
            return Vec::new();
        };
        let crawl_version = identity.crawl_version.as_ref();
        let knowledge = crawl_version.and_then(|version| self.knowledge.for_version(&version.0));
        let visible = game.visible(knowledge);
        let switched = self.followed.as_ref() != Some(&key);
        let mut events = Vec::new();
        let (last_identity, last_visible) = match &game.reported {
            Some((identity, visible)) if !switched => (Some(identity), Some(visible)),
            _ => (None, None),
        };
        if last_identity != Some(&identity) {
            events.push(SourceEvent::Identified(identity.clone()));
        }
        if last_visible != Some(&visible) {
            events.push(SourceEvent::Visible(visible.clone()));
        }
        if !events.is_empty() {
            game.reported = Some((identity, visible));
            self.followed = Some(key);
        }
        events
    }
}

impl TabGame {
    fn apply(&mut self, msg: &Value) {
        match msg.get("msg").and_then(Value::as_str) {
            Some("login_success") => self.account = string(msg, "username"),
            Some("game_started") => self.start(Role::Playing),
            Some("watching_started") => {
                self.start(Role::Spectating);
                // The game belongs to the account being watched.
                self.account = string(msg, "username");
            }
            Some("player") => self.player.merge(msg),
            Some("map") => self.map.merge(msg),
            _ => {}
        }
    }

    /// A new game process sends its state in full, so nothing from the previous game carries over.
    fn start(&mut self, role: Role) {
        self.role = Some(role);
        self.player = Player::default();
        self.map = LevelMap::default();
        self.first_seen_ms = None;
        self.reported = None;
    }

    /// What the game reports now, with its monsters and items decoded through `knowledge`.
    fn visible(&self, knowledge: Option<&Bundle>) -> VisibleState {
        VisibleState {
            turn: self.player.turn,
            stats: self.player.stats.clone(),
            religion: self.player.religion(),
            place: self.player.place(),
            position: self.player.position,
            statuses: self.player.statuses.clone(),
            monsters: self.map.monsters_in_view(knowledge),
            inventory: self
                .player
                .inventory
                .as_ref()
                .map(|inventory| inventory.items(knowledge)),
            map: self.map.level(knowledge),
        }
    }

    /// The game's identity, once the game has named the character and species. The first call
    /// that succeeds fixes when the game was first seen.
    fn identify(&mut self, envelope: &Envelope) -> Option<GameIdentity> {
        let (Some(name), Some(species)) = (&self.player.name, &self.player.species) else {
            return None;
        };
        let first_seen_ms = *self.first_seen_ms.get_or_insert(envelope.ts);
        Some(GameIdentity {
            server: envelope.origin.clone(),
            account: self.account.clone(),
            character: name.clone(),
            species: species.clone(),
            first_seen_ms,
            crawl_version: envelope.game_version.as_deref().and_then(crawl_version),
            wizard: self.player.wizard,
            role: self.role,
        })
    }
}

/// The version from a `version` message's text, `<game name> <version>`, such as
/// `Dungeon Crawl Stone Soup 0.34.1`.
fn crawl_version(text: &str) -> Option<CrawlVersion> {
    text.split_whitespace()
        .last()
        .map(|v| CrawlVersion(v.to_owned()))
}
