//! Seam 1 for Gaps: Recordings and Scenarios go in; the Gaps the Fact Sheet lists for a moment,
//! the values the panel marks in place, the Gaps chip and the prompt card come out.

// Helpers outside #[test] functions panic on bad fixtures too.
#![allow(clippy::unwrap_used)]

use copilot_core::{
    Detection, FocusCard, GameView, Gap, GapList, GapRow, MarkedValue, NowPill, ScreenFact,
};
use copilot_replay::{Happened, Recording, Replayer};
use copilot_store::Store;

/// A Minotaur Berserker, who has read none of their screens, shot at with a longbow of flaming.
const CENTAUR: &[u8] = include_bytes!("scenarios/centaur-shoots-a-flaming-longbow.dcssrec");

/// A Deep Elf Conjurer who reads their spell list, memorises a spell the game doesn't name, then
/// meets a rat.
const CONJURER: &[u8] =
    include_bytes!("scenarios/conjurer-memorises-forgets-and-joins-ashenzari.dcssrec");

/// A Minotaur Berserker, who has read none of their screens, beside an ogre at a DANGEROUS
/// moment.
const OGRE: &[u8] = include_bytes!("scenarios/ogre-beside-a-new-minotaur.dcssrec");

fn replayer(recording: &[u8]) -> Replayer {
    Replayer::new(Recording::read(recording).unwrap()).unwrap()
}

fn replayed(recording: &[u8]) -> Replayer {
    let mut replayer = replayer(recording);
    replayer.seek(usize::MAX);
    replayer
}

/// The panel at the first moment `shown` picks something out of it.
fn first_shown<T>(recording: &[u8], shown: impl Fn(&GameView) -> Option<T>) -> T {
    let mut replayer = replayer(recording);
    while !replayer.at_end() {
        replayer.step();
        if let Some(found) = replayer.view_model().game.as_ref().and_then(&shown) {
            return found;
        }
    }
    panic!("the panel never showed it");
}

/// The fight card's line, once the panel shows one.
fn fight_line(game: &GameView) -> Option<MarkedValue> {
    match &game.focus {
        FocusCard::Fight { line, .. } => Some(line.clone()),
        _ => None,
    }
}

/// The numbers behind the now pill's Threat Level, once it shows any.
fn now_detail(game: &GameView) -> Option<MarkedValue> {
    match &game.now {
        NowPill::Rated {
            detail: Some(detail),
            ..
        } => Some(detail.clone()),
        _ => None,
    }
}

/// A value the panel marks in place, with the tooltip saying what fills it.
fn marked(text: &str, gap: &str) -> MarkedValue {
    MarkedValue {
        text: text.to_owned(),
        gap: Some(gap.to_owned()),
    }
}

#[test]
fn the_fact_sheet_lists_the_gaps_that_affected_the_moment() {
    // The fire the bow adds is played as if the character resists none, as the character
    // overview is unread; the fight waits for the skills screen; the Ways Out for the ability
    // menu and the spell list; and the race to the stairs counts only the species' mutations.
    let replayer = replayed(CENTAUR);

    assert_eq!(
        replayer.advisor().fact_sheet().gaps,
        [
            Gap::Unread {
                fact: ScreenFact::Skills
            },
            Gap::Unread {
                fact: ScreenFact::SpellList
            },
            Gap::Unread {
                fact: ScreenFact::AbilityMenu
            },
            Gap::Unread {
                fact: ScreenFact::Mutations
            },
            Gap::Unread {
                fact: ScreenFact::Resistances
            }
        ]
    );
}

#[test]
fn a_fight_the_advisor_cant_judge_is_marked_with_the_screen_that_fills_it() {
    assert_eq!(
        first_shown(CENTAUR, fight_line),
        marked(
            "The Advisor can't judge a fight yet.",
            "Open the skills screen (m) to judge a fight."
        )
    );
}

#[test]
fn a_fight_judged_on_a_spell_list_out_of_date_is_marked_as_of_when_it_was_read() {
    assert_eq!(
        first_shown(CONJURER, fight_line),
        marked(
            "Best plan (Magic Dart, then melee): ~100% win, ~0% HP lost",
            "~Spell list as of turn 0: open it (I) to bring it up to date. ~Spell power as of turn \
             0: open the spell list (I) and press ! to bring it up to date. You memorised a spell \
             on turn 5 that the game didn't name: open the spell list (I) to count it."
        )
    );
}

#[test]
fn the_threat_judged_as_if_the_player_resists_nothing_is_marked_until_the_overview_is_read() {
    let detail = first_shown(CENTAUR, now_detail);

    assert_eq!(
        detail.gap.as_deref(),
        Some(
            "Open the character overview (%) to read your resistances: until then you're taken \
             to resist nothing."
        )
    );
}

