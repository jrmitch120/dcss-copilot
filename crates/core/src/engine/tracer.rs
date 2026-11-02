//! Whether a monster holds its fire, for the allies in the way: crawl's tracer (`fire_tracer`,
//! `beam.cc:2198`) and `targeting_tracer::good_to_fire` (`beam.cc:7684`), ported closely (ADR 0002)
//! behind the formula variant a Game Knowledge bundle names.
//!
//! A tracer flies as the beam would. One that doesn't pierce stops at the first monster it meets
//! (`bolt::range_used_on_hit`, `beam.cc:6744`), so with an ally in the way it never finds the
//! player, and the monster holds its fire at any experience level. An explosion goes off at the
//! first monster it meets, and counts everyone in its blast. A beam that pierces goes on, past the
//! player to the end of its range, counting each it passes.
//!
//! The player counts for their experience level, an ally for `2 * final * HD / preac`
//! (`bolt::tracer_nonenchantment_affect_monster`, `beam.cc:4847`): its hit dice, scaled by how much
//! of the beam it wouldn't resist. The monster fires if it hits the player at all, and they count
//! for enough of the whole (`foe_ratio`).

use copilot_gk::{FireTracer, Formulas};

use super::paths::rounded_share;
use super::player_attacks::Defence;
use super::resists;
use super::rng::Rng;
use super::spells::{DamagingSpell, Reach};
use super::stage::Stage;
use super::step::Offset;
use crate::crawl_ids::spell_types;

/// The share of what a tracer hits the player must make up, as a percentage, unless the spell sets
/// its own: `fire_tracer` (`beam.cc:2223`).
const DEFAULT_FOE_RATIO: i32 = 80;

/// The lowest `foe_ratio` Orb of Electricity is set up with, and how many it may be from there:
/// `random_range(40, 55)` (`mon-cast.cc:2303`).
const ORB_LOWEST_FOE_RATIO: i32 = 40;
const ORB_FOE_RATIOS: i32 = 16;

/// What a shot does as it flies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Flight {
    /// It stops at the first monster it meets.
    Stops,
    /// It passes through all it meets.
    Pierces,
    /// It goes off at the first monster it meets, or at the player.
    Explodes,
    /// It leaves a cloud, whose tracer has rules of its own that aren't ported: it is fired
    /// (`NotModelled::CasterSafety`).
    Cloud,
}

/// A beam or missile whose tracer a monster fires.
#[derive(Debug, Clone, Copy)]
pub(crate) struct TracedShot<'a> {
    flight: Flight,
    /// The share the player must make up, as a percentage.
    foe_ratio: i32,
    /// The most damage it deals before armour: `preac_max_damage`.
    most_damage: i32,
    /// The farthest it flies, where an explosion goes off if it meets no one sooner.
    range: i32,
    /// Its spell, for how an ally resists it: `None` for a missile, which none does.
    spell: Option<&'a DamagingSpell<'a>>,
}

impl<'a> TracedShot<'a> {
    /// `spell`, cast or zapped at `power` with `reach`, as its tracer flies. Some spells draw
    /// their own `foe_ratio` each time they are set up (`setup_mons_cast`).
    pub(crate) fn spell(
        formulas: Formulas,
        rng: &mut Rng,
        spell: &'a DamagingSpell<'a>,
        power: i32,
        reach: Reach,
    ) -> Self {
        let foe_ratio = foe_ratio(formulas.fire_tracer, rng, &spell.spell.id.0);
        Self::of_spell(spell, power, reach, foe_ratio)
    }

    /// `spell` as boldly as it is ever set up, for whether it may ever be fired.
    pub(crate) fn boldest_spell(
        formulas: Formulas,
        spell: &'a DamagingSpell<'a>,
        power: i32,
        reach: Reach,
    ) -> Self {
        let foe_ratio = lowest_foe_ratio(formulas.fire_tracer, &spell.spell.id.0);
        Self::of_spell(spell, power, reach, foe_ratio)
    }

