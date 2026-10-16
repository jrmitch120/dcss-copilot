//! The crawl oracle's diff, run on what its harnesses printed from a crawl 0.34.1 build: 24
//! randomised monster attackers and player defenders, 20,000 attacks each, and 24 spell cases
//! (both seed 34).

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
    let report = diff(&with_to_hit_one_higher(0)).unwrap();
    let caught: Vec<_> = report
        .mismatches
        .iter()
        .map(|m| (m.case, m.measure))
        .collect();
    assert_eq!(caught, [(0, Measure::ToHit)], "{report}");
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
    let report = diff(&dump).unwrap();
    let caught: Vec<_> = report
        .mismatches
        .iter()
        .map(|m| (m.case, m.measure))
        .collect();
    assert_eq!(caught, [(22, Measure::Damage)], "{report}");
}

#[test]
fn a_spell_power_off_by_one_is_caught() {
    let dump = with_changed(SPELLS, 2, |case| {
        case["power"] = (case["power"].as_i64().unwrap() + 1).into();
    });
    let report = diff(&dump).unwrap();
    let caught: Vec<_> = report
        .mismatches
        .iter()
        .map(|m| (m.case, m.measure))
        .collect();
    assert_eq!(caught, [(2, Measure::Power)], "{report}");
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
    let report = diff(&dump).unwrap();
    let caught: Vec<_> = report
        .mismatches
        .iter()
        .map(|m| (m.case, m.measure))
        .collect();
    assert_eq!(caught, [(1, Measure::SpellChosen)], "{report}");
}
