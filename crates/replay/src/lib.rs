//! Recording format, in-process replayer and fake-extension front end.

mod fake_extension;
mod recording;
mod replayer;

pub use fake_extension::{
    FAKE_EXTENSION_ORIGIN, FakeExtension, FakeExtensionError, Speed, app_url,
};
pub use recording::{
    RECORDING_EXTENSION, Recording, RecordingError, RecordingHeader, RecordingWriter,
};
pub use replayer::{Applied, Replayer};

use std::io::Read;

use copilot_core::{GameIdentity, ViewModel, VisibleState};

/// What the core made of a replayed Recording.
#[derive(Debug, Clone)]
pub struct Replay {
    /// Every distinct view model the core would have pushed to the panel, in order.
    pub view_models: Vec<ViewModel>,
    /// The followed game's identity at the end.
    pub identity: Option<GameIdentity>,
    /// The followed game's Visible State at the end.
    pub visible: VisibleState,
    /// The Recording was cut short, so this is only what was saved before it ended.
    pub truncated: bool,
}

/// Replays a whole Recording in process, one envelope at a time.
///
/// # Errors
/// If the Recording can't be read.
pub fn replay(recording: impl Read) -> Result<Replay, RecordingError> {
    let mut replayer = Replayer::new(Recording::read(recording)?);
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
        visible: replayer.advisor().visible().clone(),
        truncated: replayer.recording().truncated,
    })
}
