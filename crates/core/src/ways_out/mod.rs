//! Ways Out: the escapes and emergency options open to the player at a moment, each with
//! whether it can be used right now and why not, and the ground they stand on.

mod carried;
mod level;
mod list;
mod pace;

use std::collections::BTreeSet;

use copilot_gk::{Bundle, Relief, SpeedStatus, StairDirection};
use serde::Serialize;

use crate::accumulated_state::{AccumulatedState, Remembered};
use crate::engine::melee::BASELINE_DELAY;
use crate::fact_sheet::{Gap, NotModelled};
use crate::visible_state::{Position, VisibleState};

/// The time a standard move or action takes the player: `BASELINE_DELAY`.
const STANDARD_TIME: u32 = BASELINE_DELAY.unsigned_abs();

/// What judging the ground leaves out, whenever it is judged.
const GROUND_NOT_MODELLED: [NotModelled; 2] = [
    NotModelled::SwimmingAndFlying,
    NotModelled::UnexploredSquares,
];

/// What the staircase Ways Out leave out besides, whenever there are any.
const STAIRS_NOT_MODELLED: [NotModelled; 1] = [NotModelled::HitsOnTheWay];

/// The player's Ways Out at a moment, and the ground they stand on.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WaysOut {
    /// Each Way Out: the staircases on the level the player knows a path to, nearest first,
    /// then the items they carry, their abilities and their spells, in the order the game lists
    /// each.
    pub options: Vec<WayOut>,
    /// Whether the player stands in a corridor or on open ground, once the Advisor can tell.
    pub ground: Option<Ground>,
    /// The player's Pace, which the staircases were judged by, when there are any.
    pub pace: Option<PlayerPace>,
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
    /// Items the player carries, all of one kind in one inventory slot.
    Item {
        /// Its inventory letter.
        letter: char,
        /// Its name, as the game shows it, such as `2 scrolls of teleportation`. Crawl doesn't
        /// send it again when the item is identified, so it may still be the unidentified one.
        name: String,
        /// Crawl's name for its kind, such as `teleportation`.
        kind_name: String,
        /// How many there are.
        quantity: u32,
        /// What it does for the player.
        effect: Relief,
    },
    /// An ability the player has, as the ability menu (`a`) last showed it.
    Ability {
        /// Its name, such as `Berserk`.
        name: String,
        /// What it does for the player.
        effect: Relief,
        /// The chance of failing to use it, in percent.
        failure_percent: u32,
        /// When the menu showed it.
        read: ReadOn,
    },
    /// A spell the player knows, as the spell list (`I`) last showed it.
    Spell {
        /// Its name, such as `Blink`.
        name: String,
        /// What it does for the player.
        effect: Relief,
        /// The chance of failing to cast it, in percent.
        failure_percent: u32,
        /// When the spell list showed it.
        read: ReadOn,
    },
}

/// When a remembered fact a Way Out comes from was read, and whether it is still current.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadOn {
    /// The game's turn when it was read, if the game had reported one.
    pub turn: Option<u32>,
    /// Something seen since may have changed it, such as its failure rate.
    pub stale: bool,
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
    /// The player stands on the staircase, and hostiles beside them would follow them off the
    /// level: it takes them away from every other monster, but not from these.
    Followed {
        /// Those that would.
        by: Vec<Interceptor>,
    },
    /// A status the player has stops it, such as confusion stopping them reading.
    Status {
        /// The status, as the status pane names it: its words, such as `confused`, or its
        /// light when it has none.
        status: String,
    },
    /// It costs more magic than the player has.
    Magic {
        /// The magic points it costs.
        needs: u32,
        /// The magic points the player has.
        has: i32,
    },
    /// Crawl greys out an item that heals, as it does at full health, and the player is at
    /// full health.
    FullHealth,
    /// Crawl greys the item out as of no use to the player right now, for a reason the
    /// Advisor doesn't see.
    Useless,
}

/// The player's Pace: the time each of their moves to a staircase takes, at the speed crawl gives
/// them, the slowest it may roll it, which the race to a staircase plays them at.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerPace {
    /// The time a move onto dry ground takes, where 10 is a standard move.
    pub move_time: u32,
    /// The time a move into water takes, where it differs from one onto dry ground.
    pub water_move_time: Option<u32>,
    /// The time opening a door takes.
    pub action_time: u32,
    /// What the Advisor read that makes it other than standard.
    pub because: Vec<PaceReason>,
    /// When the mutations screen it read the player's mutations off was read, `None` while it
    /// hasn't been, which leaves only those of their species.
    pub mutations_read: Option<ReadOn>,
}

