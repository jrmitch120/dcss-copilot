//! The engine: crawl's formulas, ported, and what the Advisor computes with them.

pub(crate) mod brands;
pub(crate) mod clouds;
pub(crate) mod fight;
pub(crate) mod lighting;
pub(crate) mod melee;
pub(crate) mod missiles;
pub mod oracle;
pub(crate) mod paths;
pub(crate) mod player_attacks;
pub(crate) mod player_move;
pub(crate) mod resists;
mod rng;
pub(crate) mod spells;
pub(crate) mod step;
pub(crate) mod threat;
pub(crate) mod torment;
pub(crate) mod wands;

/// How far the player sees, crawl's `you.current_vision`, taken as everyone's normal sight,
/// `LOS_DEFAULT_RANGE` (`defines.h:115`). The Daystalker mutation widens it by one, and the
/// Nightstalker mutation, a scarf of shadows and a meteoran's bezotting narrow it
/// (`update_vision_range`, `player.cc:891`), which the engine doesn't follow.
pub(crate) const PLAYER_VISION: i32 = 7;
