//! The live game: what the relay has delivered, turned into the panel's view model.

use std::fs::File;
use std::io::BufWriter;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::{SystemTime, UNIX_EPOCH};

use copilot_core::{Appearance, AssistanceLevel, Connection, ViewModel};
use copilot_gk::{BundleError, Library};
use copilot_llm::{ModelChoice, ModelSlot, sidecar_path};
use copilot_replay::{Following, RECORDING_EXTENSION, RecordingError, RecordingWriter};
use copilot_store::{Store, StoreError};
use copilot_webtiles::Envelope;
use serde::Serialize;

use crate::relay::{ApprovalRequest, RelayEvent};
use crate::settings::{FORGET_GAMES_AFTER_DAYS, Settings, SettingsError};

/// Everything the app knows about the browser connection and the game it relays.
pub struct Live {
    app_version: &'static str,
    /// Why the shipped Game Knowledge couldn't be read, if it couldn't, for the player.
    unreadable_knowledge: Option<String>,
    /// Why the game history couldn't be opened, if it couldn't, for the player.
    unopened_history: Option<String>,
    state: Mutex<State>,
}

struct State {
    game: Following,
    /// Allowed browsers currently connected.
    browsers: usize,
    approval: Option<ApprovalRequest>,
    buffer_overflowed: bool,
    relay_unavailable: Option<String>,
    recorder: Recorder,
    settings: Settings,
}

/// A day, in milliseconds.
const DAY_MS: u64 = 24 * 60 * 60 * 1000;

/// The game history, in Settings → History.
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct HistoryView {
    /// How many games the Advisor remembers.
    pub games: u32,
    /// How many days a game is kept after it was last played, or `None` to keep every game.
    pub forget_after_days: Option<u32>,
    /// How many days a game is kept once the player turns forgetting on.
    pub keep_days: u32,
}

/// How often, in game-page milliseconds, a Recording is flushed, so a crash loses at most this much.
const RECORDING_FLUSH_MS: u64 = 1_000;

/// Dev builds save every envelope they receive, so a session can be replayed. Each browser
/// connection replays its tabs from the start, so each gets its own Recording.
enum Recorder {
    Off,
    /// Recording into this directory; the next file starts when an envelope arrives.
    Idle(PathBuf),
    Writing {
        dir: PathBuf,
        /// The Recording being written.
        path: PathBuf,
        writer: RecordingWriter<BufWriter<File>>,
        /// The envelope timestamp of the last flush.
        flushed_at: u64,
    },
}

impl Live {
    /// Starts with no browser connected and the player's `settings`, remembering games in the
    /// `history` store, and writing a Recording into `recordings` if given. A history that
    /// couldn't be opened is kept in memory instead, until the app closes, and the panel says
    /// why.
    pub fn new(
        app_version: &'static str,
        recordings: Option<PathBuf>,
        settings: Settings,
        history: Result<Store, StoreError>,
    ) -> Self {
        Self::with_knowledge(
            app_version,
            recordings,
            settings,
            history,
            Library::shipped(),
        )
    }

    /// As [`Live::new`], with `knowledge` in place of the Game Knowledge the app ships. Without
    /// it, games are followed undecoded and unjudged, and the panel says why.
    fn with_knowledge(
        app_version: &'static str,
        recordings: Option<PathBuf>,
        settings: Settings,
        history: Result<Store, StoreError>,
        knowledge: Result<Library, BundleError>,
    ) -> Self {
        let recorder = recordings.map_or(Recorder::Off, Recorder::Idle);
        let (knowledge, unreadable_knowledge) = match knowledge {
            Ok(knowledge) => (knowledge, None),
            Err(error) => {
                crate::log::error(format_args!(
                    "{error}: games are followed without Game Knowledge"
                ));
                (Library::default(), Some(error.to_string()))
            }
        };
        let (history, unopened_history) = match history {
            Ok(store) => (Ok(store), None),
            Err(error) => {
                crate::log::error(format_args!(
                    "{error}: games are remembered only until DCSS Copilot closes"
                ));
                (Store::in_memory(), Some(error.to_string()))
            }
        };
        let game = match history {
            Ok(store) => Following::remembering(&knowledge, store),
            Err(error) => {
                crate::log::error(format_args!(
                    "{error}: games are forgotten when a browser reconnects"
                ));
                Following::new(&knowledge)
            }
        };
        Self {
            app_version,
            unreadable_knowledge,
            unopened_history,
            state: Mutex::new(State {
                game,
                browsers: 0,
                approval: None,
                buffer_overflowed: false,
                relay_unavailable: None,
                recorder,
                settings,
            }),
        }
    }

