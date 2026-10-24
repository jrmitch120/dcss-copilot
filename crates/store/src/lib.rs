//! SQLite persistence for Accumulated State: every game the Advisor has followed, what it
//! remembers of each now, and the Change Log of how that came to be.
//!
//! A game's row holds its Game Key, what was last seen of it and its Accumulated State as JSON.
//! Each save appends a row to the Change Log for each of its facts that changed, so the log
//! tells what the Advisor knew of a game at any moment. The log is never rewritten: SQLite
//! refuses any update to it, and its rows go only with their game.

use std::collections::HashMap;
use std::path::Path;

use copilot_core::{AccumulatedState, CharacterKey, GameKey, KnownGame};
use rusqlite::{Connection, OptionalExtension, Transaction, params};
use serde_json::{Map, Value};

/// The layout the store writes, as SQLite's `user_version`. A store laid out otherwise is
/// refused rather than misread.
const SCHEMA_VERSION: i64 = 1;

const SCHEMA: &str = "
    CREATE TABLE games (
        id INTEGER PRIMARY KEY,
        server TEXT NOT NULL,
        account TEXT,
        character TEXT NOT NULL,
        species TEXT NOT NULL,
        first_seen_ms INTEGER NOT NULL,
        last_seen_ms INTEGER NOT NULL,
        turn INTEGER,
        ended INTEGER NOT NULL,
        accumulated TEXT NOT NULL
    );
    CREATE INDEX games_by_character ON games (server, character, species);
    CREATE INDEX games_by_last_seen ON games (last_seen_ms);
    CREATE TABLE changes (
        id INTEGER PRIMARY KEY,
        game INTEGER NOT NULL REFERENCES games (id) ON DELETE CASCADE,
        received_ms INTEGER NOT NULL,
        turn INTEGER,
        fact TEXT NOT NULL,
        value TEXT NOT NULL
    );
    CREATE INDEX changes_by_game ON changes (game, id);
    CREATE TRIGGER changes_append_only BEFORE UPDATE ON changes
    BEGIN
        SELECT RAISE(ABORT, 'the Change Log is append-only');
    END;
";

/// Every game the Advisor has followed, saved in one SQLite file.
#[derive(Debug)]
pub struct Store {
    connection: Connection,
    /// What each game's row held when this store last read or wrote it, so a save writes only
    /// what changed.
    rows: HashMap<GameKey, Row>,
}

/// One game's row, as last read or written.
#[derive(Debug, Clone)]
struct Row {
    id: i64,
    turn: Option<u32>,
    ended: bool,
    /// Its Accumulated State, fact by fact.
    facts: Map<String, Value>,
}

/// One entry in a game's Change Log: one fact as a save left it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    /// When the message that changed it was received, in Unix milliseconds.
    pub received_ms: u64,
    /// The game's turn then, if it had reported one.
    pub turn: Option<u32>,
    /// Which fact of the Accumulated State changed, by its name there, such as `resistances`.
    pub fact: String,
    /// What it became, as JSON.
    pub value: Value,
}

