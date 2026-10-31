//! Seam 1 for Coach's Tactical refinement: a Recording and its LLM sidecar go in; the prompts the
//! Advisor rendered, what it kept of each answer, and what the panel shows of it come out.

// Helpers outside #[test] functions panic on bad fixtures too.
#![allow(clippy::unwrap_used)]

use std::path::{Path, PathBuf};

use copilot_core::{AssistanceLevel, CoachLine, Coached, Dropped, QuestionId, SuggestedActionCard};
use copilot_llm::{
    LlmProvider, LlmRequest, LlmResponse, LlmStream, ModelChoice, ModelSlot, Part, SidecarReplayer,
    StopReason, StreamEvent, Usage, sidecar_path,
};
use copilot_replay::{Happened, Recording, Replayer};
use futures_util::StreamExt;
use futures_util::stream;

/// LETHAL as the ogre closes in at turn 5, with a scroll of teleportation the best escape; at 6,
/// hit below a quarter of their hit points, the potion of heal wounds heals them out of it.
const OGRE: &str = "ogre-corners-a-minotaur-carrying-escapes";
/// DANGEROUS as the oklob plant spits from range at turn 2: fall back to the stairs.
const OKLOB: &str = "oklob-plant-spits-from-range";
/// A jackal pack on open ground: fall back to a corridor.
const JACKALS: &str = "jackal-pack-corners-a-minotaur-in-a-corridor";
/// DANGEROUS as a second hobgoblin joins the first beside an Ice Elementalist at turn 1: fight
/// carefully, opening with Freeze.
const FREEZER: &str = "ice-elementalist-freezes-hobgoblins";

fn scenario(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("tests/scenarios/{name}.dcssrec"))
}

fn quick_model() -> ModelChoice {
    ModelChoice::default_for(ModelSlot::Quick)
}

/// A replay of the scenario `name` in which the Coach is asked through `provider`.
fn coached(name: &str, provider: impl LlmProvider + 'static) -> Replayer {
    let recording = Recording::read(std::fs::File::open(scenario(name)).unwrap()).unwrap();
    Replayer::new(recording)
        .unwrap()
        .coaching(provider, quick_model())
}

/// Moves `replayer` to just after the first answer to the Coach arrived. An answer arrives with
/// the first envelope applied once it is due.
fn just_after_the_first_answer(replayer: &mut Replayer) {
    let mut position = 0;
    while replayer.answers_delivered() == 0 {
        position += 1;
        let applied = replayer.seek(position);
        assert!(
            !applied.envelopes.is_empty(),
            "no answer arrived before the Recording ended"
        );
    }
}

/// How long the ogre scenario's answers take to arrive, on its envelopes' clock: the answer to
/// turn 5's question is still on its way when turn 6 asks anew, and turn 6's arrives on turn 7,
/// the rules still healing. In the oklob scenario, turn 2's arrives on turn 3, the rules still
/// falling back to the stairs.
const SLOW_MS: u64 = 3_500;

/// How long the ogre scenario's first answer takes to arrive just after the ogre's hit has the
/// rules pick the potion in place of the scroll, still on turn 5.
const AFTER_THE_RULES_PICK_ANEW_MS: u64 = 3_075;

/// A provider that answers every call with `text`, at once.
struct Answering(String);

impl Answering {
    fn json(pick: &str, reason: &str) -> Self {
        Self(serde_json::json!({ "pick": pick, "reason": reason }).to_string())
    }
}

impl LlmProvider for Answering {
    fn stream(&self, request: LlmRequest) -> LlmStream {
        let response = LlmResponse {
            model: request.model,
            content: vec![Part::text(&self.0)],
            stop: StopReason::EndTurn,
            usage: Usage::default(),
        };
        stream::iter([Ok(StreamEvent::Finished(response))]).boxed()
    }
}

/// What the Coach's lane did over the replay, in order.
fn coached_events(replayer: &Replayer) -> Vec<Coached> {
    replayer
        .timeline()
        .iter()
        .filter_map(|moment| match &moment.event {
            Happened::Coached(coached) => Some(coached.clone()),
            _ => None,
        })
        .collect()
}

/// The Suggested Action card the panel shows at Coach.
fn card(replayer: &Replayer) -> SuggestedActionCard {
    replayer
        .view_model_at(AssistanceLevel::Coach)
        .game
        .unwrap()
        .suggested_action
        .unwrap()
}

