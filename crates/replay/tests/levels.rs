//! Seam 1 for level and monster memory: a Scenario in which the player sees items on the floor
//! and in a shop, kills a unique, changes level and explores a level to the end goes in; what
//! the Advisor remembers of each level and of the uniques met comes out.

// Helpers outside #[test] functions panic on bad fixtures too.
#![allow(clippy::unwrap_used)]

use copilot_core::{
    Explored, ItemSeen, ItemStatus, LandmarkKind, LevelRecord, OnLevel, Position, Visit,
};
use copilot_gk::StairDirection;
use copilot_replay::{Recording, Replayer};

/// A Minotaur Berserker who sees a dagger and a ring mail at their feet and takes the dagger,
/// buys from a shop they make, kills Terence, goes down to D:2 and explores the Temple to the
/// end.
const BERSERKER: &[u8] =
    include_bytes!("scenarios/berserker-shops-slays-terence-and-explores-the-temple.dcssrec");

fn replayer() -> Replayer {
    Replayer::new(Recording::read(BERSERKER).unwrap()).unwrap()
}

fn explored(replayer: &Replayer) -> &Explored {
    &replayer.advisor().accumulated().unwrap().explored
}

/// The record of the level named `level`.
fn level<'a>(replayer: &'a Replayer, level: &str) -> &'a LevelRecord {
    explored(replayer)
        .levels
        .get(level)
        .unwrap_or_else(|| panic!("no record of {level}"))
}

/// Steps until the game has shown a message holding `said`, and the update has settled.
fn step_until_said(replayer: &mut Replayer, said: &str) {
    while !replayer.at_end() {
        replayer.step();
        let messages = &replayer.advisor().visible().messages;
        if messages.iter().any(|message| message.text.contains(said)) {
            step_until_settled(replayer);
            return;
        }
    }
    panic!("the game never said {said:?}");
}

/// Steps until the turn moves on, so the update before it has settled.
fn step_until_settled(replayer: &mut Replayer) {
    let turn = replayer.advisor().visible().turn;
    while !replayer.at_end() && replayer.advisor().visible().turn == turn {
        replayer.step();
    }
}

fn play_to_end(replayer: &mut Replayer) {
    while !replayer.at_end() {
        replayer.step();
    }
}

/// The items seen on the floor at `position` of `record`, by name and status.
fn on_the_floor(record: &LevelRecord, position: Position) -> Vec<(&str, ItemStatus)> {
    record
        .items
        .iter()
        .filter(|item| item.position == position && item.offered.is_none())
        .map(|item| (item.name.as_str(), item.status))
        .collect()
}

const SOUTH_OF_THE_START: Position = Position { x: 0, y: 1 };

#[test]
fn an_item_crawl_names_where_the_player_stands_is_spotted_there_until_it_is_gone() {
    let mut replayer = replayer();
    step_until_said(&mut replayer, "You see here a +0 dagger.");
    let dungeon = level(&replayer, "Dungeon:1");
    assert_eq!(
        on_the_floor(dungeon, SOUTH_OF_THE_START),
        [("a +0 dagger", ItemStatus::Spotted)]
    );
    step_until_said(&mut replayer, "a +0 ring mail; a +0 dagger");
    assert_eq!(
        on_the_floor(level(&replayer, "Dungeon:1"), SOUTH_OF_THE_START),
        [
            ("a +0 dagger", ItemStatus::Spotted),
            ("a +0 ring mail", ItemStatus::Spotted)
        ]
    );
    // Picked up, it isn't named as the player comes back.
    step_until_said(&mut replayer, "You see here a +0 ring mail.");
    let items = on_the_floor(level(&replayer, "Dungeon:1"), SOUTH_OF_THE_START);
    assert!(
        matches!(
            items.as_slice(),
            [
                ("a +0 dagger", ItemStatus::Gone { turn: Some(_) }),
                ("a +0 ring mail", ItemStatus::Spotted)
            ]
        ),
        "{items:?}"
    );
}

#[test]
fn a_shops_stock_is_spotted_with_its_prices_and_an_item_bought_is_gone() {
    let mut replayer = replayer();
    play_to_end(&mut replayer);
    let dungeon = level(&replayer, "Dungeon:1");
    let shop = Position { x: 0, y: 2 };
    assert!(
        dungeon
            .landmarks
            .iter()
            .any(|landmark| landmark.position == shop && landmark.kind == LandmarkKind::Shop),
        "{:?}",
        dungeon.landmarks
    );
    let stock: Vec<&ItemSeen> = dungeon
        .items
        .iter()
        .filter(|item| item.offered.is_some())
        .collect();
    assert!(stock.iter().all(|item| item.position == shop));
    let gone: Vec<&str> = stock
        .iter()
        .filter(|item| matches!(item.status, ItemStatus::Gone { .. }))
        .map(|item| item.name.as_str())
        .collect();
    assert_eq!(gone, ["a potion of ambrosia (unknown)"]);
    assert!(stock.len() > 1);
}

#[test]
fn a_unique_is_remembered_where_it_was_last_seen_and_killed() {
    let mut replayer = replayer();
    play_to_end(&mut replayer);
    let terence = explored(&replayer).uniques.get("Terence").unwrap();
    let on_d1 = |turn| {
        Some(OnLevel {
            level: "Dungeon:1".to_owned(),
            turn: Some(turn),
        })
    };
    assert_eq!(terence.killed, on_d1(12));
    // In view as the turn of the blow that killed him began.
    assert_eq!(terence.seen, on_d1(12));
}

#[test]
fn each_level_keeps_its_visits_and_its_map_as_the_player_left_it() {
    let mut replayer = replayer();
    play_to_end(&mut replayer);
    let visits = |name| level(&replayer, name).visits.clone();
    assert_eq!(
        visits("Dungeon:1"),
        [Visit {
            arrived: Some(0),
            left: Some(13),
            cleared: None
        }]
    );
    assert_eq!(visits("Dungeon:2").len(), 1);
    assert_eq!(explored(&replayer).on_level.as_deref(), Some("Temple"));
    let dungeon = level(&replayer, "Dungeon:1");
    let map = dungeon.map.as_ref().unwrap();
    assert!(map.rows.len() > 5, "{map:?}");
    assert!(
        dungeon
            .landmarks
            .iter()
            .any(|landmark| landmark.position == Position { x: -1, y: 0 }
                && landmark.kind
                    == LandmarkKind::Stairs {
                        direction: StairDirection::Down
                    }),
        "{:?}",
        dungeon.landmarks
    );
    assert!(level(&replayer, "Temple").map.is_none());
}

#[test]
fn a_floor_is_cleared_once_in_a_visit_when_nothing_is_left_to_explore() {
    let mut replayer = replayer();
    play_to_end(&mut replayer);
    let temple = level(&replayer, "Temple");
    let [visit] = temple.visits.as_slice() else {
        panic!("{:?}", temple.visits);
    };
    // Its far rooms lie past transporters, so it is cleared as crawl says exploring is done.
    assert_eq!(visit.cleared, Some(96), "{visit:?}");
    assert_eq!(
        temple
            .landmarks
            .iter()
            .filter(|landmark| landmark.kind == LandmarkKind::Altar)
            .count(),
        19
    );
    let dungeon_2 = level(&replayer, "Dungeon:2");
    assert_eq!(
        dungeon_2.visits.first().and_then(|visit| visit.cleared),
        None
    );
}

#[test]
fn what_the_advisor_remembers_of_each_level_and_the_uniques_met() {
    let mut replayer = replayer();
    play_to_end(&mut replayer);
    insta::assert_json_snapshot!(explored(&replayer));
}
