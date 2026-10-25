//! Seam 1 for Backfill: a game the Advisor joins part way through goes in; the join card, the Run
//! Plan rail and what the Advisor read off the dump and the dungeon overview come out.

// Helpers outside #[test] functions panic on bad fixtures too.
#![allow(clippy::unwrap_used)]

use std::path::Path;

use copilot_core::{Backfill, FocusCard, RunPlanRail, Screen};
use copilot_replay::{Recording, Replayer};
use copilot_store::Store;
use serde_json::{Value, json};

/// A Deep Elf Conjurer saved part way through and played again with the app open: the player
/// opens the dungeon overview, then the character dump.
const JOINED: &[u8] = include_bytes!("scenarios/conjurer-joined-mid-game-reads-the-dump.dcssrec");

/// Mira, a new Minotaur followed from her first turn.
const FROM_THE_START: &[u8] = include_bytes!("recordings/minotaur-first-steps.dcssrec");

fn replayer(recording: &[u8]) -> Replayer {
    Replayer::new(Recording::read(recording).unwrap()).unwrap()
}

/// One run of the app, remembering games in the store at `path`, as the browser relays it
/// `recording`.
fn run(recording: &[u8], path: &Path) -> Replayer {
    Replayer::remembering(
        Recording::read(recording).unwrap(),
        Store::open(path).unwrap(),
    )
    .unwrap()
}

/// The focus card and the Run Plan rail the panel shows now, with the turn.
fn panel(replayer: &Replayer) -> Value {
    let game = replayer.view_model().game.unwrap();
    json!({
        "turn": game.character.turn,
        "focus": game.focus,
        "runPlan": game.run_plan,
    })
}

/// The focus card and the Run Plan rail each time either changes, from the next envelope to the
/// end of the Recording.
fn panels_to_the_end(replayer: &mut Replayer) -> Vec<Value> {
    let mut panels: Vec<Value> = Vec::new();
    while !replayer.at_end() {
        replayer.step();
        if replayer.view_model().game.is_none() {
            continue;
        }
        let now = panel(replayer);
        if panels.last() != Some(&now) {
            panels.push(now);
        }
    }
    panels
}

/// Whether the focus card is the join card.
fn joining(replayer: &Replayer) -> bool {
    matches!(
        replayer.view_model().game.unwrap().focus,
        FocusCard::Join { .. }
    )
}

/// Steps until the panel shows the join card.
fn step_until_joining(replayer: &mut Replayer) {
    while !replayer.at_end() {
        replayer.step();
        if replayer.view_model().game.is_some() && joining(replayer) {
            return;
        }
    }
    panic!("the join card never showed");
}

/// Steps until the screen the player has open is `screen`, or closes from it.
fn step_until_open(replayer: &mut Replayer, screen: &Screen) {
    while !replayer.at_end() {
        replayer.step();
        let open = replayer.advisor().visible().screen.as_ref();
        if open.is_some_and(|read| read.screen == *screen) {
            return;
        }
    }
    panic!("{screen:?} never opened");
}

#[test]
fn the_dungeon_overview_is_remembered_with_the_branches_and_shops_found() {
    let mut replayer = replayer(JOINED);
    step_until_open(&mut replayer, &Screen::DungeonOverview);

    let remembered = replayer.advisor().accumulated().unwrap();
    insta::assert_json_snapshot!(remembered.dungeon);
}

#[test]
fn the_character_dump_fills_in_skills_spells_mutations_and_the_dungeon_overview() {
    let mut replayer = replayer(JOINED);
    step_until_open(&mut replayer, &Screen::CharacterDump);

    let remembered = replayer.advisor().accumulated().unwrap();
    insta::assert_json_snapshot!(serde_json::json!({
        "skills": remembered.skills,
        "spells": remembered.spells,
        "mutations": remembered.mutations,
        "dungeon": remembered.dungeon,
    }));
}

#[test]
fn a_game_joined_part_way_through_asks_for_the_dump_and_says_what_it_read_off_each_screen() {
    let mut replayer = replayer(JOINED);

    insta::assert_json_snapshot!(panels_to_the_end(&mut replayer));
}

#[test]
fn not_now_puts_the_join_card_away_but_the_run_plan_waits_for_the_dump_all_the_same() {
    let mut replayer = replayer(JOINED);
    step_until_joining(&mut replayer);

    replayer.close_join_card();

    let game = replayer.view_model().game.unwrap();
    assert!(matches!(game.focus, FocusCard::InView { .. }));
    assert_eq!(game.run_plan, RunPlanRail::AwaitingDump);
    // Opening the dungeon overview after all leaves the card away; the dump brings it back.
    step_until_open(&mut replayer, &Screen::DungeonOverview);
    assert!(!joining(&replayer));
    step_until_open(&mut replayer, &Screen::CharacterDump);
    assert!(joining(&replayer));
}

#[test]
fn ok_puts_the_caught_up_card_away() {
    let mut replayer = replayer(JOINED);
    step_until_open(&mut replayer, &Screen::CharacterDump);
    assert!(joining(&replayer));

    replayer.close_join_card();

    let game = replayer.view_model().game.unwrap();
    assert!(matches!(game.focus, FocusCard::InView { .. }));
    assert_eq!(game.run_plan, RunPlanRail::NotPlanned);
}

#[test]
fn a_game_followed_from_its_first_turn_has_nothing_to_catch_up_on() {
    let mut replayer = replayer(FROM_THE_START);

    let panels = panels_to_the_end(&mut replayer);

    assert!(!panels.is_empty());
    for panel in panels {
        assert_eq!(panel["focus"]["card"], "inView", "{panel}");
        assert_eq!(panel["runPlan"]["state"], "notPlanned", "{panel}");
    }
}

#[test]
fn a_game_joined_part_way_through_asks_for_the_dump_again_after_a_restart_until_it_is_read() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("history.sqlite3");
    let mut asked = run(JOINED, &path);
    step_until_joining(&mut asked);
    asked.close_join_card();
    // The app closes before the player opens the dump.
    drop(asked);

    let mut again = run(JOINED, &path);
    step_until_joining(&mut again);
    step_until_open(&mut again, &Screen::CharacterDump);
    again.seek(usize::MAX);
    assert_eq!(
        again.advisor().accumulated().unwrap().backfill,
        Backfill::Complete
    );
    drop(again);

    // The browser relays the page again from its start, the dump read later in it.
    let mut caught_up = run(JOINED, &path);
    let panels = panels_to_the_end(&mut caught_up);
    for panel in panels {
        assert_eq!(panel["focus"]["card"], "inView", "{panel}");
        assert_eq!(panel["runPlan"]["state"], "notPlanned", "{panel}");
    }
}
