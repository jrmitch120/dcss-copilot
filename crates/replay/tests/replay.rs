//! Seam 1: a Recording goes in; the view model, Visible State and game identity come out.

// Helpers outside #[test] functions panic on bad fixtures too.
#![allow(clippy::unwrap_used)]

use copilot_replay::{RecordingWriter, replay};
use copilot_webtiles::Envelope;

/// Writes a Recording from a fixture of envelopes, one JSON object per line.
fn recording(fixture: &str) -> Vec<u8> {
    let mut writer = RecordingWriter::new(Vec::new(), "0.0.0-test").unwrap();
    for line in fixture.lines() {
        let envelope: Envelope = serde_json::from_str(line).unwrap();
        writer.write(&envelope).unwrap();
    }
    writer.finish().unwrap()
}

#[test]
fn the_panel_shows_the_character_hp_and_turn_as_the_game_is_played() {
    let replay =
        replay(&recording(include_str!("recordings/minotaur-first-steps.jsonl"))[..]).unwrap();

    insta::assert_json_snapshot!(replay.view_models);
}

#[test]
fn a_game_is_identified_by_server_account_character_species_and_when_it_was_first_seen() {
    let replay =
        replay(&recording(include_str!("recordings/minotaur-first-steps.jsonl"))[..]).unwrap();

    insta::assert_json_snapshot!(replay.identity);
}

#[test]
fn a_spectated_game_belongs_to_the_watched_account() {
    let replay =
        replay(&recording(include_str!("recordings/spectating-trunk-wizard.jsonl"))[..]).unwrap();

    insta::assert_json_snapshot!(replay.identity);
}

#[test]
fn the_panel_follows_the_tab_whose_game_last_changed_and_starts_afresh_on_a_new_game() {
    let replay =
        replay(&recording(include_str!("recordings/two-tabs-and-a-new-game.jsonl"))[..]).unwrap();

    let characters: Vec<_> = replay
        .view_models
        .into_iter()
        .map(|view| view.character)
        .collect();
    insta::assert_json_snapshot!(characters);
}
