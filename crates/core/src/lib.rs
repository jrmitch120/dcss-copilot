//! Visible State, Accumulated State, engine, detectors and renderer:
//! everything behind the Recording replay seam that does no IO.

mod accumulated_state;
mod advisor;
mod carried_gear;
mod crawl_ids;
mod engine;
mod fact_sheet;
mod fight_outcome;
mod game_source;
mod screen;
mod view_model;
mod visible_state;
mod ways_out;

pub use accumulated_state::{
    AccumulatedState, AsOf, GearSource, MutationsRead, Remembered, Shield,
};
pub use advisor::Advisor;
pub use carried_gear::{CarriedGear, CarriedItem, GearSlot, ItemBrand, Unlisted, Unplayed};
pub use engine::oracle;
pub use fact_sheet::{
    ActingMonster, Chance, FactSheet, Gap, GearBasis, GearPlayed, KnownOrStale, NotModelled,
    Threat, ThreatAssessment, ThreatLevel,
};
pub use fight_outcome::{AttackPlan, Fight, FightOutcome, PlanBase, PlanOpening, PlanOutcome};
pub use game_source::{CrawlVersion, GameEnd, GameIdentity, Role, SessionEvent, SourceEvent};
pub use screen::{
    KnownAbility, KnownSpell, Resistance, Resistances, Screen, ScreenRead, ShownMutation,
    ShownMutations, Skills, SpellPower, SpellReach,
};
pub use view_model::{
    Appearance, AskBox, Character, Colour, Connection, Depiction, FocusCard, GameView, Glyph,
    Listing, NowPill, RunPlanRail, TileName, ViewModel, WayOutRow, WaysOutList,
};
pub use visible_state::{
    Attitude, CrawlThreat, GameMessage, InventoryItem, LevelMap, MapCell, Place, Position,
    RECENT_MESSAGES, Religion, Sighting, SquareLight, Stats, Status, VisibleMonster, VisibleState,
    Wielding,
};
pub use ways_out::{
    Blocker, CorridorSquare, Escape, Ground, InTheWay, Interceptor, ReadOn, Usable, WayOut, WaysOut,
};
