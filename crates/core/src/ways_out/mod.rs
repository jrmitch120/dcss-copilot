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
use crate::fact_sheet::NotModelled;
use crate::gaps::Gap;
use crate::screen::KnownAbility;
use crate::visible_state::{Position, VisibleState};

/// The time a standard move or action takes the player: `BASELINE_DELAY`.
const STANDARD_TIME: u32 = BASELINE_DELAY.unsigned_abs();

/// What judging the ground leaves out, whenever it is judged.
const GROUND_NOT_MODELLED: [NotModelled; 2] = [
    NotModelled::SwimmingAndFlying,
    NotModelled::UnexploredSquares,
];

/// What the staircase Ways Out leave out besides, whenever there are any.
const STAIRS_NOT_MODELLED: [NotModelled; 2] =
    [NotModelled::HitsOnTheWay, NotModelled::MonstersOutOfView];

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
        /// Its square, or a square on the player's path to it, is out of view, so the Advisor
        /// judges it by what the map remembers there: a door may have closed since, or a monster
        /// may stand there now. Never so for one the player stands on.
        over_remembered_squares: bool,
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
    /// The ability menu greyed the ability out as one that can't be used now, for a reason the
    /// Advisor couldn't see, such as an empty deck, a sanctuary already in place or stasis.
    GreyedOut {
        /// When the menu showed it so.
        read: ReadOn,
    },
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

impl WayOut {
    /// Whether the Advisor trusts it: it can be used now, judged on what the game shows now
    /// rather than approximately, over remembered squares or from a Stale menu.
    #[must_use]
    pub fn trusted(&self) -> bool {
        let approximate = match &self.option {
            Escape::Stairs {
                over_remembered_squares,
                ..
            } => *over_remembered_squares,
            Escape::Item { .. } => false,
            Escape::Ability { read, .. } | Escape::Spell { read, .. } => read.stale,
        };
        self.usable == Usable::Yes && !approximate
    }
}

impl WaysOut {
    /// Whether any Way Out is trusted.
    #[must_use]
    pub fn any_trusted(&self) -> bool {
        self.options.iter().any(WayOut::trusted)
    }

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
            // Crawl itself checked an ability the menu showed while the list is Known.
            if ways_out.options.iter().any(|way_out| match way_out.option {
                Escape::Ability { read, .. } => read.stale,
                Escape::Spell { .. } => true,
                _ => false,
            }) {
                ways_out.not_modelled.push(NotModelled::OwnConditions);
            }
        }
        ways_out.gaps = gaps.into_iter().collect();
        ways_out
    }
}

