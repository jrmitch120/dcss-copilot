//! The crawl oracle's diff, run on what its harnesses printed from a crawl 0.34.1 build: 24
//! randomised monster attackers and player defenders, 20,000 attacks each, 24 spell cases, 25
//! missile, gear and wand cases, and the turn harness's block and reach cases with 24 turn cases
//! (all seed 34).

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
    // Case 0, a peacekeeper, rolls to-hit over 0..=41: one in 42 rolls lands one higher.
    assert_eq!(caught(&with_to_hit_one_higher(0)), [(0, Measure::ToHit)]);
}

/// A share of `dump`'s cases, every `count`th from `index`, with a header naming shard `index` of
/// `count`, as a harness run with a shard prints it.
fn shard_of(dump: &str, index: u64, count: u64) -> String {
    dump.lines()
        .enumerate()
        .filter_map(|(line, json)| {
            let mut value: serde_json::Value = serde_json::from_str(json).unwrap();
            if line == 0 {
                value["shard"] = serde_json::json!({ "index": index, "count": count });
            } else if value["case"].as_u64().unwrap() % count != index {
                return None;
            }
            Some(format!("{value}\n"))
        })
        .collect()
}

#[test]
fn one_shard_of_a_run_is_diffed_on_its_own() {
    let report = diff(&shard_of(DUMP, 1, 3)).unwrap();
    assert_eq!(report.cases, 8);
    assert!(report.mismatches.is_empty(), "{report}");
    assert!(
        report
            .to_string()
            .starts_with("crawl 0.34.1, shard 1/3: 8 cases"),
        "{report}"
    );
}

#[test]
fn a_seraphs_halo_left_out_of_its_aim_is_caught() {
    // Case 10, a seraph, whose halo backlights the player: counted as plain light.
    let dump = with_changed(DUMP, 10, |case| case["lighting"] = "plain".into());
    assert!(caught(&dump).contains(&(10, Measure::ToHit)));
}

#[test]
fn an_umbra_left_out_of_the_aim_of_a_monster_without_nightvision_is_caught() {
    // Case 1, a merfolk siren, which can't see through the player's umbra: counted as plain light.
    let dump = with_changed(DUMP, 1, |case| case["lighting"] = "plain".into());
    assert!(caught(&dump).contains(&(1, Measure::ToHit)));
}

#[test]
fn an_umbra_doesnt_spoil_the_aim_of_a_monster_with_nightvision() {
    // Case 21, an ancient lich, undead: the player's umbra doesn't change its rolls.
    let dump = with_changed(DUMP, 21, |case| case["lighting"] = "plain".into());
    assert_eq!(caught(&dump), []);
}

#[test]
fn nightvision_the_game_knowledge_doesnt_give_a_monster_is_caught() {
    // Case 21, an ancient lich, which crawl says sees through an umbra: said not to.
    let dump = with_changed(DUMP, 21, |case| case["nightvision"] = false.into());
    assert_eq!(caught(&dump), [(21, Measure::Nightvision)]);
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
    // Case 22, a servant of whispers' lightning bolt (1 to 54): every hit one point harder.
    let dump = with_changed(SPELLS, 22, |case| case["damage"] = shifted(&case["damage"]));
    assert_eq!(caught(&dump), [(22, Measure::Damage)]);
}

#[test]
fn a_backlit_player_counted_as_in_plain_light_is_caught() {
    // Case 8, a fire giant's bolt of fire at a player under a corona: counted as plain light.
    let dump = with_changed(SPELLS, 8, |case| case["lighting"] = "plain".into());
    assert_eq!(caught(&dump), [(8, Measure::Hits)]);
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
    // Case 9, a fire giant six squares away: half its bolts of fire counted as no spell.
    let dump = with_changed(SPELLS, 9, |case| {
        let chosen = case["chosen"].as_object_mut().unwrap();
        let fire = chosen["10"].as_u64().unwrap();
        let none = chosen["-1"].as_u64().unwrap();
        chosen.insert("10".to_owned(), (fire / 2).into());
        chosen.insert("-1".to_owned(), (none + fire - fire / 2).into());
    });
    assert_eq!(caught(&dump), [(9, Measure::SpellChosen)]);
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
    // Case 21, a merfolk javelineer's javelin, which deals up to 33: every hit one point harder.
    let dump = with_changed(MISSILES, 21, |case| {
        case["damage"] = shifted(&case["damage"]);
    });
    assert_eq!(caught(&dump), [(21, Measure::Damage)]);
}

