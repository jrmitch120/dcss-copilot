//! Recording format, in-process replayer, fake-extension front end, scenarios and the corpus,
//! and the wiring of Game Source and Advisor the app and the replayer share.

mod corpus;
mod fake_extension;
mod following;
mod recording;
mod replayer;
mod scenario;

pub use corpus::{CORPUS_DIR_VAR, Corpus, CorpusError, CorpusGame};
pub use fake_extension::{
    FAKE_EXTENSION_ORIGIN, FakeExtension, FakeExtensionError, Speed, app_url,
};
pub use following::{Following, Happened};
pub use recording::{
    RECORDING_EXTENSION, Recording, RecordingError, RecordingHeader, RecordingWriter,
};
pub use replayer::{Applied, CrawlVersionMismatch, Moment, Replayer};
pub use scenario::{
    ExpectedAlert, ExpectedSuggestedAction, SCENARIO_EXTENSION, Scenario, ScenarioError,
    ScenarioReplay, Setup, Snapshot,
};

use std::io::Read;

use copilot_core::{CrawlVersion, GameIdentity, ViewModel, VisibleState};
use copilot_gk::BundleError;

/// What the core made of a replayed Recording.
#[derive(Debug, Clone)]
pub struct Replay {
    /// Every distinct view model the core would have pushed to the panel, in order.
    pub view_models: Vec<ViewModel>,
    /// The followed game's identity at the end.
    pub identity: Option<GameIdentity>,
    /// Every Crawl Version a followed game reported, in the order first seen.
    pub crawl_versions: Vec<CrawlVersion>,
    /// The followed game's Visible State at the end.
    pub visible: VisibleState,
    /// The screen reads, session events and what the detectors noticed, in order.
    pub timeline: Vec<Moment>,
    /// The Recording was cut short, so this is only what was saved before it ended.
    pub truncated: bool,
}

/// Why a Recording couldn't be replayed.
#[derive(Debug, thiserror::Error)]
pub enum ReplayError {
    /// The Recording couldn't be read.
    #[error(transparent)]
    Recording(#[from] RecordingError),
    /// The Game Knowledge the app ships couldn't be read.
    #[error(transparent)]
    GameKnowledge(#[from] BundleError),
}

/// Replays a whole Recording in process, one envelope at a time.
///
/// # Errors
/// If the Recording or the Game Knowledge the app ships can't be read.
pub fn replay(recording: impl Read) -> Result<Replay, ReplayError> {
    let mut replayer = Replayer::new(Recording::read(recording)?)?;
    let mut view_models: Vec<ViewModel> = Vec::new();
    while !replayer.at_end() {
        replayer.step();
        let view = replayer.view_model();
        if view_models.last() != Some(&view) {
            view_models.push(view);
        }
    }
    Ok(Replay {
        view_models,
        identity: replayer.advisor().identity().cloned(),
        crawl_versions: replayer.crawl_versions().to_vec(),
        visible: replayer.advisor().visible().clone(),
        timeline: replayer.timeline().to_vec(),
        truncated: replayer.recording().truncated,
    })
}
