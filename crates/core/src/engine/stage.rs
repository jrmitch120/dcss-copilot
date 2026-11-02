//! Where a fight is played: the level and who stands on it, in one trial. Crawl's rules for how a
//! monster gets to the player, ported closely (ADR 0002) behind the formula variant a Game
//! Knowledge bundle names.
//!
//! A monster with a clear line to the player heads straight for them (`_find_best_step`,
//! `mon-act.cc:501`); one without heads along the shortest path round what walls it off
//! (`try_pathfind`, `mon-movetarget.cc:87`). With that square taken it tries the squares to either
//! side of it, then those two away, for one no farther from where it heads
//! (`_find_good_alternate_move`, `mon-act.cc:3460`), or else pushes past an ally that lets it
//! (`_mons_can_displace`, `mon-act.cc:3102`), and with none it waits, the time passing all the
//! same (`_monster_move`).

use copilot_gk::{Formulas, Intelligence, Monster, MonsterPaths};

use super::paths::{Crossing, Field, Level};
use super::rng::Rng;
use super::step::Offset;
use crate::crawl_ids::{genera, holinesses, monster_types};
use crate::visible_state::Position;

/// The eight steps from a square, in order round the compass: crawl's `mon_compass`.
const COMPASS: [Offset; 8] = [
    Offset { x: -1, y: -1 },
    Offset { x: 0, y: -1 },
    Offset { x: 1, y: -1 },
    Offset { x: 1, y: 0 },
    Offset { x: 1, y: 1 },
    Offset { x: 0, y: 1 },
    Offset { x: -1, y: 1 },
    Offset { x: -1, y: 0 },
];

/// The kinds that never push past another (`monster_shover`, `mon-util.cc`), though they could.
const NEVER_SHOVES: [&str; 4] = [
    monster_types::FIRE_ELEMENTAL,
    monster_types::CURSE_TOE,
    monster_types::GERYON,
    monster_types::ROBIN,
];

/// The dice for one step: `None` plays it at its most generous, to say what a monster may do.
type Dice<'r> = Option<&'r mut Rng>;

/// How much stronger an unrelated monster must be to push past another, this step: `random2(11)`,
/// but no more than 5 (`monster_senior`, `mon-util.cc`).
fn margin(dice: &mut Dice<'_>) -> i64 {
    dice.as_mut()
        .map_or(0, |rng| i64::from(rng.random2(11).min(5)))
}

/// Whether crawl counts `monster` as demonic, among the holinesses of its kind.
fn demonic(monster: &Monster) -> bool {
    monster
        .holiness
        .iter()
        .any(|holiness| holiness.0 == holinesses::DEMONIC)
}

/// What a fight is played on, whoever is in it: where the player stands, the level as they know
/// it, if the game has sent it, and the squares of the monsters that take no part.
#[derive(Debug, Clone)]
pub(crate) struct Scene {
    /// Where the player stands.
    pub(crate) player: Position,
    /// The level: `None` for a moment whose map the game hasn't sent, played as open floor.
    pub(crate) level: Option<Level>,
    /// The monsters in view that aren't played, such as the player's allies: they stand where
    /// they are, in the way.
    pub(crate) bystanders: Vec<Position>,
    /// The player's experience level, which a tracer weighs them by: the highest there is while
    /// the game hasn't shown it, so no monster holds its fire for not knowing it.
    pub(crate) experience_level: i32,
}

/// The highest experience level there is, `MAX_EXP_LEVEL`.
pub(crate) const MOST_EXPERIENCE_LEVEL: i32 = 27;

/// The ground under one play of a moment, with the player standing on one square of it: how a
/// monster of each [`Crossing`] among those playing gets to them. It doesn't change from trial to
/// trial.
#[derive(Debug, Clone)]
pub(crate) struct Ground<'s> {
    level: Option<&'s Level>,
    player: Position,
    fields: Vec<(Crossing, Field)>,
    fixed: Vec<Position>,
    experience_level: i32,
}

