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
    /// on `turn`: the character's latest game picked up again, if it hasn't ended and its turn
    /// hasn't gone backwards, or else a new game, first seen now. Says which with the Session
    /// Event it is: [`SessionEvent::Resynced`] or [`SessionEvent::Joined`].
    pub fn identify(
        &mut self,
        character: &CharacterKey,
        turn: Option<u32>,
        at_ms: u64,
    ) -> (GameKey, SessionEvent) {
        let games = self.by_character.entry(character.clone()).or_default();
        if let Some(game) = games.last().filter(|game| game.continues_at(turn)) {
            return (game.key.clone(), SessionEvent::Resynced);
        }
        let key = GameKey {
            character: character.clone(),
            first_seen_ms: at_ms,
        };
        games.push(KnownGame {
            key: key.clone(),
            last_seen_ms: at_ms,
            turn,
            ended: false,
        });
        (key, SessionEvent::Joined)
    }

    /// The game was seen at `at_ms` on `turn`, and has `ended` or not.
    pub fn saw(&mut self, key: &GameKey, at_ms: u64, turn: Option<u32>, ended: bool) {
        if let Some(game) = self.get_mut(key) {
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
