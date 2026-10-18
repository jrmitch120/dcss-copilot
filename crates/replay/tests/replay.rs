//! Seam 1: a Recording goes in; the view model, Visible State and game identity come out.

// Helpers outside #[test] functions panic on bad fixtures too.
#![allow(clippy::unwrap_used)]

use copilot_core::{Position, Screen, SessionEvent, SourceEvent, VisibleState};
use copilot_gk::Library;
use copilot_replay::{Applied, Recording, Replayer, replay};

const MINOTAUR: &[u8] = include_bytes!("recordings/minotaur-first-steps.dcssrec");
const SPECTATING: &[u8] = include_bytes!("recordings/spectating-trunk-wizard.dcssrec");
const TWO_TABS: &[u8] = include_bytes!("recordings/two-tabs-and-a-new-game.dcssrec");
const MONSTERS: &[u8] = include_bytes!("recordings/monsters-in-view.dcssrec");
const NO_GAME_KNOWLEDGE: &[u8] =
    include_bytes!("recordings/monsters-without-game-knowledge.dcssrec");
const KOBOLD: &[u8] = include_bytes!("recordings/kobold-on-d3.dcssrec");
const SCREENS: &[u8] = include_bytes!("recordings/screens-tiv-opens.dcssrec");
const WATCHED_SCREENS: &[u8] = include_bytes!("recordings/screens-watched-mid-menu.dcssrec");
const SESSION: &[u8] = include_bytes!("recordings/session-tiv-reloads-saves-and-dies.dcssrec");
const STOPPED_UNNAMED: &[u8] =
    include_bytes!("recordings/session-tiv-reloads-and-crashes-before-naming-the-game.dcssrec");
const JACKALS: &[u8] = include_bytes!("recordings/jackals-close-in.dcssrec");
const KITE_SHIELD: &[u8] = include_bytes!("recordings/rook-reads-his-kite-shield.dcssrec");
const HALOS_AND_UMBRAS: &[u8] = include_bytes!("recordings/rook-in-halos-and-umbras.dcssrec");
const ENTERS_WIZARD_MODE: &[u8] = include_bytes!("scenarios/ogre-beside-a-new-minotaur.dcssrec");
const AMONG_FUNGI: &[u8] = include_bytes!("scenarios/jackals-among-fungi.dcssrec");