impl<'s> Ground<'s> {
    /// `scene` with the player standing at `player`, for monsters of `kinds`, with `fixed` more
    /// squares that stay taken.
    pub(crate) fn new<'m>(
        scene: &'s Scene,
        player: Position,
        kinds: impl IntoIterator<Item = &'m Monster>,
        fixed: &[Position],
    ) -> Self {
        let mut fields: Vec<(Crossing, Field)> = Vec::new();
        if let Some(level) = &scene.level {
            for monster in kinds {
                let crossing = Crossing::of(monster);
                if !fields.iter().any(|(known, _)| *known == crossing) {
                    fields.push((crossing, level.field_to(crossing, player)));
                }
            }
        }
        Self {
            level: scene.level.as_ref(),
            player,
            fields,
            fixed: scene.bystanders.iter().chain(fixed).copied().collect(),
            experience_level: scene.experience_level,
        }
    }

    /// Open floor all round, with nothing else on it.
    pub(crate) fn open_floor() -> Self {
        Self {
            level: None,
            player: Position { x: 0, y: 0 },
            fields: Vec::new(),
            fixed: Vec::new(),
            experience_level: MOST_EXPERIENCE_LEVEL,
        }
    }

    /// The same ground, with a player of `experience_level`.
    #[cfg(test)]
    pub(crate) fn with_experience_level(self, experience_level: i32) -> Self {
        Self {
            experience_level,
            ..self
        }
    }

    fn at(&self, offset: Offset) -> Position {
        Position {
            x: self.player.x + offset.x,
            y: self.player.y + offset.y,
        }
    }

    fn field(&self, crossing: Crossing) -> Option<&Field> {
        self.fields
            .iter()
            .find(|(known, _)| *known == crossing)
            .map(|(_, field)| field)
    }

    /// Whether a monster that crosses as `crossing` could stand at `offset`, whoever is there.
    fn can_stand(&self, crossing: Crossing, offset: Offset) -> bool {
        offset != Offset::PLAYER
            && self
                .level
                .is_none_or(|level| level.moves_onto(crossing, self.at(offset)).is_some())
            && !self.fixed.contains(&self.at(offset))
    }

    /// The moves stepping onto `offset` takes.
    fn moves_onto(&self, crossing: Crossing, offset: Offset) -> i32 {
        self.level
            .and_then(|level| level.moves_onto(crossing, self.at(offset)))
            .map_or(1, i32::from)
    }

    /// Whether a monster two squares from the player at `offset` can reach them past what stands
    /// between: anywhere on open floor.
    pub(crate) fn reach_clear(&self, offset: Offset) -> bool {
        self.level
            .is_none_or(|level| level.reach_clear(self.at(offset), self.player))
    }

    /// Whether a monster that crosses as `crossing`, at `offset`, has no way to the player at all:
    /// walled off, as in a vault with no door.
    pub(crate) fn cut_off(&self, crossing: Crossing, offset: Offset) -> bool {
        let Some(level) = self.level else {
            return false;
        };
        let here = self.at(offset);
        level.moves_onto(crossing, here).is_some()
            && self
                .field(crossing)
                .is_some_and(|field| field.moves(level, here).is_none())
    }

    /// Whether a monster at `offset` sees the player past the walls, on some line between them:
    /// anywhere on open floor.
    pub(crate) fn sees_player(&self, offset: Offset) -> bool {
        self.level
            .is_none_or(|level| level.sees(self.at(offset), self.player))
    }

    /// How close to the player `offset` is, for a monster that crosses as `crossing` and has a
    /// straight run at them or not: the square of the distance straight, as crawl's
    /// `distance2`, and otherwise its moves along the shortest path. Lower is nearer.
    fn nearness(&self, crossing: Crossing, straight: bool, offset: Offset) -> Option<i32> {
        if straight {
            return Some(offset.x * offset.x + offset.y * offset.y);
        }
        let moves = self.field(crossing)?.moves(self.level?, self.at(offset))?;
        Some(i32::from(moves))
    }

    /// The square a monster at `from` that crosses as `crossing` would step to if nothing stood
    /// on it: straight at the player with a clear line to them; otherwise the neighbour nearest
    /// them along the shortest path. Straight at them too when no way is known, such as for one
    /// that stands in rock, which goes through what walls others off; and when there is none,
    /// nowhere.
    fn wanted(&self, crossing: Crossing, from: Offset) -> Option<(Offset, bool)> {
        let Some(level) = self.level else {
            return Some((from.nearer(), true));
        };
        let here = self.at(from);
        if level.moves_onto(crossing, here).is_none()
            || level.clear_line(crossing, here, self.player)
        {
            return Some((from.nearer(), true));
        }
        let field = self.field(crossing)?;
        field.moves(level, here)?;
        let straight = from.nearer();
        COMPASS
            .iter()
            .map(|&step| from + step)
            .filter(|&square| square != Offset::PLAYER)
            .filter_map(|square| Some((square, field.moves(level, self.at(square))?)))
            // Of those alike, the one nearest the straight line.
            .min_by_key(|&(square, moves)| {
                (
                    moves,
                    (square.x - straight.x).abs() + (square.y - straight.y).abs(),
                )
            })
            .map(|(square, _)| (square, false))
    }
}

