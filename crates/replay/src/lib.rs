//! Recording format, in-process replayer and fake-extension front end.

mod recording;

pub use recording::{
    RECORDING_EXTENSION, Recording, RecordingError, RecordingHeader, RecordingWriter,
};

use std::io::Read;

use copilot_core::{Advisor, Connection, GameIdentity, ViewModel, VisibleState};
use copilot_webtiles::WebtilesSource;

/// What the core made of a replayed Recording.
#[derive(Debug, Clone)]
pub struct Replay {
    /// Every distinct view model the core would have pushed to the panel, in order.
    pub view_models: Vec<ViewModel>,
    /// The followed game's identity at the end.
    pub identity: Option<GameIdentity>,
    /// The followed game's Visible State at the end.
    pub visible: VisibleState,
}

/// Feeds a Recording through the webtiles Game Source and the core, in process, as if a browser
/// had relayed it live.
///
/// # Errors
/// If the Recording can't be read.
pub fn replay(recording: impl Read) -> Result<Replay, RecordingError> {
    let recording = Recording::read(recording)?;
    let connection = Connection::Connected {
        buffer_overflowed: false,
    };
    let mut source = WebtilesSource::new();
    let mut advisor = Advisor::new();
    let mut view_models: Vec<ViewModel> = Vec::new();
    for envelope in &recording.envelopes {
        for event in source.ingest(envelope) {
            advisor.apply(event);
        }
        let view = advisor.view_model(&recording.header.app_version, connection.clone());
        if view_models.last() != Some(&view) {
            view_models.push(view);
        }
    }
    Ok(Replay {
        view_models,
        identity: advisor.identity().cloned(),
        visible: advisor.visible().clone(),
    })
}
