//! The walk to a staircase's Monte Carlo: the player walking their path, action by action, and the
//! hostiles coming after them as the Threat Level plays them, through crawl's own dice, until the
//! player arrives or falls, many times over.

use copilot_gk::Formulas;

use super::melee::BASELINE_DELAY;
use super::rng::Rng;
use super::stage::{Ground, Scene, Stage};
use super::step::Offset;
use super::threat::{self, Attacker, Defender, Engaged};
use crate::visible_state::Position;

/// How many walks each staircase plays out, as many as the fights its result is weighed against
/// ([`super::fight::TRIALS`]): enough to tell most walks from the 90% line.
pub(crate) const TRIALS: u32 = 2_000;

/// The seed every walk starts from, so the same Game Situation always gives the same Fact Sheet.
const SEED: (u64, u64) = (0x00dc_55c0_9170_7156, 0x0000_0000_0000_0156);

/// How many more actions than its path asks a walk may take, waiting on a monster in the way,
/// before it counts as not made: the player won't stand and wait to be killed. A tuned number
/// (#156).
const MOST_WAITING_ACTIONS: usize = 3;

/// One of the player's actions on a walk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct WalkAction {
    /// The square it brings them to; `None` for opening a door, which leaves them where they are.
    pub(crate) to: Option<Position>,
    /// How long it takes, where 10 is a standard action.
    pub(crate) time: i32,
}

/// Plays out the player, defended as `defender`, walking `actions` from `start`, with `attackers`
/// coming after them on `scene`, [`TRIALS`] times: how many walks they survive to the end.
pub(crate) fn survived(
    formulas: Formulas,
    defender: Defender,
    start: Position,
    actions: &[WalkAction],
    attackers: &[Attacker<'_>],
    scene: &Scene,
) -> u32 {
    let order = threat::in_order_of_play(attackers);
    let kinds = || order.iter().map(|attacker| attacker.monster);
    // Where the player stands decides every monster's path, so each square has its own ground.
    let start_ground = Ground::new(scene, start, kinds(), &[]);
    let grounds: Vec<Option<Ground<'_>>> = actions
        .iter()
        .map(|action| action.to.map(|to| Ground::new(scene, to, kinds(), &[])))
        .collect();
    let stage = Stage::new(
        &start_ground,
        order
            .iter()
            .map(|attacker| (attacker.monster, attacker.offset)),
    );
    let mut rng = Rng::new(SEED.0, SEED.1);
    let walk = Walk {
        formulas,
        defender,
        start,
        actions,
        grounds: &grounds,
    };
    (0..TRIALS)
        .map(|_| u32::from(walk.one(&mut rng, &order, stage.clone())))
        .sum()
}

/// What every trial of one walk shares.
struct Walk<'w, 's> {
    formulas: Formulas,
    defender: Defender,
    start: Position,
    actions: &'w [WalkAction],
    grounds: &'w [Option<Ground<'s>>],
}