/// What makes the player's Pace other than standard.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(
    tag = "reason",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum PaceReason {
    /// A status they have, such as haste.
    Status {
        /// What it is.
        status: SpeedStatus,
    },
    /// The form they are in.
    Form {
        /// The status light it shows, such as `Bat`.
        light: String,
        /// The time a move takes in it, before anything else changes it.
        move_speed: i32,
    },
    /// A mutation they have that changes their speed.
    Mutation {
        /// Its `mutation_type` identifier, such as `MUT_CONSTRICTING_TAIL`.
        mutation: String,
        /// Their level of it.
        level: u32,
        /// Their species has it.
        from_species: bool,
        /// Their form stops it working.
        suppressed: bool,
    },
    /// A bane they have that changes their speed.
    Bane {
        /// Its `bane_type` identifier, such as `BANE_LETHARGY`.
        bane: String,
    },
    /// They follow a god who slows them, taken at the most piety their stars hold.
    God {
        /// The god, such as `Cheibriados`.
        god: String,
        /// The stars of piety the status pane shows, if it has.
        piety_rank: Option<u32>,
    },
    /// They wear armour of ponderousness.
    Ponderous {
        /// How many pieces.
        pieces: u32,
    },
    /// They wear the lightning scales.
    Quickened,
    /// Their god lets them walk on water.
    WaterWalking,
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
    /// It moves faster than the player walks at their Pace.
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
    /// The Ways Out the engine finds in `visible`, what the Advisor remembers of the game and
    /// the character's `species`, as the game names it, with `knowledge` for the game's Crawl
    /// Version if the Advisor has it.
    pub(crate) fn compute(
        visible: &VisibleState,
        remembered: Option<&AccumulatedState>,
        species: Option<&str>,
        knowledge: Option<&Bundle>,
    ) -> Self {
        let mut gaps = BTreeSet::new();
        let mut ways_out = Self::default();
        match level::on_the_level(visible, knowledge) {
            Ok(here) => {
                let pace = pace::Pace::of(visible, remembered, species, here.bundle());
                ways_out.ground = Some(here.ground());
                ways_out.not_modelled.extend(GROUND_NOT_MODELLED);
                ways_out.options.extend(here.stairs(&pace, &mut gaps));
                if !ways_out.options.is_empty() {
                    if pace.unseen() {
                        ways_out.not_modelled.push(NotModelled::PlayerSpeed);
                    }
                    ways_out.not_modelled.extend(STAIRS_NOT_MODELLED);
                    ways_out.pace = Some(pace.record());
                }
            }
            Err(missing) => gaps.extend(missing),
        }
        if let Some(bundle) = knowledge {
            let carried = carried::Carried {
                visible,
                remembered,
                bundle,
            };
            ways_out.options.extend(carried.items());
            ways_out.options.extend(carried.abilities(&mut gaps));
            ways_out.options.extend(carried.spells(&mut gaps));
            if ways_out.options.iter().any(|way_out| {
                matches!(
                    way_out.option,
                    Escape::Ability { .. } | Escape::Spell { .. }
                )
            }) {
                ways_out.not_modelled.push(NotModelled::OwnConditions);
            }
        }
        ways_out.gaps = gaps.into_iter().collect();
        ways_out
    }
}

/// A remembered fact's value and when it was read, unless it was never read.
pub(super) fn current<T>(remembered: &Remembered<T>) -> Option<(&T, ReadOn)> {
    match remembered {
        Remembered::Missing => None,
        Remembered::Known(fact) => Some((
            &fact.value,
            ReadOn {
                turn: fact.turn,
                stale: false,
            },
        )),
        Remembered::Stale(fact) => Some((
            &fact.value,
            ReadOn {
                turn: fact.turn,
                stale: true,
            },
        )),
    }
}

#[cfg(test)]
mod tests {
    use copilot_gk::{EnumId, Library};

    use super::*;
    use crate::accumulated_state::{AsOf, MutationsRead};
    use crate::screen::{Screen, ShownMutations};
    use crate::view_model::{Colour, Glyph};
    use crate::visible_state::{
        Attitude, LevelMap, MapCell, Religion, SquareLight, Stats, Status, VisibleMonster,
    };

