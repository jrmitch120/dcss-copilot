//! The LLM sidecar: every call a dev build makes, recorded beside its Recording, so tests and CI
//! replay the same answers without calling a live model.
//!
//! A sidecar (`*.llm.jsonl`) is JSON Lines, one call per line: the request's hash, the request
//! itself for reading in a diff, everything the call streamed and, if it failed, the error.
//! Replays look answers up by the hash, so any change to a prompt shows up as a call that was
//! never recorded rather than as a stale answer.

use std::collections::{HashMap, VecDeque};
use std::fmt;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};

use futures_util::StreamExt;
use futures_util::stream;
use serde::{Deserialize, Serialize};

use crate::provider::{LlmError, LlmProvider, LlmStream};
use crate::request::LlmRequest;
use crate::response::StreamEvent;

/// The file extension of an LLM sidecar, which sits beside the Recording it belongs to.
pub const SIDECAR_EXTENSION: &str = "llm.jsonl";

/// The sidecar that belongs beside the Recording at `recording`: `game.dcssrec` has
/// `game.llm.jsonl`.
#[must_use]
pub fn sidecar_path(recording: &Path) -> PathBuf {
    recording.with_extension(SIDECAR_EXTENSION)
}

/// A request's identity in a sidecar: a stable hash of everything in it.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RequestHash(String);

impl fmt::Display for RequestHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// The hash a sidecar keys `request`'s answer by. The same request always hashes the same, on
/// every machine and Rust version: it is 64-bit FNV-1a over the request's JSON.
#[must_use]
pub fn request_hash(request: &LlmRequest) -> RequestHash {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    // A request is plain data, so it always serializes; an empty hash would never match.
    let json = serde_json::to_vec(request).unwrap_or_default();
    let hash = json.iter().fold(OFFSET, |hash, &byte| {
        (hash ^ u64::from(byte)).wrapping_mul(PRIME)
    });
    RequestHash(format!("{hash:016x}"))
}

/// One recorded call: one line of a sidecar.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Recorded {
    hash: RequestHash,
    request: LlmRequest,
    events: Vec<StreamEvent>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    error: Option<LlmError>,
}

/// Why a sidecar couldn't be read.
#[derive(Debug, thiserror::Error)]
pub enum SidecarError {
    /// The file couldn't be read.
    #[error("the LLM sidecar can't be read: {0}")]
    Io(#[from] std::io::Error),
    /// A line isn't a recorded call.
    #[error("line {line} of the LLM sidecar isn't a recorded call: {source}")]
    Malformed {
        /// The line, counting from 1.
        line: usize,
        /// What was wrong with it.
        source: serde_json::Error,
    },
    /// A line's request was changed after it was recorded, so its answer can't be trusted.
    #[error("line {line} of the LLM sidecar was edited: its request no longer matches its hash")]
    HashMismatch {
        /// The line, counting from 1.
        line: usize,
    },
}

/// Passes calls on to a live provider and appends each one to a sidecar as it ends.
pub struct SidecarRecorder<P> {
    inner: P,
    file: Arc<Mutex<File>>,
}

impl<P: LlmProvider> SidecarRecorder<P> {
    /// Records `inner`'s calls into the sidecar at `path`, adding to any calls already there.
    ///
    /// # Errors
    /// If the file can't be created or opened.
    pub fn create(inner: P, path: &Path) -> Result<Self, std::io::Error> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let file = OpenOptions::new().create(true).append(true).open(path)?;
        Ok(Self {
            inner,
            file: Arc::new(Mutex::new(file)),
        })
    }
}