    fn of_spell(spell: &'a DamagingSpell<'a>, power: i32, reach: Reach, foe_ratio: i32) -> Self {
        let flight = if spell.spell.cloud.is_some() {
            Flight::Cloud
        } else if spell.zap.explodes {
            Flight::Explodes
        } else if spell.zap.pierces {
            Flight::Pierces
        } else {
            Flight::Stops
        };
        Self {
            flight,
            foe_ratio,
            most_damage: spell.most_damage(power),
            // An explosion's reach counts its blast; it flies only so far.
            range: if flight == Flight::Explodes {
                reach.strikes - spell.blast_radius()
            } else {
                reach.strikes
            },
            spell: Some(spell),
        }
    }

    /// A missile of up to `most_damage`, that `pierces` what it hits or not, flying `range`
    /// squares.
    pub(crate) fn missile(pierces: bool, most_damage: i32, range: i32) -> Self {
        Self {
            flight: if pierces {
                Flight::Pierces
            } else {
                Flight::Stops
            },
            foe_ratio: DEFAULT_FOE_RATIO,
            most_damage,
            range,
            spell: None,
        }
    }
}

/// The `foe_ratio` `spell`, a `spell_type` identifier, is set up with this time.
fn foe_ratio(variant: FireTracer, rng: &mut Rng, spell: &str) -> i32 {
    match variant {
        FireTracer::Crawl034 => {
            if spell == spell_types::ORB_OF_ELECTRICITY {
                ORB_LOWEST_FOE_RATIO + rng.random2(ORB_FOE_RATIOS)
            } else {
                DEFAULT_FOE_RATIO
            }
        }
    }
}

/// The lowest `foe_ratio` `spell`, a `spell_type` identifier, is ever set up with.
fn lowest_foe_ratio(variant: FireTracer, spell: &str) -> i32 {
    match variant {
        FireTracer::Crawl034 => {
            if spell == spell_types::ORB_OF_ELECTRICITY {
                ORB_LOWEST_FOE_RATIO
            } else {
                DEFAULT_FOE_RATIO
            }
        }
    }
}

/// `whole` divided `by`, rounded up: crawl's `div_round_up`.
fn div_round_up(whole: i32, by: i32) -> i32 {
    (whole + by - 1) / by.max(1)
}

/// Whether a tracer that finds a player worth `foe` and allies worth `friend`, with `foe_ratio`,
/// is good to fire: `targeting_tracer::good_to_fire` (`beam.cc:7684`).
pub(crate) fn good_to_fire(variant: FireTracer, foe: i32, friend: i32, foe_ratio: i32) -> bool {
    match variant {
        FireTracer::Crawl034 => friend == 0 || foe >= div_round_up(foe_ratio * (foe + friend), 100),
    }
}

/// Whether the monster at `place`, at `from`, fires `shot` at the player, for the others on
/// `stage`. A wall on its line is left to the line of sight it already has.
pub(crate) fn fires(
    formulas: Formulas,
    stage: &Stage<'_, '_>,
    place: usize,
    from: Offset,
    shot: TracedShot<'_>,
) -> bool {
    let variant = formulas.fire_tracer;
    let steps = from.distance();
    if steps == 0 || shot.flight == Flight::Cloud {
        return true;
    }
    let toward = -from;
    let square = |step: i32| Offset {
        x: from.x + rounded_share(toward.x, step, steps),
        y: from.y + rounded_share(toward.y, step, steps),
    };
    let foe = stage.player_experience_level();
    let radius = shot.spell.map_or(0, DamagingSpell::blast_radius);
    let mut friend = 0;
    let mut finds_player = false;
    // An explosion goes off where its flight ends, if it meets no one sooner.
    let mut goes_off_at = (shot.flight == Flight::Explodes).then(|| square(shot.range));
    for step in 1..=shot.range {
        let here = square(step);
        if here == Offset::PLAYER {
            finds_player = true;
        }
        let other = stage.occupant(place, here);
        if here != Offset::PLAYER && other.is_none() {
            continue;
        }
        match shot.flight {
            Flight::Pierces => {
                if let Some(other) = other {
                    friend += power_of(formulas, stage, other, shot);
                }
            }
            Flight::Stops | Flight::Cloud => break,
            Flight::Explodes => {
                goes_off_at = Some(here);
                break;
            }
        }
    }
    if let Some(centre) = goes_off_at {
        finds_player |= centre.distance() <= radius;
        friend += blast(formulas, stage, centre, radius, shot);
    }
    finds_player && good_to_fire(variant, foe, friend, shot.foe_ratio)
}

