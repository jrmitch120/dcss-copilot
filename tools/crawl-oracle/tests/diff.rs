//! The crawl oracle's diff, run on what its harnesses printed from a crawl 0.34.1 build: 24
//! randomised monster attackers and player defenders, 20,000 attacks each, 24 spell cases and 25
//! missile, gear and wand cases (all seed 34).

// Helpers outside #[test] functions panic on a broken fixture too.
#![allow(clippy::unwrap_used)]

use crawl_oracle::{Measure, diff};

const DUMP: &str = include_str!("fixtures/melee-0.34.1.jsonl");

#[test]
fn the_engine_agrees_with_crawl_on_every_case() {
    let report = diff(DUMP).unwrap();
    assert_eq!(report.cases, 24);
    assert!(report.mismatches.is_empty(), "{report}");
}

/// The dump with every to-hit roll in `case` one higher, as an off-by-one port would roll it.
fn with_to_hit_one_higher(case: usize) -> String {
    DUMP.lines()
        .enumerate()
        .map(|(line, json)| {
            let mut value: serde_json::Value = serde_json::from_str(json).unwrap();
            if line == case + 1 {
                let shifted: serde_json::Map<_, _> = value["to_hit"]
                    .as_object()
                    .unwrap()
                    .iter()
                    .map(|(roll, count)| {
                        (
                            (roll.parse::<i32>().unwrap() + 1).to_string(),
                            count.clone(),
                        )
                    })
                    .collect();
                value["to_hit"] = shifted.into();
            }
            format!("{value}\n")
        })
        .collect()
}

#[test]
fn a_to_hit_off_by_one_is_caught() {
    // Case 0, a nekomata, rolls to-hit over 0..=55: one in 56 rolls lands one higher.
    assert_eq!(caught(&with_to_hit_one_higher(0)), [(0, Measure::ToHit)]);
}

#[test]
fn a_dump_for_a_crawl_without_a_bundle_is_refused() {
    let dump = DUMP.replacen("\"0.34.1\"", "\"0.12.0\"", 1);
    let error = diff(&dump).unwrap_err().to_string();
    assert!(error.contains("0.12.0"), "{error}");
}

/// What the spell harness printed from the same crawl build: 24 cases, alternating a monster
/// casting one spell at a randomised player and a monster choosing among its spells, 20,000
/// times each (seed 34).
const SPELLS: &str = include_str!("fixtures/spells-0.34.1.jsonl");

#[test]
fn the_engine_agrees_with_crawl_on_every_spell_case() {
    let report = diff(SPELLS).unwrap();
    assert_eq!(report.cases, 24);
    assert!(report.mismatches.is_empty(), "{report}");
}

/// `dump` with case `case` changed by `change`.
fn with_changed(dump: &str, case: usize, change: impl Fn(&mut serde_json::Value)) -> String {
    dump.lines()
        .enumerate()
        .map(|(line, json)| {
            let mut value: serde_json::Value = serde_json::from_str(json).unwrap();
            if line == case + 1 {
                change(&mut value);
            }
            format!("{value}\n")
        })
        .collect()
}

/// A tally with every value one higher.
fn shifted(tally: &serde_json::Value) -> serde_json::Value {
    tally
        .as_object()
        .unwrap()
        .iter()
        .map(|(value, count)| {
            (
                (value.parse::<i32>().unwrap() + 1).to_string(),
                count.clone(),
            )
        })
        .collect::<serde_json::Map<_, _>>()
        .into()
}

#[test]
fn a_beam_damage_off_by_one_is_caught() {
    // Case 22, a steam dragon's steam ball, which deals 3 to 30: every hit one point harder.
    let dump = with_changed(SPELLS, 22, |case| case["damage"] = shifted(&case["damage"]));
    assert_eq!(caught(&dump), [(22, Measure::Damage)]);
}

#[test]
fn a_spell_power_off_by_one_is_caught() {
    let dump = with_changed(SPELLS, 2, |case| {
        case["power"] = (case["power"].as_i64().unwrap() + 1).into();
    });
    assert_eq!(caught(&dump), [(2, Measure::Power)]);
}

#[test]
fn a_spell_chosen_as_often_as_another_is_caught() {
    // Case 1, an orc sorcerer five squares away: half its bolts of fire counted as no spell.
    let dump = with_changed(SPELLS, 1, |case| {
        let chosen = case["chosen"].as_object_mut().unwrap();
        let fire = chosen["10"].as_u64().unwrap();
        let none = chosen["-1"].as_u64().unwrap();
        chosen.insert("10".to_owned(), (fire / 2).into());
        chosen.insert("-1".to_owned(), (none + fire - fire / 2).into());
    });
    assert_eq!(caught(&dump), [(1, Measure::SpellChosen)]);
}

/// What the missile harness printed from the same crawl build: 25 cases, taking turns at a kind
/// of monster made with its gear, a monster shooting or throwing at a randomised player, deciding
/// whether to fire, zapping a wand at the player and deciding whether to zap, 20,000 times each
/// (seed 34).
const MISSILES: &str = include_str!("fixtures/missiles-0.34.1.jsonl");

#[test]
fn the_engine_agrees_with_crawl_on_every_missile_gear_and_wand_case() {
    let report = diff(MISSILES).unwrap();
    assert_eq!(report.cases, 25);
    assert!(report.mismatches.is_empty(), "{report}");
}

/// What `diff` caught in `dump`, by case and measure.
fn caught(dump: &str) -> Vec<(u64, Measure)> {
    diff(dump)
        .unwrap()
        .mismatches
        .iter()
        .map(|m| (m.case, m.measure))
        .collect()
}

#[test]
fn a_missile_damage_off_by_one_is_caught() {
    // Case 21, an orc warrior's arbalest, which deals 1 to 27: every hit one point harder.
    let dump = with_changed(MISSILES, 21, |case| {
        case["damage"] = shifted(&case["damage"]);
    });
    assert_eq!(caught(&dump), [(21, Measure::Damage)]);
}

#[test]
fn a_kind_made_with_one_bow_more_often_than_crawl_makes_it_is_caught() {
    // Case 0, Vashnia, made with an arbalest or a longbow alike: a tenth of her arbalests
    // counted as longbows.
    let dump = with_changed(MISSILES, 0, |case| {
        let missile = case["missile"].as_object_mut().unwrap();
        let arbalest = missile["0:29"].as_u64().unwrap();
        let longbow = missile["0:31"].as_u64().unwrap();
        missile.insert("0:29".to_owned(), (arbalest - arbalest / 10).into());
        missile.insert("0:31".to_owned(), (longbow + arbalest / 10).into());
    });
    assert_eq!(caught(&dump), [(0, Measure::MissileMade)]);
}

#[test]
fn a_launcher_fired_as_often_as_a_master_archer_fires_it_is_caught() {
    // Case 2, a deep elf knight's shortbow, fired four times in five: counted as nine in ten.
    let dump = with_changed(MISSILES, 2, |case| {
        let energy = case["energy"].as_object_mut().unwrap();
        let held = energy["0"].as_u64().unwrap();
        let fired = energy["10"].as_u64().unwrap();
        energy.insert("0".to_owned(), (held / 2).into());
        energy.insert("10".to_owned(), (fired + held - held / 2).into());
    });
    assert_eq!(caught(&dump), [(2, Measure::Energy)]);
}
