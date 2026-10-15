//! Visible State, Accumulated State, engine, detectors and renderer:
//! everything behind the Recording replay seam that does no IO.

mod advisor;
mod game_source;
mod view_model;

pub use advisor::Advisor;
pub use game_source::{CrawlVersion, GameIdentity, Role, SourceEvent, VisibleState};
pub use view_model::{Character, Connection, ViewModel};
