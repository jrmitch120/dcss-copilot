use std::collections::HashMap;

use copilot_core::{
    CrawlVersion, GameEnd, GameIdentity, Role, SessionEvent, SourceEvent, VisibleState,
};
use copilot_gk::{Bundle, Library};
use serde_json::Value;

use crate::Envelope;
use crate::json::string;
use crate::map::LevelMap;
use crate::messages::MessageLog;
use crate::player::Player;
use crate::screens::Screens;

/// The webtiles Game Source: decodes relayed envelopes into source-neutral events.
///
/// Each tab's game is merged separately, because webtiles sends deltas per connection. The source
/// follows the tab whose game most recently changed and reports only that game.
///
/// A new game process for a game the source has already seen, such as after the player reloaded
/// the page or played on from a save, is the same game resynced. It is a new game when the last
/// one ended or its turn went backwards.
///
/// Webtiles numbers monster and item types with each Crawl Version's own enums, so the source
/// decodes a game through the Game Knowledge bundle for its version, when it has one.
#[derive(Debug)]
pub struct WebtilesSource {
    knowledge: Library,
    tabs: HashMap<TabKey, TabGame>,
    followed: Option<TabKey>,
    /// Every game seen so far, to tell a resynced game from a new one.
    seen: HashMap<GameKey, SeenGame>,
}

/// What tells one game apart from another before its first-seen time is known.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct GameKey {
    server: String,
    account: Option<String>,
    character: String,
    species: String,
}

/// What the source last saw of a game.
#[derive(Debug, Clone, Copy)]
struct SeenGame {
    first_seen_ms: u64,
    turn: Option<u32>,
    ended: bool,
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
    /// What the game process has sent since it started.
    shown: Shown,
    first_seen_ms: Option<u64>,
    reported: Option<(GameIdentity, VisibleState)>,
    /// The game process has stopped sending: the game ended, or the player left it.
    stopped: bool,
    /// The game is over.
    ended: bool,
    /// Session Events not yet reported.
    session: Vec<SessionEvent>,
}

/// What one game process has sent, merged. A new process sends its state afresh.
#[derive(Debug, Default)]
struct Shown {
    player: Player,
    map: LevelMap,
    messages: MessageLog,
    screens: Screens,
}

