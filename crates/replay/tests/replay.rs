//! Seam 1: a Recording goes in; the view model, Visible State and game identity come out.

// Helpers outside #[test] functions panic on bad fixtures too.
#![allow(clippy::unwrap_used)]

use copilot_core::VisibleState;
use copilot_replay::{Applied, Recording, Replayer, replay};

const MINOTAUR: &[u8] = include_bytes!("recordings/minotaur-first-steps.dcssrec");
const SPECTATING: &[u8] = include_bytes!("recordings/spectating-trunk-wizard.dcssrec");
const TWO_TABS: &[u8] = include_bytes!("recordings/two-tabs-and-a-new-game.dcssrec");
const MONSTERS: &[u8] = include_bytes!("recordings/monsters-in-view.dcssrec");
const NO_GAME_KNOWLEDGE: &[u8] =
    include_bytes!("recordings/monsters-without-game-knowledge.dcssrec");
const KOBOLD: &[u8] = include_bytes!("recordings/kobold-on-d3.dcssrec");

fn replayer(recording: &[u8]) -> Replayer {
    Replayer::new(Recording::read(recording).unwrap())
}

#[test]
fn the_panel_shows_the_character_hp_and_turn_as_the_game_is_played() {
    let replay = replay(MINOTAUR).unwrap();

    let characters: Vec<_> = replay
        .view_models
        .into_iter()
        .map(|view| view.game.map(|game| game.character))
        .collect();
    insta::assert_json_snapshot!(characters);
}

#[test]
fn a_game_is_identified_by_server_account_character_species_and_when_it_was_first_seen() {
    let replay = replay(MINOTAUR).unwrap();

    insta::assert_json_snapshot!(replay.identity);
}

#[test]
fn a_spectated_game_belongs_to_the_watched_account() {
    let replay = replay(SPECTATING).unwrap();

    insta::assert_json_snapshot!(replay.identity);
}

#[test]
fn the_panel_follows_the_tab_whose_game_last_changed_and_starts_afresh_on_a_new_game() {
    let replay = replay(TWO_TABS).unwrap();

    let characters: Vec<_> = replay
        .view_models
        .into_iter()
        .map(|view| view.game.map(|game| game.character))
        .collect();
    insta::assert_json_snapshot!(characters);
}

#[test]
fn the_panel_lists_the_monsters_in_view_by_name() {
    let replay = replay(MONSTERS).unwrap();

    let focus: Vec<_> = replay
        .view_models
        .into_iter()
        .map(|view| view.game.map(|game| game.focus))
        .collect();
    insta::assert_json_snapshot!(focus);
}

#[test]
fn monsters_in_view_are_decoded_through_the_game_knowledge_for_the_crawl_version() {
    let replay = replay(MONSTERS).unwrap();

    insta::assert_json_snapshot!(replay.visible.monsters.unwrap());
}

#[test]
fn inventory_items_are_decoded_through_the_game_knowledge_for_the_crawl_version() {
    let replay = replay(MONSTERS).unwrap();

    insta::assert_json_snapshot!(replay.visible.inventory);
}

#[test]
fn without_game_knowledge_for_the_crawl_version_items_keep_only_their_names() {
    let replay = replay(NO_GAME_KNOWLEDGE).unwrap();

    insta::assert_json_snapshot!(replay.visible.inventory);
}

#[test]
fn without_game_knowledge_for_the_crawl_version_monsters_keep_only_their_names() {
    let replay = replay(NO_GAME_KNOWLEDGE).unwrap();

    insta::assert_json_snapshot!(replay.visible.monsters.unwrap());
}

#[test]
fn the_characters_stats_religion_place_and_statuses_are_decoded() {
    let visible = replay(KOBOLD).unwrap().visible;

    insta::assert_json_snapshot!(serde_json::json!({
        "stats": visible.stats,
        "religion": visible.religion,
        "place": visible.place,
        "position": visible.position,
        "statuses": visible.statuses,
    }));
}

#[test]
fn monsters_in_view_have_their_square_attitude_and_crawls_threat_rating() {
    let replay = replay(KOBOLD).unwrap();

    insta::assert_json_snapshot!(replay.visible.monsters.unwrap());
}

#[test]
fn the_first_visible_state_is_what_the_game_first_reports() {
    let mut replayer = replayer(MINOTAUR);
    while replayer.advisor().visible() == &VisibleState::default() && !replayer.at_end() {
        replayer.step();
    }

    insta::assert_json_snapshot!(replayer.advisor().visible());
}

#[test]
fn the_envelope_timestamps_are_the_replays_only_clock() {
    let mut replayer = replayer(MINOTAUR);
    assert_eq!(replayer.clock_ms(), None);

    replayer.seek(3);

    assert_eq!(replayer.clock_ms(), Some(1_760_486_401_000));
}

#[test]
fn stepping_one_message_applies_exactly_the_next_envelope() {
    let mut replayer = replayer(MINOTAUR);
    replayer.seek(4);

    let applied = replayer.step();

    assert_eq!(
        applied,
        Applied {
            restarted: false,
            envelopes: 4..5
        }
    );
    assert_eq!(replayer.turn(), Some(0));
}

#[test]
fn stepping_one_turn_stops_on_the_envelope_that_reports_the_next_turn() {
    let mut replayer = replayer(MINOTAUR);
    let mut stops = Vec::new();
    while !replayer.at_end() {
        replayer.step_turn();
        stops.push((replayer.position(), replayer.turn()));
    }

    assert_eq!(stops, [(5, Some(0)), (8, Some(3)), (9, Some(4))]);
}

#[test]
fn jumping_to_a_turn_lands_on_the_envelope_that_first_reports_it() {
    let mut replayer = replayer(MINOTAUR);

    let applied = replayer.seek_turn(2);

    assert_eq!(
        applied,
        Some(Applied {
            restarted: false,
            envelopes: 0..8
        })
    );
    assert_eq!(replayer.turn(), Some(3));
    assert_eq!(replayer.clock_ms(), Some(1_760_486_405_010));
}

#[test]
fn jumping_to_a_turn_the_recording_never_reaches_stays_put() {
    let mut replayer = replayer(MINOTAUR);
    replayer.seek(6);

    assert_eq!(replayer.seek_turn(5), None);
    assert_eq!(replayer.position(), 6);
}

#[test]
fn seeking_backward_replays_from_the_start() {
    let mut replayer = replayer(TWO_TABS);
    replayer.seek(7);

    let applied = replayer.seek(2);

    assert_eq!(
        applied,
        Applied {
            restarted: true,
            envelopes: 0..2
        }
    );
    let mut fresh = self::replayer(TWO_TABS);
    fresh.seek(2);
    assert_eq!(replayer.view_model(), fresh.view_model());
    assert_eq!(
        replayer
            .view_model()
            .game
            .map(|game| game.character.species),
        Some("Minotaur".to_owned())
    );
}

#[test]
fn a_followed_game_fills_the_panel_with_only_what_the_advisor_can_tell_so_far() {
    let replay = replay(MINOTAUR).unwrap();

    insta::assert_json_snapshot!(replay.view_models.last().unwrap().game);
}