impl<'s> Walk<'_, 's> {
    /// Plays out one walk: whether the player gets there alive.
    fn one<'g>(&'g self, rng: &mut Rng, order: &[&Attacker<'_>], mut stage: Stage<'g, '_>) -> bool
    where
        's: 'g,
    {
        let formulas = self.formulas;
        let target = self.defender.target(rng);
        let mut engaged: Vec<Engaged<'_, '_>> = order
            .iter()
            .enumerate()
            .map(|(place, attacker)| Engaged::new(formulas, rng, attacker, 0, place))
            .collect();
        let mut hp = self.defender.hp;
        let mut at = self.start;
        let mut waits = 0;
        let mut next = self.actions.iter().zip(self.grounds).peekable();
        while let Some(&(action, ground)) = next.peek() {
            let time = match (action.to, ground) {
                (Some(to), Some(ground)) => {
                    let step = Offset::between(at, to);
                    if stage.taken_by_a_monster(step) {
                        // Someone stands in the way: they wait a moment, and try again.
                        waits += 1;
                        if waits > MOST_WAITING_ACTIONS {
                            return false;
                        }
                        BASELINE_DELAY
                    } else {
                        stage.player_stepped(ground, step);
                        at = to;
                        next.next();
                        action.time
                    }
                }
                _ => {
                    next.next();
                    action.time
                }
            };
            let mut blocks_left = self.defender.shield_blocks;
            for hostile in &mut engaged {
                if hostile.gone {
                    continue;
                }
                hostile.gain(formulas, rng, time);
                hp -= hostile
                    .act(formulas, rng, target, hp, &mut blocks_left, &mut stage)
                    .damage;
                if hp <= 0 {
                    return false;
                }
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use copilot_gk::{Bundle, Library, Terrain};

    use super::*;
    use crate::engine::lighting::Lighting;
    use crate::engine::melee::DisplayedDefences;
    use crate::engine::paths::Level;
    use crate::engine::resists::Resists;

    fn bundle() -> &'static Bundle {
        static LIBRARY: std::sync::OnceLock<Library> = std::sync::OnceLock::new();
        LIBRARY
            .get_or_init(|| Library::shipped().unwrap())
            .for_version("0.34.1")
            .unwrap()
    }

    fn defender(hp: i32) -> Defender {
        Defender {
            hp,
            defences: DisplayedDefences {
                ac: 2,
                ev: 10,
                sh: 0,
            },
            lighting: Lighting::Plain,
            shield_blocks: 1,
            resists: Resists::NONE,
        }
    }

    /// An open corridor along row 0, its first square the player's, and the actions walking
    /// `squares` west along it, each taking a standard action.
    fn corridor(squares: i32) -> (Scene, Vec<WalkAction>) {
        let width = squares + 12;
        let terrain = (-squares..=12).flat_map(|x| {
            (-1..=1).map(move |y| {
                let terrain = if y == 0 {
                    Terrain::Open
                } else {
                    Terrain::Solid
                };
                (Position { x, y }, terrain)
            })
        });
        let scene = Scene {
            player: Position { x: 0, y: 0 },
            level: Level::new(terrain),
            bystanders: Vec::new(),
        };
        assert!(width > 0);
        let actions = (1..=squares)
            .map(|x| WalkAction {
                to: Some(Position { x: -x, y: 0 }),
                time: 10,
            })
            .collect();
        (scene, actions)
    }

    /// The walks survived, of [`TRIALS`], on `hp` over `squares` with `name` `distance` behind.
    fn walks(hp: i32, squares: i32, name: &str, distance: i32) -> u32 {
        let (scene, actions) = corridor(squares);
        let monster = bundle().monsters.values().find(|m| m.name == name).unwrap();
        let chaser = Attacker::new(bundle(), monster, Offset::east(distance)).without_gear();
        survived(
            bundle().formulas,
            defender(hp),
            scene.player,
            &actions,
            &[chaser],
            &scene,
        )
    }

    #[test]
    fn a_walk_away_from_a_monster_no_faster_than_the_player_is_survived() {
        assert_eq!(walks(5, 30, "orc warrior", 4), TRIALS);
    }

    #[test]
    fn a_long_walk_from_something_faster_than_the_player_kills_them_and_a_short_one_doesnt() {
        // Jackals, at speed 15, catch a player with a head start of a few squares.
        let long = walks(6, 40, "jackal", 3);
        let short = walks(6, 3, "jackal", 3);
        assert!(long < TRIALS / 2, "{long}");
        assert_eq!(short, TRIALS);
    }

    #[test]
    fn a_walk_with_no_steps_is_always_survived() {
        assert_eq!(walks(1, 0, "ogre", 1), TRIALS);
    }

    #[test]
    fn a_monster_on_the_path_ends_a_walk_it_never_leaves() {
        let (scene, actions) = corridor(6);
        let plant = bundle()
            .monsters
            .values()
            .find(|m| m.name == "plant")
            .unwrap();
        let in_the_way = Attacker::new(bundle(), plant, Offset { x: -3, y: 0 }).without_gear();
        let made = survived(
            bundle().formulas,
            defender(50),
            scene.player,
            &actions,
            &[in_the_way],
            &scene,
        );
        assert_eq!(made, 0);
    }
}
