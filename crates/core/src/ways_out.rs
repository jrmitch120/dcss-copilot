//! Ways Out: the escapes and emergency options open to the player at a moment, each with
//! whether it can be used right now and why not, and the ground they stand on.

use std::collections::{BTreeSet, HashMap};

use copilot_gk::{Bundle, EnumId, Feature, ItemUse, StairDirection, Terrain};
use serde::Serialize;

use crate::engine::paths::{self, Level, Moves, Walker};
use crate::fact_sheet::{Gap, NotModelled, attacks_player};
use crate::visible_state::{Attitude, Position, VisibleMonster, VisibleState};

/// A square is corridor when monsters can stand on at most this many of the squares around it,
/// so they come at the player one or two at a time. A starting rule from #35, to be tuned on the
/// corpus.
const CORRIDOR_MOST_OPEN_SIDES: u32 = 2;

/// What the staircase Ways Out leave out, whenever there are any.
const STAIRS_NOT_MODELLED: [NotModelled; 4] = [
    NotModelled::PlayerSpeed,
    NotModelled::SwimmingAndFlying,
    NotModelled::UnexploredSquares,
    NotModelled::HitsOnTheWay,
];

/// The player's Ways Out at a moment, and the ground they stand on.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WaysOut {
    /// Each Way Out: the staircases on the level the player knows a path to, nearest first.
    pub options: Vec<WayOut>,
    /// Whether the player stands in a corridor or on open ground, once the Advisor can tell.
    pub ground: Option<Ground>,
    /// What kept the Advisor from listing more, or from judging what it lists.
    pub gaps: Vec<Gap>,
    /// What the judgement of the Ways Out leaves out, so it is read for what it is.
    pub not_modelled: Vec<NotModelled>,
}

/// An escape or emergency option, and whether the player can use it right now.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WayOut {
    /// What it is.
    pub option: Escape,
    /// Whether it can be used right now, and why not.
    pub usable: Usable,
}

/// What a Way Out is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Escape {
    /// A staircase on this level the player knows a path to.
    Stairs {
        /// Crawl's name for it, such as `stone staircase leading down`.
        name: String,
        /// Which way it leads.
        direction: StairDirection,
        /// The square it is on.
        position: Position,
        /// The player's moves to it over the explored map, counting the move that opens each
        /// door: none when they stand on it.
        steps: u32,
    },
}

/// Whether a Way Out can be used right now, as far as the Advisor can tell.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(
    tag = "state",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Usable {
    /// Nothing the Advisor can see stops it.
    Yes,
    /// It can't be used now, or won't work.
    No {
        /// Why.
        because: Vec<Blocker>,
    },
}

/// Why a Way Out can't be used right now.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(
    tag = "blocker",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Blocker {
    /// Monsters stand on the player's path to the staircase, or on it, and aren't allies the
    /// player could swap places with.
    InTheWay {
        /// Those monsters, nearest the player first.
        by: Vec<InTheWay>,
    },
    /// Hostiles in view can stand beside the staircase by the time the player gets there and
    /// takes it, to attack them or follow them off the level.
    CutOff {
        /// Those that can, the first there first.
        by: Vec<Interceptor>,
    },
}

/// A monster on the player's path to a staircase.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InTheWay {
    /// Its name, as the game names it.
    pub name: String,
    /// The square it is on.
    pub position: Position,
}

/// A hostile in view that can reach a staircase before the player takes it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Interceptor {
    /// Its name, as the game names it.
    pub name: String,
    /// The square it is on.
    pub position: Position,
    /// Its moves to beside the staircase.
    pub moves: u32,
    /// The fewest of the player's standard-speed actions it needs to make them, at the most
    /// energy it could have.
    pub arrives_after: u32,
    /// It moves faster than the player walks.
    pub faster: bool,
}

/// The ground the player stands on, which decides how many monsters can attack them at once.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(
    tag = "ground",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Ground {
    /// A corridor: monsters can attack them from few sides.
    Corridor {
        /// The squares around them a monster could stand on to attack.
        open_sides: u32,
    },
    /// Open ground: monsters can attack them from more sides.
    OpenGround {
        /// The squares around them a monster could stand on to attack.
        open_sides: u32,
        /// The nearest square of corridor they can walk to over the explored map, if any.
        nearest_corridor: Option<CorridorSquare>,
    },
}

/// A square of corridor the player can walk to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CorridorSquare {
    /// The square.
    pub position: Position,
    /// The player's moves to it.
    pub steps: u32,
}

impl WaysOut {
    /// The Ways Out the engine finds in `visible`, with `knowledge` for the game's Crawl
    /// Version if the Advisor has it.
    pub(crate) fn compute(visible: &VisibleState, knowledge: Option<&Bundle>) -> Self {
        let mut gaps = BTreeSet::new();
        let mut ways_out = Self::default();
        match on_the_level(visible, knowledge) {
            Ok(here) => {
                ways_out.ground = Some(here.ground());
                ways_out.options.extend(here.stairs(&mut gaps));
                if !ways_out.options.is_empty() {
                    ways_out.not_modelled.extend(STAIRS_NOT_MODELLED);
                }
            }
            Err(missing) => gaps.extend(missing),
        }
        ways_out.gaps = gaps.into_iter().collect();
        ways_out
    }
}

/// Where the player stands on the level, and what they know of it.
struct OnTheLevel<'a> {
    level: Level,
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
fn on_the_level<'a>(
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
        player,
        walks,
        occupied,
        visible,
        bundle,
        features,
    })
}

impl OnTheLevel<'_> {
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
    fn ground(&self) -> Ground {
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
    /// player takes it. A hostile the bundle doesn't know is a Gap in `gaps`.
    fn stairs(&self, gaps: &mut BTreeSet<Gap>) -> Vec<WayOut> {
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
                let cut_off = self.interceptors(&chasing, position, steps);
                let mut because = Vec::new();
                if !in_the_way.is_empty() {
                    because.push(Blocker::InTheWay { by: in_the_way });
                }
                if !cut_off.is_empty() {
                    because.push(Blocker::CutOff { by: cut_off });
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
    /// can reach.
    fn chasers(&self, gaps: &mut BTreeSet<Gap>) -> Vec<Chaser<'_>> {
        let mut chasers = Vec::new();
        for seen in self.visible.monsters.iter().flatten() {
            if !seen.attitude.is_some_and(attacks_player) {
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
            let walker = Walker::Monster {
                opens_doors: monster.item_use >= ItemUse::OpenDoors,
            };
            chasers.push(Chaser {
                seen,
                speed: monster.speed,
                walk: monster.energy.walk,
                moves: self.level.moves_from(walker, seen.position),
            });
        }
        chasers
    }

    /// The chasers that can stand beside the staircase at `stairs` by the time the player has
    /// made `steps` moves to it, the first there first.
    fn interceptors(
        &self,
        chasers: &[&Chaser<'_>],
        stairs: Position,
        steps: u32,
    ) -> Vec<Interceptor> {
        let energy = self.bundle.formulas.monster_energy;
        let mut by: Vec<Interceptor> = chasers
            .iter()
            .filter_map(|chaser| {
                let moves = chaser.moves.to_beside(stairs)?;
                if paths::monster_moves(energy, chaser.speed, chaser.walk, steps) < moves {
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
                    faster: chaser.speed > chaser.walk,
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
