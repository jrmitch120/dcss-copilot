//! Settings → Models and keys: the Quick and Deep slots, each provider's key, and testing it.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::Instant;

use copilot_llm::{
    ApiKey, Effort, LlmError, LlmProvider, LlmRequest, Message, ModelChoice, ModelSlot, Provider,
    SidecarRecorder, Thinking, complete,
};
use serde::{Deserialize, Serialize};
use specta::Type;

use crate::keys::{KeyError, KeySource, Keys};

/// Makes a live provider for a key. The app's calls Anthropic; tests replay an LLM sidecar.
pub type Connect = Box<dyn Fn(Provider, ApiKey) -> Box<dyn LlmProvider> + Send + Sync>;

/// The most a key test may answer with: one word is enough.
const KEY_TEST_MAX_TOKENS: u32 = 16;

/// The players' keys, and how each one's last test went.
pub struct ModelsAndKeys {
    keys: Mutex<Keys>,
    tests: Mutex<HashMap<Provider, KeyTest>>,
    connect: Connect,
}

impl ModelsAndKeys {
    /// Holds `keys`, calling providers through `connect`.
    pub fn new(keys: Keys, connect: Connect) -> Self {
        Self {
            keys: Mutex::new(keys),
            tests: Mutex::new(HashMap::new()),
            connect,
        }
    }

    /// The Models and keys section, with the slots set to `quick` and `deep`.
    pub fn view(&self, quick: &ModelChoice, deep: &ModelChoice) -> ModelsAndKeysView {
        let keys = lock(&self.keys);
        let tests = lock(&self.tests);
        ModelsAndKeysView {
            quick: slot_view(ModelSlot::Quick, quick),
            deep: slot_view(ModelSlot::Deep, deep),
            keys: Provider::ALL
                .into_iter()
                .map(|provider| KeyView {
                    provider: provider_view(provider),
                    key: match keys.get(provider) {
                        None => KeyState::Missing {
                            variable: provider.key_variable().to_owned(),
                        },
                        Some(held) => KeyState::Stored {
                            masked: held.key.masked(),
                            place: match held.source {
                                KeySource::Keychain => KeyPlace::Keychain,
                                KeySource::Session { reason } => KeyPlace::Session { reason },
                                KeySource::Environment { variable } => KeyPlace::Environment {
                                    variable: variable.to_owned(),
                                },
                            },
                        },
                    },
                    test: tests.get(&provider).cloned().unwrap_or(KeyTest::Untested),
                })
                .collect(),
        }
    }

    /// The player pasted a key for `provider`. Its last test no longer applies.
    ///
    /// # Errors
    /// If the key is empty.
    pub fn save_key(&self, provider: Provider, key: &str) -> Result<(), KeyError> {
        lock(&self.keys).save(provider, ApiKey::new(key))?;
        lock(&self.tests).remove(&provider);
        Ok(())
    }

    /// The player removed their key for `provider`.
    ///
    /// # Errors
    /// If the keychain couldn't delete it, so it would return next launch.
    pub fn remove_key(&self, provider: Provider) -> Result<(), KeyError> {
        lock(&self.tests).remove(&provider);
        lock(&self.keys).remove(provider)
    }

    /// Marks `provider`'s key as being tested, so the player sees it straight away.
    pub fn testing(&self, provider: Provider) {
        lock(&self.tests).insert(provider, KeyTest::Testing);
    }

    /// Tests `provider`'s key by asking its cheapest model for one word, recording the call in
    /// `sidecar` when a dev build is recording. The outcome shows in the view.
    pub async fn test_key(&self, provider: Provider, sidecar: Option<PathBuf>) {
        let outcome = self.run_test(provider, sidecar).await;
        lock(&self.tests).insert(provider, outcome);
    }

    async fn run_test(&self, provider: Provider, sidecar: Option<PathBuf>) -> KeyTest {
        let Some(held) = lock(&self.keys).get(provider) else {
            return KeyTest::Failed {
                message: format!("There is no {} key to test.", provider.name()),
            };
        };
        let mut live = (self.connect)(provider, held.key);
        if let Some(path) = sidecar {
            match SidecarRecorder::create(live, &path) {
                Ok(recorder) => live = Box::new(recorder),
                Err(error) => {
                    return KeyTest::Failed {
                        message: format!("The call couldn't be recorded: {error}"),
                    };
                }
            }
        }
        let (request, model_name) = key_test_request(provider);
        let started = Instant::now();
        match complete(live.as_ref(), request).await {
            Ok(_) => KeyTest::Works {
                model: model_name.to_owned(),
                millis: u32::try_from(started.elapsed().as_millis()).unwrap_or(u32::MAX),
            },
            Err(LlmError::KeyRejected { message, .. }) => KeyTest::Rejected { message },
            Err(error) => KeyTest::Failed {
                message: error.to_string(),
            },
        }
    }
}

