//! Ways Out: the escapes and emergency options open to the player at a moment, each with
//! whether it can be used right now and why not, and the ground they stand on.

mod carried;
mod level;
mod list;

use std::collections::BTreeSet;

use copilot_gk::{Bundle, Relief, StairDirection};
use serde::Serialize;

use crate::accumulated_state::AccumulatedState;
use crate::fact_sheet::{Gap, NotModelled};
use crate::visible_state::{Position, VisibleState};

/// What judging the ground leaves out, whenever it is judged.
const GROUND_NOT_MODELLED: [NotModelled; 2] = [
    NotModelled::SwimmingAndFlying,
    NotModelled::UnexploredSquares,
];

/// What the staircase Ways Out leave out besides, whenever there are any.
const STAIRS_NOT_MODELLED: [NotModelled; 2] = [NotModelled::PlayerSpeed, NotModelled::HitsOnTheWay];

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
    /// Crawl greys the item out as of no use to the player right now, for a reason the
    /// Advisor doesn't see.
    Useless,
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
    /// The Ways Out the engine finds in `visible` and what the Advisor remembers of the game,
    /// with `knowledge` for the game's Crawl Version if the Advisor has it.
    pub(crate) fn compute(
        visible: &VisibleState,
        remembered: Option<&AccumulatedState>,
        knowledge: Option<&Bundle>,
    ) -> Self {
        let mut gaps = BTreeSet::new();
        let mut ways_out = Self::default();
        match level::on_the_level(visible, knowledge) {
            Ok(here) => {
                ways_out.ground = Some(here.ground());
                ways_out.not_modelled.extend(GROUND_NOT_MODELLED);
                ways_out.options.extend(here.stairs(&mut gaps));
                if !ways_out.options.is_empty() {
                    ways_out.not_modelled.extend(STAIRS_NOT_MODELLED);
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