    /// Everything the panel shows now.
    pub fn view_model(&self) -> ViewModel {
        let state = self.lock();
        let connection = if let Some(reason) = &state.relay_unavailable {
            Connection::RelayUnavailable {
                reason: reason.clone(),
            }
        } else if let Some(approval) = &state.approval {
            Connection::AllowBrowser {
                browser: approval.browser().to_owned(),
            }
        } else if state.browsers > 0 {
            Connection::Connected {
                buffer_overflowed: state.buffer_overflowed,
            }
        } else {
            Connection::NoBrowser
        };
        ViewModel {
            app_version: self.app_version.to_owned(),
            connection,
            appearance: state.settings.appearance(),
            assistance: state.settings.assistance(),
            unreadable_game_knowledge: self.unreadable_knowledge.clone(),
            history_unsaved: state
                .game
                .history_unsaved()
                .map(str::to_owned)
                .or_else(|| self.unopened_history.clone()),
            game: state.game.advisor().game_view(state.settings.assistance()),
        }
    }

    /// Takes in something the relay reported.
    pub fn handle(&self, event: RelayEvent) {
        let mut state = self.lock();
        match event {
            // Replacing an unanswered request closes its connection, which retries.
            RelayEvent::ApprovalNeeded(request) => state.approval = Some(request),
            RelayEvent::Connected(_) => {
                // A connection replays every tab from its first message, so with no other browser
                // connected nothing relayed so far is still current: only what is remembered.
                if state.browsers == 0 {
                    state.forget_old_games();
                    state.game.restart();
                }
                state.browsers += 1;
                state.recorder.finish();
            }
            RelayEvent::Disconnected => {
                state.browsers = state.browsers.saturating_sub(1);
                if state.browsers == 0 {
                    state.buffer_overflowed = false;
                }
            }
            RelayEvent::Overflow => state.buffer_overflowed = true,
            RelayEvent::Envelope(envelope) => state.ingest(&envelope),
            // Where the panel shows is the shell's to decide; it says nothing of the game.
            RelayEvent::SidePanel(_) => {}
        }
    }

    /// The player answered "Allow this browser?".
    ///
    /// # Errors
    /// If the answer couldn't be saved, so the player will be asked again next time.
    pub fn answer_browser(&self, allow: bool) -> Result<(), String> {
        let request = self.lock().approval.take();
        match request {
            Some(request) => request.answer(allow).map_err(|error| error.to_string()),
            None => Ok(()),
        }
    }

    /// The player chose how monsters and items are drawn.
    ///
    /// # Errors
    /// If the choice couldn't be saved, so it lasts only until the app exits.
    pub fn set_appearance(&self, appearance: Appearance) -> Result<(), SettingsError> {
        self.lock().settings.set_appearance(appearance)
    }

    /// The player chose how much the Advisor offers beyond Information.
    ///
    /// # Errors
    /// If the choice couldn't be saved, so it lasts only until the app exits.
    pub fn set_assistance(&self, assistance: AssistanceLevel) -> Result<(), SettingsError> {
        self.lock().settings.set_assistance(assistance)
    }

    /// The game history, for Settings → History.
    pub fn history(&self) -> HistoryView {
        let state = self.lock();
        let games = state.game.games_remembered().unwrap_or_else(|error| {
            crate::log::error(format_args!(
                "could not count the games remembered: {error}"
            ));
            0
        });
        HistoryView {
            games: u32::try_from(games).unwrap_or(u32::MAX),
            forget_after_days: state.settings.forget_games_after_days(),
            keep_days: FORGET_GAMES_AFTER_DAYS,
        }
    }

    /// The player turned forgetting games not played for a while on or off. Turned on, it
    /// forgets them now, and again each time a browser connects.
    ///
    /// # Errors
    /// If the choice couldn't be saved, so it lasts only until the app exits.
    pub fn set_forget_old_games(&self, on: bool) -> Result<(), SettingsError> {
        let mut state = self.lock();
        let saved = state
            .settings
            .set_forget_games_after_days(on.then_some(FORGET_GAMES_AFTER_DAYS));
        state.forget_old_games();
        saved
    }

