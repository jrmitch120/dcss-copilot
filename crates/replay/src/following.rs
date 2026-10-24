//! A Game Source's games, followed by the Advisor: the wiring the app runs live and the replayer
//! replays, so a replay sees a game as the app does.

use copilot_core::{Advisor, Detection, SourceEvent};
use copilot_gk::Library;
use copilot_webtiles::{Envelope, WebtilesSource};
use serde::Serialize;

/// The webtiles Game Source and the Advisor it feeds, always built together from the same Game
/// Knowledge.
#[derive(Debug)]
pub struct Following {
    source: WebtilesSource,
    advisor: Advisor,
}

/// Something that happened, rather than what the game now shows: a screen read or a session
/// event the Game Source reported, or what the Advisor's detectors noticed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(untagged)]
pub enum Happened {
    /// The Game Source reported a screen read or a session event.
    Source(SourceEvent),
    /// The Advisor's detectors noticed something at a Settled Update.
    Detected(Detection),
}

impl Following {
    /// Nothing followed yet, decoding and judging games through `knowledge`.
    #[must_use]
    pub fn new(knowledge: &Library) -> Self {
        Self {
            source: WebtilesSource::new(knowledge.clone()),
            advisor: Advisor::new(knowledge.clone()),
        }
    }

    /// Decodes one relayed envelope, tells the Advisor what the Game Source learned, and returns
    /// what happened, in order.
    pub fn ingest(&mut self, envelope: &Envelope) -> Vec<Happened> {
        let mut happened = Vec::new();
        for event in self.source.ingest(envelope) {
            let detections = self.advisor.apply(event.clone());
            if matches!(event, SourceEvent::ScreenRead(_) | SourceEvent::Session(_)) {
                happened.push(Happened::Source(event));
            }
            happened.extend(detections.into_iter().map(Happened::Detected));
        }
        happened
    }

    /// The Advisor, as the envelopes taken in so far left it.
    #[must_use]
    pub fn advisor(&self) -> &Advisor {
        &self.advisor
    }
}
