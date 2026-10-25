//! Seam 1: games remembered across app restarts. Each run of the app is a Replayer over the same
//! store, fed what the browser relays to it.

// Helpers outside #[test] functions panic on bad fixtures too.
#![allow(clippy::unwrap_used)]

use std::path::Path;

use copilot_core::{AccumulatedState, GameKey, Remembered, SessionEvent, SourceEvent};
use copilot_replay::{Happened, Recording, Replayer};
use copilot_store::Store;

/// Tiv, a kobold at turn 1234, opens every screen the Advisor reads.
const SCREENS: &[u8] = include_bytes!("recordings/screens-tiv-opens.dcssrec");
/// Later, Tiv's game at turn 1234 again: a page reload, a save, playing on, death at 1410, and
/// a new Tiv.
const SESSION: &[u8] = include_bytes!("recordings/session-tiv-reloads-saves-and-dies.dcssrec");
/// Rook reads his resistances on turn 101 and puts on a kite shield on 102.
const KITE_SHIELD: &[u8] = include_bytes!("recordings/rook-reads-his-kite-shield.dcssrec");

/// One run of the app, remembering games in the store at `path`, as the browser relays it
/// `recording`.
fn run(recording: &[u8], path: &Path) -> Replayer {
    run_of(Recording::read(recording).unwrap(), path)
}

fn run_of(recording: Recording, path: &Path) -> Replayer {
    Replayer::remembering(recording, Store::open(path).unwrap()).unwrap()
}

/// The whole of `recording`, taken in by an app that remembers nothing between runs.
fn straight(recording: &[u8]) -> Replayer {
    let mut replayer = Replayer::new(Recording::read(recording).unwrap()).unwrap();
    replayer.seek(usize::MAX);
    replayer
}

fn key(replayer: &Replayer) -> GameKey {
    replayer.advisor().identity().unwrap().key()
}

/// The Session Events the replayer's Game Source reported, with the envelope each came from.
fn sessions(replayer: &Replayer) -> Vec<(usize, SessionEvent)> {
    replayer
        .timeline()
        .iter()
        .filter_map(|moment| match moment.event {
            Happened::Source(SourceEvent::Session(session)) => Some((moment.envelope, session)),
            _ => None,
        })
        .collect()
}

/// Whether anything the Advisor remembers of the followed game is Known or Stale.
fn remembers_anything(replayer: &Replayer) -> bool {
    replayer.advisor().accumulated().unwrap() != &AccumulatedState::default()
}

#[test]
fn a_game_played_on_after_the_app_restarts_is_picked_up_with_what_was_remembered() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("games.sqlite");
    let mut before = run(SCREENS, &path);
    before.seek(usize::MAX);
    let remembered = before.advisor().accumulated().cloned().unwrap();
    assert!(matches!(remembered.skills, Remembered::Known(_)));
    let game = key(&before);
    drop(before);

    let mut after = run(SESSION, &path);
    // Just after the game is named, at turn 1234, where it was left.
    after.seek(4);

    assert_eq!(key(&after), game);
    assert_eq!(sessions(&after), [(3, SessionEvent::Resynced)]);
    assert_eq!(after.advisor().accumulated(), Some(&remembered));
}

#[test]
fn what_was_remembered_goes_stale_when_the_game_is_picked_up_at_a_later_turn() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("games.sqlite");
    run(SCREENS, &path).seek(usize::MAX);
    // Tiv played on, unwatched, from 1234 to 1300, saved, and plays on from there in the tab
    // logged in as tiv.
    let mut later = Recording::read(SESSION).unwrap();
    later.envelopes.drain(1..12);

    let mut after = run_of(later, &path);
    after.seek(3);

    let skills = &after.advisor().accumulated().unwrap().skills;
    assert!(matches!(skills, Remembered::Stale(read) if read.turn == Some(1234)));
}

#[test]
fn a_game_that_ended_is_never_picked_up_again() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("games.sqlite");
    run(SCREENS, &path).seek(usize::MAX);
    let mut died = run(SESSION, &path);
    died.seek(17);
    let dead = key(&died);
    drop(died);
    // The new Tiv, after the app restarted, in the tab logged in as tiv.
    let mut new = Recording::read(SESSION).unwrap();
    new.envelopes.drain(1..17);

    let mut after = run_of(new, &path);
    after.seek(usize::MAX);

    assert_ne!(key(&after), dead);
    assert_eq!(sessions(&after), [(2, SessionEvent::Joined)]);
    assert!(!remembers_anything(&after));
}

#[test]
fn a_game_whose_turn_went_backwards_is_a_new_game() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("games.sqlite");
    let mut first = run(SESSION, &path);
    // Played to turn 1410, before Tiv dies.
    first.seek(15);
    let left = key(&first);
    drop(first);
    // A day later, Tiv at turn 1234 again, as from a backed-up save.
    let mut backed_up = Recording::read(SCREENS).unwrap();
    for envelope in &mut backed_up.envelopes {
        envelope.ts += 86_400_000 + 100_000_000;
    }

    let mut after = run_of(backed_up, &path);
    after.seek(usize::MAX);

    assert_ne!(key(&after), left);
    assert_eq!(sessions(&after), [(3, SessionEvent::Joined)]);
}

#[test]
fn an_app_restarted_mid_game_takes_the_page_replayed_to_it_up_where_it_left_off() {
    let dir = tempfile::tempdir().unwrap();
    let (path, uninterrupted) = (
        dir.path().join("games.sqlite"),
        dir.path().join("uninterrupted.sqlite"),
    );
    // Quit just after the resistances are read on turn 101.
    run(KITE_SHIELD, &path).seek(8);

    // The browser replays the page from its first message to the restarted app.
    let mut restarted = run(KITE_SHIELD, &path);
    restarted.seek(usize::MAX);

    let straight = straight(KITE_SHIELD);
    assert_eq!(
        restarted.advisor().identity(),
        straight.advisor().identity()
    );
    assert_eq!(
        restarted.advisor().accumulated(),
        straight.advisor().accumulated()
    );
    assert_eq!(sessions(&restarted), [(3, SessionEvent::Joined)]);
    run(KITE_SHIELD, &uninterrupted).seek(usize::MAX);
    let game = key(&restarted);
    assert_eq!(
        Store::open(&path).unwrap().changes(&game).unwrap(),
        Store::open(&uninterrupted).unwrap().changes(&game).unwrap(),
        "each change is logged once"
    );
}

#[test]
fn a_game_watched_rather_than_played_is_never_saved() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("games.sqlite");

    run(
        include_bytes!("recordings/spectating-trunk-wizard.dcssrec"),
        &path,
    )
    .seek(usize::MAX);

    assert_eq!(Store::open(&path).unwrap().known_games().unwrap(), []);
}

#[test]
fn a_page_replayed_after_a_restart_picks_up_each_game_it_played_as_it_did_before() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("games.sqlite");
    let mut first = run(SESSION, &path);
    // Played to turn 1410, before Tiv dies.
    first.seek(15);
    let game = key(&first);
    drop(first);
    // The page since Tiv reloaded it at 1240: saving at 1300, then playing on in it.
    let mut page = Recording::read(SESSION).unwrap();
    page.envelopes = page.envelopes[5..15].to_vec();

    let mut restarted = run_of(page, &path);
    restarted.seek(usize::MAX);

    assert_eq!(key(&restarted), game);
    assert_eq!(
        sessions(&restarted),
        [
            (3, SessionEvent::Resynced),
            (5, SessionEvent::ConnectionLost),
            (8, SessionEvent::Resynced)
        ]
    );
}
