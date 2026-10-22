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
//!
//! Where a batty monster steps: after it attacks the player in melee or casts a spell that takes
//! time, it flits (`_handle_battiness`, `mon-act.cc:474`) for `div_rand_round(speed, 10)` of its
//! actions but at least two, counting that one ([`flight_ends`]), then hunts the player again.
//! Flitting, it wanders (`mons_is_wandering`), so it casts, zaps and shoots nothing
//! (`_mons_take_special_action`, `mon-act.cc:1833`): each action it steps toward a square picked
//! at random ([`flit`]), and attacks the player only when that step is into them, which starts
//! its flight afresh. A shard shrike beside the player, at speed 30, bites in the first of its
//! three actions before their next, then flits for the other two, biting again about one time in
//! six.
//!
//! Where a monster stands is an [`Offset`] from the player. Any monster walks straight at the
//! player or away, a square at a time, which changes its distance by one; only a batty monster's
//! steps are drawn as crawl's are, as only its flight depends on the square it stands on.

use std::cmp::Ordering;
use std::ops::{Add, Neg};

use copilot_gk::{BattyFlight, FiringPosition, MonsterStep, WatchedStep};

use super::PLAYER_VISION;
use super::rng::Rng;
use crate::visible_state::Position;

/// Where a monster stands from the player: squares east of them, and south, as crawl's
/// coordinates run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Offset {
    /// Squares east of the player, or west when negative.
    pub(crate) x: i32,
    /// Squares south of the player, or north when negative.
    pub(crate) y: i32,
}

impl Offset {
    /// The player's own square.
    pub(crate) const PLAYER: Self = Self { x: 0, y: 0 };

    /// Where a monster at `monster` stands from the player at `player`.
    pub(crate) fn between(player: Position, monster: Position) -> Self {
        Self {
            x: monster.x - player.x,
            y: monster.y - player.y,
        }
    }

    /// `distance` squares due east of the player.
    pub(crate) fn east(distance: i32) -> Self {
        Self { x: distance, y: 0 }
    }

    /// Squares between it and the player, counting diagonals as one: crawl's `grid_distance`.
    pub(crate) fn distance(self) -> i32 {
        self.x.abs().max(self.y.abs())
    }

    /// A square straight toward the player, one nearer them.
    pub(crate) fn nearer(self) -> Self {
        Self {
            x: self.x - self.x.signum(),
            y: self.y - self.y.signum(),
        }
    }

    /// A square straight away from the player, one farther from them.
    pub(crate) fn farther(self) -> Self {
        Self {
            x: self.x + self.x.signum(),
            y: self.y + self.y.signum(),
        }
    }

    /// The square beside the player on its side of them: as a step, a square straight toward
    /// where it stands.
    pub(crate) fn beside(self) -> Self {
        Self {
            x: self.x.signum(),
            y: self.y.signum(),
        }
    }
}

impl Add for Offset {
    type Output = Self;

    fn add(self, step: Self) -> Self {
        Self {
            x: self.x + step.x,
            y: self.y + step.y,
        }
    }
}

impl Neg for Offset {
    type Output = Self;

    fn neg(self) -> Self {
        Self {
            x: -self.x,
            y: -self.y,
        }
    }
}

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

/// Where a monster that keeps its range at `range` squares steps to from `from`, when it steps.
///
/// Crawl looks for its firing position among the squares around it, never its own
/// (`_set_firing_pos`, `mon-behv.cc:113`), so it always steps: to one as near `range` as it can,
/// but no farther from the player than it is unless that is still within `range`. From farther
/// it closes in a square; at `range` it steps around the player, played as staying where it is,
/// which keeps its distance; from nearer, even from beside them, it backs off a square. It never
/// steps into the player, so it fights them in melee only with a reaching attack, which it makes
/// before it moves (`_handle_reaching`).
///
/// Played as on open floor (`NotModelled::Paths`): crawl takes only squares it can stand on and
/// see the player from, and one hemmed in, with no such square around it, may step toward a
/// farther one and into the player to fight them.
pub(crate) fn toward_firing_range(variant: FiringPosition, from: Offset, range: i32) -> Offset {
    match variant {
        FiringPosition::Crawl034 => match from.distance().cmp(&range) {
            Ordering::Greater => from.nearer(),
            Ordering::Less => from.farther(),
            Ordering::Equal => from,
        },
    }
}

/// How far a flitting batty monster looks for a square to flit toward, each way:
/// `set_random_target` (`mon-movetarget.cc:697`).
const FLIGHT_REACH: i32 = 6;

/// The farthest off a square a monster heads straight for, rather than sometimes along the line
/// it is farthest off along alone: `_find_best_step` (`mon-act.cc:532`).
const STRAIGHT_FOR: i32 = 3;

/// The step a batty monster takes as it flits: toward a square up to [`FLIGHT_REACH`] off from
/// it each way, any but its own as likely as another (`set_random_target`, `mon-movetarget.cc:697`,
/// which picks it afresh every action, `check_wander_target`), as [`step_toward`] steps. A step
/// into the player is an attack on them (`handle_monster_move`, `mon-act.cc:2306`).
///
/// Played as on open floor (`NotModelled::Paths`), where crawl may pick any such square: it
/// picks only one it can stand on, and steps round what stands in its way.
pub(crate) fn flit(variant: BattyFlight, rng: &mut Rng) -> Offset {
    match variant {
        BattyFlight::Crawl034 => {
            let toward = loop {
                let toward = Offset {
                    x: rng.random2(2 * FLIGHT_REACH + 1) - FLIGHT_REACH,
                    y: rng.random2(2 * FLIGHT_REACH + 1) - FLIGHT_REACH,
                };
                if toward != Offset::PLAYER {
                    break toward;
                }
            };
            step_toward(variant, rng, toward)
        }
    }
}

/// The step a batty monster takes toward a square `toward` off from it: `_find_best_step`
/// (`mon-act.cc:501`) on open floor. It heads straight for it, but from more than
/// [`STRAIGHT_FOR`] squares off, half the time along the line it is farthest off along alone.
pub(crate) fn step_toward(variant: BattyFlight, rng: &mut Rng, toward: Offset) -> Offset {
    match variant {
        BattyFlight::Crawl034 => {
            let mut step = toward.beside();
            if toward.distance() > STRAIGHT_FOR {
                if toward.x.abs() > toward.y.abs() && rng.one_chance_in(2) {
                    step.y = 0;
                }
                if toward.y.abs() > toward.x.abs() && rng.one_chance_in(2) {
                    step.x = 0;
                }
            }
            step
        }
    }
}

/// Whether a batty monster of `speed` stops flitting after the `actions`th of its actions since
/// it last attacked or cast, counting that one: `_post_monster_move` (`mon-act.cc:2652`) rolls
/// how long it flits after each, `div_rand_round(speed, 10)` actions but at least two, then has it
/// hunt the player again.
pub(crate) fn flight_ends(variant: BattyFlight, rng: &mut Rng, speed: i32, actions: i32) -> bool {
    match variant {
        BattyFlight::Crawl034 => actions >= rng.div_rand_round(speed, 10).max(2),
    }
}
