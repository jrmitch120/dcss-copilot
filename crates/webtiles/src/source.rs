use std::collections::HashMap;

use copilot_core::{
    CharacterKey, CrawlVersion, GameEnd, GameIdentity, GameKey, KnownGame, KnownGames, Role,
    SessionEvent, SourceEvent, VisibleState,
};
use copilot_gk::{Bundle, Library};
use serde_json::Value;

use crate::Envelope;
use crate::crawl_ids::mouse_modes;
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
    known: KnownGames,
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
    /// The game this process plays, once it has named it.
    game: Option<GameKey>,
    /// When this process named its game, in Unix milliseconds.
    named_ms: Option<u64>,
    reported: Option<(GameIdentity, VisibleState)>,
    /// The game process has stopped sending: the game ended, or the player left it.
    stopped: bool,
    /// The game is over.
    ended: bool,
    /// Session Events not yet reported.
    session: Vec<SessionEvent>,
    /// How far the game has got with its latest wait for the player's command.
    wait: CommandWait,
    /// The game settled with the last message, and that isn't reported yet.
    settled: bool,
}

/// How far the game has got with a wait for the player's next command. Crawl says it waits for
/// one (`input_mode`, `MOUSE_MODE_COMMAND`) before it redraws for it, and redraws the player,
/// then the messages, then the map (`TilesFramework::redraw`), each only if it changed; so the
/// map ends the redraw, and a redraw without one ends only as the player acts again.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
enum CommandWait {
    /// The game isn't waiting for a command, or has settled in this wait already.
    #[default]
    Idle,
    /// The game waits for a command and is redrawing for it.
    Drawing,
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
        Self::knowing(knowledge, KnownGames::default())
    }

    /// A source that knows the `known` games, such as those seen before the app last closed, to
    /// tell a game picked up again from a new one, decoding games through `knowledge`.
    #[must_use]
    pub fn knowing(knowledge: Library, known: KnownGames) -> Self {
        Self {
            knowledge,
            tabs: HashMap::new(),
            followed: None,
            known,
        }
    }

    /// The followed game is a new game, as the player says, rather than the one its game process
    /// picked up again: one first seen as the process named it. Returns what changed, which is
    /// nothing for a game the process joined, new already.
    pub fn start_afresh(&mut self) -> Vec<SourceEvent> {
        let Some(game) = self
            .followed
            .as_ref()
            .and_then(|tab| self.tabs.get_mut(tab))
        else {
            return Vec::new();
        };
        let (Some(picked_up), Some(named_ms), Some((identity, _))) =
            (&game.game, game.named_ms, &mut game.reported)
        else {
            return Vec::new();
        };
        if picked_up.first_seen_ms == named_ms {
            return Vec::new();
        }
        let key = self.known.start_afresh(picked_up, named_ms);
        identity.first_seen_ms = key.first_seen_ms;
        game.game = Some(key);
        vec![
            SourceEvent::Identified(identity.clone()),
            SourceEvent::Session(SessionEvent::Joined),
        ]
    }

    /// What was last seen of the game `key` names, if the source has seen it or knew it.
    #[must_use]
    pub fn known_game(&self, key: &GameKey) -> Option<&KnownGame> {
        self.known.get(key)
    }

    /// Takes in one relayed envelope and returns what changed about the followed game.
    pub fn ingest(&mut self, envelope: &Envelope) -> Vec<SourceEvent> {
        let key = TabKey {
            origin: envelope.origin.clone(),
            tab: envelope.tab,
        };
        let crawl_version = envelope.game_version.as_deref().and_then(crawl_version);
        let knowledge = crawl_version
            .as_ref()
            .and_then(|version| self.knowledge.for_version(&version.0));
        let game = self.tabs.entry(key.clone()).or_default();
        game.apply(&envelope.msg, knowledge);

        let identity = game.identify(envelope, crawl_version, &mut self.known);
        let switched = self.followed.as_ref() != Some(&key);
        let Some(identity) = identity else {
            // A game process can stop before it names its game. In the followed tab, that is the
            // followed game's stream stopping; any other tab's waits until its game is named.
            if switched {
                return Vec::new();
            }
            return std::mem::take(&mut game.session)
                .into_iter()
                .map(SourceEvent::Session)
                .collect();
        };
        let session = std::mem::take(&mut game.session);
        let settled = std::mem::take(&mut game.settled);
        let visible = game.visible(knowledge);
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
        // Another tab's game settling, with nothing new to show, doesn't make it the one followed.
        if settled && (!switched || !events.is_empty()) {
            events.push(SourceEvent::Settled);
        }
        if !events.is_empty() {
            game.reported = Some((identity, visible));
            self.followed = Some(key);
        }
        events
    }
}