impl<P: LlmProvider> LlmProvider for SidecarRecorder<P> {
    fn stream(&self, request: LlmRequest) -> LlmStream {
        let call = Recording {
            file: Arc::clone(&self.file),
            recorded: Some(Recorded {
                hash: request_hash(&request),
                request: request.clone(),
                events: Vec::new(),
                error: None,
            }),
        };
        stream::unfold(
            (self.inner.stream(request), call),
            |(mut inner, mut call)| async {
                let item = match inner.next().await {
                    Some(item) => item,
                    // A stream that ends without its answer is recorded as ending incomplete.
                    None if call.recorded.is_some() => Err(LlmError::Incomplete),
                    None => return None,
                };
                let item = match item {
                    Ok(StreamEvent::Finished(response)) => {
                        call.end(Ok(StreamEvent::Finished(response)))
                    }
                    Ok(event) => {
                        call.push(&event);
                        Ok(event)
                    }
                    Err(error) => call.end(Err(error)),
                };
                Some((item, (inner, call)))
            },
        )
        .boxed()
    }
}

/// One call while it is being recorded.
struct Recording {
    file: Arc<Mutex<File>>,
    /// The call so far, until it is written.
    recorded: Option<Recorded>,
}

impl Recording {
    fn push(&mut self, event: &StreamEvent) {
        if let Some(recorded) = &mut self.recorded {
            recorded.events.push(event.clone());
        }
    }

    /// Writes the call with its last item, and passes that item on. A call that can't be
    /// recorded fails, so a dev build never makes calls a replay won't have.
    fn end(&mut self, last: Result<StreamEvent, LlmError>) -> Result<StreamEvent, LlmError> {
        let Some(mut recorded) = self.recorded.take() else {
            return last;
        };
        match &last {
            Ok(event) => recorded.events.push(event.clone()),
            Err(error) => recorded.error = Some(error.clone()),
        }
        let written = serde_json::to_string(&recorded)
            .map_err(std::io::Error::other)
            .and_then(|line| {
                let mut file = self.file.lock().unwrap_or_else(PoisonError::into_inner);
                writeln!(file, "{line}")?;
                file.flush()
            });
        match written {
            Ok(()) => last,
            Err(error) => Err(LlmError::Sidecar(error.to_string())),
        }
    }
}

/// Answers calls from a sidecar, never from a live model.
pub struct SidecarReplayer {
    /// Each request's recorded calls, in the order they were made.
    answers: Mutex<HashMap<RequestHash, VecDeque<Recorded>>>,
}

impl SidecarReplayer {
    /// Reads the sidecar at `path`.
    ///
    /// # Errors
    /// If it can't be read or a line isn't a recorded call.
    pub fn open(path: &Path) -> Result<Self, SidecarError> {
        Self::read(&std::fs::read(path)?)
    }

    /// Reads a sidecar's contents.
    ///
    /// # Errors
    /// If a line isn't a recorded call.
    pub fn read(sidecar: &[u8]) -> Result<Self, SidecarError> {
        let mut answers: HashMap<RequestHash, VecDeque<Recorded>> = HashMap::new();
        for (index, line) in sidecar.lines().enumerate() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            let recorded: Recorded =
                serde_json::from_str(&line).map_err(|source| SidecarError::Malformed {
                    line: index + 1,
                    source,
                })?;
            if request_hash(&recorded.request) != recorded.hash {
                return Err(SidecarError::HashMismatch { line: index + 1 });
            }
            answers
                .entry(recorded.hash.clone())
                .or_default()
                .push_back(recorded);
        }
        Ok(Self {
            answers: Mutex::new(answers),
        })
    }

    /// The next recorded call for `hash`. A request asked more often than it was recorded gets
    /// its last answer again.
    fn next(&self, hash: &RequestHash) -> Option<Recorded> {
        let mut answers = self.answers.lock().unwrap_or_else(PoisonError::into_inner);
        let calls = answers.get_mut(hash)?;
        if calls.len() > 1 {
            calls.pop_front()
        } else {
            calls.front().cloned()
        }
    }
}

impl LlmProvider for SidecarReplayer {
    fn stream(&self, request: LlmRequest) -> LlmStream {
        let hash = request_hash(&request);
        let items: Vec<Result<StreamEvent, LlmError>> = match self.next(&hash) {
            Some(recorded) => recorded
                .events
                .into_iter()
                .map(Ok)
                .chain(recorded.error.map(Err))
                .collect(),
            None => vec![Err(LlmError::NotRecorded {
                hash: hash.to_string(),
            })],
        };
        stream::iter(items).boxed()
    }
}
