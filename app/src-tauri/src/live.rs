//! The live game: what the relay has delivered, turned into the panel's view model.

use std::fs::File;
use std::io::BufWriter;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, PoisonError};

use copilot_core::{Advisor, Appearance, Connection, ViewModel};
use copilot_gk::{BundleError, Library};
use copilot_llm::{ModelChoice, ModelSlot, sidecar_path};
use copilot_replay::{RECORDING_EXTENSION, RecordingError, RecordingWriter};
use copilot_webtiles::{Envelope, WebtilesSource};

use crate::relay::{ApprovalRequest, RelayEvent};
use crate::settings::{Settings, SettingsError};

/// Everything the app knows about the browser connection and the game it relays.
pub struct Live {
    app_version: &'static str,
    /// The Game Knowledge games are decoded through.
    knowledge: Library,
    /// Why the shipped Game Knowledge couldn't be read, if it couldn't, for the player.
    unreadable_knowledge: Option<String>,
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
    /// Starts with no browser connected and the player's `settings`, writing a Recording into
    /// `recordings` if given.
    pub fn new(app_version: &'static str, recordings: Option<PathBuf>, settings: Settings) -> Self {
        Self::with_knowledge(app_version, recordings, settings, Library::shipped())
    }

    /// As [`Live::new`], with `knowledge` in place of the Game Knowledge the app ships. Without
    /// it, games are followed undecoded and unjudged, and the panel says why.
    fn with_knowledge(
        app_version: &'static str,
        recordings: Option<PathBuf>,
        settings: Settings,
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
        Self {
            app_version,
            unreadable_knowledge,
            knowledge: knowledge.clone(),
            state: Mutex::new(State {
                game: Following::new(&knowledge),
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
            unreadable_game_knowledge: self.unreadable_knowledge.clone(),
            game: state.game.advisor.game_view(),
        }
    }

    /// Takes in something the relay reported.
    pub fn handle(&self, event: RelayEvent) {
        let mut state = self.lock();
        match event {
            // Replacing an unanswered request closes its connection, which retries.
            RelayEvent::ApprovalNeeded(request) => state.approval = Some(request),
            RelayEvent::Connected => {
                // A connection replays every tab from its first message, so with no other browser
                // connected nothing known so far is still current.
                if state.browsers == 0 {
                    state.game = Following::new(&self.knowledge);
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
    fn ingest(&mut self, envelope: &Envelope) {
        self.recorder.record(envelope);
        self.game.ingest(envelope);
    }
}

/// The game the relay delivers: the webtiles Game Source and the Advisor it feeds, always built
/// together from the same Game Knowledge.
struct Following {
    source: WebtilesSource,
    advisor: Advisor,
}

impl Following {
    /// Nothing followed yet, decoding and judging games through `knowledge`.
    fn new(knowledge: &Library) -> Self {
        Self {
            source: WebtilesSource::new(knowledge.clone()),
            advisor: Advisor::new(knowledge.clone()),
        }
    }

    /// Decodes one relayed envelope and tells the Advisor what the Game Source learned.
    fn ingest(&mut self, envelope: &Envelope) {
        for event in self.source.ingest(envelope) {
            self.advisor.apply(event);
        }
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

    use copilot_core::Appearance;
    use copilot_llm::{Effort, Provider, Thinking};

    use super::*;
    use crate::settings::Settings;

    fn live() -> Live {
        let path = std::env::temp_dir().join("dcss-copilot-unused-settings.json");
        Live::new("1.2.3", None, Settings::defaults(&path))
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

        let without = Live::with_knowledge("1.2.3", None, Settings::defaults(&path), Err(broken));

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
        live.handle(RelayEvent::Connected);
        live.handle(envelope(json!({ "msg": "game_started" })));
        live.handle(envelope(
            json!({ "msg": "player", "name": "Mira", "species": "Minotaur", "turn": 500 }),
        ));
        assert!(live.view_model().game.is_some());

        live.handle(RelayEvent::Disconnected);
        live.handle(RelayEvent::Connected);

        assert_eq!(live.view_model().game, None);
    }

    #[test]
    fn the_players_appearance_reaches_the_panel_and_is_remembered_across_restarts() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let live = Live::new("1.2.3", None, Settings::load(&path).unwrap());
        assert_eq!(live.view_model().appearance, Appearance::Tiles);

        live.set_appearance(Appearance::Glyphs).unwrap();
        assert_eq!(live.view_model().appearance, Appearance::Glyphs);

        let restarted = Live::new("1.2.3", None, Settings::load(&path).unwrap());
        assert_eq!(restarted.view_model().appearance, Appearance::Glyphs);
    }

    #[test]
    fn the_players_model_for_each_slot_is_remembered_across_restarts() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let live = Live::new("1.2.3", None, Settings::load(&path).unwrap());
        let haiku_thinking = ModelChoice::new(
            Provider::Anthropic,
            "claude-haiku-5-5",
            Thinking::Adaptive(Effort::Low),
        )
        .unwrap();

        live.set_model(ModelSlot::Quick, haiku_thinking.clone())
            .unwrap();

        let restarted = Live::new("1.2.3", None, Settings::load(&path).unwrap());
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
        );
        assert_eq!(live.llm_sidecar(), None);

        live.handle(envelope(json!({ "msg": "game_started" })));

        assert_eq!(
            live.llm_sidecar(),
            Some(dir.path().join("recordings/1760486400000.llm.jsonl"))
        );
    }
}
