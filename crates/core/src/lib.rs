//! Visible State, Accumulated State, engine, detectors and renderer:
//! everything behind the Recording replay seam that does no IO.

mod accumulated_state;
mod advisor;
mod candidate_action;
mod carried_gear;
mod coach;
mod crawl_ids;
mod dangerous_status;
mod detectors;
mod engine;
mod exceptional;
mod explored;
mod fact_sheet;
mod fight_outcome;
mod game_source;
mod gaps;
mod grounding;
mod join_card;
mod known_games;
mod prompt;
mod screen;
mod suggested_action;
mod told;
mod view_model;
mod visible_state;
mod ways_out;

pub use accumulated_state::{
    AccumulatedState, AsOf, Backfill, GearSource, Memorised, MutationsRead, Remembered, Shield,
    Source,
};
pub use advisor::Advisor;
pub use carried_gear::{CarriedGear, CarriedItem, GearSlot, ItemBrand, Unlisted, Unplayed};
pub use coach::{Coached, Dropped, QuestionId, TacticalAnswer, TacticalQuestion};
pub use dangerous_status::DangerousStatus;
pub use detectors::{Alert, AlertCause, AlertKind, Detection};
pub use engine::oracle;
pub use exceptional::{Exceptional, ExceptionalMonster};
pub use explored::{
    Explored, ItemSeen, ItemStatus, LandmarkKind, LandmarkSeen, LevelRecord, OnLevel, ShopOffer,
    UniqueMet, Visit,
};
pub use fact_sheet::{
    ActingMonster, Chance, FactSheet, GearBasis, GearPlayed, KnownOrStale, NotModelled, Threat,
    ThreatAssessment, ThreatLevel,
};
pub use fight_outcome::{
    AttackPlan, Fight, FightOutcome, NotPlayed, NotPlayedReason, PlanBase, PlanOpening, PlanOutcome,
};
pub use game_source::{CrawlVersion, GameEnd, GameIdentity, Role, SessionEvent, SourceEvent};
pub use gaps::{Gap, ScreenFact};
pub use known_games::{CharacterKey, GameKey, Identification, KnownGame, KnownGames};
pub use prompt::Prompt;
pub use screen::{
    BranchFound, DungeonOverview, KnownAbility, KnownSpell, Resistance, Resistances, Screen,
    ScreenRead, ShopsFound, ShownMutations, Skills, SpellPower, SpellReach,
};
pub use suggested_action::SuggestionRule;
pub use view_model::{
    Appearance, AskBox, AssistanceLevel, Character, ChecklistRow, CoachLine, Colour, Connection,
    Depiction, FactCard, FocusCard, GameView, GapList, GapRow, Glyph, JoinCard, Listing,
    MarkedValue, NowPill, PickedUp, RunPlanRail, SuggestedActionCard, TileName, ToolbarBadge,
    UnreadPart, ViewModel, WayOutRow, WaysOutList,
};
pub use visible_state::{
    Attitude, CrawlThreat, GameMessage, InventoryItem, LevelMap, LevelName, MapCell, MapSnapshot,
    Place, Position, RECENT_MESSAGES, Religion, Sighting, SquareLight, Stats, Status,
    VisibleMonster, VisibleState, Wielding,
};
pub use ways_out::{
    Blocker, CorridorSquare, Escape, Ground, InTheWay, Interceptor, ReadOn, Usable, WayOut, WaysOut,
};