/// Whether crawl greyed out `ability` in the ability menu for a reason the Advisor can't see,
/// given that it did: it is a Way Out, as `bundle` tells, and nothing `visible` shows explains
/// it, neither a status that stops it nor too little magic for it.
pub(crate) fn greyed_out_unseen(
    visible: &VisibleState,
    bundle: &Bundle,
    ability: &KnownAbility,
) -> bool {
    let carried = carried::Carried {
        visible,
        remembered: None,
        bundle,
    };
    carried.escape_ability(&ability.name).is_some_and(|escape| {
        carried
            .ability_stopped(escape, ability.magic_cost)
            .is_empty()
    })
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
    use crate::gaps::ScreenFact;
    use crate::screen::{KnownSpell, Screen, ScreenRead, ShownMutations};
    use crate::view_model::{Colour, Glyph, MarkedValue, WayOutRow};
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
                    shown: ShownMutations {
                        mutations: Vec::new(),
                        banes: Some(Vec::new()),
                    },
                    xl: Some(1),
                },
                turn: Some(1),
                source: Screen::Mutations.into(),
            });
        }
        remembered
    }

    /// The ability menu as crawl draws it showing `abilities`, each costing no magic, and drawn
    /// in the colour crawl greys an ability out in when it is.
    fn ability_menu(abilities: &[(&str, bool)]) -> ScreenRead {
        let greyed = Colour::from_crawl(bundle().ways_out.inapplicable_colour).unwrap();
        ScreenRead {
            screen: Screen::Abilities,
            title: None,
            lines: abilities
                .iter()
                .map(|(name, _)| format!(" a - {name:<30} None            0%"))
                .collect(),
            colours: abilities
                .iter()
                .enumerate()
                .filter(|(_, (_, greyed_out))| *greyed_out)
                .map(|(index, _)| (index, greyed))
                .collect(),
        }
    }

    /// What the Advisor remembers once it read `menu` as `visible` shows the game.
    fn read_on(menu: &ScreenRead, visible: &VisibleState) -> AccumulatedState {
        let mut remembered = remembered(true);
        remembered.read(menu, visible, Some(bundle()));
        remembered
    }

    /// The Ways Out of `visible` for a human, as the Advisor remembers the game in `remembered`.
    fn remembering_menu(visible: &VisibleState, remembered: &AccumulatedState) -> WaysOut {
        WaysOut::compute(visible, Some(remembered), Some("Human"), Some(bundle()))
    }

    /// The panel's row for the ability named `name` among the Ways Out of `visible`, as the
    /// Advisor remembers the game in `remembered`.
    fn ability_row(visible: &VisibleState, remembered: &AccumulatedState, name: &str) -> WayOutRow {
        remembering_menu(visible, remembered)
            .list()
            .rows
            .into_iter()
            .find(|row| row.name == name)
            .unwrap()
    }

    /// Whether the ability named `name` among the Ways Out of `visible` can be used, as the
    /// Advisor remembers the game in `remembered`.
    fn ability_usable(visible: &VisibleState, remembered: &AccumulatedState, name: &str) -> Usable {
        remembering_menu(visible, remembered)
            .options
            .into_iter()
            .find(|way_out| matches!(&way_out.option, Escape::Ability { name: listed, .. } if listed == name))
            .unwrap()
            .usable
    }

    /// `visible` on turn `turn`.
    fn on_turn(mut visible: VisibleState, turn: u32) -> VisibleState {
        visible.turn = Some(turn);
        visible
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

    /// `visible` with its map's squares redrawn by `redraw`, which drops one by returning `None`.
    fn redrawn(
        mut visible: VisibleState,
        redraw: impl Fn(Position, &MapCell) -> Option<MapCell>,
    ) -> VisibleState {
        let map = visible.map.take().unwrap();
        visible.map =
            Some(LevelMap::new(map.cells().filter_map(|(position, cell)| {
                Some((position, redraw(position, cell)?))
            })));
        visible
    }

    /// `visible` with the map's squares at x `out_of_view` out of view, as the game shows
    /// squares it remembers.
    fn remembering(visible: VisibleState, out_of_view: &[i32]) -> VisibleState {
        redrawn(visible, |position, cell| {
            Some(MapCell {
                in_view: !out_of_view.contains(&position.x),
                ..cell.clone()
            })
        })
    }

    /// The staircase among `ways_out`.
    fn stairs(ways_out: &WaysOut) -> &WayOut {
        ways_out
            .options
            .iter()
            .find(|way_out| matches!(way_out.option, Escape::Stairs { .. }))
            .unwrap()
    }

    /// Whether the staircase among `ways_out` is over remembered squares.
    fn over_remembered_squares(ways_out: &WaysOut) -> bool {
        match stairs(ways_out).option {
            Escape::Stairs {
                over_remembered_squares,
                ..
            } => over_remembered_squares,
            _ => unreachable!(),
        }
    }

    fn stairs_usable(ways_out: &WaysOut) -> bool {
        stairs(ways_out).usable == Usable::Yes
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

    /// The character dump lists mutations but never banes, such as lethargy, which slows them.
    #[test]
    fn player_speed_is_named_while_the_banes_are_unread_as_after_the_character_dump() {
        let mut remembered = remembered(true);
        if let Remembered::Known(read) = &mut remembered.mutations {
            read.value.shown.banes = None;
            read.source = Screen::CharacterDump.into();
        }
        let visible = corridor(&[], 0);
        let ways_out = WaysOut::compute(&visible, Some(&remembered), Some("Human"), Some(bundle()));
        assert!(ways_out.not_modelled.contains(&NotModelled::PlayerSpeed));
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

    #[test]
    fn a_staircase_over_squares_in_view_is_not_marked() {
        let in_view = ways_out(&corridor(&[], 0), "Human", true);
        assert!(!over_remembered_squares(&in_view));
        assert_eq!(
            in_view.list().rows[0]
                .detail
                .as_ref()
                .map(|detail| detail.text.as_str()),
            Some("6 steps")
        );
    }

    #[test]
    fn a_staircase_whose_path_crosses_a_remembered_square_is_marked_and_judged_alike() {
        let visible = corridor(&[], 0);
        for species in ["Human", "Naga"] {
            let in_view = ways_out(&visible, species, true);
            let remembered = ways_out(&remembering(visible.clone(), &[7]), species, true);
            assert!(over_remembered_squares(&remembered), "{species}");
            assert_eq!(
                stairs(&remembered).usable,
                stairs(&in_view).usable,
                "{species}"
            );
            assert_eq!(
                remembered.list().rows[0]
                    .detail
                    .as_ref()
                    .map(|detail| detail.text.as_str()),
                Some("~6 steps · partly out of view")
            );
        }
    }

    #[test]
    fn a_staircase_on_a_remembered_square_is_marked() {
        let remembered = ways_out(&remembering(corridor(&[], 0), &[10]), "Human", true);
        assert!(over_remembered_squares(&remembered));
    }

    #[test]
    fn a_staircase_the_player_stands_on_is_never_marked() {
        let mut visible = remembering(corridor(&[], 0), &(0..=10).collect::<Vec<_>>());
        visible.position = Some(Position { x: 10, y: 0 });
        assert!(!over_remembered_squares(&ways_out(&visible, "Human", true)));
    }

    #[test]
    fn monsters_out_of_view_are_named_only_while_a_staircase_is_listed() {
        let with_stairs = ways_out(&corridor(&[], 0), "Human", true);
        assert!(
            with_stairs
                .not_modelled
                .contains(&NotModelled::MonstersOutOfView)
        );
        let visible = redrawn(corridor(&[], 0), |position, cell| {
            (position.x < 10).then(|| cell.clone())
        });
        let without = ways_out(&visible, "Human", true);
        assert!(without.ground.is_some());
        assert!(without.options.is_empty());
        assert!(
            !without
                .not_modelled
                .contains(&NotModelled::MonstersOutOfView)
        );
    }

    #[test]
    fn an_ability_is_stopped_by_the_statuses_that_stop_it_in_particular() {
        let menu = ability_menu(&[
            ("Evoke Blink", false),
            ("Hop", false),
            ("Blinkbolt", false),
            ("Berserk", false),
        ]);
        let remembered = read_on(&menu, &corridor(&[], 0));
        for (ability, light) in [
            ("Evoke Blink", "-Blink"),
            ("Hop", "-Hop"),
            ("Blinkbolt", "-Bbolt"),
            ("Berserk", "-Berserk"),
        ] {
            assert_eq!(
                ability_usable(&corridor(&[], 0), &remembered, ability),
                Usable::Yes,
                "{ability}"
            );
            assert_eq!(
                ability_usable(&corridor(&[light], 0), &remembered, ability),
                Usable::No {
                    because: vec![Blocker::Status {
                        status: light.to_owned()
                    }]
                },
                "{ability}"
            );
        }
        assert_eq!(
            ability_usable(&corridor(&["-Hop"], 0), &remembered, "Evoke Blink"),
            Usable::Yes
        );
    }

    #[test]
    fn an_ability_greyed_out_for_no_reason_in_view_is_stopped_and_goes_stale_with_the_list() {
        let visible = on_turn(corridor(&[], 0), 5);
        let mut remembered = read_on(&ability_menu(&[("Berserk", true)]), &visible);
        let known = ability_row(&visible, &remembered, "Berserk");
        assert!(!known.usable);
        assert_eq!(
            known.why_not,
            Some(MarkedValue::known(
                "greyed out in the ability menu as of turn 5".to_owned()
            ))
        );
        let mut levelled = visible.clone();
        levelled.stats.xl = Some(2);
        remembered.update(&visible, &levelled, Some(bundle()));
        let stale = ability_row(&levelled, &remembered, "Berserk");
        assert!(!stale.usable);
        let menu_out_of_date = "~Ability menu as of turn 5: open it (a) to bring it up to date.";
        assert_eq!(
            stale.why_not,
            Some(MarkedValue::marked(
                "~greyed out in the ability menu as of turn 5".to_owned(),
                [menu_out_of_date.to_owned()]
            ))
        );
        assert_eq!(
            stale.detail.and_then(|detail| detail.gap).as_deref(),
            Some(menu_out_of_date)
        );
    }

    #[test]
    fn an_ability_greyed_out_by_a_status_in_view_is_usable_once_the_status_ends() {
        let cooling_down = on_turn(corridor(&["-Berserk"], 0), 5);
        let remembered = read_on(&ability_menu(&[("Berserk", true)]), &cooling_down);
        assert_eq!(
            ability_row(&cooling_down, &remembered, "Berserk")
                .why_not
                .map(|why_not| why_not.text)
                .as_deref(),
            Some("you're -Berserk")
        );
        let recovered = on_turn(corridor(&[], 0), 30);
        assert_eq!(
            ability_usable(&recovered, &remembered, "Berserk"),
            Usable::Yes
        );
    }

    #[test]
    fn an_ability_greyed_out_for_too_little_magic_is_usable_once_the_player_has_it() {
        let mut drained = on_turn(corridor(&[], 0), 5);
        drained.stats.mp = Some(0);
        let mut menu = ability_menu(&[("Evoke Blink", true)]);
        menu.lines = vec![format!(" a - {:<30} 1 MP            0%", "Evoke Blink")];
        let remembered = read_on(&menu, &drained);
        assert_eq!(
            ability_usable(&drained, &remembered, "Evoke Blink"),
            Usable::No {
                because: vec![Blocker::Magic { needs: 1, has: 0 }]
            }
        );
        let mut rested = drained;
        rested.stats.mp = Some(3);
        assert_eq!(
            ability_usable(&rested, &remembered, "Evoke Blink"),
            Usable::Yes
        );
    }

    #[test]
    fn crawls_own_conditions_are_named_for_a_stale_ability_list_or_a_spell() {
        let visible = corridor(&[], 0);
        let mut remembered = read_on(&ability_menu(&[("Berserk", false)]), &visible);
        let own_conditions = |remembered: &AccumulatedState, visible: &VisibleState| {
            remembering_menu(visible, remembered)
                .not_modelled
                .contains(&NotModelled::OwnConditions)
        };
        assert!(!own_conditions(&remembered, &visible));
        let mut levelled = visible.clone();
        levelled.stats.xl = Some(2);
        remembered.update(&visible, &levelled, Some(bundle()));
        assert!(own_conditions(&remembered, &levelled));
        let mut with_a_spell = read_on(&ability_menu(&[("Berserk", false)]), &visible);
        with_a_spell.spells = Remembered::Known(AsOf {
            value: vec![KnownSpell {
                name: "Blink".to_owned(),
                failure_percent: 10,
                level: 2,
            }],
            turn: Some(1),
            source: Screen::Spells.into(),
        });
        assert!(own_conditions(&with_a_spell, &visible));
    }

    #[test]
    fn an_ability_menu_or_a_spell_list_out_of_date_is_a_gap() {
        let visible = corridor(&[], 0);
        let mut remembered = read_on(&ability_menu(&[("Berserk", false)]), &visible);
        remembered.spells = Remembered::Known(AsOf {
            value: vec![KnownSpell {
                name: "Blink".to_owned(),
                failure_percent: 10,
                level: 2,
            }],
            turn: Some(1),
            source: Screen::Spells.into(),
        });
        let gaps = |remembered: &AccumulatedState, visible: &VisibleState| {
            remembering_menu(visible, remembered).gaps
        };
        assert_eq!(gaps(&remembered, &visible), []);
        let mut levelled = visible.clone();
        levelled.stats.xl = Some(2);
        remembered.update(&visible, &levelled, Some(bundle()));
        assert_eq!(
            gaps(&remembered, &levelled),
            [
                Gap::Stale {
                    fact: ScreenFact::SpellList,
                    turn: Some(1)
                },
                Gap::Stale {
                    fact: ScreenFact::AbilityMenu,
                    turn: visible.turn
                },
            ]
        );
    }
}