    /// The player cleared their game history: every game but the one followed is forgotten.
    ///
    /// # Errors
    /// If the history couldn't be cleared.
    pub fn clear_history(&self) -> Result<(), StoreError> {
        self.lock().game.forget_all_but_followed()
    }

    /// The player says the followed game is a new game, not the one the Advisor took it for.
    pub fn new_game(&self) {
        self.lock().game.new_game();
    }

    /// The model a slot calls.
    pub fn model(&self, slot: ModelSlot) -> ModelChoice {
        self.lock().settings.model(slot).clone()
    }

    /// The player chose the model a slot calls.
    ///
    /// # Errors
    /// If the choice couldn't be saved, so it lasts only until the app exits.
    pub fn set_model(&self, slot: ModelSlot, choice: ModelChoice) -> Result<(), SettingsError> {
        self.lock().settings.set_model(slot, choice)
    }

    /// Where LLM calls are recorded while a dev build records a Recording: its LLM sidecar.
    pub fn llm_sidecar(&self) -> Option<PathBuf> {
        match &self.lock().recorder {
            Recorder::Writing { path, .. } => Some(sidecar_path(path)),
            Recorder::Off | Recorder::Idle(_) => None,
        }
    }

    /// The relay couldn't start, so no browser can connect.
    pub fn relay_unavailable(&self, reason: String) {
        self.lock().relay_unavailable = Some(reason);
    }

    /// Ends the Recording so it can be read to its last envelope. Call before the app exits.
    pub fn finish_recording(&self) {
        self.lock().recorder.finish();
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        // Each update leaves the state whole, so a panic elsewhere can't leave it half-changed.
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl State {
    /// Forgets every game not played for as long as the player keeps games, if they forget any.
    fn forget_old_games(&mut self) {
        let Some(days) = self.settings.forget_games_after_days() else {
            return;
        };
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |since| {
                u64::try_from(since.as_millis()).unwrap_or(u64::MAX)
            });
        let before_ms = now_ms.saturating_sub(u64::from(days) * DAY_MS);
        if let Err(error) = self.game.forget_games_seen_before(before_ms) {
            crate::log::error(format_args!(
                "could not forget games not played for {days} days: {error}"
            ));
        }
    }

    fn ingest(&mut self, envelope: &Envelope) {
        self.recorder.record(envelope);
        // What the detectors notice reaches the panel through the view model.
        self.game.ingest(envelope);
    }
}

impl Recorder {
    fn record(&mut self, envelope: &Envelope) {
        if let Self::Idle(dir) = self {
            let path = dir.join(format!("{}.{RECORDING_EXTENSION}", envelope.ts));
            *self = match start_recording(dir, &path) {
                Ok(writer) => Self::Writing {
                    dir: dir.clone(),
                    path,
                    writer,
                    flushed_at: envelope.ts,
                },
                Err(error) => {
                    crate::log::error(format_args!(
                        "recording: could not start {}: {error}",
                        path.display()
                    ));
                    Self::Off
                }
            };
        }
        if let Self::Writing {
            writer, flushed_at, ..
        } = self
        {
            let due = envelope.ts.saturating_sub(*flushed_at) >= RECORDING_FLUSH_MS;
            let written = writer.write(envelope).and_then(|()| {
                if due {
                    *flushed_at = envelope.ts;
                    writer.flush()?;
                }
                Ok(())
            });
            if let Err(error) = written {
                crate::log::error(format_args!("recording: stopped, could not write: {error}"));
                *self = Self::Off;
            }
        }
    }