impl WebtilesSource {
    /// A source that has seen nothing yet, decoding games through `knowledge`.
    #[must_use]
    pub fn new(knowledge: Library) -> Self {
        Self {
            knowledge,
            tabs: HashMap::new(),
            followed: None,
            seen: HashMap::new(),
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

        let identity = game.identify(envelope, &mut self.seen);
        let session = std::mem::take(&mut game.session);
        let Some(identity) = identity else {
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
        events.extend(session.into_iter().map(SourceEvent::Session));
        if let Some(screen) = &visible.screen
            && last_visible.and_then(|last| last.screen.as_ref()) != Some(screen)
        {
            events.push(SourceEvent::ScreenRead(screen.clone()));
        }
        if last_visible != Some(&visible) {
            events.push(SourceEvent::Visible(Box::new(visible.clone())));
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
            Some("player") => self.shown.player.merge(msg),
            Some("map") => self.shown.map.merge(msg),
            Some("msgs") => self.shown.messages.merge(msg),
            Some("game_ended") => {
                let reason = string(msg, "reason");
                let end = reason.as_deref().and_then(game_end);
                self.ended = end.is_some();
                self.stop(end.map_or(SessionEvent::ConnectionLost, |how| {
                    SessionEvent::GameEnded { how }
                }));
            }
            // Back in the lobby without the game ending: the player stopped watching, or the
            // server left the game.
            Some("go_lobby") => self.stop(SessionEvent::ConnectionLost),
            _ => self.shown.screens.merge(msg),
        }
    }

    /// A new game process sends its state in full, so nothing from the previous game carries over.
    fn start(&mut self, role: Role) {
        self.role = Some(role);
        self.shown = Shown::default();
        self.first_seen_ms = None;
        self.reported = None;
        self.stopped = false;
        self.ended = false;
        self.session.clear();
    }

    /// The game process stopped sending, which only the first message that says so reports.
    fn stop(&mut self, event: SessionEvent) {
        if !self.stopped {
            self.stopped = true;
            self.session.push(event);
        }
    }

    /// What the game reports now, with its monsters and items decoded through `knowledge`.
    fn visible(&self, knowledge: Option<&Bundle>) -> VisibleState {
        let Shown {
            player,
            map,
            messages,
            screens,
        } = &self.shown;
        VisibleState {
            turn: player.turn,
            stats: player.stats.clone(),
            religion: player.religion(),
            place: player.place(),
            position: player.position,
            statuses: player.statuses.clone(),
            monsters: map.monsters_in_view(knowledge),
            inventory: player
                .inventory
                .as_ref()
                .map(|inventory| inventory.items(knowledge)),
            map: map.level(knowledge),
            messages: messages.recent(),
            screen: screens.open(),
        }
    }

    /// The game's identity, once the game has named the character and species. The first call
    /// that succeeds tells whether this process joined a new game or resynced one already
    /// `seen`, which fixes when the game was first seen.
    fn identify(
        &mut self,
        envelope: &Envelope,
        seen: &mut HashMap<GameKey, SeenGame>,
    ) -> Option<GameIdentity> {
        let player = &self.shown.player;
        let (Some(name), Some(species)) = (&player.name, &player.species) else {
            return None;
        };
        let key = GameKey {
            server: envelope.origin.clone(),
            account: self.account.clone(),
            character: name.clone(),
            species: species.clone(),
        };
        let first_seen_ms = if let Some(first_seen_ms) = self.first_seen_ms {
            first_seen_ms
        } else {
            let (first_seen_ms, event) = match seen.get(&key) {
                Some(game) if game.continues_at(player.turn) => {
                    (game.first_seen_ms, SessionEvent::Resynced)
                }
                _ => (envelope.ts, SessionEvent::Joined),
            };
            self.first_seen_ms = Some(first_seen_ms);
            // Before anything else this process has said about the game.
            self.session.insert(0, event);
            first_seen_ms
        };
        seen.insert(
            key,
            SeenGame {
                first_seen_ms,
                turn: player.turn,
                ended: self.ended,
            },
        );
        Some(GameIdentity {
            server: envelope.origin.clone(),
            account: self.account.clone(),
            character: name.clone(),
            species: species.clone(),
            first_seen_ms,
            crawl_version: envelope.game_version.as_deref().and_then(crawl_version),
            wizard: player.wizard,
            role: self.role,
        })
    }
}

impl SeenGame {
    /// A game process at `turn` can be this game picked up again: it hasn't ended and its turn
    /// hasn't gone backwards.
    fn continues_at(&self, turn: Option<u32>) -> bool {
        !self.ended
            && match (self.turn, turn) {
                (Some(last), Some(now)) => now >= last,
                _ => true,
            }
    }
}

/// How the game ended, from a `game_ended` message's reason: the exit types crawl's `end.cc`
/// names. Any other reason, such as `saved`, `disconnect` or `crash`, leaves the game unfinished.
fn game_end(reason: &str) -> Option<GameEnd> {
    match reason {
        "won" => Some(GameEnd::Won),
        "dead" => Some(GameEnd::Died),
        "quit" => Some(GameEnd::Quit),
        "bailed out" => Some(GameEnd::LeftTheDungeon),
        _ => None,
    }
}

/// The version from a `version` message's text, `<game name> <version>`, such as
/// `Dungeon Crawl Stone Soup 0.34.1`.
fn crawl_version(text: &str) -> Option<CrawlVersion> {
    text.split_whitespace()
        .last()
        .map(|v| CrawlVersion(v.to_owned()))
}
