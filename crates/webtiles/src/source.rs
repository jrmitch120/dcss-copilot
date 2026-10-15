use std::collections::HashMap;

use copilot_core::{CrawlVersion, GameIdentity, Role, SourceEvent, VisibleState};
use copilot_gk::{Bundle, Library};
use serde_json::Value;

use crate::Envelope;
use crate::map::LevelMap;

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
    player: PlayerFields,
    map: LevelMap,
    first_seen_ms: Option<u64>,
    reported: Option<(GameIdentity, VisibleState)>,
}

/// The `player` message fields decoded so far, merged across deltas.
#[derive(Debug, Default)]
struct PlayerFields {
    name: Option<String>,
    species: Option<String>,
    wizard: bool,
    hp: Option<i32>,
    hp_max: Option<i32>,
    turn: Option<u32>,
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
        self.player = PlayerFields::default();
        self.map = LevelMap::default();
        self.first_seen_ms = None;
        self.reported = None;
    }

    /// What the game reports now, with its monsters and items decoded through `knowledge`.
    fn visible(&self, knowledge: Option<&Bundle>) -> VisibleState {
        VisibleState {
            hp: self.player.hp,
            hp_max: self.player.hp_max,
            turn: self.player.turn,
            monsters: self.map.monsters_in_view(knowledge),
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

impl PlayerFields {
    /// Merges a `player` delta: a field is sent only when it changed.
    fn merge(&mut self, msg: &Value) {
        if let Some(name) = string(msg, "name") {
            self.name = Some(name);
        }
        if let Some(species) = string(msg, "species") {
            self.species = Some(species);
        }
        if let Some(wizard) = msg.get("wizard") {
            self.wizard = wizard.as_i64().is_some_and(|w| w != 0) || wizard.as_bool() == Some(true);
        }
        if let Some(hp) = int(msg, "hp") {
            self.hp = Some(hp);
        }
        if let Some(hp_max) = int(msg, "hp_max") {
            self.hp_max = Some(hp_max);
        }
        if let Some(turn) = int(msg, "turn") {
            self.turn = Some(turn);
        }
    }
}

/// The version from a `version` message's text, `<game name> <version>`, such as
/// `Dungeon Crawl Stone Soup 0.34.1`.
fn crawl_version(text: &str) -> Option<CrawlVersion> {
    text.split_whitespace()
        .last()
        .map(|v| CrawlVersion(v.to_owned()))
}

fn string(msg: &Value, field: &str) -> Option<String> {
    msg.get(field).and_then(Value::as_str).map(str::to_owned)
}

fn int<T: TryFrom<i64>>(msg: &Value, field: &str) -> Option<T> {
    msg.get(field)
        .and_then(Value::as_i64)
        .and_then(|n| T::try_from(n).ok())
}
