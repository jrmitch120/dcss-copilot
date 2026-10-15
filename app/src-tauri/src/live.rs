//! The live game: what the relay has delivered, turned into the panel's view model.

use std::fs::File;
use std::io::BufWriter;
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard, PoisonError};

use copilot_core::{Advisor, Connection, ViewModel};
use copilot_replay::{RECORDING_EXTENSION, RecordingWriter};
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
    recording: Recording,
}

/// How often, in game-page milliseconds, a Recording is flushed, so a crash loses at most this much.
const RECORDING_FLUSH_MS: u64 = 1_000;

/// Dev builds save every envelope they receive, so a session can be replayed.
enum Recording {
    Off,
    /// Recording is on; the file is created when the first envelope arrives.
    Pending(PathBuf),
    Writing {
        writer: RecordingWriter<BufWriter<File>>,
        /// The envelope timestamp of the last flush.
        flushed_at: u64,
    },
}

impl Live {
    /// Starts with no browser connected, writing a Recording into `recordings` if given.
    pub fn new(app_version: &'static str, recordings: Option<PathBuf>) -> Self {
        let recording = recordings.map_or(Recording::Off, Recording::Pending);
        Self {
            app_version,
            state: Mutex::new(State {
                source: WebtilesSource::new(),
                advisor: Advisor::new(),
                browsers: 0,
                approval: None,
                buffer_overflowed: false,
                relay_unavailable: None,
                recording,
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
            // A newer request from the same or another browser replaces an unanswered one, whose
            // connection then closes and retries.
            RelayEvent::ApprovalNeeded(request) => state.approval = Some(request),
            RelayEvent::Connected => state.browsers += 1,
            RelayEvent::Disconnected => {
                state.browsers = state.browsers.saturating_sub(1);
                if state.browsers == 0 {
                    state.buffer_overflowed = false;
                }
            }
            RelayEvent::Overflow { .. } => state.buffer_overflowed = true,
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
        let recording = std::mem::replace(&mut self.lock().recording, Recording::Off);
        if let Recording::Writing { writer, .. } = recording
            && let Err(error) = writer.finish()
        {
            eprintln!("recording: could not finish: {error}");
        }
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        // Each update leaves the state whole, so a panic elsewhere can't leave it half-changed.
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl State {
    fn ingest(&mut self, envelope: &Envelope) {
        self.record(envelope);
        for event in self.source.ingest(envelope) {
            self.advisor.apply(event);
        }
    }

    fn record(&mut self, envelope: &Envelope) {
        if let Recording::Pending(dir) = &self.recording {
            let path = dir.join(format!("{}.{RECORDING_EXTENSION}", envelope.ts));
            self.recording = match start_recording(dir, &path) {
                Ok(writer) => Recording::Writing {
                    writer,
                    flushed_at: envelope.ts,
                },
                Err(error) => {
                    eprintln!("recording: could not start {}: {error}", path.display());
                    Recording::Off
                }
            };
        }
        if let Recording::Writing { writer, flushed_at } = &mut self.recording {
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
                self.recording = Recording::Off;
            }
        }
    }
}

fn start_recording(
    dir: &std::path::Path,
    path: &std::path::Path,
) -> Result<RecordingWriter<BufWriter<File>>, Box<dyn std::error::Error>> {
    std::fs::create_dir_all(dir)?;
    let file = BufWriter::new(File::create(path)?);
    Ok(RecordingWriter::new(file, env!("CARGO_PKG_VERSION"))?)
}