/// Every prompt `replayer` sent, as text: the static prefix once, then each moment tail with the
/// turn it was asked on.
fn prompts(replayer: &Replayer) -> String {
    let calls = replayer.calls();
    let mut text = String::new();
    if let Some(first) = calls.first() {
        text.push_str("=== static prefix ===\n");
        text.push_str(&first.request.system[0].text);
        text.push('\n');
    }
    for call in calls {
        let Part::Text { text: tail, .. } = &call.request.messages[0].content[0] else {
            panic!("a moment tail is text");
        };
        text.push_str(&format!(
            "\n=== moment tail, turn {} ===\n{tail}\n",
            call.turn.unwrap()
        ));
    }
    text
}

#[test]
fn tactical_prompts_and_the_picks_kept_replay_from_each_scenarios_llm_sidecar() {
    for name in [OGRE, OKLOB, JACKALS] {
        let sidecar = SidecarReplayer::open(&sidecar_path(&scenario(name))).unwrap();
        let mut replayer = coached(name, sidecar);
        replayer.seek(usize::MAX);

        insta::assert_snapshot!(format!("coach-prompts-{name}"), prompts(&replayer));
        insta::assert_json_snapshot!(format!("coach-picks-{name}"), coached_events(&replayer));
    }
}

#[test]
fn the_coach_is_asked_only_at_a_tactical_alert_with_a_suggested_action() {
    let mut replayer = coached(OGRE, Answering::json("a1", "It works."));
    replayer.seek(usize::MAX);

    let turns: Vec<Option<u32>> = replayer.calls().iter().map(|call| call.turn).collect();
    assert_eq!(turns, [Some(5), Some(6)]);
}

#[test]
fn a_freeze_plan_is_a_candidate_action_the_coach_may_pick() {
    let mut replayer = coached(FREEZER, Answering::json("a1", "It works."));
    replayer.seek(usize::MAX);

    let prompts = prompts(&replayer);
    let fights: Vec<&str> = prompts
        .lines()
        .filter(|line| line.contains(" Fight · "))
        .collect();
    assert!(
        fights[0].starts_with("a1 Fight · Freeze, then melee: ~"),
        "{prompts}"
    );
    assert!(
        fights.iter().any(|line| line.contains(" Fight · Melee: ~")),
        "{prompts}"
    );
}

#[test]
fn a_replay_that_isnt_coached_asks_nothing() {
    let recording = Recording::read(std::fs::File::open(scenario(OGRE)).unwrap()).unwrap();
    let mut replayer = Replayer::new(recording).unwrap();
    replayer.seek(usize::MAX);

    assert!(replayer.calls().is_empty());
    assert!(coached_events(&replayer).is_empty());
}

#[test]
fn a_tactical_call_asks_the_quick_model_for_json_capped_at_about_twice_its_word_limit() {
    let mut replayer = coached(OGRE, Answering::json("a1", "It works."));
    replayer.seek(usize::MAX);
    let request = &replayer.calls()[0].request;

    assert_eq!(request.model, "claude-haiku-5-5");
    assert_eq!(
        request.max_tokens, 116,
        "25 words at ~2 tokens, doubled, and the JSON"
    );
    assert_eq!(
        request.system.len(),
        1,
        "the static prefix, cached; the tail is the user turn"
    );
    assert!(request.system[0].cache.is_some());
    assert!(request.system[0].text.contains("at most 25 words"));
    assert!(request.output.is_some());
    assert!(request.tools.is_empty());
}

#[test]
fn the_coachs_pick_shows_on_the_suggested_action_card_at_coach() {
    let mut replayer = coached(
        OGRE,
        Answering::json("a1", "The ogre is beside you. Read it now."),
    );
    just_after_the_first_answer(&mut replayer);

    assert_eq!(
        card(&replayer).coach,
        Some(CoachLine::Picked {
            action: card(&replayer).action,
            agrees: true,
            reason: Some("The ogre is beside you. Read it now.".to_owned()),
            as_of_turn: None,
        })
    );
    assert_eq!(
        replayer
            .view_model_at(AssistanceLevel::Hints)
            .game
            .unwrap()
            .suggested_action
            .unwrap()
            .coach,
        None,
        "Hints shows the rule's pick alone"
    );
}

#[test]
fn the_coach_may_pick_another_candidate_action_than_the_rules() {
    let mut replayer = coached(OGRE, Answering::json("a2", "Stand and fight."));
    just_after_the_first_answer(&mut replayer);

    let Some(CoachLine::Picked { action, agrees, .. }) = card(&replayer).coach else {
        panic!("the pick is kept");
    };
    assert!(!agrees);
    assert_eq!(action.text, "The Advisor can't judge this fight");
}

