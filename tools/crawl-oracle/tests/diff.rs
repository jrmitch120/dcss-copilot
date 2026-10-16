//! The crawl oracle's diff, run on what its harness printed from a crawl 0.34.1 build: 24
//! randomised monster attackers and player defenders, 20,000 attacks each (seed 34).

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