/// Why the store couldn't read or write.
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    /// SQLite failed, such as when the file can't be opened or the disk is full.
    #[error("the game history can't be read or written: {0}")]
    Sqlite(#[from] rusqlite::Error),
    /// The file's directory couldn't be made.
    #[error("the game history's folder can't be made: {0}")]
    Folder(#[from] std::io::Error),
    /// The file was written by a version of the app that lays it out otherwise.
    #[error("the game history was written by another version of DCSS Copilot (layout {found})")]
    OtherLayout {
        /// The layout the file has.
        found: i64,
    },
    /// What is saved of a game isn't the Accumulated State this version of the app reads.
    #[error("what is saved of this game can't be read: {0}")]
    Unreadable(serde_json::Error),
    /// What the Advisor remembers of a game couldn't be written as JSON.
    #[error("what is remembered of this game can't be saved: {0}")]
    Unwritable(serde_json::Error),
}

impl Store {
    /// Opens the store saved at `path`, making it if there is none.
    ///
    /// # Errors
    /// If the file can't be opened or made, or another version of the app laid it out.
    pub fn open(path: &Path) -> Result<Self, StoreError> {
        if let Some(folder) = path.parent() {
            std::fs::create_dir_all(folder)?;
        }
        Self::start(Connection::open(path)?)
    }

    /// A store kept in memory only, lost when it is dropped: for when no file can be opened.
    ///
    /// # Errors
    /// If SQLite can't start, which leaves nowhere to keep games at all.
    pub fn in_memory() -> Result<Self, StoreError> {
        Self::start(Connection::open_in_memory()?)
    }

    fn start(connection: Connection) -> Result<Self, StoreError> {
        connection.pragma_update(None, "foreign_keys", true)?;
        // A save is one small transaction per change, so WAL keeps each from waiting on a sync.
        connection.pragma_update(None, "journal_mode", "WAL")?;
        let layout: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
        match layout {
            0 => {
                connection.execute_batch(SCHEMA)?;
                connection.pragma_update(None, "user_version", SCHEMA_VERSION)?;
            }
            SCHEMA_VERSION => {}
            found => return Err(StoreError::OtherLayout { found }),
        }
        Ok(Self {
            connection,
            rows: HashMap::new(),
        })
    }

    /// What was last saved of every game, to tell a game picked up again from a new one.
    ///
    /// # Errors
    /// If SQLite fails.
    pub fn known_games(&self) -> Result<Vec<KnownGame>, StoreError> {
        let mut statement = self.connection.prepare(
            "SELECT server, account, character, species, first_seen_ms, last_seen_ms, turn, ended
             FROM games ORDER BY first_seen_ms",
        )?;
        let games = statement.query_map([], |row| {
            Ok(KnownGame {
                key: GameKey {
                    character: CharacterKey {
                        server: row.get(0)?,
                        account: row.get(1)?,
                        character: row.get(2)?,
                        species: row.get(3)?,
                    },
                    first_seen_ms: row.get(4)?,
                },
                last_seen_ms: row.get(5)?,
                turn: row.get(6)?,
                ended: row.get(7)?,
            })
        })?;
        Ok(games.collect::<Result<_, _>>()?)
    }

    /// What the Advisor remembered of the game `key` names when it was last saved, if it was.
    ///
    /// # Errors
    /// If SQLite fails, or what is saved isn't the Accumulated State this version of the app
    /// reads. Saving the game again still works, from what was saved.
    pub fn accumulated(&mut self, key: &GameKey) -> Result<Option<AccumulatedState>, StoreError> {
        let Some(row) = self.row(key)? else {
            return Ok(None);
        };
        serde_json::from_value(Value::Object(row.facts))
            .map(Some)
            .map_err(StoreError::Unreadable)
    }

    /// Saves what was last seen of a game and what the Advisor remembers of it, appending each
    /// fact that changed since the last save to its Change Log, as of when the game was last
    /// seen. A game saved for the first time logs the facts it remembers anything of.
    ///
    /// # Errors
    /// If SQLite fails. Nothing of this save is kept then.
    pub fn save(
        &mut self,
        game: &KnownGame,
        accumulated: &AccumulatedState,
    ) -> Result<(), StoreError> {
        let facts = facts_of(accumulated)?;
        let before = match self.row(&game.key)? {
            Some(row) => row,
            None => self.insert(game)?,
        };
        let changed: Vec<(&String, &Value)> = facts
            .iter()
            .filter(|(fact, value)| before.facts.get(*fact) != Some(value))
            .collect();
        if changed.is_empty() && (before.turn, before.ended) == (game.turn, game.ended) {
            return Ok(());
        }
        let transaction = self.connection.transaction()?;
        transaction.execute(
            "UPDATE games SET last_seen_ms = ?2, turn = ?3, ended = ?4, accumulated = ?5
             WHERE id = ?1",
            params![
                before.id,
                game.last_seen_ms,
                game.turn,
                game.ended,
                Value::Object(facts.clone()).to_string()
            ],
        )?;
        log(&transaction, before.id, game, &changed)?;
        transaction.commit()?;
        self.rows.insert(
            game.key.clone(),
            Row {
                id: before.id,
                turn: game.turn,
                ended: game.ended,
                facts,
            },
        );
        Ok(())
    }

    /// The Change Log of the game `key` names, oldest first.
    ///
    /// # Errors
    /// If SQLite fails, or a logged value isn't JSON.
    pub fn changes(&mut self, key: &GameKey) -> Result<Vec<Change>, StoreError> {
        let Some(row) = self.row(key)? else {
            return Ok(Vec::new());
        };
        let mut statement = self.connection.prepare(
            "SELECT received_ms, turn, fact, value FROM changes WHERE game = ?1 ORDER BY id",
        )?;
        let rows = statement.query_map([row.id], |row| {
            Ok((
                row.get::<_, u64>(0)?,
                row.get::<_, Option<u32>>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        })?;
        rows.map(|row| {
            let (received_ms, turn, fact, value) = row?;
            Ok(Change {
                received_ms,
                turn,
                fact,
                value: serde_json::from_str(&value).map_err(StoreError::Unreadable)?,
            })
        })
        .collect()
    }

    /// Forgets every game last seen before `before_ms`, in Unix milliseconds, with its Change
    /// Log, and says how many it forgot.
    ///
    /// # Errors
    /// If SQLite fails. Nothing is forgotten then.
    pub fn forget_seen_before(&mut self, before_ms: u64) -> Result<usize, StoreError> {
        let forgotten = self
            .connection
            .execute("DELETE FROM games WHERE last_seen_ms < ?1", [before_ms])?;
        self.rows.clear();
        Ok(forgotten)
    }

    /// Forgets every game but `keep`, if given, with their Change Logs.
    ///
    /// # Errors
    /// If SQLite fails. Nothing is forgotten then.
    pub fn forget_all_but(&mut self, keep: Option<&GameKey>) -> Result<(), StoreError> {
        let kept = match keep {
            Some(key) => self.row(key)?.map(|row| row.id),
            None => None,
        };
        self.connection
            .execute("DELETE FROM games WHERE id IS NOT ?1", [kept])?;
        self.rows.retain(|_, row| Some(row.id) == kept);
        Ok(())
    }

    /// The row of the game `key` names, if it has been saved.
    fn row(&mut self, key: &GameKey) -> Result<Option<Row>, StoreError> {
        if let Some(row) = self.rows.get(key) {
            return Ok(Some(row.clone()));
        }
        let character = &key.character;
        let found = self
            .connection
            .query_row(
                "SELECT id, turn, ended, accumulated FROM games
                 WHERE server = ?1 AND account IS ?2 AND character = ?3 AND species = ?4
                   AND first_seen_ms = ?5",
                params![
                    character.server,
                    character.account,
                    character.character,
                    character.species,
                    key.first_seen_ms
                ],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, Option<u32>>(1)?,
                        row.get::<_, bool>(2)?,
                        row.get::<_, String>(3)?,
                    ))
                },
            )
            .optional()?;
        let Some((id, turn, ended, accumulated)) = found else {
            return Ok(None);
        };
        // A row whose facts don't parse is saved over from nothing, rather than never again.
        let facts = match serde_json::from_str(&accumulated) {
            Ok(Value::Object(facts)) => facts,
            _ => Map::new(),
        };
        let row = Row {
            id,
            turn,
            ended,
            facts,
        };
        self.rows.insert(key.clone(), row.clone());
        Ok(Some(row))
    }

    /// Adds a row for `game`, remembering nothing of it yet.
    fn insert(&mut self, game: &KnownGame) -> Result<Row, StoreError> {
        let facts = facts_of(&AccumulatedState::default())?;
        let character = &game.key.character;
        self.connection.execute(
            "INSERT INTO games (server, account, character, species, first_seen_ms,
                                last_seen_ms, turn, ended, accumulated)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                character.server,
                character.account,
                character.character,
                character.species,
                game.key.first_seen_ms,
                game.last_seen_ms,
                game.turn,
                game.ended,
                Value::Object(facts.clone()).to_string()
            ],
        )?;
        Ok(Row {
            id: self.connection.last_insert_rowid(),
            turn: game.turn,
            ended: game.ended,
            facts,
        })
    }
}

