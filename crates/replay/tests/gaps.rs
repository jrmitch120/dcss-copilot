//! Seam 1 for Gaps: Recordings and Scenarios go in; the Gaps the Fact Sheet lists for a moment,
//! the values the panel marks in place, the Gaps chip and the prompt card come out.

// Helpers outside #[test] functions panic on bad fixtures too.
#![allow(clippy::unwrap_used)]

use copilot_core::{FocusCard, GameView, Gap, GapRow, NowPill, Value};
use copilot_replay::{Recording, Replayer};

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
fn fight_line(game: &GameView) -> Option<Value> {
    match &game.focus {
        FocusCard::Fight { line, .. } => Some(line.clone()),
        _ => None,
    }
}

/// The numbers behind the now pill's Threat Level, once it shows any.
fn now_detail(game: &GameView) -> Option<Value> {
    match &game.now {
        NowPill::Rated {
            detail: Some(detail),
            ..
        } => Some(detail.clone()),
        _ => None,
    }
}

/// A value the panel marks in place, with the tooltip saying what fills it.
fn marked(text: &str, gap: &str) -> Value {
    Value {
        text: text.to_owned(),
        gap: Some(gap.to_owned()),
    }
}

#[test]
fn the_fact_sheet_lists_the_gaps_that_affected_the_moment() {
    // The fire the bow adds is played as if the character resists none, as the character
    // overview is unread; the fight waits for the skills screen; the Ways Out for the ability
    // menu and the spell list.
    let replayer = replayed(CENTAUR);

    assert_eq!(
        replayer.advisor().fact_sheet().gaps,
        [
            Gap::AbilityMenu,
            Gap::SpellList,
            Gap::Skills,
            Gap::Resistances
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
            row(
                "Ability menu",
                "Your abilities aren't counted among your Ways Out.",
                &["a"]
            ),
            row(
                "Spell list",
                "Your spells aren't counted among your Ways Out or attacks.",
                &["I"]
            ),
            row("Skills", "No fight can be judged without them.", &["m"]),
            row(
                "Resistances",
                "Monsters' fire, cold, poison and the like are played as if you resist none.",
                &["%"]
            ),
        ]
    );
    assert_eq!(chip.dump_fills, ["skills", "the spell list", "resistances"]);
}

#[test]
fn the_gaps_chip_lists_the_games_gaps_and_blind_spots_not_only_the_moments() {
    // At its end the Conjurer follows Ashenzari, with the spell list read again since the
    // spell they memorised, but not its power, and a rat beside them, whose bite no resistance
    // changes: the character overview is unread all the same.
    let replayer = replayed(CONJURER);
    assert!(
        !replayer
            .advisor()
            .fact_sheet()
            .gaps
            .contains(&Gap::Resistances)
    );
    let chip = replayer.view_model().game.unwrap().gaps;

    assert_eq!(
        chip.gaps,
        [
            row(
                "Ability menu",
                "Your abilities aren't counted among your Ways Out.",
                &["a"]
            ),
            row(
                "Resistances",
                "Monsters' fire, cold, poison and the like are played as if you resist none.",
                &["%"]
            ),
            row(
                "~Spell power as of turn 0",
                "Fights play it as it was.",
                &["I", "!"]
            ),
            row(
                "Skills",
                "Ashenzari shifts them with your piety, so they may differ from the skills \
                 screen's.",
                &[]
            ),
        ]
    );
    assert_eq!(chip.dump_fills, ["resistances"]);
}

#[test]
fn a_spectator_is_told_of_the_gaps_but_not_asked_to_press_keys_in_a_game_not_theirs() {
    let replayer = replayed(SPECTATING);
    let chip = replayer.view_model().game.unwrap().gaps;

    assert!(!chip.gaps.is_empty());
    assert!(chip.gaps.iter().all(|row| row.keys.is_empty()), "{chip:?}");
    assert!(chip.dump_fills.is_empty(), "{chip:?}");
}