#[test]
fn a_suggested_fight_the_advisor_cant_judge_is_marked_with_the_screen_that_fills_it() {
    let action = first_shown(OGRE, |game| {
        game.suggested_action
            .as_ref()
            .map(|suggested| suggested.action.clone())
    });

    assert_eq!(
        action,
        marked(
            "The Advisor can't judge this fight",
            "Open the skills screen (m) to judge a fight."
        )
    );
}

#[test]
fn the_threat_from_damage_no_resistance_changes_is_not_marked() {
    // The ogre only clubs: unread resistances change nothing it does.
    let detail = first_shown(OGRE, now_detail);

    assert_eq!(detail.gap, None, "{detail:?}");
}

/// A spectator watching a wizard-mode game on trunk.
const SPECTATING: &[u8] = include_bytes!("recordings/spectating-trunk-wizard.dcssrec");

/// A row of the Gaps chip: what is missing, why it matters and the keys that fill it.
fn row(what: &str, why: &str, keys: &[&str]) -> GapRow {
    GapRow {
        what: what.to_owned(),
        why: why.to_owned(),
        keys: keys.iter().map(|&key| key.to_owned()).collect(),
    }
}

#[test]
fn the_gaps_chip_lists_each_gap_why_it_matters_and_its_key() {
    let replayer = replayed(CENTAUR);
    let chip = replayer.view_model().game.unwrap().gaps;

    assert_eq!(
        chip.gaps,
        [
            row("Skills", "No fight can be judged without them.", &["m"]),
            row(
                "Spell list",
                "Spells aren't counted among the Ways Out or attacks.",
                &["I"]
            ),
            row(
                "Ability menu",
                "Abilities aren't counted among the Ways Out.",
                &["a"]
            ),
            row(
                "Mutations",
                "The race to the stairs counts only the species' own mutations.",
                &["A"]
            ),
            row(
                "Resistances",
                "Monsters' fire, cold, poison and the like are played as if resisted by nothing.",
                &["%"]
            ),
            row(
                "Equipped gear",
                "A shield, a halo or armour that slows the character is taken as not worn.",
                &["%"]
            ),
        ]
    );
    assert_eq!(
        chip.dump_fills,
        [
            "skills",
            "the spell list",
            "mutations",
            "resistances",
            "equipped gear"
        ]
    );
}

#[test]
fn the_gaps_chip_lists_the_games_gaps_and_blind_spots_not_only_the_moments() {
    // At its end the Conjurer follows Ashenzari, with the spell list read again since the
    // spell they memorised, but not its power, and a rat beside them, whose bite no resistance
    // changes: the character overview is unread all the same.
    let replayer = replayed(CONJURER);
    assert!(
        !replayer.advisor().fact_sheet().gaps.contains(&Gap::Unread {
            fact: ScreenFact::Resistances
        })
    );
    let chip = replayer.view_model().game.unwrap().gaps;

    assert_eq!(
        chip.gaps,
        [
            row(
                "Ability menu",
                "Abilities aren't counted among the Ways Out.",
                &["a"]
            ),
            row(
                "Mutations",
                "The race to the stairs counts only the species' own mutations.",
                &["A"]
            ),
            row(
                "Resistances",
                "Monsters' fire, cold, poison and the like are played as if resisted by nothing.",
                &["%"]
            ),
            row(
                "Equipped gear",
                "A shield, a halo or armour that slows the character is taken as not worn.",
                &["%"]
            ),
            row(
                "~Spell power as of turn 0",
                "Fights play it as it was.",
                &["I", "!"]
            ),
            row(
                "Skills",
                "Ashenzari shifts them with piety, so they may differ from the skills screen's.",
                &[]
            ),
        ]
    );
    assert_eq!(
        chip.dump_fills,
        ["mutations", "resistances", "equipped gear"]
    );
}

#[test]
fn a_spectator_is_told_of_the_gaps_but_not_asked_to_press_keys_in_a_game_not_theirs() {
    let replayer = replayed(SPECTATING);
    let chip = replayer.view_model().game.unwrap().gaps;

    assert!(!chip.gaps.is_empty());
    assert!(chip.gaps.iter().all(|row| row.keys.is_empty()), "{chip:?}");
    // The game isn't theirs: why each Gap matters is said of the character, not of them.
    assert!(
        chip.gaps
            .iter()
            .all(|row| !row.why.to_lowercase().contains("you")),
        "{chip:?}"
    );
    assert!(chip.dump_fills.is_empty(), "{chip:?}");
}