#[test]
fn the_card_says_the_coach_is_asked_until_it_answers_and_forgets_it_as_the_rules_pick_anew() {
    let mut replayer =
        coached(OGRE, Answering::json("a1", "Read it now.")).answering_after_ms(SLOW_MS);
    replayer.seek(usize::MAX);
    let asked = replayer.calls()[0].envelope;

    replayer.seek(asked + 1);
    assert_eq!(card(&replayer).coach, Some(CoachLine::Asking));

    // Still turn 5, but the ogre's hit has the rules pick the potion instead.
    let healing = replayer.seek_turn(6).unwrap().envelopes.end - 1;
    replayer.seek(healing);
    assert_eq!(card(&replayer).rule, "Heal out of it");
    assert_eq!(card(&replayer).coach, None);
}

#[test]
fn a_pick_of_an_id_the_prompt_never_listed_is_dropped_and_the_suggested_action_stands() {
    let mut replayer = coached(OGRE, Answering::json("a42", "Do the thing."));
    just_after_the_first_answer(&mut replayer);

    assert!(matches!(
        coached_events(&replayer).last(),
        Some(Coached::Dropped {
            why: Dropped::UnknownPick { pick },
            ..
        }) if pick == "a42"
    ));
    assert_eq!(card(&replayer).coach, None);
}

#[test]
fn a_sentence_naming_a_number_or_monster_the_prompt_doesnt_is_dropped_silently_and_the_pick_kept() {
    let mut replayer = coached(
        OGRE,
        Answering::json(
            "a1",
            "You have 37 turns left. An orc priest is coming. The ogre is beside you.",
        ),
    );
    just_after_the_first_answer(&mut replayer);

    let Some(CoachLine::Picked { reason, .. }) = card(&replayer).coach else {
        panic!("the pick is kept");
    };
    assert_eq!(reason.as_deref(), Some("The ogre is beside you."));
}

#[test]
fn a_reason_over_its_word_limit_is_trimmed_to_whole_sentences() {
    let mut replayer = coached(
        OGRE,
        Answering::json(
            "a1",
            "The ogre is beside you and hits hard, so do not stay to trade blows with it. Read the \
             scroll now. The ogre hits very hard indeed.",
        ),
    );
    just_after_the_first_answer(&mut replayer);

    let Some(CoachLine::Picked { reason, .. }) = card(&replayer).coach else {
        panic!("the pick is kept");
    };
    assert_eq!(
        reason.as_deref(),
        Some(
            "The ogre is beside you and hits hard, so do not stay to trade blows with it. Read the \
             scroll now."
        )
    );
}

#[test]
fn an_answer_that_isnt_the_json_asked_for_is_dropped_and_the_suggested_action_stands() {
    let mut replayer = coached(OGRE, Answering("{\"pick\": \"a1\", \"rea".to_owned()));
    just_after_the_first_answer(&mut replayer);

    assert!(matches!(
        coached_events(&replayer).last(),
        Some(Coached::Dropped {
            why: Dropped::Failed { .. },
            ..
        })
    ));
    assert_eq!(card(&replayer).coach, None);
}

#[test]
fn an_answer_arriving_after_the_rules_picked_anew_is_dropped() {
    let mut replayer = coached(OGRE, Answering::json("a1", "Read it now."))
        .answering_after_ms(AFTER_THE_RULES_PICK_ANEW_MS);
    replayer.seek(usize::MAX);

    assert!(matches!(
        coached_events(&replayer)[1],
        Coached::Dropped {
            question: QuestionId(1),
            why: Dropped::GameMovedOn,
        }
    ));
}

#[test]
fn an_answer_arriving_turns_later_is_kept_while_the_rules_pick_holds_with_the_turn_it_was_asked_on()
{
    let mut replayer =
        coached(OKLOB, Answering::json("a1", "Take the hatch.")).answering_after_ms(SLOW_MS);
    just_after_the_first_answer(&mut replayer);

    let card = card(&replayer);
    assert_eq!(card.rule, "Fall back to stairs");
    assert_eq!(
        card.coach,
        Some(CoachLine::Picked {
            action: card.action.clone(),
            agrees: true,
            reason: Some("Take the hatch.".to_owned()),
            as_of_turn: Some(2),
        }),
        "asked on turn 2, answered on turn 3"
    );
}

#[test]
fn a_newer_question_replaces_the_one_in_flight_whose_answer_is_never_used() {
    let mut replayer =
        coached(OGRE, Answering::json("a1", "Read it now.")).answering_after_ms(SLOW_MS);
    replayer.seek(usize::MAX);

    let asked = coached_events(&replayer)
        .iter()
        .filter(|event| matches!(event, Coached::Asked { .. }))
        .count();
    assert_eq!(asked, 2);
    assert_eq!(
        replayer.answers_delivered(),
        1,
        "the first call was cancelled as the second was made"
    );
}