/// Who stands where in one trial, as the player acts and the monsters move.
#[derive(Debug, Clone)]
pub(crate) struct Stage<'g, 'm> {
    ground: &'g Ground<'g>,
    /// Each monster playing, by its place.
    monsters: Vec<&'m Monster>,
    /// Where each stands from the player, by its place: `None` once it is gone.
    standing: Vec<Option<Offset>>,
}

impl<'g, 'm> Stage<'g, 'm> {
    /// `monsters`, each with where it stands from the player, on `ground`. Each one's place among
    /// them is how it is known.
    pub(crate) fn new(
        ground: &'g Ground<'g>,
        monsters: impl IntoIterator<Item = (&'m Monster, Offset)>,
    ) -> Self {
        let (monsters, standing) = monsters
            .into_iter()
            .map(|(monster, offset)| (monster, Some(offset)))
            .unzip();
        Self {
            ground,
            monsters,
            standing,
        }
    }

    /// Where the monster at `place` stands from the player, while it is there.
    pub(crate) fn standing(&self, place: usize) -> Option<Offset> {
        self.standing.get(place).copied().flatten()
    }

    /// The ground it is played on.
    pub(crate) fn ground(&self) -> &'g Ground<'g> {
        self.ground
    }

    /// The monster at `place` is gone, and no longer in the way.
    pub(crate) fn leave(&mut self, place: usize) {
        if let Some(standing) = self.standing.get_mut(place) {
            *standing = None;
        }
    }

    /// The monster at `place` is at `offset`, as a spell landed it.
    pub(crate) fn put(&mut self, place: usize, offset: Offset) {
        if let Some(standing) = self.standing.get_mut(place) {
            *standing = Some(offset);
        }
    }

    /// The player has stepped by `step` onto the square `ground` has them on: everyone stands that
    /// much otherwise from them.
    pub(crate) fn player_stepped(&mut self, ground: &'g Ground<'g>, step: Offset) {
        self.ground = ground;
        for standing in self.standing.iter_mut().flatten() {
            *standing = *standing + -step;
        }
    }

    /// Whether a monster playing stands at `offset`.
    pub(crate) fn taken_by_a_monster(&self, offset: Offset) -> bool {
        self.standing.contains(&Some(offset))
    }

    /// The player's experience level.
    pub(crate) fn player_experience_level(&self) -> i32 {
        self.ground.experience_level
    }

    /// The monster at `place`.
    pub(crate) fn monster(&self, place: usize) -> Option<&'m Monster> {
        self.monsters.get(place).copied()
    }