/// An Accumulated State, fact by fact, as JSON.
fn facts_of(accumulated: &AccumulatedState) -> Result<Map<String, Value>, StoreError> {
    serde_json::to_value(accumulated)
        .and_then(serde_json::from_value)
        .map_err(StoreError::Unwritable)
}

/// Appends each of the `changed` facts of `game`, saved under row `id`, to its Change Log.
fn log(
    transaction: &Transaction<'_>,
    id: i64,
    game: &KnownGame,
    changed: &[(&String, &Value)],
) -> Result<(), StoreError> {
    let mut statement = transaction.prepare(
        "INSERT INTO changes (game, received_ms, turn, fact, value) VALUES (?1, ?2, ?3, ?4, ?5)",
    )?;
    for (fact, value) in changed {
        statement.execute(params![
            id,
            game.last_seen_ms,
            game.turn,
            fact,
            value.to_string()
        ])?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use copilot_core::{
        AsOf, CarriedGear, CarriedItem, GearSlot, GearSource, ItemBrand, Remembered, Resistance,
        Resistances, Screen, Sighting, Unlisted,
    };

    use super::*;

    fn key(first_seen_ms: u64) -> GameKey {
        GameKey {
            character: CharacterKey {
                server: "https://crawl.example.org".to_owned(),
                account: Some("tiv".to_owned()),
                character: "Tiv".to_owned(),
                species: "Minotaur".to_owned(),
            },
            first_seen_ms,
        }
    }

    fn seen(key: &GameKey, last_seen_ms: u64, turn: u32) -> KnownGame {
        KnownGame {
            key: key.clone(),
            last_seen_ms,
            turn: Some(turn),
            ended: false,
        }
    }

    /// Remembering the character's resistances, read on `turn`.
    fn resisting_fire(turn: u32) -> AccumulatedState {
        let mut remembered = AccumulatedState::default();
        remembered.resistances = Remembered::Known(AsOf {
            value: Resistances {
                fire: Some(Resistance::Pips(1)),
                ..Resistances::default()
            },
            turn: Some(turn),
            source: Screen::CharacterOverview,
        });
        remembered
    }

    #[test]
    fn a_game_reads_back_as_it_was_saved_after_the_store_is_opened_again() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("history/games.sqlite");
        let game = seen(&key(1_000), 9_000, 120);
        let mut remembered = resisting_fire(100);
        remembered.carried_gear.insert(
            Sighting(7),
            Remembered::Stale(AsOf {
                value: CarriedGear {
                    items: vec![CarriedItem {
                        slot: GearSlot::Wielding,
                        name: "club".to_owned(),
                        kind: None,
                        plus: None,
                        brand: ItemBrand::None,
                        pair: false,
                    }],
                    unlisted: Unlisted::Nothing,
                },
                turn: Some(110),
                source: GearSource::Sighting,
            }),
        );
        Store::open(&path)
            .unwrap()
            .save(&game, &remembered)
            .unwrap();

        let mut reopened = Store::open(&path).unwrap();

        assert_eq!(reopened.known_games().unwrap(), std::slice::from_ref(&game));
        assert_eq!(reopened.accumulated(&game.key).unwrap(), Some(remembered));
        assert_eq!(reopened.accumulated(&key(2_000)).unwrap(), None);
    }

    #[test]
    fn each_save_logs_only_the_facts_that_changed_as_of_when_the_game_was_last_seen() {
        let mut store = Store::in_memory().unwrap();
        let game = key(1_000);

        store
            .save(&seen(&game, 2_000, 100), &resisting_fire(100))
            .unwrap();
        store
            .save(&seen(&game, 2_500, 100), &resisting_fire(100))
            .unwrap();
        let mut stale = resisting_fire(100);
        stale.resistances = match stale.resistances {
            Remembered::Known(fact) => Remembered::Stale(fact),
            other => other,
        };
        store.save(&seen(&game, 3_000, 101), &stale).unwrap();

        let changes = store.changes(&game).unwrap();
        let logged: Vec<_> = changes
            .iter()
            .map(|change| (change.received_ms, change.turn, change.fact.as_str()))
            .collect();
        assert_eq!(
            logged,
            [
                (2_000, Some(100), "resistances"),
                (3_000, Some(101), "resistances")
            ]
        );
        assert_eq!(changes[1].value["state"], "stale");
    }

    #[test]
    fn the_change_log_cant_be_rewritten() {
        let mut store = Store::in_memory().unwrap();
        store
            .save(&seen(&key(1_000), 2_000, 100), &resisting_fire(100))
            .unwrap();

        let rewritten = store
            .connection
            .execute("UPDATE changes SET value = '{}'", []);

        assert!(rewritten.is_err());
        assert_eq!(store.changes(&key(1_000)).unwrap().len(), 1);
    }

    #[test]
    fn games_last_seen_before_a_time_are_forgotten_with_their_change_logs() {
        let mut store = Store::in_memory().unwrap();
        let (old, recent) = (key(1_000), key(5_000));
        store
            .save(&seen(&old, 2_000, 100), &resisting_fire(100))
            .unwrap();
        store
            .save(&seen(&recent, 6_000, 10), &resisting_fire(10))
            .unwrap();

        assert_eq!(store.forget_seen_before(6_000).unwrap(), 1);

        assert_eq!(store.known_games().unwrap(), [seen(&recent, 6_000, 10)]);
        assert_eq!(store.accumulated(&old).unwrap(), None);
        let logged: i64 = store
            .connection
            .query_row("SELECT count(*) FROM changes", [], |row| row.get(0))
            .unwrap();
        assert_eq!(logged, 1);
    }

    #[test]
    fn every_game_but_the_one_kept_can_be_forgotten() {
        let mut store = Store::in_memory().unwrap();
        let (old, followed) = (key(1_000), key(5_000));
        store
            .save(&seen(&old, 2_000, 100), &resisting_fire(100))
            .unwrap();
        store
            .save(&seen(&followed, 6_000, 10), &resisting_fire(10))
            .unwrap();

        store.forget_all_but(Some(&followed)).unwrap();
        assert_eq!(store.known_games().unwrap(), [seen(&followed, 6_000, 10)]);

        store.forget_all_but(None).unwrap();
        assert_eq!(store.known_games().unwrap(), []);
    }

    #[test]
    fn a_store_another_version_laid_out_is_refused_rather_than_misread() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("games.sqlite");
        Connection::open(&path)
            .unwrap()
            .pragma_update(None, "user_version", 99)
            .unwrap();

        let opened = Store::open(&path);

        assert!(matches!(opened, Err(StoreError::OtherLayout { found: 99 })));
    }

    #[test]
    fn a_saved_game_this_version_cant_read_is_said_so_and_can_still_be_saved_over() {
        let mut store = Store::in_memory().unwrap();
        let game = seen(&key(1_000), 2_000, 100);
        store.save(&game, &resisting_fire(100)).unwrap();
        store
            .connection
            .execute(
                "UPDATE games SET accumulated = '{\"resistances\":{\"state\":\"pondered\"}}'",
                [],
            )
            .unwrap();
        store.rows.clear();

        assert!(matches!(
            store.accumulated(&game.key),
            Err(StoreError::Unreadable(_))
        ));
        store.save(&game, &resisting_fire(100)).unwrap();
        assert_eq!(
            store.accumulated(&game.key).unwrap(),
            Some(resisting_fire(100))
        );
    }
}