/// What the monsters caught in an explosion at `centre` count for, the caster among them.
fn blast(
    formulas: Formulas,
    stage: &Stage<'_, '_>,
    centre: Offset,
    radius: i32,
    shot: TracedShot<'_>,
) -> i32 {
    stage
        .within(centre, radius)
        .map(|other| power_of(formulas, stage, other, shot))
        .sum()
}

/// What the ally at `place` counts for against `shot`: `2 * final * HD / preac`, where `final` is
/// half what it wouldn't resist of the most the shot deals, rounded up (`bolt::determine_damage`,
/// `beam.cc:4690`). One it wouldn't hurt counts for nothing, as does firewood, which isn't worth
/// caring about (`mons_is_threatening`).
fn power_of(formulas: Formulas, stage: &Stage<'_, '_>, place: usize, shot: TracedShot<'_>) -> i32 {
    let Some(monster) = stage.monster(place) else {
        return 0;
    };
    if monster.firewood || shot.most_damage <= 0 {
        return 0;
    }
    let adjusted = shot
        .spell
        .and_then(|spell| resists::monster_beam(formulas.player_beam, &spell.zap.flavour))
        .map_or(shot.most_damage, |resist| {
            let defence = Defence::of(formulas.player_beam, monster);
            resists::monster_beam_damage(
                formulas.player_beam,
                resist,
                defence.resists,
                shot.most_damage,
            )
        });
    let finished = div_round_up(adjusted, 2);
    2 * finished * i32::try_from(monster.hd).unwrap_or(i32::MAX) / shot.most_damage
}

#[cfg(test)]
mod tests {
    use std::sync::OnceLock;

    use copilot_gk::{Bundle, Library, Monster};

    use super::*;
    use crate::engine::spells;
    use crate::engine::stage::Battleground;

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

    /// Whether an orc wizard east of the player at `distance`, with an orc at each of `orcs`,
    /// fires the beam of `spell` at a player of `level`.
    fn fired(spell: &str, distance: i32, orcs: &[Offset], level: i32) -> bool {
        let known = bundle()
            .spells
            .values()
            .find(|known| known.id.0 == spell)
            .unwrap();
        let spell = spells::damaging(bundle().formulas.monster_spell_beam, known).unwrap();
        let reach = Reach {
            casts: 8,
            strikes: 8,
        };
        let ground = Battleground::open_floor().with_experience_level(level);
        let on_stage = std::iter::once((monster("orc wizard"), Offset::east(distance)))
            .chain(orcs.iter().map(|&at| (monster("orc"), at)));
        let stage = Stage::new(&ground, on_stage);
        let shot = TracedShot::boldest_spell(bundle().formulas, &spell, 36, reach);
        fires(bundle().formulas, &stage, 0, Offset::east(distance), shot)
    }

    const BOLT: &str = "SPELL_MAGIC_DART";
    const BEAM: &str = "SPELL_BOLT_OF_FIRE";
    const BLAST: &str = "SPELL_FIREBALL";

    #[test]
    fn with_nothing_in_the_way_a_monster_fires() {
        assert!(fired(BOLT, 3, &[], 1));
        assert!(fired(BEAM, 3, &[], 1));
    }

    #[test]
    fn a_bolt_that_doesnt_pierce_stops_at_an_ally_whatever_the_players_level() {
        for level in [1, 3, 9, 27] {
            assert!(!fired(BOLT, 3, &[Offset::east(1)], level));
        }
    }

    #[test]
    fn a_beam_past_one_orc_is_held_at_level_three_and_fired_at_level_nine() {
        assert!(!fired(BEAM, 3, &[Offset::east(1)], 3));
        assert!(fired(BEAM, 3, &[Offset::east(1)], 9));
    }

    #[test]
    fn a_beam_counts_an_ally_past_the_player_too() {
        assert!(!fired(BEAM, 3, &[Offset::east(-1)], 3));
    }

