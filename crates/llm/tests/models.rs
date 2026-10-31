//! The Quick and Deep slots: which models a player can choose, and how they may think.

// Helpers outside #[test] functions panic on a broken setup too.
#![allow(clippy::unwrap_used)]

use copilot_llm::{Access, Effort, ModelChoice, ModelChoiceError, ModelSlot, Provider, Thinking};
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
fn every_model_offered_suggests_a_thinking_setting_it_accepts() {
    for provider in Provider::ALL {
        for model in provider.models() {
            assert!(
                !model.thinking.is_empty(),
                "{} has no thinking settings",
                model.id
            );
            assert!(model.thinking.contains(&model.suggested), "{}", model.id);
            for &thinking in model.thinking {
                assert!(ModelChoice::new(provider, model.id, thinking).is_ok());
            }
        }
    }
}

#[test]
fn every_provider_with_a_key_lists_a_model_its_key_can_be_tested_with() {
    for provider in Provider::ALL {
        if matches!(provider.access(), Access::Key { .. }) {
            assert!(!provider.models().is_empty(), "{provider:?}");
        }
    }
}

#[test]
fn a_player_can_name_any_model_of_a_provider_with_an_open_catalogue() {
    let choice = ModelChoice::new(
        Provider::OpenRouter,
        "mistralai/some-new-model",
        Thinking::Off,
    );

    assert_eq!(choice.unwrap().model(), "mistralai/some-new-model");
    assert_eq!(
        ModelChoice::new(Provider::OpenAi, "  ", Thinking::Off),
        Err(ModelChoiceError::NoModel {
            provider: Provider::OpenAi
        })
    );
}

#[test]
fn a_listed_model_of_an_open_catalogue_still_only_takes_the_thinking_it_accepts() {
    assert!(ModelChoice::new(Provider::OpenAi, "gpt-6-luna", Thinking::Off).is_ok());
    assert_eq!(
        ModelChoice::new(Provider::OpenAi, "gpt-6-astra", Thinking::Off),
        Err(ModelChoiceError::Thinking {
            model: "gpt-6-astra".to_owned(),
            thinking: Thinking::Off
        })
    );
}

#[test]
fn a_local_model_is_any_the_server_has_and_never_thinks() {
    assert!(ModelChoice::new(Provider::Ollama, "qwen3:8b", Thinking::Off).is_ok());
    assert!(matches!(
        ModelChoice::new(
            Provider::LlamaServer,
            "qwen3.gguf",
            Thinking::Adaptive(Effort::Low)
        ),
        Err(ModelChoiceError::Thinking { .. })
    ));
}

#[test]
fn only_local_models_are_experimental_and_they_need_an_address_rather_than_a_key() {
    for provider in Provider::ALL {
        match provider.access() {
            Access::Key { .. } => assert!(!provider.is_local(), "{provider:?}"),
            Access::Address { .. } => assert!(provider.is_local(), "{provider:?}"),
        }
    }
    let local: Vec<Provider> = Provider::ALL.into_iter().filter(|p| p.is_local()).collect();
    assert_eq!(local, [Provider::Ollama, Provider::LlamaServer]);
    assert_eq!(
        Provider::OpenAi.access(),
        Access::Key {
            variable: "OPENAI_API_KEY"
        }
    );
    assert_eq!(
        Provider::OpenRouter.access(),
        Access::Key {
            variable: "OPENROUTER_API_KEY"
        }
    );
    assert_eq!(
        Provider::Ollama.access(),
        Access::Address {
            default: "http://localhost:11434"
        }
    );
}

#[test]
fn a_saved_provider_and_its_id_are_the_same_word_for_every_provider() {
    for provider in Provider::ALL {
        assert_eq!(
            serde_json::to_value(provider).unwrap(),
            json!(provider.id())
        );
        assert_eq!(Provider::from_id(provider.id()), Some(provider));
    }
}

#[test]
fn a_saved_choice_of_a_model_the_app_doesnt_list_still_loads() {
    let saved = json!({ "provider": "ollama", "model": "qwen3:8b", "thinking": "off" });

    let choice: ModelChoice = serde_json::from_value(saved.clone()).unwrap();

    assert_eq!(choice.provider(), Provider::Ollama);
    assert_eq!(serde_json::to_value(&choice).unwrap(), saved);
}