fn replayer(recording: &[u8]) -> Replayer {
    Replayer::new(Recording::read(recording).unwrap()).unwrap()
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
fn a_game_is_a_wizard_mode_game_from_the_moment_the_player_enters_wizard_mode() {
    let mut replayer = replayer(ENTERS_WIZARD_MODE);
    let mut changes: Vec<(usize, bool)> = Vec::new();
    while !replayer.at_end() {
        replayer.step();
        if let Some(wizard) = replayer
            .advisor()
            .identity()
            .map(|identity| identity.wizard)
            && changes.last().map(|(_, last)| *last) != Some(wizard)
        {
            changes.push((replayer.position(), wizard));
        }
    }

    // Just after the game's first `player` message, then just after the one crawl sends once
    // "wiz" is confirmed.
    assert_eq!(changes, [(20, false), (37, true)]);
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
fn without_game_knowledge_for_the_crawl_version_no_monster_in_view_is_taken_for_firewood() {
    let mut replayer =
        Replayer::with_knowledge(Recording::read(AMONG_FUNGI).unwrap(), Library::default());
    replayer.seek(usize::MAX);

    insta::assert_json_snapshot!(replayer.view_model().game.unwrap().focus);
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
fn what_a_screen_showed_is_remembered_after_it_closes_as_of_the_turn_it_was_read() {
    let mut replayer = replayer(SCREENS);
    replayer.seek(6);
    assert_eq!(replayer.advisor().visible().screen, None);

    insta::assert_json_snapshot!(replayer.advisor().accumulated());
}

#[test]
fn a_spectator_sees_the_screens_already_open_and_a_redrawn_menu_closes_once() {
    let replay = replay(WATCHED_SCREENS).unwrap();

    insta::assert_json_snapshot!(replay.timeline);
    assert_eq!(replay.visible.screen, None);
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
fn the_followed_tabs_stream_stopping_before_the_game_is_named_is_still_reported() {
    let replay = replay(STOPPED_UNNAMED).unwrap();

    let sessions: Vec<_> = replay
        .timeline
        .iter()
        .filter_map(|moment| match moment.event {
            SourceEvent::Session(session) => Some((moment.envelope, session)),
            _ => None,
        })
        .collect();
    assert_eq!(
        sessions,
        [(3, SessionEvent::Joined), (6, SessionEvent::ConnectionLost)]
    );
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

/// The Fact Sheet as each turn of `recording` ended, by turn.
fn fact_sheets_by_turn(recording: &[u8]) -> Vec<serde_json::Value> {
    let mut replayer = replayer(recording);
    let mut sheets: Vec<(Option<u32>, serde_json::Value)> = Vec::new();
    while !replayer.at_end() {
        replayer.step();
        let turn = replayer.turn();
        let sheet = serde_json::to_value(replayer.advisor().fact_sheet()).unwrap();
        match sheets.last_mut() {
            Some((last, last_sheet)) if *last == turn => *last_sheet = sheet,
            _ => sheets.push((turn, sheet)),
        }
    }
    sheets
        .into_iter()
        .map(|(turn, sheet)| serde_json::json!({ "turn": turn, "factSheet": sheet }))
        .collect()
}

#[test]
fn the_threat_level_rises_as_monsters_close_in_and_hit_points_fall() {
    insta::assert_json_snapshot!(fact_sheets_by_turn(JACKALS));
}

#[test]
fn a_hostile_in_reach_is_judged_by_crawls_melee_and_a_neutral_one_is_not() {
    let replayer = {
        let mut replayer = replayer(KOBOLD);
        replayer.seek(usize::MAX);
        replayer
    };

    insta::assert_json_snapshot!(replayer.advisor().fact_sheet());
}

#[test]
fn without_game_knowledge_for_the_crawl_version_the_threat_is_unrated_and_says_why() {
    let replayer = {
        let mut replayer = replayer(NO_GAME_KNOWLEDGE);
        replayer.seek(usize::MAX);
        replayer
    };

    insta::assert_json_snapshot!(replayer.advisor().fact_sheet());
}

#[test]
fn the_now_pill_shows_the_threat_level_and_one_line() {
    let pills: Vec<_> = replay(JACKALS)
        .unwrap()
        .view_models
        .into_iter()
        .filter_map(|view| view.game.map(|game| game.now))
        .collect();

    insta::assert_json_snapshot!(pills);
}

#[test]
fn a_worn_shield_read_off_the_character_overview_blocks_as_often_as_crawl_lets_it() {
    // Turn 100: the shield isn't known, so one block is assumed. Turn 101: `%` shows a kite
    // shield, which blocks up to three attacks. Turn 102: SH changed, so what was read is Stale.
    let mut replayer = replayer(KITE_SHIELD);
    let mut by_turn: Vec<serde_json::Value> = Vec::new();
    while !replayer.at_end() {
        replayer.step();
        let turn = serde_json::json!({
            "turn": replayer.turn(),
            "wornShield": replayer.advisor().accumulated().map(|state| &state.worn_shield),
            "factSheet": replayer.advisor().fact_sheet(),
        });
        // As each turn ended.
        match by_turn.last_mut() {
            Some(last) if last["turn"] == turn["turn"] => *last = turn,
            _ => by_turn.push(turn),
        }
    }

    insta::assert_json_snapshot!(by_turn);
}

#[test]
fn the_light_on_the_player_is_read_off_the_game_as_crawl_judges_it() {
    // Rook, three jackals beside him, turn by turn:
    // 100: plain light. 101: a halo reaches his square: backlit. 102: the square's tile changes
    // without a halo value, which keeps it: still backlit. 103: he follows the Shining One, whose
    // halo is his own: plain. 104: under penance, it is gone: backlit. 105: no god, an umbra on
    // his square, but liquid fire burns hot (`Fire++`): backlit. 106: the fire is out: umbra.
    // 107: a halo again, and he carries the morningstar "Eos", not known to be wielded: backlit,
    // and his own halo not modelled. 108: `%` shows Eos wielded: plain. 109: the halo leaves his
    // square, so what he wields goes Stale: plain. 110: a halo again: backlit, his own halo not
    // modelled. 111: a heavenly storm is a halo of his own: plain.
    let mut replayer = replayer(HALOS_AND_UMBRAS);
    let mut by_turn: Vec<serde_json::Value> = Vec::new();
    while !replayer.at_end() {
        replayer.step();
        let threat =
            serde_json::to_value(replayer.advisor().fact_sheet()).unwrap()["threat"].clone();
        let turn = serde_json::json!({
            "turn": replayer.turn(),
            "lighting": threat["lighting"],
            "ownHaloNotModelled": threat["notModelled"]
                .as_array()
                .is_some_and(|entries| entries.contains(&"ownHalo".into())),
            "equipped": replayer.advisor().accumulated().map(|state| &state.equipped),
        });
        // As each turn ended.
        match by_turn.last_mut() {
            Some(last) if last["turn"] == turn["turn"] => *last = turn,
            _ => by_turn.push(turn),
        }
    }

    insta::assert_json_snapshot!(by_turn);
}

/// Every test Recording in `recordings/`, tagged with the Crawl Versions its games report, in the order
/// first seen.
const TAGGED: [(&str, &[u8], &[&str]); 13] = [
    ("minotaur-first-steps", MINOTAUR, &["0.34.1"]),
    (
        "spectating-trunk-wizard",
        SPECTATING,
        &["0.35-a0-412-g28a137d"],
    ),
    ("two-tabs-and-a-new-game", TWO_TABS, &["0.34.1", "0.33.1"]),
    ("monsters-in-view", MONSTERS, &["0.34.1"]),
    (
        "monsters-without-game-knowledge",
        NO_GAME_KNOWLEDGE,
        &["0.35-a0-512-g1a2b3c4"],
    ),
    ("kobold-on-d3", KOBOLD, &["0.34.1"]),
    ("screens-tiv-opens", SCREENS, &["0.34.1"]),
    ("screens-watched-mid-menu", WATCHED_SCREENS, &["0.34.1"]),
    ("session-tiv-reloads-saves-and-dies", SESSION, &["0.34.1"]),
    (
        "session-tiv-reloads-and-crashes-before-naming-the-game",
        STOPPED_UNNAMED,
        &["0.34.1"],
    ),
    ("jackals-close-in", JACKALS, &["0.34.1"]),
    ("rook-reads-his-kite-shield", KITE_SHIELD, &["0.34.1"]),
    ("rook-in-halos-and-umbras", HALOS_AND_UMBRAS, &["0.34.1"]),
];

#[test]
fn every_test_recording_reports_the_crawl_versions_it_is_tagged_with() {
    for (name, recording, tagged) in TAGGED {
        let reported: Vec<_> = replay(recording)
            .unwrap()
            .crawl_versions
            .into_iter()
            .map(|version| version.0)
            .collect();

        assert_eq!(reported, tagged, "{name}");
    }
}

#[test]
fn every_test_recording_is_tagged() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/recordings");
    let mut committed: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter_map(|path| Some(path.file_stem()?.to_str()?.to_owned()))
        .collect();
    committed.sort();
    let mut tagged: Vec<String> = TAGGED
        .iter()
        .map(|(name, _, _)| (*name).to_owned())
        .collect();
    tagged.sort();

    assert_eq!(committed, tagged);
}