/// Rook, joined on turn 300, is poisoned, All Clear on 304, then confused beside lava on 306.
const STATUSES: &[u8] =
    include_bytes!("recordings/rook-poisoned-then-confused-beside-lava.dcssrec");

/// Steps until `reached` holds of the replay.
fn step_until(replayer: &mut Replayer, reached: impl Fn(&Replayer) -> bool) {
    while !replayer.at_end() {
        replayer.step();
        if reached(replayer) {
            return;
        }
    }
    panic!("the replay never got there");
}

/// Whether the detectors have noticed anything matching `detection` so far.
fn noticed(replayer: &Replayer, detection: impl Fn(&Detection) -> bool) -> bool {
    replayer.detections().any(|moment| match &moment.event {
        Happened::Detected(detected) => detection(detected),
        Happened::Source(_) | Happened::Coached(_) => false,
    })
}

/// The Gap prompt card, while the panel shows it.
fn prompt(replayer: &Replayer) -> Option<GapList> {
    match replayer.view_model().game?.focus {
        FocusCard::GapPrompt { gaps } => Some(gaps),
        _ => None,
    }
}

/// Rook's game, with the join card put away as it first asks for the dump.
fn rook_without_join_card(mut replayer: Replayer) -> Replayer {
    step_until(&mut replayer, |replayer| {
        replayer
            .view_model()
            .game
            .is_some_and(|game| matches!(game.focus, FocusCard::Join { .. }))
    });
    replayer.close_join_card();
    replayer
}

fn all_clear(replayer: &Replayer) -> bool {
    noticed(replayer, |detected| matches!(detected, Detection::AllClear))
}

#[test]
fn a_prompt_card_asks_at_the_first_all_clear_for_the_gaps_that_affected_a_moment() {
    // Nothing hostile came into view, so no fight or damage was judged, and no staircase was
    // raced to: only the spell list and the ability menu, unread, were missing from the Ways Out.
    let mut replayer = rook_without_join_card(replayer(STATUSES));
    step_until(&mut replayer, all_clear);

    let card = prompt(&replayer).unwrap();
    let chip = replayer.view_model().game.unwrap().gaps;
    assert!(
        card.gaps.iter().all(|row| chip.gaps.contains(row)),
        "{card:?}"
    );
    assert!(card.gaps.iter().all(|row| !row.keys.is_empty()), "{card:?}");
    let asked = replayer
        .detections()
        .find_map(|moment| match &moment.event {
            Happened::Detected(Detection::GapPrompt { asks }) => Some((moment.turn, asks.clone())),
            _ => None,
        });
    assert_eq!(
        asked,
        Some((
            Some(304),
            vec![ScreenFact::SpellList, ScreenFact::AbilityMenu]
        ))
    );

    // It goes as the next Encounter starts.
    step_until(&mut replayer, |replayer| {
        replayer
            .detections()
            .filter(|moment| moment.event == Happened::Detected(Detection::EncounterStarted))
            .count()
            == 2
    });
    assert_eq!(prompt(&replayer), None);
    replayer.seek(usize::MAX);
    assert_eq!(prompt(&replayer), None);
}

#[test]
fn no_prompt_card_asks_while_the_join_card_does() {
    let mut replayer = replayer(STATUSES);
    step_until(&mut replayer, all_clear);

    assert!(matches!(
        replayer.view_model().game.unwrap().focus,
        FocusCard::Join { .. }
    ));
    assert!(!noticed(&replayer, |detected| matches!(
        detected,
        Detection::GapPrompt { .. }
    )));
}

#[test]
fn the_player_can_put_the_prompt_card_away() {
    let mut replayer = rook_without_join_card(replayer(STATUSES));
    step_until(&mut replayer, all_clear);
    assert!(prompt(&replayer).is_some());

    replayer.dismiss_gap_prompt();

    assert_eq!(prompt(&replayer), None);
    replayer.step();
    assert_eq!(prompt(&replayer), None);
}

#[test]
fn a_gap_a_prompt_card_asked_for_is_never_asked_for_again_in_that_game() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("games.sqlite");
    let run = || {
        let recording = Recording::read(STATUSES).unwrap();
        Replayer::remembering(recording, Store::open(&path).unwrap()).unwrap()
    };
    let mut before = rook_without_join_card(run());
    step_until(&mut before, all_clear);
    assert!(prompt(&before).is_some());
    drop(before);

    // The app restarts, and the browser relays the game again.
    let mut after = rook_without_join_card(run());
    step_until(&mut after, all_clear);

    assert_eq!(prompt(&after), None);
    assert!(!noticed(&after, |detected| matches!(
        detected,
        Detection::GapPrompt { .. }
    )));
}

