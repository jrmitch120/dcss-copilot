//! The Ways Out on the level: the staircases the player can walk to, who is in the way or can
//! cut them off, and the ground the player stands on.

use std::collections::{BTreeSet, HashMap};

use copilot_gk::{Bundle, EnumId, Feature, ItemUse, Monster, Terrain};

use super::STANDARD_TIME;
use super::pace::Pace;
use super::{
    Blocker, CORRIDOR_MOST_STEPS, CorridorSquare, Escape, Ground, InTheWay, Interceptor, Usable,
    WayOut,
};
use crate::engine::paths::{self, Level, Moves, Walker};
use crate::engine::step;
use crate::engine::walk::WalkAction;
use crate::fact_sheet::attacks_player;
use crate::gaps::Gap;
use crate::visible_state::{Attitude, LevelMap, Position, VisibleMonster, VisibleState};

/// A square is corridor when monsters can stand on at most this many of the squares around it,
/// so they come at the player one or two at a time. A starting rule from #35, to be tuned on the
/// corpus.
const CORRIDOR_MOST_OPEN_SIDES: u32 = 2;

/// A side of the player counts against a hostile only if it can reach it within this many moves
/// of the first side it can reach. Farther, the player has long been holding the near side by the
/// time it gets round. Set when a broken door on D:2 read as open ground while testing #46, to be
/// tuned on the corpus.
const GETS_ROUND_IN_MOVES: u32 = 10;

/// The squares around a square, in order round it.
const RING: [(i32, i32); 8] = [
    (-1, -1),
    (0, -1),
    (1, -1),
    (1, 0),
    (1, 1),
    (0, 1),
    (-1, 1),
    (-1, 0),
];

/// Where the player stands on the level, and what they know of it.
pub(super) struct OnTheLevel<'a> {
    level: Level,
    /// The map the level is read from, which says which squares are in view.
    map: &'a LevelMap,
    /// The explored squares of water.
    water: BTreeSet<Position>,
    player: Position,
    /// The player's moves to each square they can walk to.
    walks: Moves,
    /// The squares monsters in view stand on.
    occupied: Vec<Position>,
    visible: &'a VisibleState,
    bundle: &'a Bundle,
    /// The feature at each explored square, where the bundle decodes it.
    features: Vec<(Position, &'a Feature)>,
}

/// Every feature of `bundle`, by its identifier.
fn features_by_id(bundle: &Bundle) -> HashMap<&EnumId, &Feature> {
    bundle
        .features
        .values()
        .map(|feature| (&feature.id, feature))
        .collect()
}

/// The level as `visible` shows it, read through `bundle`: `None` until the game has sent its map.
pub(crate) fn known(visible: &VisibleState, bundle: &Bundle) -> Option<Level> {
    let by_id = features_by_id(bundle);
    Level::new(
        visible
            .map
            .as_ref()?
            .cells()
            .filter_map(|(position, cell)| {
                Some((position, by_id.get(cell.feature.as_ref()?)?.terrain))
            }),
    )
}

/// The level as the player knows it, or the Gaps that keep the Advisor from knowing it.
pub(super) fn on_the_level<'a>(
    visible: &'a VisibleState,
    knowledge: Option<&'a Bundle>,
) -> Result<OnTheLevel<'a>, Vec<Gap>> {
    let (Some(map), Some(player), Some(bundle)) = (&visible.map, visible.position, knowledge)
    else {
        let mut gaps = Vec::new();
        if visible.map.is_none() {
            gaps.push(Gap::Map);
        }
        if visible.position.is_none() {
            gaps.push(Gap::Position);
        }
        if knowledge.is_none() {
            gaps.push(Gap::GameKnowledge);
        }
        return Err(gaps);
    };
    let by_id = features_by_id(bundle);
    let features: Vec<(Position, &Feature)> = map
        .cells()
        .filter_map(|(position, cell)| Some((position, *by_id.get(cell.feature.as_ref()?)?)))
        .collect();
    let level = Level::new(
        features
            .iter()
            .map(|&(position, feature)| (position, feature.terrain)),
    )
    .ok_or_else(|| vec![Gap::Map])?;
    let water = features
        .iter()
        .filter(|(_, feature)| feature.water)
        .map(|&(position, _)| position)
        .collect();
    let occupied: Vec<Position> = visible
        .monsters
        .iter()
        .flatten()
        .map(|monster| monster.position)
        .collect();
    let walks = level.moves_from(
        Walker::Player {
            occupied: &occupied,
        },
        player,
    );
    Ok(OnTheLevel {
        level,
        map,
        water,
        player,
        walks,
        occupied,
        visible,
        bundle,
        features,
    })
}