/// The call that tests a key: the provider's first model, asked for one word.
fn key_test_request(provider: Provider) -> (LlmRequest, &'static str) {
    let models = provider.models();
    let (id, name, thinking) = models.first().map_or(("", "", Thinking::Off), |model| {
        (model.id, model.name, model.suggested)
    });
    let request = LlmRequest {
        thinking,
        ..LlmRequest::new(
            id,
            KEY_TEST_MAX_TOKENS,
            vec![Message::user("Reply with the single word OK.")],
        )
    };
    (request, name)
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    // Each change leaves the state whole, so a panic elsewhere can't leave it half-changed.
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

fn provider_view(provider: Provider) -> ProviderView {
    ProviderView {
        id: provider.id().to_owned(),
        name: provider.name().to_owned(),
    }
}

fn slot_view(slot: ModelSlot, choice: &ModelChoice) -> SlotView {
    SlotView {
        slot,
        provider: provider_view(choice.provider()),
        model: choice.model().to_owned(),
        thinking: ThinkingOption::from(choice.thinking()),
        models: choice
            .provider()
            .models()
            .iter()
            .map(|model| ModelOption {
                id: model.id.to_owned(),
                name: model.name.to_owned(),
                thinking: model
                    .thinking
                    .iter()
                    .copied()
                    .map(ThinkingOption::from)
                    .collect(),
                suggested: model.suggested.into(),
            })
            .collect(),
    }
}

/// Settings → Models and keys, as the Settings window draws it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ModelsAndKeysView {
    /// The Quick Model slot.
    pub quick: SlotView,
    /// The Deep Model slot.
    pub deep: SlotView,
    /// Each provider's key.
    pub keys: Vec<KeyView>,
}

/// One slot's model, and what it could be changed to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SlotView {
    /// Which slot this is.
    pub slot: ModelSlot,
    /// The slot's provider.
    pub provider: ProviderView,
    /// The model's id.
    pub model: String,
    /// How it thinks.
    pub thinking: ThinkingOption,
    /// The models the provider offers, with the thinking each accepts.
    pub models: Vec<ModelOption>,
}

/// A provider, as the Settings window names it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ProviderView {
    /// The id commands take, such as `anthropic`.
    pub id: String,
    /// The provider's name.
    pub name: String,
}

/// A model the player can choose.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ModelOption {
    /// The model's id.
    pub id: String,
    /// Its name.
    pub name: String,
    /// The thinking settings it accepts, from least to most.
    pub thinking: Vec<ThinkingOption>,
    /// The one to pick when the player switches to this model.
    pub suggested: ThinkingOption,
}

/// A thinking setting, as the Settings window offers it: one flat list, where `Thinking` nests the
/// effort inside adaptive thinking, so it isn't `Thinking` itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum ThinkingOption {
    /// No thinking.
    Off,
    /// Adaptive thinking at low effort.
    Low,
    /// Adaptive thinking at medium effort.
    Medium,
    /// Adaptive thinking at high effort.
    High,
}

impl From<Thinking> for ThinkingOption {
    fn from(thinking: Thinking) -> Self {
        match thinking {
            Thinking::Off => Self::Off,
            Thinking::Adaptive(Effort::Low) => Self::Low,
            Thinking::Adaptive(Effort::Medium) => Self::Medium,
            Thinking::Adaptive(Effort::High) => Self::High,
        }
    }
}

impl From<ThinkingOption> for Thinking {
    fn from(option: ThinkingOption) -> Self {
        match option {
            ThinkingOption::Off => Self::Off,
            ThinkingOption::Low => Self::Adaptive(Effort::Low),
            ThinkingOption::Medium => Self::Adaptive(Effort::Medium),
            ThinkingOption::High => Self::Adaptive(Effort::High),
        }
    }
}

/// One provider's key, masked.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct KeyView {
    /// The provider.
    pub provider: ProviderView,
    /// Whether there is a key, and where it is kept.
    pub key: KeyState,
    /// How its last test went.
    pub test: KeyTest,
}

