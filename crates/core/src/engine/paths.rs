//! Paths over the current level: how many moves the player or a monster needs to reach a square,
//! walking as crawl lets each walk, and how many moves a monster makes while the player makes
//! theirs.

use std::cmp::Reverse;
use std::collections::{BTreeMap, BinaryHeap};

use copilot_gk::{MonsterEnergy, Terrain};

use crate::engine::melee::{ENERGY_THRESHOLD, most_starting_energy};
use crate::visible_state::Position;

/// The moves a closed door costs: one to open it and one to step through (`open_door`,
/// `monster::move_to_pos`).
const DOOR_MOVES: u32 = 2;

/// The eight steps from a square: crawl moves diagonally as freely as straight.
const STEPS: [(i32, i32); 8] = [
    (-1, -1),
    (0, -1),
    (1, -1),
    (-1, 0),
    (1, 0),
    (-1, 1),
    (0, 1),
    (1, 1),
];

/// The current level as walkers see it: the terrain of every square the player has explored.
/// Squares beyond them are unexplored, out to one square past the explored ones.
#[derive(Debug, Clone)]
pub(crate) struct Level {
    /// By `(y, x)`.
    terrain: BTreeMap<(i32, i32), Terrain>,
    /// The squares unexplored ones may be on: the explored ones' bounds, one square wider.
    top_left: Position,
    bottom_right: Position,
}

/// Who is walking, which decides the squares they can step onto.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Walker<'a> {
    /// The player, who walks only over explored squares, around the `occupied` ones where they
    /// can and through them where they can't: who is in the way is told by the path.
    Player {
        /// The squares monsters stand on.
        occupied: &'a [Position],
    },
    /// A monster, which may cross unexplored squares, as if open, and walks past other
    /// monsters.
    Monster {
        /// It opens doors: crawl's `mons_can_open_doors`, for one that uses at least doors.
        opens_doors: bool,
    },
}

/// What a path costs a walker: first the monsters it passes, then its moves.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct Cost {
    /// The squares it steps onto that a monster stands on.
    past: u32,
    /// Its moves.
    moves: u32,
}

impl Level {
    /// The level with this terrain at each explored square.
    pub(crate) fn new(terrain: impl IntoIterator<Item = (Position, Terrain)>) -> Option<Self> {
        let terrain: BTreeMap<(i32, i32), Terrain> = terrain
            .into_iter()
            .map(|(position, terrain)| ((position.y, position.x), terrain))
            .collect();
        let top = terrain.keys().map(|&(y, _)| y).min()?;
        let bottom = terrain.keys().map(|&(y, _)| y).max()?;
        let left = terrain.keys().map(|&(_, x)| x).min()?;
        let right = terrain.keys().map(|&(_, x)| x).max()?;
        Some(Self {
            terrain,
            top_left: Position {
                x: left - 1,
                y: top - 1,
            },
            bottom_right: Position {
                x: right + 1,
                y: bottom + 1,
            },
        })
    }

    /// The terrain at `position`, `None` for an unexplored square.
    pub(crate) fn terrain(&self, position: Position) -> Option<Terrain> {
        self.terrain.get(&(position.y, position.x)).copied()
    }

    /// The squares around `position`, explored or not, within the level's bounds.
    pub(crate) fn around(&self, position: Position) -> impl Iterator<Item = Position> + '_ {
        STEPS
            .iter()
            .map(move |&(dx, dy)| Position {
                x: position.x + dx,
                y: position.y + dy,
            })
            .filter(|next| self.in_bounds(*next))
    }

    fn in_bounds(&self, position: Position) -> bool {
        (self.top_left.x..=self.bottom_right.x).contains(&position.x)
            && (self.top_left.y..=self.bottom_right.y).contains(&position.y)
    }

    /// What stepping onto `position` costs `walker`, `None` when it can't.
    fn cost_onto(&self, walker: Walker<'_>, position: Position) -> Option<Cost> {
        let (explored_only, opens_doors, past) = match walker {
            Walker::Player { occupied } => (true, true, u32::from(occupied.contains(&position))),
            Walker::Monster { opens_doors } => (false, opens_doors, 0),
        };
        let moves = match self.terrain(position) {
            None if explored_only => return None,
            None | Some(Terrain::Open) => 1,
            Some(Terrain::ClosedDoor) if opens_doors => DOOR_MOVES,
            Some(Terrain::ClosedDoor | Terrain::DeepWater | Terrain::Lava | Terrain::Solid) => {
                return None;
            }
        };
        Some(Cost { past, moves })
    }

    /// The cheapest path `walker` has from `from` to each square it can reach: past the fewest
    /// monsters, then in the fewest moves.
    pub(crate) fn moves_from(&self, walker: Walker<'_>, from: Position) -> Moves {
        let start = Cost { past: 0, moves: 0 };
        let mut best: BTreeMap<(i32, i32), (Cost, Position)> = BTreeMap::new();
        let mut frontier = BinaryHeap::new();
        best.insert((from.y, from.x), (start, from));
        frontier.push(Reverse((start, from.y, from.x)));
        while let Some(Reverse((so_far, y, x))) = frontier.pop() {
            if best.get(&(y, x)).is_some_and(|&(cost, _)| cost < so_far) {
                continue;
            }
            let here = Position { x, y };
            for next in self.around(here) {
                let Some(step) = self.cost_onto(walker, next) else {
                    continue;
                };
                let total = Cost {
                    past: so_far.past + step.past,
                    moves: so_far.moves + step.moves,
                };
                if best
                    .get(&(next.y, next.x))
                    .is_none_or(|&(cost, _)| total < cost)
                {
                    best.insert((next.y, next.x), (total, here));
                    frontier.push(Reverse((total, next.y, next.x)));
                }
            }
        }
        Moves { from, best }
    }
}