    /// Ends the current Recording, if any; the next envelope starts a new one.
    fn finish(&mut self) {
        let Self::Writing { dir, .. } = self else {
            return;
        };
        let idle = Self::Idle(dir.clone());
        if let Self::Writing { writer, .. } = std::mem::replace(self, idle)
            && let Err(error) = writer.finish()
        {
            crate::log::error(format_args!("recording: could not finish: {error}"));
        }
    }
}

fn start_recording(
    dir: &Path,
    path: &Path,
) -> Result<RecordingWriter<BufWriter<File>>, RecordingError> {
    std::fs::create_dir_all(dir)?;
    let file = BufWriter::new(File::create(path)?);
    RecordingWriter::new(file, env!("CARGO_PKG_VERSION"))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use copilot_core::{
        AccumulatedState, Appearance, AssistanceLevel, CharacterKey, GameKey, KnownGame,
    };
    use copilot_llm::{Effort, Provider, Thinking};

    use super::*;
    use crate::relay::BrowserLink;
    use crate::settings::Settings;

    fn live() -> Live {
        let path = std::env::temp_dir().join("dcss-copilot-unused-settings.json");
        Live::new("1.2.3", None, Settings::defaults(&path), Store::in_memory())
    }

    fn envelope(msg: serde_json::Value) -> RelayEvent {
        RelayEvent::Envelope(Envelope {
            origin: "https://crawl.example.org".to_owned(),
            tab: 7,
            ts: 1_760_486_400_000,
            game_version: None,
            msg,
        })
    }

    #[test]
    fn shipped_game_knowledge_that_doesnt_read_is_shown_to_the_player() {
        let path = std::env::temp_dir().join("dcss-copilot-unused-settings.json");
        let broken = BundleError {
            tag: "0.34.1".to_owned(),
            reason: "expected value at line 1 column 1".to_owned(),
        };

        let without = Live::with_knowledge(
            "1.2.3",
            None,
            Settings::defaults(&path),
            Store::in_memory(),
            Err(broken),
        );

        assert_eq!(
            without.view_model().unreadable_game_knowledge.as_deref(),
            Some(
                "the Game Knowledge bundle for crawl 0.34.1 couldn't be read: expected value at \
                 line 1 column 1"
            )
        );
        assert_eq!(live().view_model().unreadable_game_knowledge, None);
    }

    #[test]
    fn a_browser_connecting_when_none_is_starts_afresh_because_its_tabs_replay_from_the_start() {
        let live = live();
        live.handle(RelayEvent::Connected(BrowserLink::unconnected()));
        live.handle(envelope(json!({ "msg": "game_started" })));
        live.handle(envelope(
            json!({ "msg": "player", "name": "Mira", "species": "Minotaur", "turn": 500 }),
        ));
        assert!(live.view_model().game.is_some());

        live.handle(RelayEvent::Disconnected);
        live.handle(RelayEvent::Connected(BrowserLink::unconnected()));

        assert_eq!(live.view_model().game, None);
    }

    #[test]
    fn the_games_followed_are_saved_in_the_game_history() {
        let dir = tempfile::tempdir().unwrap();
        let history = dir.path().join("games.sqlite");
        let live = Live::new(
            "1.2.3",
            None,
            Settings::defaults(&dir.path().join("settings.json")),
            Store::open(&history),
        );

        live.handle(envelope(json!({ "msg": "game_started" })));
        live.handle(envelope(
            json!({ "msg": "player", "name": "Mira", "species": "Minotaur", "turn": 500 }),
        ));

        let saved = Store::open(&history).unwrap().known_games().unwrap();
        assert_eq!(saved.len(), 1);
        assert_eq!(saved[0].key.character.character, "Mira");
        assert_eq!(saved[0].turn, Some(500));
    }

    /// A store at `path` remembering one game, last played `days_ago`.
    fn played_days_ago(path: &Path, days_ago: u64) {
        let now_ms = u64::try_from(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis(),
        )
        .unwrap();
        let last_seen_ms = now_ms - days_ago * DAY_MS;
        let game = KnownGame {
            key: GameKey {
                character: CharacterKey {
                    server: "https://crawl.example.org".to_owned(),
                    account: Some("mira".to_owned()),
                    character: "Mira".to_owned(),
                    species: "Minotaur".to_owned(),
                },
                first_seen_ms: last_seen_ms - DAY_MS,
            },
            last_seen_ms,
            turn: Some(500),
            ended: false,
        };
        Store::open(path)
            .unwrap()
            .save(&game, &AccumulatedState::default())
            .unwrap();
    }

    #[test]
    fn games_not_played_for_ninety_days_are_forgotten_as_a_browser_connects_unless_kept() {
        let dir = tempfile::tempdir().unwrap();
        let (settings, history) = (
            dir.path().join("settings.json"),
            dir.path().join("games.sqlite"),
        );
        played_days_ago(&history, 89);
        let live = Live::new(
            "1.2.3",
            None,
            Settings::load(&settings).unwrap(),
            Store::open(&history),
        );
        live.handle(RelayEvent::Connected(BrowserLink::unconnected()));
        assert_eq!(live.history().games, 1);

        played_days_ago(&history, 91);
        live.set_forget_old_games(false).unwrap();
        live.handle(RelayEvent::Disconnected);
        live.handle(RelayEvent::Connected(BrowserLink::unconnected()));
        assert_eq!(live.history().games, 2, "kept while forgetting is off");

        let restarted = Live::new(
            "1.2.3",
            None,
            Settings::load(&settings).unwrap(),
            Store::open(&history),
        );
        assert_eq!(restarted.history().forget_after_days, None);
        restarted.set_forget_old_games(true).unwrap();
        assert_eq!(restarted.history().games, 1, "forgotten as it is turned on");
        assert_eq!(restarted.history().forget_after_days, Some(90));
    }

    #[test]
    fn a_game_history_that_cant_be_opened_is_shown_to_the_player() {
        let dir = tempfile::tempdir().unwrap();
        let unopened = Live::new(
            "1.2.3",
            None,
            Settings::defaults(&dir.path().join("settings.json")),
            Err(StoreError::OtherLayout { found: 9 }),
        );

        assert_eq!(
            unopened.view_model().history_unsaved.as_deref(),
            Some("the game history was written by another version of DCSS Copilot (layout 9)")
        );
        assert_eq!(live().view_model().history_unsaved, None);
    }

    #[test]
    fn the_players_appearance_reaches_the_panel_and_is_remembered_across_restarts() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let live = Live::new(
            "1.2.3",
            None,
            Settings::load(&path).unwrap(),
            Store::in_memory(),
        );
        assert_eq!(live.view_model().appearance, Appearance::Tiles);

        live.set_appearance(Appearance::Glyphs).unwrap();
        assert_eq!(live.view_model().appearance, Appearance::Glyphs);

        let restarted = Live::new(
            "1.2.3",
            None,
            Settings::load(&path).unwrap(),
            Store::in_memory(),
        );
        assert_eq!(restarted.view_model().appearance, Appearance::Glyphs);
    }