/// Whether the app has a key for a provider.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(
    tag = "state",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum KeyState {
    /// No key yet.
    Missing {
        /// The environment variable the app would also read it from.
        variable: String,
    },
    /// A key, shown only masked.
    Stored {
        /// The key's start and last four characters, such as `sk-ant-…a1b2`.
        masked: String,
        /// Where it is kept.
        place: KeyPlace,
    },
}

/// Where a key is kept.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(
    tag = "place",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum KeyPlace {
    /// In the OS keychain.
    Keychain,
    /// In memory until the app exits, because the keychain can't be used.
    Session {
        /// Why the keychain can't be used.
        reason: String,
    },
    /// In an environment variable the player set outside the app.
    Environment {
        /// The variable.
        variable: String,
    },
}

/// How a key's last test went.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(
    tag = "state",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum KeyTest {
    /// Not tested since it was given.
    Untested,
    /// A test call is on its way.
    Testing,
    /// A model answered.
    Works {
        /// The model that answered.
        model: String,
        /// How long the answer took.
        millis: u32,
    },
    /// The provider refused the key.
    Rejected {
        /// What the provider said.
        message: String,
    },
    /// The test couldn't tell, such as when the provider can't be reached.
    Failed {
        /// What went wrong.
        message: String,
    },
}

#[cfg(test)]
pub(crate) mod tests {
    use std::sync::Arc;

    use copilot_llm::SidecarReplayer;

    use super::*;
    use crate::keys::{Keychain, KeychainError};

    pub(crate) const KEY: &str = "sk-ant-api03-test-secret-0123456789-a1b2";
    pub(crate) const WORKS: &[u8] = include_bytes!("../tests/sidecars/key-test-works.llm.jsonl");
    const REJECTED: &[u8] = include_bytes!("../tests/sidecars/key-test-rejected.llm.jsonl");

    /// A keychain in memory, which can be made unusable like a Linux desktop without one.
    #[derive(Clone, Default)]
    pub(crate) struct MemoryKeychain {
        keys: Arc<Mutex<HashMap<Provider, String>>>,
        unusable: bool,
    }

    impl MemoryKeychain {
        fn unusable() -> Self {
            Self {
                unusable: true,
                ..Self::default()
            }
        }

        fn check(&self) -> Result<(), KeychainError> {
            if self.unusable {
                return Err(KeychainError("no Secret Service is running".to_owned()));
            }
            Ok(())
        }
    }

    impl Keychain for MemoryKeychain {
        fn read(&self, provider: Provider) -> Result<Option<ApiKey>, KeychainError> {
            self.check()?;
            Ok(lock(&self.keys).get(&provider).map(|key| ApiKey::new(key)))
        }

        fn write(&self, provider: Provider, key: &ApiKey) -> Result<(), KeychainError> {
            self.check()?;
            lock(&self.keys).insert(provider, key.expose().to_owned());
            Ok(())
        }

        fn delete(&self, provider: Provider) -> Result<(), KeychainError> {
            self.check()?;
            lock(&self.keys).remove(&provider);
            Ok(())
        }
    }

    /// Calls are answered from `sidecar`, never by a live model.
    pub(crate) fn replaying(sidecar: &'static [u8]) -> Connect {
        Box::new(move |_, _| Box::new(SidecarReplayer::read(sidecar).unwrap()))
    }

    fn no_environment(_: &str) -> Option<String> {
        None
    }

    pub(crate) fn models(keychain: &MemoryKeychain, connect: Connect) -> ModelsAndKeys {
        ModelsAndKeys::new(
            Keys::load(Box::new(keychain.clone()), no_environment),
            connect,
        )
    }

    fn anthropic(models: &ModelsAndKeys) -> KeyView {
        let quick = ModelChoice::default_for(ModelSlot::Quick);
        let deep = ModelChoice::default_for(ModelSlot::Deep);
        models.view(&quick, &deep).keys.remove(0)
    }

    fn stored_in(place: KeyPlace) -> KeyState {
        KeyState::Stored {
            masked: "sk-ant-…a1b2".to_owned(),
            place,
        }
    }

    #[test]
    fn with_no_key_the_section_says_so_and_names_the_variable_it_would_also_read() {
        let models = models(&MemoryKeychain::default(), replaying(WORKS));

        assert_eq!(
            anthropic(&models).key,
            KeyState::Missing {
                variable: "ANTHROPIC_API_KEY".to_owned()
            }
        );
    }

