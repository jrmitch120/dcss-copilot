//! Seam 1 for the detectors: a Recording goes in; what they noticed at each Settled Update, and
//! what the panel shows of it, comes out.

// Helpers outside #[test] functions panic on bad fixtures too.
#![allow(clippy::unwrap_used)]

use copilot_core::{AlertCause, Detection, NowPill, ThreatLevel, ViewModel};
use copilot_replay::{Happened, Moment, Recording, Replayer, replay};

const JACKALS_FALL: &[u8] = include_bytes!("recordings/jackals-close-in-and-fall.dcssrec");
const EXCEPTIONAL: &[u8] = include_bytes!("recordings/sigmund-follows-an-ogre-into-view.dcssrec");
const STATUSES: &[u8] =
    include_bytes!("recordings/rook-poisoned-then-confused-beside-lava.dcssrec");
const JOINED: &[u8] = include_bytes!("recordings/joined-as-jackals-bite.dcssrec");
const OKLOB: &[u8] = include_bytes!("scenarios/oklob-plant-spits-from-range.dcssrec");
/// A Recording without the game's `input_mode` messages, so no update of it ever settles.
const NEVER_SETTLES: &[u8] = include_bytes!("recordings/jackals-close-in.dcssrec");

fn replayed(recording: &[u8]) -> Replayer {
    let mut replayer = Replayer::new(Recording::read(recording).unwrap()).unwrap();
    replayer.seek(usize::MAX);
    replayer
}

/// What the detectors noticed over the whole of `recording`, in order.
fn detections(recording: &[u8]) -> Vec<Moment> {
    replayed(recording).detections().cloned().collect()
}

/// The causes of the Alerts raised on `turn`.
fn alerts_on(detections: &[Moment], turn: u32) -> Vec<AlertCause> {
    detections
        .iter()
        .filter(|moment| moment.turn == Some(turn))
        .filter_map(|moment| match &moment.event {
            Happened::Detected(Detection::Alert(alert)) => Some(alert.cause.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn an_encounter_starts_as_hostiles_come_into_view_and_ends_three_clear_actions_after_they_fall() {
    // Turn 101: jackals come into view. 102: they bite, and the Threat Level rises. 105: the last
    // falls; the update that follows at 105 is no action. 107: the third clear action.
    insta::assert_json_snapshot!(detections(JACKALS_FALL));
}

#[test]
fn at_most_one_alert_is_raised_at_a_player_action_and_the_rest_wait_for_the_next() {
    // Turn 103: hit points fall below half and a quarter as the Threat Level rises to LETHAL. At
    // 104 they are still below a quarter, which says all that below half would.
    let detections = detections(JACKALS_FALL);

    assert_eq!(
        alerts_on(&detections, 103),
        [AlertCause::ThreatLevelRose {
            to: ThreatLevel::Lethal
        }]
    );
    assert_eq!(
        alerts_on(&detections, 104),
        [AlertCause::HitPointsBelowQuarter]
    );
}

#[test]
fn joining_a_fight_in_progress_starts_an_encounter_and_says_how_dangerous_it_is() {
    insta::assert_json_snapshot!(detections(JOINED));
}

#[test]
fn the_now_pill_shows_an_alert_until_the_player_acts_again() {
    let mut alerts: Vec<(Option<u32>, Option<String>)> = Vec::new();
    for view in replay(JACKALS_FALL).unwrap().view_models {
        let Some(game) = view.game else { continue };
        let alert = match game.now {
            NowPill::Rated { alert, .. } | NowPill::Unrated { alert } => alert,
        };
        let turn = game.character.turn;
        match alerts.last_mut() {
            Some((last, shown)) if *last == turn => *shown = alert,
            _ => alerts.push((turn, alert)),
        }
    }

    insta::assert_json_snapshot!(alerts);
}

#[test]
fn a_unique_or_out_of_depth_monster_coming_into_view_has_a_fact_card_and_an_alert_only_if_dangerous_alone()
 {
    // Turn 201: an ogre, out of depth on D:1 but far off. 202: Sigmund. 203: an orc warrior, out
    // of depth and in reach.
    insta::assert_json_snapshot!(detections(EXCEPTIONAL));
}

#[test]
fn the_panel_shows_a_fact_card_for_each_unique_or_out_of_depth_monster_in_view() {
    let replayer = replayed(EXCEPTIONAL);

    insta::assert_json_snapshot!(replayer.view_model().game.unwrap().fact_cards);
}

#[test]
fn a_dangerous_status_landing_starts_an_encounter_and_raises_an_alert() {
    // Turn 301: lethal poison, cured at 302. 305: confusion, far from the lava. 306: a step beside
    // it. 307: liquid fire.
    insta::assert_json_snapshot!(detections(STATUSES));
}

#[test]
fn a_redraw_without_a_map_settles_only_as_the_player_acts_again() {
    // Turn 301's redraw sends the player but no map, so the game settles as the next key is
    // pressed: crawl's `input_mode` leaving its command wait.
    let replayer = replayed(STATUSES);
    let poisoned = replayer
        .timeline()
        .iter()
        .find(|moment| moment.turn == Some(301))
        .unwrap();

    let settled_by = &replayer.recording().envelopes[poisoned.envelope].msg;
    assert_eq!(settled_by["msg"], "input_mode");
    assert_eq!(settled_by["mode"], 0);
}

#[test]
fn the_detectors_judge_only_settled_updates() {
    assert_eq!(detections(NEVER_SETTLES), []);
}

/// Each LETHAL turn of `views`, with whether the Toolbar Badge calls for the panel then. It never
/// does on any other turn.
fn lethal_turns(views: Vec<ViewModel>) -> Vec<(Option<u32>, bool)> {
    let mut turns = Vec::new();
    for game in views.into_iter().filter_map(|view| view.game) {
        let lethal = matches!(
            game.now,
            NowPill::Rated {
                level: ThreatLevel::Lethal,
                ..
            }
        );
        assert!(lethal || !game.badge.calls_for_panel, "{game:?}");
        let turn = (game.character.turn, game.badge.calls_for_panel);
        if lethal && turns.last() != Some(&turn) {
            turns.push(turn);
        }
    }
    turns
}

#[test]
fn the_toolbar_badge_calls_for_the_panel_only_at_a_lethal_moment_with_no_trusted_way_out() {
    // Turns 103 to 105: LETHAL, with no staircase known and nothing carried to escape with.
    assert_eq!(
        lethal_turns(replay(JACKALS_FALL).unwrap().view_models),
        [(Some(103), true), (Some(104), true), (Some(105), true)]
    );
    // Turns 3 and 4: LETHAL, but an escape hatch in view can be taken.
    assert_eq!(
        lethal_turns(replay(OKLOB).unwrap().view_models),
        [(Some(3), false), (Some(4), false)]
    );
}
