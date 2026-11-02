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

/// How far an explosion's blast reaches from where it goes off, `ex_size`, unless the spell sets
/// its own: `bolt::refine_for_explosion` (`beam.cc:6866`).
const BLAST_RADIUS: i32 = 1;

/// What a shot does as it flies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Flight {
    /// It stops at the first monster it meets.
    Stops,
    /// It passes through all it meets.
    Pierces,
    /// It goes off at the first monster it meets, or at the player.
    Explodes,
}

/// A beam or missile whose tracer a monster fires.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Shot<'a> {
    flight: Flight,
    /// The share the player must make up, as a percentage.
    foe_ratio: i32,
    /// The most damage it deals before armour: `preac_max_damage`.
    most_damage: i32,
    /// The farthest it flies.
    range: i32,
    /// Its spell, for how an ally resists it: `None` for a missile, which none does.
    spell: Option<&'a DamagingSpell<'a>>,
}

impl<'a> Shot<'a> {
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
        let flight = if spell.zap.explodes {
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
            range: reach.casts,
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

/// How far the blast of `spell`, a `spell_type` identifier, reaches.
fn blast_radius(variant: FireTracer, spell: Option<&str>) -> i32 {
    match variant {
        FireTracer::Crawl034 => {
            if spell == Some(spell_types::ORB_OF_ELECTRICITY) {
                2
            } else {
                BLAST_RADIUS
            }
        }
    }
}

/// The `foe_ratio` `spell`, a `spell_type` identifier, is set up with this time.
fn foe_ratio(variant: FireTracer, rng: &mut Rng, spell: &str) -> i32 {
    match variant {
        FireTracer::Crawl034 => {
            if spell == spell_types::ORB_OF_ELECTRICITY {
                // `random_range(40, 55)` (`mon-cast.cc:2303`).
                40 + rng.random2(16)
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
                40
            } else {
                DEFAULT_FOE_RATIO
            }
        }
    }
}

/// The share of `whole` that `part` of a hundred makes, rounded up: `div_round_up`.
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
    shot: Shot<'_>,
) -> bool {
    let variant = formulas.fire_tracer;
    let steps = from.distance();
    if steps == 0 {
        return true;
    }
    let toward = -from;
    let square = |step: i32| Offset {
        x: from.x + rounded_share(toward.x, step, steps),
        y: from.y + rounded_share(toward.y, step, steps),
    };
    let foe = stage.player_experience_level();
    let mut friend = 0;
    let mut finds_player = false;
    let radius = blast_radius(variant, shot.spell.map(|spell| spell.spell.id.0.as_str()));
    for step in 1..=shot.range {
        let here = square(step);
        if here == Offset::PLAYER {
            finds_player = true;
            match shot.flight {
                Flight::Pierces => continue,
                Flight::Stops => break,
                Flight::Explodes => {
                    friend += blast(formulas, stage, here, radius, shot);
                    break;
                }
            }
        }
        let Some(other) = stage.occupant(place, here) else {
            continue;
        };
        match shot.flight {
            Flight::Pierces => friend += power_of(formulas, stage, other, shot),
            Flight::Stops => {
                // It stops short of the player unless they are past it already.
                break;
            }
            Flight::Explodes => {
                finds_player |= here.distance() <= radius;
                friend += blast(formulas, stage, here, radius, shot);
                break;
            }
        }
    }
    finds_player && good_to_fire(variant, foe, friend, shot.foe_ratio)
}

/// What the monsters caught in an explosion at `centre` count for, the caster among them.
fn blast(
    formulas: Formulas,
    stage: &Stage<'_, '_>,
    centre: Offset,
    radius: i32,
    shot: Shot<'_>,
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
fn power_of(formulas: Formulas, stage: &Stage<'_, '_>, place: usize, shot: Shot<'_>) -> i32 {
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

/// `whole * part / of`, rounded to the nearest whole number, away from zero at a half.
fn rounded_share(whole: i32, part: i32, of: i32) -> i32 {
    (2 * whole * part + whole.signum() * of) / (2 * of)
}

#[cfg(test)]
mod tests {
    use std::sync::OnceLock;

    use copilot_gk::{Bundle, Library, Monster};

    use super::*;
    use crate::engine::spells;
    use crate::engine::stage::Ground;

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
        let ground = Ground::open_floor().with_experience_level(level);
        let on_stage = std::iter::once((monster("orc wizard"), Offset::east(distance)))
            .chain(orcs.iter().map(|&at| (monster("orc"), at)));
        let stage = Stage::new(&ground, on_stage);
        let shot = Shot::boldest_spell(bundle().formulas, &spell, 36, reach);
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
        let ground = Ground::open_floor();
        let from = Offset::east(6);
        let stage = Stage::new(&ground, [(monster("orc wizard"), from)]);
        let reach = Reach {
            casts: 5,
            strikes: 5,
        };
        let shot = Shot::boldest_spell(bundle().formulas, &spell, 36, reach);
        assert!(!fires(bundle().formulas, &stage, 0, from, shot));
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