    #[test]
    fn a_saved_key_is_shown_masked_and_is_still_there_after_a_restart() {
        let keychain = MemoryKeychain::default();
        let first = models(&keychain, replaying(WORKS));

        first
            .save_key(Provider::Anthropic, &format!(" {KEY}\n"))
            .unwrap();

        assert_eq!(anthropic(&first).key, stored_in(KeyPlace::Keychain));
        let restarted = models(&keychain, replaying(WORKS));
        assert_eq!(anthropic(&restarted).key, stored_in(KeyPlace::Keychain));
    }

    #[test]
    fn without_a_keychain_a_key_is_held_for_this_session_only() {
        let keychain = MemoryKeychain::unusable();
        let first = models(&keychain, replaying(WORKS));

        first.save_key(Provider::Anthropic, KEY).unwrap();

        assert_eq!(
            anthropic(&first).key,
            stored_in(KeyPlace::Session {
                reason: "no Secret Service is running".to_owned()
            })
        );
        let restarted = models(&keychain, replaying(WORKS));
        assert!(matches!(
            anthropic(&restarted).key,
            KeyState::Missing { .. }
        ));
    }

    #[test]
    fn a_key_in_the_environment_is_used_until_the_player_gives_their_own() {
        let environment = |variable: &str| {
            (variable == "ANTHROPIC_API_KEY")
                .then(|| "sk-ant-api03-from-the-environment-zzzz-9z9z".to_owned())
        };
        let models = ModelsAndKeys::new(
            Keys::load(Box::new(MemoryKeychain::default()), environment),
            replaying(WORKS),
        );
        let from_environment = KeyState::Stored {
            masked: "sk-ant-…9z9z".to_owned(),
            place: KeyPlace::Environment {
                variable: "ANTHROPIC_API_KEY".to_owned(),
            },
        };
        assert_eq!(anthropic(&models).key, from_environment);

        models.save_key(Provider::Anthropic, KEY).unwrap();
        assert_eq!(anthropic(&models).key, stored_in(KeyPlace::Keychain));

        models.remove_key(Provider::Anthropic).unwrap();
        assert_eq!(anthropic(&models).key, from_environment);
    }

    #[test]
    fn a_removed_key_is_gone_from_the_keychain_too() {
        let keychain = MemoryKeychain::default();
        let models = models(&keychain, replaying(WORKS));
        models.save_key(Provider::Anthropic, KEY).unwrap();

        models.remove_key(Provider::Anthropic).unwrap();

        assert!(matches!(anthropic(&models).key, KeyState::Missing { .. }));
        assert_eq!(keychain.read(Provider::Anthropic), Ok(None));
    }

    #[test]
    fn an_empty_paste_is_refused() {
        let models = models(&MemoryKeychain::default(), replaying(WORKS));

        assert_eq!(
            models.save_key(Provider::Anthropic, "  \n"),
            Err(KeyError::Empty)
        );
    }

    #[tokio::test]
    async fn a_working_key_says_which_model_answered() {
        let models = models(&MemoryKeychain::default(), replaying(WORKS));
        models.save_key(Provider::Anthropic, KEY).unwrap();

        models.testing(Provider::Anthropic);
        assert_eq!(anthropic(&models).test, KeyTest::Testing);
        models.test_key(Provider::Anthropic, None).await;

        let test = anthropic(&models).test;
        assert!(
            matches!(&test, KeyTest::Works { model, .. } if model == "Claude Haiku 5.5"),
            "{test:?}"
        );
    }

    #[tokio::test]
    async fn a_rejected_key_says_what_the_provider_said_and_a_new_key_clears_it() {
        let models = models(&MemoryKeychain::default(), replaying(REJECTED));
        models.save_key(Provider::Anthropic, KEY).unwrap();

        models.test_key(Provider::Anthropic, None).await;
        assert_eq!(
            anthropic(&models).test,
            KeyTest::Rejected {
                message: "invalid x-api-key".to_owned()
            }
        );

        models.save_key(Provider::Anthropic, KEY).unwrap();
        assert_eq!(anthropic(&models).test, KeyTest::Untested);
    }

    #[test]
    fn each_slot_offers_its_providers_models_with_the_thinking_each_accepts() {
        let models = models(&MemoryKeychain::default(), replaying(WORKS));
        let quick = ModelChoice::default_for(ModelSlot::Quick);
        let deep = ModelChoice::default_for(ModelSlot::Deep);

        let view = models.view(&quick, &deep);

        insta::assert_json_snapshot!((view.quick, view.deep));
    }
}
