//! Seam 1 for message-driven updates: Scenarios whose games tell of skills, spells, god's powers,
//! mutations and banes changing go in; what the Advisor remembers of the character, and what the
//! panel says of facts out of date, come out.

// Helpers outside #[test] functions panic on bad fixtures too.
#![allow(clippy::unwrap_used)]

use copilot_core::{AccumulatedState, FocusCard, Remembered, Screen, Source};
use copilot_replay::{Recording, Replayer};

/// A Minotaur Berserker who reads their screens, then hears of their mutations, a bane, Trog's
/// powers and their skills, and reads the mutations and skills screens again.
const BERSERKER: &[u8] =
    include_bytes!("scenarios/berserker-told-of-mutations-banes-powers-and-skills.dcssrec");

/// A Deep Elf Conjurer who memorises a spell, meets a rat, forgets Magic Dart and joins
/// Ashenzari.
const CONJURER: &[u8] =
    include_bytes!("scenarios/conjurer-memorises-forgets-and-joins-ashenzari.dcssrec");

fn replayer(recording: &[u8]) -> Replayer {
    Replayer::new(Recording::read(recording).unwrap()).unwrap()
}

fn remembered(replayer: &Replayer) -> &AccumulatedState {
    replayer.advisor().accumulated().unwrap()
}

/// Steps until the game has shown a message holding `said`.
fn step_until_said(replayer: &mut Replayer, said: &str) {
    while !replayer.at_end() {
        replayer.step();
        let messages = &replayer.advisor().visible().messages;
        if messages.iter().any(|message| message.text.contains(said)) {
            return;
        }
    }
    panic!("the game never said {said:?}");
}

/// The descriptions of the remembered mutations, Known or Stale, in order.
fn mutations(remembered: &AccumulatedState) -> Vec<String> {
    let mut shown = remembered
        .mutations
        .read_value()
        .unwrap()
        .shown
        .mutations
        .clone();
    shown.sort();
    shown
}

/// The banes remembered, Known or Stale.
fn banes(remembered: &AccumulatedState) -> Vec<String> {
    remembered
        .mutations
        .read_value()
        .unwrap()
        .shown
        .banes
        .clone()
        .unwrap()
}

/// The source and turn of a fact Known as of now.
fn known_from<T>(fact: &Remembered<T>) -> (&Source, Option<u32>) {
    match fact {
        Remembered::Known(read) => (&read.source, read.turn),
        other => panic!("not Known: {:?}", std::mem::discriminant(other)),
    }
}

/// The fight card's Gaps, once the panel shows one.
fn fight_gaps(replayer: &mut Replayer) -> Vec<String> {
    while !replayer.at_end() {
        if let Some(game) = replayer.view_model().game
            && let FocusCard::Fight { gaps, .. } = game.focus
        {
            return gaps;
        }
        replayer.step();
    }
    panic!("the panel never showed a fight card");
}

/// Steps until the screen that `fact` was last read off is `screen`.
fn step_until_read<T>(
    replayer: &mut Replayer,
    fact: impl Fn(&AccumulatedState) -> &Remembered<T>,
    screen: Screen,
) {
    let read_off = Source::Screen(screen);
    while !replayer.at_end() {
        replayer.step();
        if let Remembered::Known(read) = fact(remembered(replayer))
            && read.source == read_off
        {
            return;
        }
    }
    panic!("never read off {read_off:?}");
}

#[test]
fn mutation_messages_keep_the_mutations_as_the_mutations_screen_shows_them() {
    let mut replayer = replayer(BERSERKER);
    step_until_said(&mut replayer, "You feel resistant to cold.");
    let (source, _) = known_from(&remembered(&replayer).mutations);
    assert_eq!(source, &Source::Message);
    assert!(mutations(remembered(&replayer)).contains(&"You are cold resistant. (rC+)".to_owned()));

    step_until_said(&mut replayer, "You feel more resistant to cold.");
    assert!(
        mutations(remembered(&replayer))
            .contains(&"You are very cold resistant. (rC++)".to_owned())
    );

    step_until_said(&mut replayer, "You feel less cold resistant.");
    // "You feel robust." is also what crawl says as frailty is lost: there is none to lose.
    step_until_said(&mut replayer, "You feel robust.");
    let told = mutations(remembered(&replayer));

    step_until_read(&mut replayer, |state| &state.mutations, Screen::Mutations);
    assert_eq!(told, mutations(remembered(&replayer)));
}

