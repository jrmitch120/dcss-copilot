//! The games a Game Source has seen, and which of them a game process is: the same game picked
//! up again, or a new one.

use std::collections::HashMap;

use crate::game_source::SessionEvent;

/// What a game is played as: a character on a server, from an account. Its games are told apart
/// by when each was first seen.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CharacterKey {
    /// Where the game is played, such as a webtiles server's origin.
    pub server: String,
    /// The account playing the game, when the source has seen it.
    pub account: Option<String>,
    /// The character's name.
    pub character: String,
    /// The character's species, as the game names it.
    pub species: String,
}

/// What tells one game apart from every other, for as long as it is played: the Game Key.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct GameKey {
    /// What it is played as.
    pub character: CharacterKey,
    /// When a Game Source first saw it, in Unix milliseconds.
    pub first_seen_ms: u64,
}

/// What a Game Source last saw of one game.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KnownGame {
    /// Which game it is.
    pub key: GameKey,
    /// When the source last took in a message of it, in Unix milliseconds.
    pub last_seen_ms: u64,
    /// Its turn then, if it had reported one.
    pub turn: Option<u32>,
    /// It is over.
    pub ended: bool,
}

/// Every game a Game Source has seen, to tell a game picked up again from a new one.
#[derive(Debug, Clone, Default)]
pub struct KnownGames {
    /// Each character's games, in the order first seen.
    by_character: HashMap<CharacterKey, Vec<KnownGame>>,
}

impl KnownGames {
    /// Knowing `games`, such as those seen before the app last closed.
    #[must_use]
    pub fn new(games: impl IntoIterator<Item = KnownGame>) -> Self {
        let mut known = Self::default();
        for game in games {
            known
                .by_character
                .entry(game.key.character.clone())
                .or_default()
                .push(game);
        }
        for games in known.by_character.values_mut() {
            games.sort_by_key(|game| game.key.first_seen_ms);
        }
        known
    }

    /// Which game a game process playing `character` is, as it first names the game at `at_ms`
    /// on `turn`, and the Session Event that is.
    ///
    /// The character's latest game first seen by then is picked up again
    /// ([`SessionEvent::Resynced`]) if it hasn't ended and its turn hasn't gone backwards;
    /// otherwise this is a new game, first seen now ([`SessionEvent::Joined`]). A process that
    /// names a game no later than it was last seen is that game taken in again, as when a browser
    /// replays a page to an app that restarted: it joined the game if it named it as the game was
    /// first seen, and resynced it otherwise.
    pub fn identify(
        &mut self,
        character: &CharacterKey,
        turn: Option<u32>,
        at_ms: u64,
    ) -> (GameKey, SessionEvent) {
        let games = self.by_character.entry(character.clone()).or_default();
        let latest = games
            .iter()
            .rev()
            .find(|game| game.key.first_seen_ms <= at_ms);
        if let Some(game) = latest {
            if at_ms <= game.last_seen_ms {
                let event = if at_ms == game.key.first_seen_ms {
                    SessionEvent::Joined
                } else {
                    SessionEvent::Resynced
                };
                return (game.key.clone(), event);
            }
            if game.continues_at(turn) {
                return (game.key.clone(), SessionEvent::Resynced);
            }
        }
        let key = GameKey {
            character: character.clone(),
            first_seen_ms: at_ms,
        };
        let at = games.partition_point(|game| game.key.first_seen_ms <= at_ms);
        games.insert(
            at,
            KnownGame {
                key: key.clone(),
                last_seen_ms: at_ms,
                turn,
                ended: false,
            },
        );
        (key, SessionEvent::Joined)
    }

    /// The game process that picked the game `picked_up` up again, naming it at `named_ms`,
    /// plays a new game instead, as the player says: one first seen then, and last seen as the
    /// process last saw `picked_up`. Returns its key.
    pub fn start_afresh(&mut self, picked_up: &GameKey, named_ms: u64) -> GameKey {
        let key = GameKey {
            character: picked_up.character.clone(),
            first_seen_ms: named_ms,
        };
        let last = self.get(picked_up).cloned();
        let games = self.by_character.entry(key.character.clone()).or_default();
        if games.iter().any(|game| game.key == key) {
            return key;
        }
        let at = games.partition_point(|game| game.key.first_seen_ms <= named_ms);
        games.insert(
            at,
            KnownGame {
                key: key.clone(),
                last_seen_ms: last.as_ref().map_or(named_ms, |game| game.last_seen_ms),
                turn: last.and_then(|game| game.turn),
                ended: false,
            },
        );
        key
    }

    /// The game was seen at `at_ms` on `turn`, and has `ended` or not. A message from before it
    /// was last seen, taken in again, changes nothing.
    pub fn saw(&mut self, key: &GameKey, at_ms: u64, turn: Option<u32>, ended: bool) {
        if let Some(game) = self.get_mut(key).filter(|game| at_ms >= game.last_seen_ms) {
            game.last_seen_ms = at_ms;
            game.turn = turn;
            game.ended = ended;
        }
    }

    /// What was last seen of the game `key` names, if it has been seen.
    #[must_use]
    pub fn get(&self, key: &GameKey) -> Option<&KnownGame> {
        self.by_character
            .get(&key.character)?
            .iter()
            .find(|game| game.key == *key)
    }

    fn get_mut(&mut self, key: &GameKey) -> Option<&mut KnownGame> {
        self.by_character
            .get_mut(&key.character)?
            .iter_mut()
            .find(|game| game.key == *key)
    }
}

impl KnownGame {
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
