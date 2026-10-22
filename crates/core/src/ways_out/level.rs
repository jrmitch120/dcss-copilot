//! The Ways Out on the level: the staircases the player can walk to, who is in the way or can
//! cut them off, and the ground the player stands on.

use std::collections::{BTreeSet, HashMap};

use copilot_gk::{Bundle, EnumId, Feature, ItemUse, Terrain};

use super::STANDARD_TIME;
use super::pace::Pace;
use super::{Blocker, CorridorSquare, Escape, Ground, InTheWay, Interceptor, Usable, WayOut};
use crate::engine::paths::{self, Level, Moves, Walker};
use crate::engine::step;
use crate::fact_sheet::{Gap, attacks_player};
use crate::visible_state::{Attitude, Position, VisibleMonster, VisibleState};

/// A square is corridor when monsters can stand on at most this many of the squares around it,
/// so they come at the player one or two at a time. A starting rule from #35, to be tuned on the
/// corpus.
const CORRIDOR_MOST_OPEN_SIDES: u32 = 2;

/// Where the player stands on the level, and what they know of it.
pub(super) struct OnTheLevel<'a> {
    level: Level,
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
    let by_id: HashMap<&EnumId, &Feature> = bundle
        .features
        .values()
        .map(|feature| (&feature.id, feature))
        .collect();
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
    fn open_sides(&self, position: Position) -> u32 {
        let open = self
            .level
            .around(position)
            .filter(|&square| {
                matches!(
                    self.level.terrain(square),
                    None | Some(Terrain::Open | Terrain::ClosedDoor)
                )
            })
            .count();
        u32::try_from(open).unwrap_or(u32::MAX)
    }

    /// The ground the player stands on, and on open ground the nearest corridor.
    pub(super) fn ground(&self) -> Ground {
        let open_sides = self.open_sides(self.player);
        if open_sides <= CORRIDOR_MOST_OPEN_SIDES {
            return Ground::Corridor { open_sides };
        }
        let nearest_corridor = self
            .walks
            .reached()
            .filter(|&(square, _)| {
                self.level.terrain(square) == Some(Terrain::Open)
                    && !self.occupied.contains(&square)
                    && self.open_sides(square) <= CORRIDOR_MOST_OPEN_SIDES
            })
            .min_by_key(|&(_, steps)| steps)
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
                let in_the_way = self.in_the_way(position);
                let chasing: Vec<&Chaser<'_>> = chasers
                    .iter()
                    .filter(|chaser| {
                        !in_the_way
                            .iter()
                            .any(|blocking| blocking.position == chaser.seen.position)
                    })
                    .collect();
                let cut_off =
                    self.interceptors(&chasing, position, &self.times_to(position, pace), pace);
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
                };
                Some((steps, WayOut { option, usable }))
            })
            .collect();
        stairs.sort_by_key(|&(steps, _)| steps);
        stairs.into_iter().map(|(_, way_out)| way_out).collect()
    }

    /// The monsters on the player's path to `stairs`, or on it, other than allies they could
    /// swap places with, nearest the player first.
    fn in_the_way(&self, stairs: Position) -> Vec<InTheWay> {
        self.walks
            .path_to(stairs)
            .into_iter()
            .filter_map(|square| {
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

    /// The hostiles in view that can move, each with its kind and its moves to every square it
    /// can reach. One that steps toward the player only from beside them while they watch it,
    /// such as the wandering mushroom, stays where it is, so it cuts off only a staircase it stands
    /// beside. One that steps by chance, the lurking horror, walks as if every roll let it, which
    /// errs high ([`step::walks_while_watched`]).
    fn chasers(&self, gaps: &mut BTreeSet<Gap>) -> Vec<Chaser<'_>> {
        let variant = self.bundle.formulas.monster_step;
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
            let moves = if step::walks_while_watched(variant, monster.watched_step) {
                let walker = Walker::Monster {
                    opens_doors: monster.item_use >= ItemUse::OpenDoors,
                };
                self.level.moves_from(walker, seen.position)
            } else {
                Moves::staying(seen.position)
            };
            chasers.push(Chaser {
                seen,
                speed: monster.speed,
                walk: monster.energy.walk,
                moves,
            });
        }
        chasers
    }

    /// The time each of the player's actions on their path to `stairs` takes at their `pace`:
    /// a step onto each square, into water or not, and before a step onto a closed door the
    /// action that opens it.
    fn times_to(&self, stairs: Position, pace: &Pace<'_>) -> Vec<u32> {
        let mut times = Vec::new();
        for square in self.walks.path_to(stairs) {
            if self.level.terrain(square) == Some(Terrain::ClosedDoor) {
                times.push(pace.action());
            }
            times.push(pace.step(self.water.contains(&square)));
        }
        times
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