    #[test]
    fn the_players_assistance_level_reaches_the_panel_and_is_remembered_across_restarts() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let live = Live::new(
            "1.2.3",
            None,
            Settings::load(&path).unwrap(),
            Store::in_memory(),
        );
        assert_eq!(live.view_model().assistance, AssistanceLevel::Hints);

        live.set_assistance(AssistanceLevel::Info).unwrap();
        assert_eq!(live.view_model().assistance, AssistanceLevel::Info);

        let restarted = Live::new(
            "1.2.3",
            None,
            Settings::load(&path).unwrap(),
            Store::in_memory(),
        );
        assert_eq!(restarted.view_model().assistance, AssistanceLevel::Info);
    }

    #[test]
    fn the_players_model_for_each_slot_is_remembered_across_restarts() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let live = Live::new(
            "1.2.3",
            None,
            Settings::load(&path).unwrap(),
            Store::in_memory(),
        );
        let haiku_thinking = ModelChoice::new(
            Provider::Anthropic,
            "claude-haiku-5-5",
            Thinking::Adaptive(Effort::Low),
        )
        .unwrap();

        live.set_model(ModelSlot::Quick, haiku_thinking.clone())
            .unwrap();

        let restarted = Live::new(
            "1.2.3",
            None,
            Settings::load(&path).unwrap(),
            Store::in_memory(),
        );
        assert_eq!(restarted.model(ModelSlot::Quick), haiku_thinking);
        assert_eq!(
            restarted.model(ModelSlot::Deep),
            ModelChoice::default_for(ModelSlot::Deep)
        );
    }

    #[test]
    fn llm_calls_are_recorded_beside_the_recording_being_written() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let live = Live::new(
            "1.2.3",
            Some(dir.path().join("recordings")),
            Settings::load(&path).unwrap(),
            Store::in_memory(),
        );
        assert_eq!(live.llm_sidecar(), None);

        live.handle(envelope(json!({ "msg": "game_started" })));

        assert_eq!(
            live.llm_sidecar(),
            Some(dir.path().join("recordings/1760486400000.llm.jsonl"))
        );
    }
}
