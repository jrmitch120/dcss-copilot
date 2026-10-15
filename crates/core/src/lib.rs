//! Visible State, Accumulated State, engine, detectors and renderer:
//! everything behind the Recording replay seam that does no IO.

mod advisor;
mod game_source;
mod view_model;
mod visible_state;

pub use advisor::Advisor;
pub use game_source::{CrawlVersion, GameIdentity, Role, SourceEvent};
pub use view_model::{
    Appearance, AskBox, Character, Colour, Connection, Depiction, FocusCard, GameView, Glyph,
    Listing, NowPill, RunPlanRail, TileName, ViewModel,
};
pub use visible_state::{
    Attitude, CrawlThreat, InventoryItem, Place, Position, Religion, Stats, Status, VisibleMonster,
    VisibleState,
};