    /// The places of the monsters within `radius` squares of `centre`.
    pub(crate) fn within(&self, centre: Offset, radius: i32) -> impl Iterator<Item = usize> + '_ {
        self.standing
            .iter()
            .enumerate()
            .filter(move |&(_, standing)| {
                standing.is_some_and(|at| {
                    let from_centre = Offset {
                        x: at.x - centre.x,
                        y: at.y - centre.y,
                    };
                    from_centre.distance() <= radius
                })
            })
            .map(|(place, _)| place)
    }

    /// The monster playing at `offset`, other than the one at `place`.
    pub(crate) fn occupant(&self, place: usize, offset: Offset) -> Option<usize> {
        self.standing
            .iter()
            .enumerate()
            .find(|&(other, standing)| other != place && *standing == Some(offset))
            .map(|(other, _)| other)
    }

    /// Whether the monster at `place` can step onto `offset`, with no one there.
    fn free(&self, place: usize, crossing: Crossing, offset: Offset) -> bool {
        self.ground.can_stand(crossing, offset) && self.occupant(place, offset).is_none()
    }

    fn crossing(&self, place: usize) -> Option<Crossing> {
        Some(Crossing::of(self.monsters.get(place)?))
    }

    /// Whether the monster at `place` may push past the one at `other` this step:
    /// `_mons_can_displace` (`mon-act.cc:3102`) and `monster_senior` (`mon-util.cc`), as far as
    /// the engine knows them.
    fn displaces(
        &self,
        variant: MonsterPaths,
        place: usize,
        other: usize,
        dice: &mut Dice<'_>,
    ) -> bool {
        let (Some(&mover), Some(&pushed)) = (self.monsters.get(place), self.monsters.get(other))
        else {
            return false;
        };
        match variant {
            MonsterPaths::Crawl034 => {
                let shoves =
                    mover.intelligence > Intelligence::Brainless || mover.genus.0 == genera::JELLY;
                let held =
                    mover.genus.0 == genera::EFREET || NEVER_SHOVES.contains(&mover.id.0.as_str());
                if !shoves
                    || held
                    || mover.stationary
                    || pushed.stationary
                    || mover.batty
                    || pushed.batty
                {
                    return false;
                }
                let related = mover.genus == pushed.genus || demonic(mover) && demonic(pushed);
                let (senior, junior) = (i64::from(mover.hd), i64::from(pushed.hd));
                related && senior > junior || senior > junior + margin(dice)
            }
        }
    }

    /// The monster at `place` steps toward the player if it can, and returns the moves that took:
    /// a step that finds no way still passes the time of one.
    pub(crate) fn step_toward_player(
        &mut self,
        formulas: Formulas,
        rng: &mut Rng,
        place: usize,
    ) -> i32 {
        let Some(from) = self.standing(place) else {
            return 1;
        };
        let Some(next) = self.next_step(formulas.monster_paths, place, from, &mut Some(&mut *rng))
        else {
            return 1;
        };
        if let Some(other) = self.occupant(place, next) {
            // It rolls again as it swaps places (`_monster_swaps_places`), having rolled as it
            // found the move good (`mon_can_move_to_pos`), and fails with its mind made up.
            if !self.displaces(formulas.monster_paths, place, other, &mut Some(&mut *rng)) {
                return 1;
            }
            self.put(other, from);
        }
        self.put(place, next);
        self.crossing(place)
            .map_or(1, |crossing| self.ground.moves_onto(crossing, next))
    }

    /// Whether the monster at `place` could step onto `square`, pushing past whoever is there if
    /// it can.
    fn good_move(
        &self,
        variant: MonsterPaths,
        place: usize,
        crossing: Crossing,
        square: Offset,
        dice: &mut Dice<'_>,
    ) -> bool {
        self.ground.can_stand(crossing, square)
            && self
                .occupant(place, square)
                .is_none_or(|other| self.displaces(variant, place, other, dice))
    }

    /// The square the monster at `place`, at `from`, steps to next: where it wants to go, or else
    /// the nearer of the squares to either side of that, then of those two away, while no farther
    /// off than it stands (`_find_good_alternate_move`).
    fn next_step(
        &self,
        variant: MonsterPaths,
        place: usize,
        from: Offset,
        dice: &mut Dice<'_>,
    ) -> Option<Offset> {
        let crossing = self.crossing(place)?;
        let (wanted, straight) = self.ground.wanted(crossing, from)?;
        if self.good_move(variant, place, crossing, wanted, dice) {
            return Some(wanted);
        }
        let step = Offset {
            x: wanted.x - from.x,
            y: wanted.y - from.y,
        };
        let dir = COMPASS.iter().position(|&known| known == step)?;
        let current = self.ground.nearness(crossing, straight, from)?;
        for away in 1..=2_usize {
            // `random_choose(j, -j)`: which side it looks to first decides between two alike.
            let left_first = dice.as_mut().is_none_or(|rng| rng.one_chance_in(2));
            let turns = if left_first {
                [dir + 8 - away, dir + away]
            } else {
                [dir + away, dir + 8 - away]
            };
            let mut best: Option<(i32, Offset)> = None;
            for turn in turns {
                let square = from + COMPASS.get(turn % 8).copied()?;
                if !self.good_move(variant, place, crossing, square, dice) {
                    continue;
                }
                let Some(near) = self.ground.nearness(crossing, straight, square) else {
                    continue;
                };
                if near <= current && best.is_none_or(|(known, _)| near < known) {
                    best = Some((near, square));
                }
            }
            if let Some((_, square)) = best {
                return Some(square);
            }
        }
        None
    }

    /// The monster at `place` moves to `to`, if it can stand there and no one does; the moves it
    /// took.
    pub(crate) fn step_to(&mut self, place: usize, to: Offset) -> i32 {
        let Some(crossing) = self.crossing(place) else {
            return 1;
        };
        if self.free(place, crossing, to) {
            self.put(place, to);
            self.ground.moves_onto(crossing, to)
        } else {
            1
        }
    }

    /// Where the monster at `place`, at `from`, could step next at best.
    pub(crate) fn may_step_to(
        &self,
        variant: MonsterPaths,
        place: usize,
        from: Offset,
    ) -> Option<Offset> {
        self.next_step(variant, place, from, &mut None)
    }

    /// Whether the monster at `place` can step onto `to` this moment, with no one there.
    pub(crate) fn may_stand(&self, place: usize, to: Offset) -> bool {
        self.crossing(place)
            .is_some_and(|crossing| self.free(place, crossing, to))
    }

    /// The moves stepping onto `to` takes the monster at `place`.
    pub(crate) fn moves_onto(&self, place: usize, to: Offset) -> i32 {
        self.crossing(place)
            .map_or(1, |crossing| self.ground.moves_onto(crossing, to))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::OnceLock;

    use copilot_gk::{Bundle, Library, Terrain};

    use super::*;

    fn bundle() -> &'static Bundle {
        static LIBRARY: OnceLock<Library> = OnceLock::new();
        LIBRARY
            .get_or_init(|| Library::shipped().unwrap())
            .for_version("0.34.1")
            .unwrap()
    }

    fn monster(name: &str) -> &'static Monster {
        bundle().monsters.values().find(|m| m.name == name).unwrap()
    }

    /// A scene from `rows`: `@` is the player, `#` solid, ` ` unexplored, anything else open.
    fn scene(rows: &[&str]) -> Scene {
        let mut player = Position { x: 0, y: 0 };
        let mut squares = Vec::new();
        for (row, y) in rows.iter().zip(0..) {
            for (square, x) in row.chars().zip(0..) {
                let position = Position { x, y };
                match square {
                    ' ' => continue,
                    '#' => squares.push((position, Terrain::Solid)),
                    '@' => {
                        player = position;
                        squares.push((position, Terrain::Open));
                    }
                    _ => squares.push((position, Terrain::Open)),
                }
            }
        }
        Scene {
            player,
            level: Level::new(squares),
            bystanders: Vec::new(),
            experience_level: MOST_EXPERIENCE_LEVEL,
        }
    }

    /// Each monster's steps toward the player, in turn, `rounds` times over, from `standing`.
    fn walked(
        scene: &Scene,
        standing: &[(&'static str, Offset)],
        rounds: usize,
    ) -> Vec<Option<Offset>> {
        let kinds: Vec<&Monster> = standing.iter().map(|&(name, _)| monster(name)).collect();
        let ground = Ground::new(scene, scene.player, kinds.iter().copied(), &[]);
        let mut stage = Stage::new(
            &ground,
            standing
                .iter()
                .map(|&(name, offset)| (monster(name), offset)),
        );
        let mut rng = Rng::new(1, 2);
        for _ in 0..rounds {
            for place in 0..standing.len() {
                let beside = stage.standing(place).is_some_and(|at| at.distance() == 1);
                if !beside {
                    stage.step_toward_player(bundle().formulas, &mut rng, place);
                }
            }
        }
        (0..standing.len())
            .map(|place| stage.standing(place))
            .collect()
    }

    #[test]
    fn a_monster_walks_round_a_wall_it_cant_see_past() {
        let scene = scene(&[
            "#########",
            "#...#...#",
            "#.@.#...#",
            "#...#...#",
            "#.......#",
            "#########",
        ]);
        let orc = [("orc", Offset { x: 4, y: 0 })];
        // Down round the end of the wall, then beside the player.
        assert_eq!(walked(&scene, &orc, 2)[0].map(Offset::distance), Some(2));
        assert_eq!(walked(&scene, &orc, 3)[0].map(Offset::distance), Some(1));
    }

    #[test]
    fn a_monster_with_no_way_to_the_player_stays_where_it_is() {
        let scene = scene(&["#######", "#.@#..#", "#######"]);
        let orc = [("orc", Offset { x: 3, y: 0 })];
        assert_eq!(walked(&scene, &orc, 4)[0], Some(Offset { x: 3, y: 0 }));
    }

    #[test]
    fn in_a_corridor_a_monster_waits_behind_one_it_cant_push_past() {
        let scene = scene(&["#########", "@.......#", "#########"]);
        let queue = [("orc", Offset::east(1)), ("orc", Offset::east(3))];
        let after = walked(&scene, &queue, 5);
        assert_eq!(after[0], Some(Offset::east(1)));
        assert_eq!(after[1], Some(Offset::east(2)));
    }

    #[test]
    fn a_stronger_monster_of_its_kind_pushes_past_a_weaker_one_in_a_corridor() {
        let scene = scene(&["#########", "@.......#", "#########"]);
        // The orc wizard, of more hit dice, swaps places with the orc in its way.
        let queue = [("orc", Offset::east(1)), ("orc wizard", Offset::east(2))];
        let kinds = [monster("orc"), monster("orc wizard")];
        let ground = Ground::new(&scene, scene.player, kinds, &[]);
        let mut stage = Stage::new(
            &ground,
            queue.iter().map(|&(name, offset)| (monster(name), offset)),
        );
        let mut rng = Rng::new(1, 2);
        stage.step_toward_player(bundle().formulas, &mut rng, 1);
        assert_eq!(stage.standing(1), Some(Offset::east(1)));
        assert_eq!(stage.standing(0), Some(Offset::east(2)));
    }

    #[test]
    fn on_open_floor_a_monster_steps_round_another_in_its_way() {
        let scene = scene(&[
            "#########",
            "#.......#",
            "#@......#",
            "#.......#",
            "#########",
        ]);
        let pair = [("orc", Offset::east(1)), ("orc", Offset::east(2))];
        let after = walked(&scene, &pair, 1);
        let second = after[1].unwrap();
        assert_ne!(second, Offset::east(1));
        assert_eq!(second.distance(), 1);
    }

    #[test]
    fn a_bystander_stands_in_the_way() {
        let mut scene = scene(&["#########", "@.......#", "#########"]);
        scene.bystanders.push(Position { x: 2, y: 1 });
        let orc = [("orc", Offset::east(4))];
        assert_eq!(walked(&scene, &orc, 6)[0], Some(Offset::east(3)));
    }

    #[test]
    fn a_reach_attack_needs_no_wall_between() {
        let scene = scene(&["#####", "@.#.#", "#####"]);
        let ground = Ground::new(&scene, scene.player, [monster("orc")], &[]);
        assert!(ground.reach_clear(Offset::east(2)));
        let walled = self::scene(&["#####", "@#..#", "#####"]);
        let ground = Ground::new(&walled, walled.player, [monster("orc")], &[]);
        assert!(!ground.reach_clear(Offset::east(2)));
    }
}
