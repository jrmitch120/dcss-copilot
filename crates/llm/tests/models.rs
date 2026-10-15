//! The Quick and Deep slots: which models a player can choose, and how they may think.

// Helpers outside #[test] functions panic on a broken setup too.
#![allow(clippy::unwrap_used)]

use copilot_llm::{Effort, ModelChoice, ModelChoiceError, ModelSlot, Provider, Thinking};
use serde_json::json;

#[test]
fn the_quick_model_is_haiku_without_thinking_and_the_deep_model_is_sonnet_thinking_adaptively() {
    let quick = ModelChoice::default_for(ModelSlot::Quick);
    let deep = ModelChoice::default_for(ModelSlot::Deep);

    assert_eq!(
        (quick.provider(), quick.model(), quick.thinking()),
        (Provider::Anthropic, "claude-haiku-5-5", Thinking::Off)
    );
    assert_eq!(
        (deep.provider(), deep.model(), deep.thinking()),
        (
            Provider::Anthropic,
            "claude-sonnet-5-5",
            Thinking::Adaptive(Effort::Medium)
        )
    );
}

#[test]
fn a_model_can_only_be_chosen_with_thinking_it_accepts() {
    let choice = ModelChoice::new(Provider::Anthropic, "claude-sonnet-5-5", Thinking::Off);

    assert_eq!(
        choice,
        Err(ModelChoiceError::Thinking {
            model: "claude-sonnet-5-5".to_owned(),
            thinking: Thinking::Off
        })
    );
}

#[test]
fn a_model_the_provider_doesnt_offer_cant_be_chosen() {
    let choice = ModelChoice::new(Provider::Anthropic, "claude-haiku-3", Thinking::Off);

    assert_eq!(
        choice,
        Err(ModelChoiceError::UnknownModel {
            provider: Provider::Anthropic,
            model: "claude-haiku-3".to_owned()
        })
    );
}

#[test]
fn a_saved_choice_round_trips_and_an_invalid_one_doesnt_load() {
    let choice = ModelChoice::new(
        Provider::Anthropic,
        "claude-opus-5-5",
        Thinking::Adaptive(Effort::High),
    )
    .unwrap();
    let saved = serde_json::to_value(&choice).unwrap();

    assert_eq!(
        saved,
        json!({ "provider": "anthropic", "model": "claude-opus-5-5", "thinking": { "adaptive": "high" } })
    );
    assert_eq!(
        serde_json::from_value::<ModelChoice>(saved).unwrap(),
        choice
    );
    let invalid = json!({ "provider": "anthropic", "model": "claude-opus-5-5", "thinking": "off" });
    assert!(serde_json::from_value::<ModelChoice>(invalid).is_err());
}

#[test]
fn every_model_offered_takes_at_least_one_thinking_setting() {
    for model in Provider::Anthropic.models() {
        assert!(
            !model.thinking.is_empty(),
            "{} has no thinking settings",
            model.id
        );
        for &thinking in model.thinking {
            assert!(ModelChoice::new(Provider::Anthropic, model.id, thinking).is_ok());
        }
    }
}
