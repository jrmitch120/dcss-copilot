//! Crawl's rule for whether a monster the player watches steps toward them, ported closely (ADR
//! 0002) behind the formula variant a Game Knowledge bundle names: `_fungal_move_check`, which
//! holds back the kinds its Game Knowledge gives a [`WatchedStep`] other than `Always`. A monster
//! attacks in melee by stepping into the player, so one held back beside them doesn't attack
//! either. Every monster the engine plays is in view of the player, so watched.

use copilot_gk::{MonsterStep, WatchedStep};

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
