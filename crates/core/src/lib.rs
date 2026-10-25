//! Visible State, Accumulated State, engine, detectors and renderer:
//! everything behind the Recording replay seam that does no IO.

mod accumulated_state;
mod advisor;
mod carried_gear;
mod crawl_ids;
mod dangerous_status;
mod detectors;
mod engine;
mod exceptional;
mod fact_sheet;
mod fight_outcome;
mod game_source;
mod known_games;
mod screen;
mod suggested_action;
mod view_model;
mod visible_state;
mod ways_out;

pub use accumulated_state::{
    AccumulatedState, AsOf, GearSource, MutationsRead, Remembered, Shield,
};
pub use advisor::Advisor;
pub use carried_gear::{CarriedGear, CarriedItem, GearSlot, ItemBrand, Unlisted, Unplayed};
pub use dangerous_status::DangerousStatus;
pub use detectors::{Alert, AlertCause, AlertKind, Detection};
pub use engine::oracle;
pub use exceptional::{Exceptional, ExceptionalMonster};
pub use fact_sheet::{
    ActingMonster, Chance, FactSheet, Gap, GearBasis, GearPlayed, KnownOrStale, NotModelled,
    Threat, ThreatAssessment, ThreatLevel,
};
pub use fight_outcome::{AttackPlan, Fight, FightOutcome, PlanBase, PlanOpening, PlanOutcome};
pub use game_source::{CrawlVersion, GameEnd, GameIdentity, Role, SessionEvent, SourceEvent};
pub use known_games::{CharacterKey, GameKey, Identification, KnownGame, KnownGames};
pub use screen::{
    KnownAbility, KnownSpell, Resistance, Resistances, Screen, ScreenRead, ShownMutations, Skills,
    SpellPower, SpellReach,
};
pub use suggested_action::SuggestionRule;
pub use view_model::{
    Appearance, AskBox, AssistanceLevel, Character, Colour, Connection, Depiction, FactCard,
    FocusCard, GameView, Glyph, Listing, NowPill, PickedUp, RunPlanRail, SuggestedActionCard,
    TileName, ToolbarBadge, ViewModel, WayOutRow, WaysOutList,
};
pub use visible_state::{
    Attitude, CrawlThreat, GameMessage, InventoryItem, LevelMap, MapCell, Place, Position,
    RECENT_MESSAGES, Religion, Sighting, SquareLight, Stats, Status, VisibleMonster, VisibleState,
    Wielding,
};
pub use ways_out::{
    Blocker, CorridorSquare, Escape, Ground, InTheWay, Interceptor, ReadOn, Usable, WayOut, WaysOut,
};
