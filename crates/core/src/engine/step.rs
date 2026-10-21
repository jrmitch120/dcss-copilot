//! Crawl's rules for where a monster steps, ported closely (ADR 0002) behind the formula variants
//! a Game Knowledge bundle names.
//!
//! Whether a monster the player watches steps toward them: `_fungal_move_check`, which holds back
//! the kinds its Game Knowledge gives a [`WatchedStep`] other than `Always`. A monster attacks in
//! melee by stepping into the player, so one held back beside them doesn't attack either. Every
//! monster the engine plays is in view of the player, so watched.
//!
//! Where a monster that keeps its range steps: to a firing position at a distance from the player
//! it holds to, never into them.

use copilot_gk::{FiringPosition, MonsterStep, WatchedStep};

use super::PLAYER_VISION;
use super::rng::Rng;

/// Whether a monster that steps as `watched` does, `distance` squares from the player, steps
/// toward them this action: `_fungal_move_check` (`mon-act.cc:483`), rolled each time it would
/// step, with the player's sight taken as [`PLAYER_VISION`], the widest most players have, so it
/// steps as often as it can for them.
pub(crate) fn steps(
    variant: MonsterStep,
    rng: &mut Rng,
    watched: WatchedStep,
    distance: i32,
) -> bool {
    farthest_step(variant, watched, || rng.random2(PLAYER_VISION + 1))
        .is_none_or(|farthest| distance <= farthest)
}

/// Whether some roll of [`steps`] lets a monster that steps as `watched` step toward the player
/// from `distance` squares.
pub(crate) fn may_step(variant: MonsterStep, watched: WatchedStep, distance: i32) -> bool {
    farthest_step(variant, watched, || PLAYER_VISION).is_none_or(|farthest| distance <= farthest)
}

/// Whether a monster that steps as `watched` may walk at all while the player watches it, other
/// than into them: whether some roll of [`steps`] lets it step from farther than beside them.
pub(crate) fn walks_while_watched(variant: MonsterStep, watched: WatchedStep) -> bool {
    farthest_step(variant, watched, || PLAYER_VISION).is_none_or(|farthest| farthest > 1)
}

/// The farthest from the player a monster that steps as `watched` steps toward them this action,
/// `roll` giving the farthest one that steps by chance does: `None` when nothing holds it back.
fn farthest_step(
    variant: MonsterStep,
    watched: WatchedStep,
    roll: impl FnOnce() -> i32,
) -> Option<i32> {
    match variant {
        MonsterStep::Crawl034 => match watched {
            WatchedStep::Always => None,
            WatchedStep::FromBeside => Some(1),
            WatchedStep::ByChance => Some(roll()),
        },
    }
}

/// The distance from the player a monster that keeps its range holds to, when the farthest it
/// can harm them from is `threat_range` squares (`monster::threat_range`): crawl's ideal range in
/// `_set_firing_pos` (`mon-behv.cc:113`), that, but no more than half of `LOS_DEFAULT_RANGE`,
/// which [`PLAYER_VISION`] is.
pub(crate) fn firing_range(variant: FiringPosition, threat_range: i32) -> i32 {
    match variant {
        FiringPosition::Crawl034 => threat_range.min(PLAYER_VISION / 2),
    }
}

/// The distance from the player a monster that keeps its range at `range` squares steps to from
/// `distance` squares, when it steps.
///
/// Crawl looks for its firing position among the squares around it, never its own
/// (`_set_firing_pos`, `mon-behv.cc:113`), so it always steps: to one as near `range` as it can,
/// but no farther from the player than it is unless that is still within `range`. From farther
/// it closes in a square; at `range` it steps around the player; from nearer, even from beside
/// them, it backs off a square. It never steps into the player, so it fights them in melee
/// only with a reaching attack, which it makes before it moves (`_handle_reaching`).
///
/// Played as on open floor (`NotModelled::Paths`): crawl takes only squares it can stand on and
/// see the player from, and one hemmed in, with no such square around it, may step toward a
/// farther one and into the player to fight them.
pub(crate) fn toward_firing_range(variant: FiringPosition, distance: i32, range: i32) -> i32 {
    match variant {
        FiringPosition::Crawl034 => distance + (range - distance).signum(),
    }
}
