//! Paths over the current level: how many moves the player or a monster needs to reach a square,
//! walking as crawl lets each walk, and how many moves a monster makes while the player makes
//! theirs.

use std::cmp::Reverse;
use std::collections::{BTreeMap, BinaryHeap};

use copilot_gk::{Habitat, ItemUse, Monster, MonsterEnergy, Terrain};

use crate::engine::melee::{BASELINE_DELAY, ENERGY_THRESHOLD, most_starting_energy};
use crate::visible_state::Position;

/// The moves a closed door costs: one to open it and one to step through (`open_door`,
/// `monster::move_to_pos`).
const DOOR_MOVES: u32 = 2;

/// The time a standard-speed action takes the player: `BASELINE_DELAY`.
const STANDARD_ACTION: u32 = BASELINE_DELAY.unsigned_abs();

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
    /// Every square the bounds hold, row by row: `None` for an unexplored one.
    squares: Vec<Option<Terrain>>,
    /// The squares unexplored ones may be on: the explored ones' bounds, one square wider.
    top_left: Position,
    bottom_right: Position,
}

/// What a monster can cross on its way to the player, of what walls off a walker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Crossing {
    /// It opens closed doors: crawl's `mons_can_open_doors`, for one that uses at least doors.
    pub(crate) opens_doors: bool,
    /// It flies or swims over deep water.
    pub(crate) deep_water: bool,
    /// It flies over or wades through lava.
    pub(crate) lava: bool,
}

impl Crossing {
    /// What a monster of `monster`'s kind crosses.
    pub(crate) fn of(monster: &Monster) -> Self {
        let habitat = monster.habitat;
        Self {
            opens_doors: monster.item_use >= ItemUse::OpenDoors,
            deep_water: monster.flies
                || matches!(
                    habitat,
                    Habitat::Amphibious
                        | Habitat::Water
                        | Habitat::DeepWater
                        | Habitat::EldritchTentacle
                ),
            lava: monster.flies || matches!(habitat, Habitat::Lava | Habitat::AmphibiousLava),
        }
    }
}

