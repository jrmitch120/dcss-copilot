//! The live game: what the relay has delivered, turned into the panel's view model.

use std::fs::File;
use std::io::BufWriter;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, PoisonError};

use copilot_core::{Advisor, Connection, ViewModel};
use copilot_replay::{RECORDING_EXTENSION, RecordingError, RecordingWriter};
use copilot_webtiles::{Envelope, WebtilesSource};

use crate::relay::{ApprovalRequest, RelayEvent};

/// Everything the app knows about the browser connection and the game it relays.
pub struct Live {
    app_version: &'static str,
    state: Mutex<State>,
}

struct State {
    source: WebtilesSource,
    advisor: Advisor,
    /// Allowed browsers currently connected.
    browsers: usize,
    approval: Option<ApprovalRequest>,
    buffer_overflowed: bool,
    relay_unavailable: Option<String>,
    recorder: Recorder,
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
        writer: RecordingWriter<BufWriter<File>>,
        /// The envelope timestamp of the last flush.
        flushed_at: u64,
    },
}

impl Live {
    /// Starts with no browser connected, writing a Recording into `recordings` if given.
    pub fn new(app_version: &'static str, recordings: Option<PathBuf>) -> Self {
        let recorder = recordings.map_or(Recorder::Off, Recorder::Idle);
        Self {
            app_version,
            state: Mutex::new(State {
                source: WebtilesSource::new(),
                advisor: Advisor::new(),
                browsers: 0,
                approval: None,
                buffer_overflowed: false,
                relay_unavailable: None,
                recorder,
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
        state.advisor.view_model(self.app_version, connection)
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
                    state.source = WebtilesSource::new();
                    state.advisor = Advisor::new();
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
                    writer,
                    flushed_at: envelope.ts,
                },
                Err(error) => {
                    eprintln!("recording: could not start {}: {error}", path.display());
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
                eprintln!("recording: stopped, could not write: {error}");
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
            eprintln!("recording: could not finish: {error}");
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

    use super::*;

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
    fn a_browser_connecting_when_none_is_starts_afresh_because_its_tabs_replay_from_the_start() {
        let live = Live::new("1.2.3", None);
        live.handle(RelayEvent::Connected);
        live.handle(envelope(json!({ "msg": "game_started" })));
        live.handle(envelope(
            json!({ "msg": "player", "name": "Mira", "species": "Minotaur", "turn": 500 }),
        ));
        assert!(live.view_model().character.is_some());

        live.handle(RelayEvent::Disconnected);
        live.handle(RelayEvent::Connected);

        assert_eq!(live.view_model().character, None);
    }
}