impl<'a> OnTheLevel<'a> {
    /// The Game Knowledge the level is read through.
    pub(super) fn bundle(&self) -> &'a Bundle {
        self.bundle
    }

    /// The squares around `position` a monster could stand on to attack: open ones, closed
    /// doors it could open, and unexplored ones, which may be either.
    fn sides(&self, position: Position) -> impl Iterator<Item = Position> + '_ {
        self.level.around(position).filter(|&square| {
            matches!(
                self.level.terrain(square),
                None | Some(Terrain::Open | Terrain::ClosedDoor)
            )
        })
    }

    fn open_sides(&self, position: Position) -> u32 {
        u32::try_from(self.sides(position).count()).unwrap_or(u32::MAX)
    }

    /// Whether `monster` walks while the player watches it.
    fn walks(&self, monster: &Monster) -> bool {
        !monster.stationary
            && step::walks_while_watched(self.bundle.formulas.monster_step, monster.watched_step)
    }

    /// How `monster` steps, for a walker: it opens doors if it uses at least doors.
    fn walker(monster: &Monster) -> Walker<'static> {
        Walker::Monster {
            opens_doors: monster.item_use >= ItemUse::OpenDoors,
        }
    }

    /// The moves of `monster` from where it stands.
    fn moves_of(&self, monster: &Monster, from: Position) -> Moves {
        if self.walks(monster) {
            self.level.moves_from(Self::walker(monster), from)
        } else {
            Moves::staying(from)
        }
    }

    /// How many of the open sides around `square` the hostiles in view can come at it from while
    /// it is held, if few enough to make it a corridor. `None` otherwise, or when that can't be
    /// told: a monster in view isn't known, or no hostile that walks can reach any side, so where
    /// they stand says nothing of the ground.
    fn sides_hostiles_reach(&self, square: Position) -> Option<u32> {
        let sides: Vec<Position> = self.sides(square).collect();
        let mut reached = BTreeSet::new();
        let mut a_walker_reaches = false;
        for seen in self.visible.monsters.iter().flatten() {
            if !attacks_player(seen.attitude?) {
                continue;
            }
            let monster = self.bundle.monster_by_id(seen.id.as_ref()?)?;
            let moves = if self.walks(monster) {
                self.level.moves_to_sides(
                    Self::walker(monster),
                    seen.position,
                    square,
                    &sides,
                    GETS_ROUND_IN_MOVES,
                )
            } else {
                Moves::staying(seen.position)
            };
            let first = sides.iter().filter_map(|&side| moves.to(side)).min();
            let Some(first) = first else { continue };
            a_walker_reaches |= self.walks(monster);
            reached.extend(sides.iter().copied().filter(|&side| {
                moves
                    .to(side)
                    .is_some_and(|to| to <= first + GETS_ROUND_IN_MOVES)
            }));
            if reached.len() > CORRIDOR_MOST_OPEN_SIDES as usize {
                return None;
            }
        }
        a_walker_reaches
            .then(|| u32::try_from(reached.len()).ok())
            .flatten()
    }

    /// The moves of each hostile in view that walks, over the level as it is.
    fn walkers_moves(&self) -> Vec<Moves> {
        self.visible
            .monsters
            .iter()
            .flatten()
            .filter(|seen| seen.attitude.is_none_or(attacks_player))
            .filter_map(|seen| {
                let monster = self.bundle.monster_by_id(seen.id.as_ref()?)?;
                self.walks(monster)
                    .then(|| self.moves_of(monster, seen.position))
            })
            .collect()
    }

    /// Whether walls part the open squares around `square`, as they do around a doorway, so
    /// holding it can keep hostiles to one side.
    fn parts_its_sides(&self, square: Position) -> bool {
        let sides: Vec<Position> = self.sides(square).collect();
        let open = RING.map(|(dx, dy)| {
            sides.contains(&Position {
                x: square.x + dx,
                y: square.y + dy,
            })
        });
        let stretches = (0..RING.len())
            .filter(|&at| open[at] && !open[(at + 1) % RING.len()])
            .count();
        stretches >= 2
    }

    /// Whether the player, `steps` from `square`, can get there before any of `walkers` and hold
    /// it as a corridor, though more of its sides are open. Asks last what takes a search per
    /// hostile, as a crowded level has many squares to ask it of on every update.
    fn can_be_held(&self, square: Position, steps: u32, walkers: &[Moves]) -> bool {
        steps <= CORRIDOR_MOST_STEPS
            && self.parts_its_sides(square)
            && !self
                .walks
                .path_to(square)
                .iter()
                .any(|step| self.occupied.contains(step))
            && walkers
                .iter()
                .all(|moves| moves.to(square).is_none_or(|to| to >= steps))
            && self.sides_hostiles_reach(square).is_some()
    }

    /// The ground the player stands on, and on open ground the nearest corridor.
    pub(super) fn ground(&self) -> Ground {
        let open_sides = self.open_sides(self.player);
        if open_sides <= CORRIDOR_MOST_OPEN_SIDES {
            return Ground::Corridor { open_sides };
        }
        if let Some(open_sides) = self.sides_hostiles_reach(self.player) {
            return Ground::Corridor { open_sides };
        }
        let mut squares: Vec<(Position, u32)> = self
            .walks
            .reached()
            .filter(|&(square, _)| {
                self.level.terrain(square) == Some(Terrain::Open)
                    && square != self.player
                    && !self.occupied.contains(&square)
            })
            .collect();
        squares.sort_by_key(|&(_, steps)| steps);
        let walkers = self.walkers_moves();
        let nearest_corridor = squares
            .into_iter()
            .find(|&(square, steps)| {
                self.open_sides(square) <= CORRIDOR_MOST_OPEN_SIDES
                    || self.can_be_held(square, steps, &walkers)
            })
            .map(|(position, steps)| CorridorSquare { position, steps });
        Ground::OpenGround {
            open_sides,
            nearest_corridor,
        }
    }

    /// The staircases the player can walk to, nearest first, each blocked by the monsters in
    /// the way and cut off by the hostiles in view that can stand beside it by the time the
    /// player, at their `pace`, takes it. A hostile the bundle doesn't know is a Gap in `gaps`.
    pub(super) fn stairs(&self, pace: &Pace<'_>, gaps: &mut BTreeSet<Gap>) -> Vec<WayOut> {
        let chasers = self.chasers(gaps);
        let mut stairs: Vec<(u32, WayOut)> = self
            .features
            .iter()
            .filter_map(|&(position, feature)| {
                let direction = feature.stairs?;
                let steps = self.walks.to(position)?;
                let path = self.walks.path_to(position);
                let in_the_way = self.in_the_way(&path);
                let chasing: Vec<&Chaser<'_>> = chasers
                    .iter()
                    .filter(|chaser| {
                        !in_the_way
                            .iter()
                            .any(|blocking| blocking.position == chaser.seen.position)
                    })
                    .collect();
                let cut_off =
                    self.interceptors(&chasing, position, &self.times_along(&path, pace), pace);
                let mut because = Vec::new();
                if !in_the_way.is_empty() {
                    because.push(Blocker::InTheWay { by: in_the_way });
                }
                match (cut_off.is_empty(), steps) {
                    (true, _) => {}
                    // Standing on it, nothing races them there: those beside them come along.
                    (false, 0) => because.push(Blocker::Followed { by: cut_off }),
                    (false, _) => because.push(Blocker::CutOff { by: cut_off }),
                }
                let usable = if because.is_empty() {
                    Usable::Yes
                } else {
                    Usable::No { because }
                };
                let option = Escape::Stairs {
                    name: feature.name.clone(),
                    direction,
                    position,
                    steps,
                    over_remembered_squares: self.leaves_view(&path),
                };
                Some((steps, WayOut { option, usable }))
            })
            .collect();
        stairs.sort_by_key(|&(steps, _)| steps);
        stairs.into_iter().map(|(_, way_out)| way_out).collect()
    }

    /// The monsters on the player's `path` to a staircase, the staircase's square last, other
    /// than allies they could swap places with, nearest the player first.
    fn in_the_way(&self, path: &[Position]) -> Vec<InTheWay> {
        path.iter()
            .filter_map(|&square| {
                let monster = self
                    .visible
                    .monsters
                    .iter()
                    .flatten()
                    .find(|monster| monster.position == square)?;
                let ally = matches!(
                    monster.attitude,
                    Some(Attitude::Friendly | Attitude::Marionette)
                );
                (!ally).then(|| InTheWay {
                    name: monster.name.clone(),
                    position: square,
                })
            })
            .collect()
    }

    /// Whether any square of the player's `path` to a staircase, the staircase's square last, is
    /// out of view.
    fn leaves_view(&self, path: &[Position]) -> bool {
        path.iter()
            .any(|&square| !self.map.cell(square).is_some_and(|cell| cell.in_view))
    }

    /// The hostiles in view that can move, each with its kind and its moves to every square it
    /// can reach. One that steps toward the player only from beside them while they watch it,
    /// such as the wandering mushroom, stays where it is, so it cuts off only a staircase it stands
    /// beside. One that steps by chance, the lurking horror, walks as if every roll let it, which
    /// errs high ([`step::walks_while_watched`]).
    fn chasers(&self, gaps: &mut BTreeSet<Gap>) -> Vec<Chaser<'_>> {
        let mut chasers = Vec::new();
        for seen in self.visible.monsters.iter().flatten() {
            // A monster whose side isn't decoded may be hostile: without Game Knowledge, as the
            // Threat Level counts it.
            let Some(attitude) = seen.attitude else {
                gaps.insert(Gap::GameKnowledge);
                continue;
            };
            if !attacks_player(attitude) {
                continue;
            }
            let Some(monster) = seen
                .id
                .as_ref()
                .and_then(|id| self.bundle.monster_by_id(id))
            else {
                gaps.insert(Gap::MonsterKnowledge {
                    monster: seen.name.clone(),
                });
                continue;
            };
            if monster.stationary {
                continue;
            }
            let moves = self.moves_of(monster, seen.position);
            chasers.push(Chaser {
                seen,
                speed: monster.speed,
                walk: monster.energy.walk,
                moves,
            });
        }
        chasers
    }

    /// The time each of the player's actions along their `path` to a staircase takes at their
    /// `pace`: a step onto each square, into water or not, and before a step onto a closed door
    /// the action that opens it.
    fn times_along(&self, path: &[Position], pace: &Pace<'_>) -> Vec<u32> {
        let mut times = Vec::new();
        for &square in path {
            if self.level.terrain(square) == Some(Terrain::ClosedDoor) {
                times.push(pace.action());
            }
            times.push(pace.step(self.water.contains(&square)));
        }
        times
    }

    /// The player's actions on the way to the staircase at `stairs`, at their `pace`, one for each
    /// door opened and each step.
    pub(super) fn walk_to(&self, stairs: Position, pace: &Pace<'_>) -> Vec<WalkAction> {
        let mut actions = Vec::new();
        for square in self.walks.path_to(stairs) {
            if self.level.terrain(square) == Some(Terrain::ClosedDoor) {
                actions.push(WalkAction {
                    to: None,
                    time: i32::try_from(pace.action()).unwrap_or(i32::MAX),
                });
            }
            actions.push(WalkAction {
                to: Some(square),
                time: i32::try_from(pace.step(self.water.contains(&square))).unwrap_or(i32::MAX),
            });
        }
        actions
    }

    /// The chasers that can stand beside the staircase at `stairs` by the time the player has
    /// taken actions on their way to it that take `times`, the first there first. One that
    /// moves faster than the player walks at their `pace` is said to be faster.
    fn interceptors(
        &self,
        chasers: &[&Chaser<'_>],
        stairs: Position,
        times: &[u32],
        pace: &Pace<'_>,
    ) -> Vec<Interceptor> {
        let energy = self.bundle.formulas.monster_energy;
        let walk_time = pace.step(false);
        let mut by: Vec<Interceptor> = chasers
            .iter()
            .filter_map(|chaser| {
                let moves = chaser.moves.to_beside(stairs)?;
                let during = times.iter().copied();
                if paths::monster_moves_during(energy, chaser.speed, chaser.walk, during) < moves {
                    return None;
                }
                Some(Interceptor {
                    name: chaser.seen.name.clone(),
                    position: chaser.seen.position,
                    moves,
                    arrives_after: paths::actions_to_move(
                        energy,
                        chaser.speed,
                        chaser.walk,
                        moves,
                    )?,
                    // A move of its takes `walk / speed` of a standard action's time.
                    faster: chaser.speed * walk_time > chaser.walk * STANDARD_TIME,
                })
            })
            .collect();
        by.sort_by_key(|interceptor| interceptor.arrives_after);
        by
    }
}

/// A hostile in view that can move.
struct Chaser<'a> {
    seen: &'a VisibleMonster,
    /// Its speed, where 10 is the player's normal speed.
    speed: u32,
    /// The energy a move on land costs it.
    walk: u32,
    /// Its moves to every square it can reach.
    moves: Moves,
}