/// How many moves a monster that crosses as one [`Crossing`] needs from every square to one it
/// heads for, round what walls it off: other monsters don't count.
#[derive(Debug, Clone)]
pub(crate) struct Field {
    /// As [`Level`]'s squares: `u16::MAX` for a square it can't stand on or that has no way there.
    moves: Vec<u16>,
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
        let terrain: Vec<(Position, Terrain)> = terrain.into_iter().collect();
        let top = terrain.iter().map(|(position, _)| position.y).min()?;
        let bottom = terrain.iter().map(|(position, _)| position.y).max()?;
        let left = terrain.iter().map(|(position, _)| position.x).min()?;
        let right = terrain.iter().map(|(position, _)| position.x).max()?;
        let top_left = Position {
            x: left - 1,
            y: top - 1,
        };
        let bottom_right = Position {
            x: right + 1,
            y: bottom + 1,
        };
        let width = usize::try_from(bottom_right.x - top_left.x + 1).ok()?;
        let height = usize::try_from(bottom_right.y - top_left.y + 1).ok()?;
        let mut level = Self {
            squares: vec![None; width.checked_mul(height)?],
            top_left,
            bottom_right,
        };
        for (position, terrain) in terrain {
            let index = level.index(position)?;
            *level.squares.get_mut(index)? = Some(terrain);
        }
        Some(level)
    }

    /// Where `position` is in the squares, `None` beyond the level's bounds.
    fn index(&self, position: Position) -> Option<usize> {
        if !self.in_bounds(position) {
            return None;
        }
        let width = self.bottom_right.x - self.top_left.x + 1;
        let index = (position.y - self.top_left.y) * width + position.x - self.top_left.x;
        usize::try_from(index).ok()
    }

    /// The terrain at `position`, `None` for an unexplored square.
    pub(crate) fn terrain(&self, position: Position) -> Option<Terrain> {
        self.squares.get(self.index(position)?).copied().flatten()
    }

    /// The moves stepping onto `position` costs a monster that crosses as `crossing`: `None` when
    /// it can't. It crosses an unexplored square as if open.
    pub(crate) fn moves_onto(&self, crossing: Crossing, position: Position) -> Option<u16> {
        self.index(position)?;
        match self.terrain(position) {
            None | Some(Terrain::Open) => Some(1),
            Some(Terrain::ClosedDoor) if crossing.opens_doors => Some(DOOR_MOVES as u16),
            Some(Terrain::DeepWater) if crossing.deep_water => Some(1),
            Some(Terrain::Lava) if crossing.lava => Some(1),
            Some(Terrain::ClosedDoor | Terrain::DeepWater | Terrain::Lava | Terrain::Solid) => None,
        }
    }

    /// The moves from every square to `to` for a monster that crosses as `crossing`: wherever it
    /// heads for, even a square it couldn't stand on itself, such as the player's in deep water.
    pub(crate) fn field_to(&self, crossing: Crossing, to: Position) -> Field {
        let mut moves = vec![u16::MAX; self.squares.len()];
        let Some(start) = self.index(to) else {
            return Field { moves };
        };
        if let Some(square) = moves.get_mut(start) {
            *square = 0;
        }
        // Moves come in ones and twos, so a frontier for each number of moves is enough.
        let mut now: Vec<Position> = vec![to];
        let mut next: Vec<Position> = Vec::new();
        let mut after: Vec<Position> = Vec::new();
        let mut so_far: u16 = 0;
        while !now.is_empty() || !next.is_empty() || !after.is_empty() {
            for &here in &now {
                let settled = self
                    .index(here)
                    .and_then(|index| moves.get(index))
                    .is_some_and(|&moves| moves == so_far);
                if !settled {
                    continue;
                }
                // A step from a square onto `here` costs what stepping onto `here` does, and
                // onto the square it heads for it is the attack, which costs one.
                let onto = if here == to {
                    1
                } else {
                    self.moves_onto(crossing, here).unwrap_or(1)
                };
                let total = so_far.saturating_add(onto);
                for square in self.around(here) {
                    if self.moves_onto(crossing, square).is_none() {
                        continue;
                    }
                    let Some(best) = self.index(square).and_then(|index| moves.get_mut(index))
                    else {
                        continue;
                    };
                    if total < *best {
                        *best = total;
                        if onto == 1 {
                            next.push(square);
                        } else {
                            after.push(square);
                        }
                    }
                }
            }
            now.clear();
            std::mem::swap(&mut now, &mut next);
            std::mem::swap(&mut next, &mut after);
            so_far = so_far.saturating_add(1);
        }
        Field { moves }
    }

    /// Whether a line from `from` to `to` crosses nothing but what a monster that crosses as
    /// `crossing` can step onto: crawl's `can_go_straight` (`mon-movetarget.cc`), as a line drawn
    /// by rounding where crawl finds a ray.
    pub(crate) fn clear_line(&self, crossing: Crossing, from: Position, to: Position) -> bool {
        line_between(from, to).all(|square| self.moves_onto(crossing, square).is_some())
    }

    /// Whether `from` sees `to` past what is solid, on a line between them rounded either way: as
    /// crawl's `cell_see_cell` (`los.cc`), which finds any ray it can.
    pub(crate) fn sees(&self, from: Position, to: Position) -> bool {
        let clear = |square| !matches!(self.terrain(square), Some(Terrain::Solid));
        line_between(from, to).all(clear) || line_between(to, from).all(clear)
    }

    /// Whether nothing solid stands on every line of a reaching attack from `from` to `to`, two
    /// squares away: `can_reach_attack_between` (`fight.cc:1639`), which looks at the terrain
    /// alone, so a monster between doesn't stop it. Either square a knight's move passes will do.
    pub(crate) fn reach_clear(&self, from: Position, to: Position) -> bool {
        let (dx, dy) = (to.x - from.x, to.y - from.y);
        let first = Position {
            x: from.x + dx / 2,
            y: from.y + dy / 2,
        };
        let second = Position {
            x: to.x - dx / 2,
            y: to.y - dy / 2,
        };
        let past = |square| {
            !matches!(
                self.terrain(square),
                Some(Terrain::Solid | Terrain::ClosedDoor)
            )
        };
        past(first) || past(second)
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
        self.search(walker, from, None, |_, _| false)
    }

    /// The moves of `walker` from `from` to the `sides` of `held`, a square it can't step onto:
    /// to those it reaches within `within` moves of the first, which are all it searches for.
    pub(crate) fn moves_to_sides(
        &self,
        walker: Walker<'_>,
        from: Position,
        held: Position,
        sides: &[Position],
        within: u32,
    ) -> Moves {
        let mut first = None;
        let mut left = sides.len();
        self.search(walker, from, Some(held), |square, moves| {
            if first.is_some_and(|first: u32| moves > first + within) {
                return true;
            }
            if sides.contains(&square) {
                first.get_or_insert(moves);
                left -= 1;
            }
            left == 0
        })
    }

    /// The search behind the paths: it walks around `held`, and stops when `done` says so of the
    /// square it reaches next, cheapest first, and the moves to it.
    fn search(
        &self,
        walker: Walker<'_>,
        from: Position,
        held: Option<Position>,
        mut done: impl FnMut(Position, u32) -> bool,
    ) -> Moves {
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
            if done(here, so_far.moves) {
                break;
            }
            for next in self.around(here) {
                if Some(next) == held {
                    continue;
                }
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

impl Field {
    /// The moves from `position` to the square it heads for on `level`, `None` when there is no
    /// way there.
    pub(crate) fn moves(&self, level: &Level, position: Position) -> Option<u16> {
        let moves = *self.moves.get(level.index(position)?)?;
        (moves != u16::MAX).then_some(moves)
    }
}

/// The squares strictly between `from` and `to`, nearest `from` first, along the straight line
/// between them, a square for each step of the longer way, rounded.
pub(crate) fn line_between(from: Position, to: Position) -> impl Iterator<Item = Position> {
    let (dx, dy) = (to.x - from.x, to.y - from.y);
    let steps = dx.abs().max(dy.abs());
    (1..steps).map(move |step| Position {
        x: from.x + rounded_share(dx, step, steps),
        y: from.y + rounded_share(dy, step, steps),
    })
}

/// `whole * part / of`, rounded to the nearest whole number, away from zero at a half.
fn rounded_share(whole: i32, part: i32, of: i32) -> i32 {
    (2 * whole * part + whole.signum() * of) / (2 * of)
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
    /// The moves of a walker that stays where it is, `from`: it reaches only that square, in no
    /// moves.
    pub(crate) fn staying(from: Position) -> Self {
        let start = Cost { past: 0, moves: 0 };
        Moves {
            from,
            best: BTreeMap::from([((from.y, from.x), (start, from))]),
        }
    }

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
/// takes actions that each take as long as `times` says, where 10 is a standard action: as
/// `can_attack` reckons it, starting with the most energy it could have and gaining
/// `div_rand_round(speed * time, 10)` each action (`_monster_add_energy`, `mon-act.cc:1653`),
/// rounded up, the most it could.
pub(crate) fn monster_moves_during(
    variant: MonsterEnergy,
    speed: u32,
    walk: u32,
    times: impl IntoIterator<Item = u32>,
) -> u32 {
    let speed = i64::from(speed);
    let walk = i64::from(walk.max(1));
    let standard = i64::from(STANDARD_ACTION);
    let mut energy = i64::from(most_starting_energy(variant));
    let mut moves = 0;
    for time in times {
        let gained = speed * i64::from(time);
        energy += gained / standard + i64::from(gained % standard != 0);
        while energy >= i64::from(ENERGY_THRESHOLD) {
            energy -= walk;
            moves += 1;
        }
    }
    moves
}

/// The most moves a monster of `speed` that spends `walk` energy a move makes while the player
/// makes `actions` standard-speed actions, as [`monster_moves_during`] reckons them.
fn monster_moves(variant: MonsterEnergy, speed: u32, walk: u32, actions: u32) -> u32 {
    let times = std::iter::repeat_n(STANDARD_ACTION, usize::try_from(actions).unwrap_or(0));
    monster_moves_during(variant, speed, walk, times)
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

    /// A level from `rows`, its first row and column at 0: `#` is solid, `+` a closed door, `~`
    /// deep water, ` ` unexplored and anything else open.
    fn level(rows: &[&str]) -> Level {
        let squares = rows.iter().zip(0..).flat_map(|(row, y)| {
            row.chars().zip(0..).filter_map(move |(square, x)| {
                let terrain = match square {
                    ' ' => return None,
                    '#' => Terrain::Solid,
                    '+' => Terrain::ClosedDoor,
                    '~' => Terrain::DeepWater,
                    _ => Terrain::Open,
                };
                Some((Position { x, y }, terrain))
            })
        });
        Level::new(squares).unwrap()
    }

    const WALKS: Crossing = Crossing {
        opens_doors: true,
        deep_water: false,
        lava: false,
    };

    #[test]
    fn the_moves_to_a_square_go_round_what_walls_it_off() {
        let level = level(&["#####", "#.#.#", "#.#.#", "#...#", "#####"]);
        let field = level.field_to(WALKS, Position { x: 1, y: 1 });
        assert_eq!(field.moves(&level, Position { x: 1, y: 2 }), Some(1));
        assert_eq!(field.moves(&level, Position { x: 3, y: 1 }), Some(4));
        assert_eq!(field.moves(&level, Position { x: 2, y: 1 }), None);
    }

    #[test]
    fn a_closed_door_costs_two_moves_to_one_that_opens_it_and_stops_one_that_doesnt() {
        let level = level(&["#####", "#.+.#", "#####"]);
        let to = Position { x: 1, y: 1 };
        let far = Position { x: 3, y: 1 };
        assert_eq!(level.field_to(WALKS, to).moves(&level, far), Some(3));
        let shut_out = Crossing {
            opens_doors: false,
            ..WALKS
        };
        assert_eq!(level.field_to(shut_out, to).moves(&level, far), None);
    }

    #[test]
    fn water_stops_a_walker_but_not_a_swimmer_and_the_square_headed_for_needs_no_footing() {
        let level = level(&["#####", "#.~.#", "#####"]);
        let far = Position { x: 3, y: 1 };
        let to = Position { x: 1, y: 1 };
        assert_eq!(level.field_to(WALKS, to).moves(&level, far), None);
        let swims = Crossing {
            deep_water: true,
            ..WALKS
        };
        assert_eq!(level.field_to(swims, to).moves(&level, far), Some(2));
        let in_water = Position { x: 2, y: 1 };
        assert_eq!(level.field_to(WALKS, in_water).moves(&level, far), Some(1));
    }

    #[test]
    fn a_reaching_attack_passes_a_monster_but_not_a_wall() {
        let walled = level(&["#####", "#.#.#", "#####"]);
        let from = Position { x: 1, y: 1 };
        let to = Position { x: 3, y: 1 };
        assert!(!walled.reach_clear(from, to));
        let open = level(&["#####", "#...#", "#####"]);
        assert!(open.reach_clear(from, to));
    }

    #[test]
    fn a_line_crosses_only_what_stands_between() {
        let level = level(&["#####", "#.#.#", "#...#", "#####"]);
        assert!(!level.clear_line(WALKS, Position { x: 1, y: 1 }, Position { x: 3, y: 1 }));
        assert!(level.clear_line(WALKS, Position { x: 1, y: 2 }, Position { x: 3, y: 2 }));
        let line: Vec<Position> =
            line_between(Position { x: 0, y: 0 }, Position { x: 4, y: 2 }).collect();
        assert_eq!(
            line,
            [
                Position { x: 1, y: 1 },
                Position { x: 2, y: 1 },
                Position { x: 3, y: 2 }
            ]
        );
    }

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

    #[test]
    fn a_monster_moves_less_while_a_hasted_player_moves_and_more_while_a_slow_one_does() {
        // Hasted, a move takes 7: from 79 energy it gains 7, so 86, 83 and 80 act and 77 waits.
        assert_eq!(monster_moves_during(ENERGY, 10, 10, [7; 4]), 3);
        // A naga's move takes 14: 93 acts twice, 87 once, 91 twice.
        assert_eq!(monster_moves_during(ENERGY, 10, 10, [14; 3]), 5);
        // Energy is rounded up, as the most it could gain: 15 in 7 is 10.5, so 11.
        assert_eq!(monster_moves_during(ENERGY, 15, 10, [7; 1]), 2);
    }
}
