//! Seam 1: a Recording goes in; the view model, Visible State and game identity come out.

// Helpers outside #[test] functions panic on bad fixtures too.
#![allow(clippy::unwrap_used)]

use copilot_core::{Position, Screen, SourceEvent, VisibleState};
use copilot_replay::{Applied, Recording, Replayer, replay};

const MINOTAUR: &[u8] = include_bytes!("recordings/minotaur-first-steps.dcssrec");
const SPECTATING: &[u8] = include_bytes!("recordings/spectating-trunk-wizard.dcssrec");
const TWO_TABS: &[u8] = include_bytes!("recordings/two-tabs-and-a-new-game.dcssrec");
const MONSTERS: &[u8] = include_bytes!("recordings/monsters-in-view.dcssrec");
const NO_GAME_KNOWLEDGE: &[u8] =
    include_bytes!("recordings/monsters-without-game-knowledge.dcssrec");
const KOBOLD: &[u8] = include_bytes!("recordings/kobold-on-d3.dcssrec");
const SCREENS: &[u8] = include_bytes!("recordings/screens-tiv-opens.dcssrec");
const SESSION: &[u8] = include_bytes!("recordings/session-tiv-reloads-saves-and-dies.dcssrec");

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
fn the_current_levels_map_holds_the_squares_in_view_and_those_remembered() {
    let replay = replay(KOBOLD).unwrap();

    insta::assert_json_snapshot!(replay.visible.map);
}

#[test]
fn each_square_of_the_map_is_decoded_to_its_dungeon_feature() {
    let map = replay(KOBOLD).unwrap().visible.map.unwrap();

    let feature = |x, y| {
        map.cell(Position { x, y })
            .and_then(|cell| cell.feature.clone())
            .map(|feature| feature.0)
    };
    assert_eq!(feature(2, 0).as_deref(), Some("DNGN_STONE_STAIRS_DOWN_I"));
    assert_eq!(feature(3, 1).as_deref(), Some("DNGN_OPEN_DOOR"));
    assert_eq!(feature(-3, 0).as_deref(), Some("DNGN_ROCK_WALL"));
    // A monster stands on the floor.
    assert_eq!(feature(1, -1).as_deref(), Some("DNGN_FLOOR"));
}

#[test]
fn messages_are_the_recent_lines_as_plain_text_after_the_game_takes_any_back() {
    let replay = replay(KOBOLD).unwrap();

    insta::assert_json_snapshot!(replay.visible.messages);
}

#[test]
fn each_screen_the_player_opens_is_read_as_plain_text() {
    let replay = replay(SCREENS).unwrap();

    insta::assert_json_snapshot!(replay.timeline);
}

#[test]
fn a_console_style_screen_is_read_once_crawl_has_drawn_it() {
    let replay = replay(SCREENS).unwrap();

    let skills: Vec<_> = replay
        .timeline
        .iter()
        .filter_map(|moment| match &moment.event {
            SourceEvent::ScreenRead(read) if read.screen == Screen::Skills => Some(read),
            _ => None,
        })
        .collect();
    assert_eq!(skills.len(), 1);
    assert_eq!(skills[0].lines[0], "Tiv the Kobold Berserker");
}

#[test]
fn the_character_overview_gives_the_resistances() {
    let replay = replay(SCREENS).unwrap();

    let resistances = replay
        .timeline
        .iter()
        .find_map(|moment| match &moment.event {
            SourceEvent::ScreenRead(read) => read.resistances(),
            _ => None,
        });
    insta::assert_json_snapshot!(resistances);
}

#[test]
fn the_character_dump_gives_the_same_resistances_as_the_overview() {
    let replay = replay(SCREENS).unwrap();

    let resistances_on = |screen: Screen| {
        replay
            .timeline
            .iter()
            .find_map(|moment| match &moment.event {
                SourceEvent::ScreenRead(read) if read.screen == screen => read.resistances(),
                _ => None,
            })
    };
    let from_dump = resistances_on(Screen::CharacterDump);
    assert!(from_dump.is_some());
    assert_eq!(from_dump, resistances_on(Screen::CharacterOverview));
}

#[test]
fn a_screen_is_visible_while_it_is_open() {
    let mut replayer = replayer(SCREENS);
    let open = |replayer: &Replayer| {
        replayer
            .advisor()
            .visible()
            .screen
            .as_ref()
            .map(|read| read.screen.clone())
    };

    replayer.seek(5);
    assert_eq!(open(&replayer), Some(Screen::CharacterOverview));
    replayer.step();
    assert_eq!(open(&replayer), None);
    replayer.seek(16);
    assert_eq!(
        open(&replayer),
        Some(Screen::Other("describe-monster".to_owned()))
    );
    replayer.step();
    assert_eq!(open(&replayer), Some(Screen::DungeonOverview));
    while !replayer.at_end() {
        replayer.step();
    }
    assert_eq!(open(&replayer), None);
}

#[test]
fn session_events_tell_joining_resyncing_losing_and_ending_a_game() {
    let replay = replay(SESSION).unwrap();

    insta::assert_json_snapshot!(replay.timeline);
}

#[test]
fn a_reloaded_game_is_the_same_game_and_a_new_character_is_not() {
    let mut replayer = replayer(SESSION);
    let first_seen = |replayer: &Replayer| replayer.advisor().identity().unwrap().first_seen_ms;

    replayer.seek(4);
    let started = first_seen(&replayer);
    replayer.seek(9);
    assert_eq!(first_seen(&replayer), started, "after the page reload");
    replayer.seek(14);
    assert_eq!(
        first_seen(&replayer),
        started,
        "after playing on from the save"
    );
    replayer.seek(19);
    assert_ne!(
        first_seen(&replayer),
        started,
        "a new character of the same name"
    );
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