#[test]
fn a_bane_is_remembered_from_its_message_until_it_is_lifted() {
    let mut replayer = replayer(BERSERKER);
    step_until_said(&mut replayer, "You are stricken with the Bane of Lethargy.");
    assert_eq!(banes(remembered(&replayer)), ["You cover ground slowly."]);

    step_until_said(&mut replayer, "The Bane of Lethargy upon you is lifted.");
    assert!(banes(remembered(&replayer)).is_empty());
    assert!(matches!(
        remembered(&replayer).mutations,
        Remembered::Known(_)
    ));
}

#[test]
fn a_skill_message_sets_that_skills_level_and_the_skills_screen_agrees() {
    let mut replayer = replayer(BERSERKER);
    step_until_said(&mut replayer, "Your Dodging skill gained 2 levels");
    let skills = remembered(&replayer).skills.read_value().unwrap().clone();
    let told = ["Fighting", "Axes", "Armour", "Dodging"];
    assert_eq!(told.map(|name| skills.tenths(name)), [50, 50, 50, 40]);
    // The experience level that came with them is no sign of a skill changing unsaid.
    step_until_said(&mut replayer, "You have reached level 2!");
    let (source, _) = known_from(&remembered(&replayer).skills);
    assert_eq!(source, &Source::Message);

    // Crawl says only a skill's base level, which the skills screen shows unless something such
    // as crosstraining raises it: Maces & Flails rises unsaid as Axes trains here.
    step_until_read(&mut replayer, |state| &state.skills, Screen::Skills);
    let read = remembered(&replayer).skills.known().unwrap().clone();
    for name in told {
        assert_eq!(read.tenths(name) / 10, skills.tenths(name) / 10, "{name}");
    }
}

#[test]
fn a_gods_power_coming_puts_the_ability_menu_out_of_date() {
    let mut replayer = replayer(BERSERKER);
    step_until_said(&mut replayer, "You can now call in reinforcements.");
    assert!(matches!(
        remembered(&replayer).abilities,
        Remembered::Stale(_)
    ));
}

#[test]
fn a_spell_memorised_unnamed_is_a_blind_spot_on_the_fight_card_until_the_spell_list_is_read() {
    let mut replayer = replayer(CONJURER);
    step_until_said(&mut replayer, "You finish memorising.");
    assert!(matches!(remembered(&replayer).spells, Remembered::Stale(_)));
    assert!(remembered(&replayer).spell_memorised.is_some());

    step_until_said(&mut replayer, "You encounter a rat.");
    insta::assert_json_snapshot!(fight_gaps(&mut replayer));

    // The scroll of amnesia's menu lists the spells.
    step_until_said(&mut replayer, "Your memory of Magic Dart unravels.");
    assert_eq!(remembered(&replayer).spell_memorised, None);
}

/// The scroll of amnesia's menu lists the spells; the scroll used up, the list is Stale.
#[test]
fn a_spell_forgotten_leaves_the_spell_list() {
    let mut replayer = replayer(CONJURER);
    step_until_said(&mut replayer, "Your memory of Magic Dart unravels.");
    let remembered = remembered(&replayer);
    let Remembered::Stale(read) = &remembered.spells else {
        panic!("{:?}", remembered.spells);
    };
    assert_eq!(read.source, Source::Screen(Screen::Spells));
    let names: Vec<&str> = read.value.iter().map(|spell| spell.name.as_str()).collect();
    assert_eq!(names, ["Searing Ray"]);
}

#[test]
fn an_ashenzari_followers_skills_may_differ_from_the_skills_screen() {
    let mut replayer = replayer(CONJURER);
    step_until_said(&mut replayer, "Ashenzari welcomes you!");
    step_until_said(&mut replayer, "The rat bites you.");
    insta::assert_json_snapshot!(fight_gaps(&mut replayer));
}
