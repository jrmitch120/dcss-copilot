//! Visible State, Accumulated State, engine, detectors and renderer:
//! everything behind the Recording replay seam that does no IO.

mod accumulated_state;
mod advisor;
mod carried_gear;
mod engine;
mod fact_sheet;
mod game_source;
mod screen;
mod view_model;
mod visible_state;
mod ways_out;

pub use accumulated_state::{AccumulatedState, AsOf, GearSource, Remembered, Shield};
pub use advisor::Advisor;
pub use carried_gear::{CarriedGear, CarriedItem, GearSlot, ItemBrand, Unlisted};
pub use engine::oracle;
pub use fact_sheet::{
    ActingMonster, Chance, FactSheet, Freshness, Gap, GearBasis, GearPlayed, NotModelled, Threat,
    ThreatAssessment, ThreatLevel, Unplayed,
};
pub use game_source::{CrawlVersion, GameEnd, GameIdentity, Role, SessionEvent, SourceEvent};
pub use screen::{KnownAbility, KnownSpell, Resistance, Resistances, Screen, ScreenRead};
pub use view_model::{
    Appearance, AskBox, Character, Colour, Connection, Depiction, FocusCard, GameView, Glyph,
    Listing, NowPill, RunPlanRail, TileName, ViewModel, WayOutRow, WaysOutList,
};
pub use visible_state::{
    Attitude, CrawlThreat, GameMessage, InventoryItem, LevelMap, MapCell, Place, Position,
    RECENT_MESSAGES, Religion, Sighting, SquareLight, Stats, Status, VisibleMonster, VisibleState,
};
pub use ways_out::{
    Blocker, CorridorSquare, Escape, Ground, InTheWay, Interceptor, ReadOn, Usable, WayOut, WaysOut,
};