#[test]
fn a_kind_made_with_its_nets_less_often_than_crawl_makes_it_is_caught() {
    // Case 15, Harold, always made with throwing nets: a tenth counted as made with nothing.
    let dump = with_changed(MISSILES, 15, |case| {
        let missile = case["missile"].as_object_mut().unwrap();
        let nets = missile["1:8"].as_u64().unwrap();
        missile.insert("1:8".to_owned(), (nets - nets / 10).into());
        missile.insert("none".to_owned(), (nets / 10).into());
    });
    assert_eq!(caught(&dump), [(15, Measure::MissileMade)]);
}

#[test]
fn a_launcher_fired_as_often_as_a_master_archer_fires_it_is_caught() {
    // Case 2, a merfolk's boomerang, thrown one time in three: counted as two in three.
    let dump = with_changed(MISSILES, 2, |case| {
        let energy = case["energy"].as_object_mut().unwrap();
        let held = energy["0"].as_u64().unwrap();
        let fired = energy["10"].as_u64().unwrap();
        energy.insert("0".to_owned(), (held / 2).into());
        energy.insert("10".to_owned(), (fired + held - held / 2).into());
    });
    assert_eq!(caught(&dump), [(2, Measure::Energy)]);
}

/// What the turn harness printed from the same crawl build: how many attacks each shield blocks,
/// how far every kind of monster with a damaging attack reaches in melee and with which attacks,
/// and 24 monsters playing out the player's next action 20,000 times each, taking turns at the
/// rules they exercise (seed 34).
const TURNS: &str = include_str!("fixtures/turns-0.34.1.jsonl");

#[test]
fn the_engine_agrees_with_crawl_on_every_block_reach_and_turn_case() {
    let report = diff(TURNS).unwrap();
    assert_eq!(report.cases, 552);
    assert!(report.mismatches.is_empty(), "{report}");
}

#[test]
fn a_shield_that_blocks_one_more_attack_is_caught() {
    // Case 1, a buckler, which blocks two attacks before the player's next action.
    let dump = with_changed(TURNS, 1, |case| case["limit"] = 3.into());
    assert_eq!(caught(&dump), [(1, Measure::BlockLimit)]);
}

#[test]
fn a_reaching_attack_held_to_two_squares_is_caught() {
    // Case 468, a starflower, whose lash follows its rift three squares: counted as two.
    let dump = with_changed(TURNS, 468, |case| {
        case["attacks_from"]["3"] = serde_json::json!([0]);
    });
    assert_eq!(caught(&dump), [(468, Measure::AttacksFrom)]);
}

#[test]
fn a_monster_reaching_one_square_too_far_is_caught() {
    // Case 203, an orange demon, whose sting reaches two squares.
    let dump = with_changed(TURNS, 203, |case| case["reach"] = 3.into());
    assert_eq!(caught(&dump), [(203, Measure::Reach)]);
}

#[test]
fn a_monster_attacking_once_more_than_crawl_lets_it_is_caught() {
    // Case 529, an air elemental beside the player at speed 25, which fights up to three times.
    let dump = with_changed(TURNS, 529, |case| case["melee"] = shifted(&case["melee"]));
    assert_eq!(caught(&dump), [(529, Measure::MeleeActions)]);
}

#[test]
fn a_monster_closing_a_square_more_than_crawl_lets_it_is_caught() {
    // Case 533, an alligator snapping turtle at speed 8, which closes a square four times in
    // five.
    let dump = with_changed(TURNS, 533, |case| case["closed"] = shifted(&case["closed"]));
    assert_eq!(caught(&dump), [(533, Measure::Closed)]);
}

#[test]
fn a_monster_the_threat_level_leaves_out_that_crawl_shows_fighting_is_caught() {
    // Case 529, an air elemental, counted twenty squares away while crawl shows it fighting.
    let dump = with_changed(TURNS, 529, |case| case["distance"] = 20.into());
    assert!(caught(&dump).contains(&(529, Measure::CanAttack)));
}