    #[test]
    fn a_shot_that_falls_short_of_the_player_is_held() {
        let known = bundle()
            .spells
            .values()
            .find(|known| known.id.0 == BEAM)
            .unwrap();
        let spell = spells::damaging(bundle().formulas.monster_spell_beam, known).unwrap();
        let ground = Battleground::open_floor();
        let from = Offset::east(6);
        let stage = Stage::new(&ground, [(monster("orc wizard"), from)]);
        let reach = Reach {
            casts: 5,
            strikes: 5,
        };
        let shot = TracedShot::boldest_spell(bundle().formulas, &spell, 36, reach);
        assert!(!fires(bundle().formulas, &stage, 0, from, shot));
    }

    #[test]
    fn a_blast_that_runs_out_of_range_beside_the_player_still_finds_them() {
        let known = bundle()
            .spells
            .values()
            .find(|known| known.id.0 == BLAST)
            .unwrap();
        let spell = spells::damaging(bundle().formulas.monster_spell_beam, known).unwrap();
        let ground = Battleground::open_floor();
        let from = Offset::east(6);
        let stage = Stage::new(&ground, [(monster("orc wizard"), from)]);
        // Its reach counts the blast's square, so 6 is a flight of 5, ending beside the player.
        let reach = |strikes| Reach { casts: 8, strikes };
        let fired = |strikes| {
            let shot = TracedShot::boldest_spell(bundle().formulas, &spell, 36, reach(strikes));
            fires(bundle().formulas, &stage, 0, from, shot)
        };
        assert!(fired(6));
        assert!(!fired(5));
    }

    #[test]
    fn an_ally_off_the_line_doesnt_count() {
        assert!(fired(BEAM, 3, &[Offset { x: 1, y: 1 }], 3));
    }

    #[test]
    fn a_blast_goes_off_at_the_first_ally_and_reaches_the_player_only_from_beside_them() {
        // Far from the player, it never finds them.
        assert!(!fired(BLAST, 5, &[Offset::east(3)], 27));
        // Beside them, it catches both, and level decides it.
        assert!(!fired(BLAST, 5, &[Offset::east(1)], 3));
        assert!(fired(BLAST, 5, &[Offset::east(1)], 9));
    }

    /// Whether an orc four squares east of the player fires a missile that `pierces` or doesn't,
    /// with an orc beside the player, at `level`.
    fn missile_fired(pierces: bool, level: i32) -> bool {
        let ground = Battleground::open_floor().with_experience_level(level);
        let from = Offset::east(4);
        let stage = Stage::new(
            &ground,
            [(monster("orc"), from), (monster("orc"), Offset::east(1))],
        );
        fires(
            bundle().formulas,
            &stage,
            0,
            from,
            TracedShot::missile(pierces, 10, 6),
        )
    }

    #[test]
    fn a_missile_stops_at_an_ally_and_only_a_javelin_flies_on_past_it() {
        assert!(!missile_fired(false, 27));
        assert!(!missile_fired(true, 3));
        assert!(missile_fired(true, 9));
    }

    #[test]
    fn orb_of_electricity_draws_its_foe_ratio() {
        let variant = FireTracer::Crawl034;
        let mut rng = Rng::new(1, 2);
        let drawn: Vec<i32> = (0..500)
            .map(|_| foe_ratio(variant, &mut rng, spell_types::ORB_OF_ELECTRICITY))
            .collect();
        assert_eq!(drawn.iter().min(), Some(&40));
        assert_eq!(drawn.iter().max(), Some(&55));
        assert_eq!(
            lowest_foe_ratio(variant, spell_types::ORB_OF_ELECTRICITY),
            40
        );
        assert_eq!(foe_ratio(variant, &mut rng, BOLT), DEFAULT_FOE_RATIO);
    }

    #[test]
    fn good_to_fire_weighs_the_player_against_those_it_would_hit_by_the_foe_ratio() {
        let variant = FireTracer::Crawl034;
        assert!(good_to_fire(variant, 3, 0, 80));
        // Of 4, the player must make up 4 at 80%: 3.2 rounded up.
        assert!(!good_to_fire(variant, 3, 1, 80));
        // At 55%, 2.2 rounded up is 3.
        assert!(good_to_fire(variant, 3, 1, 55));
        assert!(good_to_fire(variant, 9, 1, 80));
    }
}