/// The cheapest path a walker has from where it starts to each square it can reach.
#[derive(Debug, Clone)]
pub(crate) struct Moves {
    /// Where the walker starts.
    from: Position,
    /// By `(y, x)`: the path's cost, and the square it steps from.
    best: BTreeMap<(i32, i32), (Cost, Position)>,
}

impl Moves {
    /// The moves of the path to `position`, `None` when the walker can't reach it.
    pub(crate) fn to(&self, position: Position) -> Option<u32> {
        self.best
            .get(&(position.y, position.x))
            .map(|&(cost, _)| cost.moves)
    }

    /// The squares the path to `position` steps onto, `position` last: empty for where the
    /// walker starts, or a square it can't reach.
    pub(crate) fn path_to(&self, position: Position) -> Vec<Position> {
        let mut path = Vec::new();
        let mut at = position;
        while at != self.from {
            let Some(&(_, previous)) = self.best.get(&(at.y, at.x)) else {
                return Vec::new();
            };
            path.push(at);
            at = previous;
        }
        path.reverse();
        path
    }

    /// The fewest moves to `position` or a square next to it.
    pub(crate) fn to_beside(&self, position: Position) -> Option<u32> {
        STEPS
            .iter()
            .map(|&(dx, dy)| Position {
                x: position.x + dx,
                y: position.y + dy,
            })
            .chain([position])
            .filter_map(|square| self.to(square))
            .min()
    }

    /// Every square reached, with its moves, top row first, left to right.
    pub(crate) fn reached(&self) -> impl Iterator<Item = (Position, u32)> + '_ {
        self.best
            .iter()
            .map(|(&(y, x), &(cost, _))| (Position { x, y }, cost.moves))
    }
}

/// The most moves a monster of `speed` that spends `walk` energy a move makes while the player
/// makes `actions` standard-speed actions: as `can_attack` reckons it, starting with the most
/// energy it could have and gaining its speed each action (`_monster_add_energy`,
/// `mon-act.cc:1653`).
pub(crate) fn monster_moves(variant: MonsterEnergy, speed: u32, walk: u32, actions: u32) -> u32 {
    let speed = i64::from(speed);
    let walk = i64::from(walk.max(1));
    let mut energy = i64::from(most_starting_energy(variant));
    let mut moves = 0;
    for _ in 0..actions {
        energy += speed;
        while energy >= i64::from(ENERGY_THRESHOLD) {
            energy -= walk;
            moves += 1;
        }
    }
    moves
}

/// The fewest standard-speed player actions in which a monster of `speed` that spends `walk`
/// energy a move can make `needed` moves, as [`monster_moves`] reckons them; `None` if it never
/// can, as one of no speed never moves.
pub(crate) fn actions_to_move(
    variant: MonsterEnergy,
    speed: u32,
    walk: u32,
    needed: u32,
) -> Option<u32> {
    if needed == 0 {
        return Some(0);
    }
    if speed == 0 {
        return None;
    }
    // Each action brings at least one move's energy for a monster as fast as it walks; a slower
    // one needs at most `walk` actions a move.
    let most = needed.saturating_mul(walk.max(1));
    (1..=most).find(|&actions| monster_moves(variant, speed, walk, actions) >= needed)
}

#[cfg(test)]
mod tests {
    use super::*;

    const ENERGY: MonsterEnergy = MonsterEnergy::Crawl034;

    #[test]
    fn a_monster_as_fast_as_the_player_moves_once_each_action() {
        assert_eq!(monster_moves(ENERGY, 10, 10, 1), 1);
        assert_eq!(monster_moves(ENERGY, 10, 10, 7), 7);
    }

    #[test]
    fn a_monster_half_again_as_fast_can_move_twice_on_the_first_action() {
        // From 79 energy it gains 15: 94 acts, 84 acts, 74 waits.
        assert_eq!(monster_moves(ENERGY, 15, 10, 1), 2);
        assert_eq!(monster_moves(ENERGY, 15, 10, 4), 6);
        assert_eq!(actions_to_move(ENERGY, 15, 10, 6), Some(4));
    }

    #[test]
    fn a_half_speed_monster_moves_on_every_other_action_at_best() {
        // From 79 energy it gains 5: 84 acts, 74 waits, 79 waits, 84 acts.
        assert_eq!(monster_moves(ENERGY, 5, 10, 1), 1);
        assert_eq!(monster_moves(ENERGY, 5, 10, 2), 1);
        assert_eq!(monster_moves(ENERGY, 5, 10, 3), 2);
        assert_eq!(actions_to_move(ENERGY, 5, 10, 3), Some(5));
        assert_eq!(actions_to_move(ENERGY, 0, 10, 1), None);
    }
}