    /// The shipped 0.34.1 bundle.
    fn bundle() -> &'static Bundle {
        static LIBRARY: std::sync::OnceLock<Library> = std::sync::OnceLock::new();
        LIBRARY
            .get_or_init(|| Library::shipped().unwrap())
            .for_version("0.34.1")
            .unwrap()
    }

    /// A corridor of floor from x 0 to 10 with a staircase down at its east end, the player at x
    /// 4, six steps from it, lit with `lights`, and a hostile goblin at x `goblin`, west of them.
    fn corridor(lights: &[&str], goblin: i32) -> VisibleState {
        let cell = |feature: &str, glyph: &str| MapCell {
            glyph: Glyph {
                text: glyph.to_owned(),
                colour: Colour::White,
            },
            feature: Some(EnumId(feature.to_owned())),
            in_view: true,
            light: SquareLight::Plain,
        };
        let map = LevelMap::new((0..=10).map(|x| {
            let square = if x == 10 {
                cell("DNGN_STONE_STAIRS_DOWN_I", ">")
            } else {
                cell("DNGN_FLOOR", ".")
            };
            (Position { x, y: 0 }, square)
        }));
        let monster = bundle()
            .monsters
            .values()
            .find(|monster| monster.name == "goblin")
            .unwrap();
        VisibleState {
            stats: Stats {
                xl: Some(1),
                ..Stats::default()
            },
            religion: Some(Religion::NoGod),
            position: Some(Position { x: 4, y: 0 }),
            statuses: Some(
                lights
                    .iter()
                    .map(|&light| Status {
                        light: Some(light.to_owned()),
                        text: None,
                        colour: None,
                    })
                    .collect(),
            ),
            monsters: Some(vec![VisibleMonster {
                name: "goblin".to_owned(),
                glyph: Glyph {
                    text: "g".to_owned(),
                    colour: Colour::White,
                },
                id: Some(monster.id.clone()),
                position: Position { x: goblin, y: 0 },
                attitude: Some(Attitude::Hostile),
                crawl_threat: None,
                sighting: None,
            }]),
            inventory: Some(Vec::new()),
            map: Some(map),
            ..VisibleState::default()
        }
    }

    /// What the Advisor remembers once the mutations screen showed nothing beyond the species'
    /// own mutations, or, unless `read`, nothing.
    fn remembered(read: bool) -> AccumulatedState {
        let mut remembered = AccumulatedState::default();
        if read {
            remembered.mutations = Remembered::Known(AsOf {
                value: MutationsRead {
                    shown: ShownMutations::default(),
                    xl: Some(1),
                },
                turn: Some(1),
                source: Screen::Mutations,
            });
        }
        remembered
    }

    /// The Ways Out of `visible` for a character of `species`, the mutations screen read or not.
    fn ways_out(visible: &VisibleState, species: &str, read: bool) -> WaysOut {
        WaysOut::compute(
            visible,
            Some(&remembered(read)),
            Some(species),
            Some(bundle()),
        )
    }

    fn stairs_usable(ways_out: &WaysOut) -> bool {
        let stairs = ways_out
            .options
            .iter()
            .find(|way_out| matches!(way_out.option, Escape::Stairs { .. }))
            .unwrap();
        stairs.usable == Usable::Yes
    }

    #[test]
    fn a_naga_is_cut_off_from_stairs_a_human_reaches() {
        // The goblin is nine moves from beside the stairs: a human walks there in six standard
        // actions, a naga, at 14 a move, in as long as the goblin takes for nine.
        let visible = corridor(&[], 0);
        assert!(stairs_usable(&ways_out(&visible, "Human", true)));
        let naga = ways_out(&visible, "Naga", true);
        assert!(!stairs_usable(&naga));
        let pace = naga.pace.unwrap();
        assert_eq!(pace.move_time, 14);
        assert_eq!(
            pace.because,
            [PaceReason::Mutation {
                mutation: "MUT_CONSTRICTING_TAIL".to_owned(),
                level: 1,
                from_species: true,
                suppressed: false,
            }]
        );
    }

    #[test]
    fn player_speed_is_named_until_the_mutations_screen_is_read() {
        let visible = corridor(&["Fast"], 0);
        let unread = ways_out(&visible, "Human", false);
        assert!(unread.not_modelled.contains(&NotModelled::PlayerSpeed));
        assert_eq!(unread.pace.unwrap().mutations_read, None);
        let read = ways_out(&visible, "Human", true);
        assert!(!read.not_modelled.contains(&NotModelled::PlayerSpeed));
    }

    #[test]
    fn player_speed_is_named_for_a_status_light_the_game_knowledge_doesnt_classify() {
        let classified = ways_out(
            &corridor(&["Fast", "Pois", "Torch (3)", "Bat"], 0),
            "Human",
            true,
        );
        assert!(!classified.not_modelled.contains(&NotModelled::PlayerSpeed));
        let unclassified = ways_out(&corridor(&["Fast", "Zoom"], 0), "Human", true);
        assert!(
            unclassified
                .not_modelled
                .contains(&NotModelled::PlayerSpeed)
        );
    }

    #[test]
    fn a_stale_mutations_screen_is_played_and_labelled_stale() {
        let mut remembered = remembered(true);
        if let Remembered::Known(read) = std::mem::take(&mut remembered.mutations) {
            remembered.mutations = Remembered::Stale(read);
        }
        let visible = corridor(&[], 0);
        let ways_out = WaysOut::compute(&visible, Some(&remembered), Some("Human"), Some(bundle()));
        assert!(!ways_out.not_modelled.contains(&NotModelled::PlayerSpeed));
        assert_eq!(
            ways_out.pace.unwrap().mutations_read,
            Some(ReadOn {
                turn: Some(1),
                stale: true
            })
        );
    }
}