#[test]
fn the_fight_cards_other_plans_are_marked_as_its_best_plan_is() {
    let (line, others) = first_shown(CONJURER, |game| match &game.focus {
        FocusCard::Fight { line, others, .. } => Some((line.clone(), others.clone())),
        _ => None,
    });

    assert!(line.gap.is_some(), "{line:?}");
    assert!(!others.is_empty());
    assert!(
        others.iter().all(|other| other.gap == line.gap),
        "{others:?}"
    );
}

/// A Minotaur Berserker, who has read none of their screens, racing a centaur to the stairs.
const STAIR_RACE: &[u8] = include_bytes!("scenarios/centaur-cuts-off-the-stairs.dcssrec");

/// Rook reads his resistances on turn 101 and puts on a kite shield on 102.
const KITE_SHIELD: &[u8] = include_bytes!("recordings/rook-reads-his-kite-shield.dcssrec");

#[test]
fn unread_mutations_are_a_gap_when_they_may_change_the_stair_race_and_mark_the_stairs() {
    let replayer = replayed(STAIR_RACE);
    let ways_out = replayer.view_model().game.unwrap().ways_out.unwrap();
    let stairs = ways_out
        .rows
        .iter()
        .find(|row| row.name.contains("staircase"))
        .unwrap();
    assert_eq!(
        stairs
            .detail
            .as_ref()
            .and_then(|detail| detail.gap.as_deref()),
        Some("Open the mutations screen (A) to judge your speed to the stairs.")
    );

    assert!(
        replayer.advisor().fact_sheet().gaps.contains(&Gap::Unread {
            fact: ScreenFact::Mutations
        }),
        "{:?}",
        replayer.advisor().fact_sheet().gaps
    );
}

#[test]
fn what_the_player_wears_is_a_gap_while_their_shield_is_unknown() {
    let mut replayer = replayer(KITE_SHIELD);
    let worn_gaps = |replayer: &Replayer| -> Vec<Gap> {
        replayer
            .advisor()
            .fact_sheet()
            .gaps
            .iter()
            .filter(|gap| {
                matches!(
                    gap,
                    Gap::Unread {
                        fact: ScreenFact::Equipped
                    } | Gap::Stale {
                        fact: ScreenFact::Equipped,
                        ..
                    }
                )
            })
            .cloned()
            .collect()
    };
    replayer.seek_turn(101).unwrap();
    let before_overview = worn_gaps(&replayer);
    replayer.seek(usize::MAX);

    assert_eq!(
        before_overview,
        [Gap::Unread {
            fact: ScreenFact::Equipped
        }]
    );
    assert_eq!(
        worn_gaps(&replayer),
        [Gap::Stale {
            fact: ScreenFact::Equipped,
            turn: Some(101)
        }]
    );
}

#[test]
fn a_way_out_list_never_read_is_an_unknown_row_marked_in_place() {
    let replayer = replayed(CENTAUR);
    let ways_out = replayer.view_model().game.unwrap().ways_out.unwrap();
    let unknown: Vec<(String, Option<bool>, Option<MarkedValue>)> = ways_out
        .rows
        .iter()
        .filter(|row| row.usable.is_none())
        .map(|row| (row.name.clone(), row.usable, row.detail.clone()))
        .collect();

    assert_eq!(
        unknown,
        [
            (
                "Abilities".to_owned(),
                None,
                Some(marked(
                    "unknown",
                    "Open the ability menu (a) to judge your abilities."
                ))
            ),
            (
                "Spells".to_owned(),
                None,
                Some(marked(
                    "unknown",
                    "Open the spell list (I) to judge your spells."
                ))
            ),
        ]
    );
    assert!(
        ways_out
            .gaps
            .iter()
            .all(|line| !line.contains("ability menu") && !line.contains("spell list")),
        "{:?}",
        ways_out.gaps
    );
}

#[test]
fn the_panel_is_warning_the_player_while_an_alert_shows_or_the_moment_is_above_calm() {
    // Rook is All Clear on 304, at CALM, then confused beside lava on 306, still at CALM, with an
    // Alert.
    let mut replayer = replayer(STATUSES);
    step_until(&mut replayer, all_clear);
    assert!(!replayer.view_model().game.unwrap().warning);

    step_until(&mut replayer, |replayer| {
        replayer
            .view_model()
            .game
            .is_some_and(|game| match game.now {
                NowPill::Rated { alert, .. } | NowPill::Unrated { alert } => alert.is_some(),
            })
    });
    assert!(replayer.view_model().game.unwrap().warning);
}