impl TabGame {
    /// Merges one webtiles message, decoding what it needs to through `knowledge`.
    fn apply(&mut self, msg: &Value, knowledge: Option<&Bundle>) {
        match msg.get("msg").and_then(Value::as_str) {
            Some("login_success") => self.account = string(msg, "username"),
            Some("game_started") => self.start(Role::Playing),
            Some("watching_started") => {
                self.start(Role::Spectating);
                // The game belongs to the account being watched.
                self.account = string(msg, "username");
            }
            Some("input_mode") => self.input_mode(msg, knowledge),
            Some("player") => self.shown.player.merge(msg, knowledge),
            Some("map") => {
                self.shown.map.merge(msg);
                self.settle();
            }
            Some("cursor") => self.shown.map.merge_cursor(msg),
            Some("msgs") => self.shown.merge_messages(msg, knowledge),
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

    /// What the game now waits for, from an `input_mode` message, decoded through `knowledge`: a
    /// command starts a wait whose redraw follows; anything else ends any wait still redrawing,
    /// which settles with what it drew.
    fn input_mode(&mut self, msg: &Value, knowledge: Option<&Bundle>) {
        let mode = msg
            .get("mode")
            .and_then(Value::as_u64)
            .and_then(|mode| u8::try_from(mode).ok())
            .and_then(|mode| knowledge?.mouse_mode(mode));
        if mode.is_some_and(|mode| mode.0 == mouse_modes::COMMAND) {
            self.wait = CommandWait::Drawing;
        } else {
            self.settle();
        }
    }

    /// Ends a wait for a command that is still redrawing: the game has settled.
    fn settle(&mut self) {
        if self.wait == CommandWait::Drawing {
            self.wait = CommandWait::Idle;
            self.settled = true;
        }
    }

    /// A new game process sends its state in full, so nothing from the previous game carries over.
    fn start(&mut self, role: Role) {
        self.role = Some(role);
        self.shown = Shown::default();
        self.game = None;
        self.named_ms = None;
        self.reported = None;
        self.stopped = false;
        self.ended = false;
        self.session.clear();
        self.wait = CommandWait::Idle;
        self.settled = false;
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
            wielded: player.wielded(),
            map: map.level(knowledge),
            messages: messages.recent(),
            screen: screens.open(),
            look_cursor: map.look_cursor(),
        }
    }

    /// The game's identity, on `crawl_version`, once the game has named the character and
    /// species. The first call that succeeds tells whether this process joined a new game or
    /// resynced one already `known`, which fixes when the game was first seen; so does a call
    /// after the character changed, as when the player finishes making a new one.
    fn identify(
        &mut self,
        envelope: &Envelope,
        crawl_version: Option<CrawlVersion>,
        known: &mut KnownGames,
    ) -> Option<GameIdentity> {
        let player = &self.shown.player;
        let (Some(name), Some(species)) = (&player.name, &player.species) else {
            return None;
        };
        // While the player makes a new character, crawl reports it nameless, as some species.
        if name.is_empty() {
            return None;
        }
        let character = CharacterKey {
            server: envelope.origin.clone(),
            account: self.account.clone(),
            character: name.clone(),
            species: species.clone(),
        };
        let key = match &self.game {
            Some(key) if key.character == character => key.clone(),
            _ => {
                let (key, event) = known.identify(&character, player.turn, envelope.ts);
                self.game = Some(key.clone());
                self.named_ms = Some(envelope.ts);
                // Before anything else this process has said about the game.
                self.session.insert(0, event);
                key
            }
        };
        known.saw(&key, envelope.ts, player.turn, self.ended);
        Some(GameIdentity {
            server: key.character.server,
            account: key.character.account,
            character: key.character.character,
            species: key.character.species,
            first_seen_ms: key.first_seen_ms,
            crawl_version,
            wizard: player.wizard,
            role: self.role,
        })
    }
}

impl Shown {
    /// Merges a `msgs` message, learning through `knowledge` any kind of item it says the
    /// player now knows.
    fn merge_messages(&mut self, msg: &Value, knowledge: Option<&Bundle>) {
        let added = self.messages.merge(msg);
        if let (Some(bundle), Some(inventory)) = (knowledge, &mut self.player.inventory) {
            for message in &added {
                inventory.learn_from_message(&message.text, bundle);
            }
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
