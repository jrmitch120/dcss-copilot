//! Settings → Models and keys: the Quick and Deep slots, each provider's key and testing it, and
//! the local servers and the models they have.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::Instant;

use copilot_llm::{
    Access, ApiKey, Effort, LlmError, LlmProvider, LlmRequest, LlmResponse, Message, ModelChoice,
    ModelInfo, ModelSlot, Provider, ProviderConnection, ServerAddress, ServerAddressError,
    SidecarRecorder, Thinking, complete,
};
use futures_util::future::BoxFuture;
use serde::{Deserialize, Serialize};
use specta::Type;

use crate::keys::{KeyError, KeySource, Keys};

/// Makes a live provider from a connection. The app's calls it; tests replay an LLM sidecar.
pub type Connect = Box<dyn Fn(ProviderConnection) -> Box<dyn LlmProvider> + Send + Sync>;

/// Asks a local server which models it has. The app's asks it; tests answer for it.
pub type Detect = Box<
    dyn Fn(ProviderConnection) -> BoxFuture<'static, Result<Vec<String>, LlmError>> + Send + Sync,
>;

/// The most a key test may answer with: one word is enough.
const KEY_TEST_MAX_TOKENS: u32 = 16;

/// A local server couldn't be moved.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AddressError {
    /// The provider has no server to move.
    #[error("{} isn't a server the player runs", .0.name())]
    NotAServer(Provider),
    /// The address isn't one the app can call.
    #[error(transparent)]
    Invalid(#[from] ServerAddressError),
}

/// One local server: where it listens, and what asking it last found.
struct Server {
    address: ServerAddress,
    detection: Detection,
}

/// The players' keys, how each one's last test went, and their local servers.
pub struct ModelsAndKeys {
    keys: Mutex<Keys>,
    tests: Mutex<HashMap<Provider, KeyTest>>,
    servers: Mutex<HashMap<Provider, Server>>,
    connect: Connect,
    detect: Detect,
}

impl ModelsAndKeys {
    /// Holds `keys`, calling providers through `connect` and asking local servers through `detect`.
    /// A server missing from `addresses`, the player's settings, is at its default address.
    pub fn new(
        keys: Keys,
        connect: Connect,
        detect: Detect,
        addresses: &HashMap<Provider, ServerAddress>,
    ) -> Self {
        let servers = Provider::ALL
            .into_iter()
            .filter_map(|provider| {
                let server = Server {
                    address: addresses
                        .get(&provider)
                        .cloned()
                        .or_else(|| provider.default_address())?,
                    detection: Detection::Unchecked,
                };
                Some((provider, server))
            })
            .collect();
        Self {
            keys: Mutex::new(keys),
            tests: Mutex::new(HashMap::new()),
            servers: Mutex::new(servers),
            connect,
            detect,
        }
    }

    /// The Models and keys section, with the slots set to `quick` and `deep`.
    pub fn view(&self, quick: &ModelChoice, deep: &ModelChoice) -> ModelsAndKeysView {
        let keys = lock(&self.keys);
        let tests = lock(&self.tests);
        let servers = lock(&self.servers);
        ModelsAndKeysView {
            quick: slot_view(ModelSlot::Quick, quick),
            deep: slot_view(ModelSlot::Deep, deep),
            providers: Provider::ALL
                .into_iter()
                .map(|provider| provider_option(provider, servers.get(&provider)))
                .collect(),
            keys: Provider::ALL
                .into_iter()
                .filter_map(|provider| {
                    let Access::Key { variable } = provider.access() else {
                        return None;
                    };
                    Some(KeyView {
                        provider: provider_view(provider),
                        key: key_state(&keys, provider, variable),
                        test: tests.get(&provider).cloned().unwrap_or(KeyTest::Untested),
                    })
                })
                .collect(),
            servers: Provider::ALL
                .into_iter()
                .filter_map(|provider| {
                    servers.get(&provider).map(|server| ServerView {
                        provider: provider_view(provider),
                        address: server.address.as_str().to_owned(),
                        detection: server.detection.clone(),
                    })
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

    /// The player said to use the key in `provider`'s environment variable, or to stop. Returns
    /// every provider's they now use, for their settings.
    pub fn use_environment_key(&self, provider: Provider, on: bool) -> Vec<Provider> {
        lock(&self.tests).remove(&provider);
        let mut keys = lock(&self.keys);
        keys.use_environment(provider, on);
        keys.accepted()
    }

    /// The player opted in to storing `provider`'s session key unencrypted, or out again.
    ///
    /// # Errors
    /// If there is no such key, or the key file can't be changed.
    pub fn store_key_unencrypted(&self, provider: Provider, on: bool) -> Result<(), KeyError> {
        lock(&self.keys).store_unencrypted(provider, on)
    }

    /// The player moved a local server. Returns the address to save: an empty one puts the server
    /// back at its default. What the server had no longer applies.
    ///
    /// # Errors
    /// If it isn't an address the app can call, or `provider` isn't a local server.
    pub fn set_address(
        &self,
        provider: Provider,
        address: &str,
    ) -> Result<ServerAddress, AddressError> {
        let default = provider
            .default_address()
            .ok_or(AddressError::NotAServer(provider))?;
        let address = if address.trim().is_empty() {
            default
        } else {
            ServerAddress::new(address)?
        };
        lock(&self.servers).insert(
            provider,
            Server {
                address: address.clone(),
                detection: Detection::Unchecked,
            },
        );
        Ok(address)
    }

    /// Marks `provider`'s server as being asked, so the player sees it straight away.
    pub fn detecting(&self, provider: Provider) {
        if let Some(server) = lock(&self.servers).get_mut(&provider) {
            server.detection = Detection::Checking;
        }
    }

    /// Asks `provider`'s server which models it has. The outcome shows in the view.
    pub async fn detect_models(&self, provider: Provider) {
        let Some(connection) = self.connection(provider) else {
            return;
        };
        let address = self.address(provider);
        let answer = (self.detect)(connection).await;
        let mut servers = lock(&self.servers);
        // The player may have moved the server while it was being asked.
        if let Some(server) = servers
            .get_mut(&provider)
            .filter(|server| Some(&server.address) == address.as_ref())
        {
            server.detection = match answer {
                Ok(models) => Detection::Found { models },
                Err(error) => Detection::Unreachable {
                    message: error.to_string(),
                },
            };
        }
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

    /// Makes `request` of `provider` with the player's key for it, recording the call in
    /// `sidecar` when a dev build is recording, and waits for the whole answer. Errors never hold
    /// the key.
    pub async fn ask(
        &self,
        provider: Provider,
        request: LlmRequest,
        sidecar: Option<PathBuf>,
    ) -> Result<LlmResponse, LlmError> {
        let Some(connection) = self.connection(provider) else {
            return Err(LlmError::NoKey {
                provider: provider.name().to_owned(),
            });
        };
        let key = connection.key().cloned();
        let live = self
            .connected(connection, sidecar)
            .map_err(|error| LlmError::Sidecar(error.to_string()))?;
        complete(live.as_ref(), request)
            .await
            .map_err(|error| match &key {
                Some(key) => masked_error(key, error),
                None => error,
            })
    }

    async fn run_test(&self, provider: Provider, sidecar: Option<PathBuf>) -> KeyTest {
        let Some((connection, key)) = self
            .connection(provider)
            .and_then(|connection| Some((connection.clone(), connection.key()?.clone())))
        else {
            return KeyTest::Failed {
                message: format!("There is no {} key to test.", provider.name()),
            };
        };
        let live = match self.connected(connection, sidecar) {
            Ok(live) => live,
            Err(error) => {
                return KeyTest::Failed {
                    message: format!("The call couldn't be recorded: {error}"),
                };
            }
        };
        let Some((request, model_name)) = key_test_request(provider.models()) else {
            return KeyTest::Failed {
                message: format!("{} offers no model to test the key with.", provider.name()),
            };
        };
        let started = Instant::now();
        match complete(live.as_ref(), request).await {
            Ok(_) => KeyTest::Works {
                model: model_name.to_owned(),
                millis: u32::try_from(started.elapsed().as_millis()).unwrap_or(u32::MAX),
            },
            Err(LlmError::KeyRejected { message, .. }) => KeyTest::Rejected {
                message: masking(&key, &message),
            },
            Err(error) => KeyTest::Failed {
                message: masking(&key, &error.to_string()),
            },
        }
    }

    /// What calling `provider` takes: `None` while it wants a key the player hasn't given.
    fn connection(&self, provider: Provider) -> Option<ProviderConnection> {
        let key = || lock(&self.keys).get(provider).map(|held| held.key);
        let address = || self.address(provider);
        match provider {
            Provider::Anthropic => key().map(ProviderConnection::Anthropic),
            Provider::OpenAi => key().map(ProviderConnection::OpenAi),
            Provider::OpenRouter => key().map(ProviderConnection::OpenRouter),
            Provider::Ollama => address().map(|address| ProviderConnection::Ollama { address }),
            Provider::LlamaServer => {
                address().map(|address| ProviderConnection::LlamaServer { address })
            }
        }
    }

    fn address(&self, provider: Provider) -> Option<ServerAddress> {
        lock(&self.servers)
            .get(&provider)
            .map(|server| server.address.clone())
    }

    /// The live provider for `connection`, recording each call in `sidecar` when given.
    fn connected(
        &self,
        connection: ProviderConnection,
        sidecar: Option<PathBuf>,
    ) -> Result<Box<dyn LlmProvider>, std::io::Error> {
        let live = (self.connect)(connection);
        match sidecar {
            Some(path) => Ok(Box::new(SidecarRecorder::create(live, &path)?)),
            None => Ok(live),
        }
    }
}

/// `error`, with `key` masked in any message a provider echoed it back in.
fn masked_error(key: &ApiKey, error: LlmError) -> LlmError {
    let mask = |message: String| masking(key, &message);
    match error {
        LlmError::KeyRejected { status, message } => LlmError::KeyRejected {
            status,
            message: mask(message),
        },
        LlmError::RateLimited { message } => LlmError::RateLimited {
            message: mask(message),
        },
        LlmError::Unavailable { status, message } => LlmError::Unavailable {
            status,
            message: mask(message),
        },
        LlmError::BadRequest { status, message } => LlmError::BadRequest {
            status,
            message: mask(message),
        },
        LlmError::Unreachable(message) => LlmError::Unreachable(mask(message)),
        LlmError::Malformed(message) => LlmError::Malformed(mask(message)),
        LlmError::Sidecar(message) => LlmError::Sidecar(mask(message)),
        error @ (LlmError::NoKey { .. }
        | LlmError::NoUsage
        | LlmError::Incomplete
        | LlmError::NotRecorded { .. }) => error,
    }
}

/// What a provider said, with `key` masked wherever it echoed it back.
fn masking(key: &ApiKey, said: &str) -> String {
    if key.is_empty() {
        return said.to_owned();
    }
    said.replace(key.expose(), &key.masked())
}

/// The call that tests a key: the first of the provider's `models`, asked for one word, and the
/// model's name. `None` when the provider offers no model to ask.
fn key_test_request(models: &'static [ModelInfo]) -> Option<(LlmRequest, &'static str)> {
    let model = models.first()?;
    let request = LlmRequest {
        thinking: model.suggested,
        ..LlmRequest::new(
            model.id,
            KEY_TEST_MAX_TOKENS,
            vec![Message::user("Reply with the single word OK.")],
        )
    };
    Some((request, model.name))
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    // Each change leaves the state whole, so a panic elsewhere can't leave it half-changed.
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

fn provider_view(provider: Provider) -> ProviderView {
    ProviderView {
        id: provider.id().to_owned(),
        name: provider.name().to_owned(),
        local: provider.is_local(),
    }
}

fn key_state(keys: &Keys, provider: Provider, variable: &str) -> KeyState {
    let Some(held) = keys.get(provider) else {
        return match keys.offered(provider) {
            Some(key) => KeyState::Offered {
                masked: key.masked(),
                variable: variable.to_owned(),
            },
            None => KeyState::Missing {
                variable: variable.to_owned(),
            },
        };
    };
    KeyState::Stored {
        masked: held.key.masked(),
        place: match held.source {
            KeySource::Keychain => KeyPlace::Keychain,
            KeySource::Session { reason } => KeyPlace::Session { reason },
            KeySource::File => KeyPlace::File,
            KeySource::Environment { variable } => KeyPlace::Environment {
                variable: variable.to_owned(),
            },
        },
    }
}

/// What a slot can choose from `provider`: the models the app lists, and what `server` has.
fn provider_option(provider: Provider, server: Option<&Server>) -> ProviderOption {
    let listed = provider.models().iter().map(listed_option);
    let detected = server
        .and_then(|server| match &server.detection {
            Detection::Found { models } => Some(models.as_slice()),
            _ => None,
        })
        .unwrap_or_default()
        .iter()
        .map(|id| unlisted_option(provider, id));
    ProviderOption {
        provider: provider_view(provider),
        models: listed.chain(detected).collect(),
        unlisted_thinking: provider
            .unlisted_model_thinking()
            .map(|thinking| thinking.iter().copied().map(ThinkingOption::from).collect()),
    }
}

fn listed_option(model: &ModelInfo) -> ModelOption {
    ModelOption {
        id: model.id.to_owned(),
        name: model.name.to_owned(),
        thinking: model
            .thinking
            .iter()
            .copied()
            .map(ThinkingOption::from)
            .collect(),
        suggested: model.suggested.into(),
    }
}

/// A model the app doesn't list, known by its id. Nothing about it suggests thinking.
fn unlisted_option(provider: Provider, id: &str) -> ModelOption {
    ModelOption {
        id: id.to_owned(),
        name: id.to_owned(),
        thinking: provider
            .unlisted_model_thinking()
            .unwrap_or_default()
            .iter()
            .copied()
            .map(ThinkingOption::from)
            .collect(),
        suggested: ThinkingOption::Off,
    }
}

fn slot_view(slot: ModelSlot, choice: &ModelChoice) -> SlotView {
    let provider = choice.provider();
    let listed = provider.models().iter().find(|m| m.id == choice.model());
    SlotView {
        slot,
        provider: provider_view(provider),
        model: listed.map_or_else(|| unlisted_option(provider, choice.model()), listed_option),
        thinking: ThinkingOption::from(choice.thinking()),
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
    /// What each provider offers a slot.
    pub providers: Vec<ProviderOption>,
    /// The key of each provider that takes one.
    pub keys: Vec<KeyView>,
    /// Each local server.
    pub servers: Vec<ServerView>,
}

/// One slot's model, and what it could be changed to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SlotView {
    /// Which slot this is.
    pub slot: ModelSlot,
    /// The slot's provider.
    pub provider: ProviderView,
    /// The slot's model, with the thinking it accepts.
    pub model: ModelOption,
    /// How it thinks.
    pub thinking: ThinkingOption,
}

/// What one provider offers a slot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ProviderOption {
    /// The provider.
    pub provider: ProviderView,
    /// The models to pick from: the ones the app lists and the ones a local server has.
    pub models: Vec<ModelOption>,
    /// The thinking a model the player names themselves may use, or `None` if they can't.
    pub unlisted_thinking: Option<Vec<ThinkingOption>>,
}

/// A provider, as the Settings window names it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ProviderView {
    /// The id commands take, such as `anthropic`.
    pub id: String,
    /// The provider's name.
    pub name: String,
    /// Whether it is a Local Model's server: experimental, and $0.
    pub local: bool,
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
    /// No key yet, and none in the environment.
    Missing {
        /// The environment variable the app would also read it from.
        variable: String,
    },
    /// No key in use, but the environment holds one the player can say to use.
    Offered {
        /// The key's start and last four characters.
        masked: String,
        /// The environment variable holding it.
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
    /// In a file only the player's user can read, unencrypted, because they opted in.
    File,
    /// In an environment variable the player set outside the app, and said to use.
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

/// A local server, as the Settings window draws it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ServerView {
    /// The provider it serves.
    pub provider: ProviderView,
    /// Where it listens.
    pub address: String,
    /// What asking it last found.
    pub detection: Detection,
}

/// What asking a local server for its models found.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(
    tag = "state",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Detection {
    /// Not asked since the app started, or since it moved.
    Unchecked,
    /// It is being asked.
    Checking,
    /// It answered.
    Found {
        /// The models it has.
        models: Vec<String>,
    },
    /// It didn't answer, or not with its models.
    Unreachable {
        /// What went wrong.
        message: String,
    },
}

#[cfg(test)]
pub(crate) mod tests {
    use std::sync::Arc;

    use std::path::Path;

    use copilot_llm::SidecarReplayer;
    use futures_util::{FutureExt, StreamExt};

    use super::*;
    use crate::keys::{KeyFile, Keychain, KeychainError};

    pub(crate) const KEY: &str = "sk-ant-api03-test-secret-0123456789-a1b2";
    pub(crate) const WORKS: &[u8] = include_bytes!("../tests/sidecars/key-test-works.llm.jsonl");
    const REJECTED: &[u8] = include_bytes!("../tests/sidecars/key-test-rejected.llm.jsonl");

    /// A keychain in memory, which can be made unusable like a Linux desktop without one, or
    /// made to refuse deleting a key.
    #[derive(Clone, Default)]
    pub(crate) struct MemoryKeychain {
        keys: Arc<Mutex<HashMap<Provider, String>>>,
        unusable: bool,
        refuses_delete: bool,
    }

    impl MemoryKeychain {
        pub(crate) fn unusable() -> Self {
            Self {
                unusable: true,
                ..Self::default()
            }
        }

        pub(crate) fn refusing_delete() -> Self {
            Self {
                refuses_delete: true,
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
            if self.refuses_delete {
                return Err(KeychainError("the keychain is locked".to_owned()));
            }
            lock(&self.keys).remove(&provider);
            Ok(())
        }
    }

    /// Calls are answered from `sidecar`, never by a live model.
    pub(crate) fn replaying(sidecar: &'static [u8]) -> Connect {
        Box::new(move |_| Box::new(SidecarReplayer::read(sidecar).unwrap()))
    }

    /// Local servers answer with `models`, whichever is asked.
    pub(crate) fn detecting(models: &'static [&'static str]) -> Detect {
        Box::new(move |_| {
            let models = models.iter().map(|&model| model.to_owned()).collect();
            futures_util::future::ready(Ok(models)).boxed()
        })
    }

    /// No local server is running.
    fn nothing_running() -> Detect {
        Box::new(|_| {
            futures_util::future::ready(Err(LlmError::Unreachable("connection refused".to_owned())))
                .boxed()
        })
    }

    /// A provider that says which connection it was made from, as the name of its model.
    struct Echoing(String);

    impl LlmProvider for Echoing {
        fn stream(&self, _: LlmRequest) -> copilot_llm::LlmStream {
            let response = LlmResponse {
                model: self.0.clone(),
                content: Vec::new(),
                stop: copilot_llm::StopReason::EndTurn,
                usage: copilot_llm::Usage::default(),
            };
            futures_util::stream::once(futures_util::future::ready(Ok(
                copilot_llm::StreamEvent::Finished(response),
            )))
            .boxed()
        }
    }

    fn echoing() -> Connect {
        Box::new(|connection| Box::new(Echoing(format!("{connection:?}"))))
    }

    fn no_environment(_: &str) -> Option<String> {
        None
    }

    /// A key file nothing has written. Tests that opt in use [`with_file`].
    fn no_file() -> KeyFile {
        KeyFile::new(Path::new("no-such-dir/keys.json"))
    }

    fn loaded(
        keychain: &MemoryKeychain,
        file: KeyFile,
        environment: impl Fn(&str) -> Option<String>,
        accepted: &[Provider],
    ) -> ModelsAndKeys {
        ModelsAndKeys::new(
            Keys::load(Box::new(keychain.clone()), file, environment, accepted),
            replaying(WORKS),
            nothing_running(),
            &HashMap::new(),
        )
    }

    pub(crate) fn models(keychain: &MemoryKeychain, connect: Connect) -> ModelsAndKeys {
        ModelsAndKeys::new(
            Keys::load(Box::new(keychain.clone()), no_file(), no_environment, &[]),
            connect,
            nothing_running(),
            &HashMap::new(),
        )
    }

    fn with_file(keychain: &MemoryKeychain, path: &Path) -> ModelsAndKeys {
        loaded(keychain, KeyFile::new(path), no_environment, &[])
    }

    fn local(detect: Detect, connect: Connect) -> ModelsAndKeys {
        ModelsAndKeys::new(
            Keys::load(
                Box::new(MemoryKeychain::default()),
                no_file(),
                no_environment,
                &[],
            ),
            connect,
            detect,
            &HashMap::new(),
        )
    }

    fn view(models: &ModelsAndKeys) -> ModelsAndKeysView {
        models.view(
            &ModelChoice::default_for(ModelSlot::Quick),
            &ModelChoice::default_for(ModelSlot::Deep),
        )
    }

    fn ollama(models: &ModelsAndKeys) -> ServerView {
        view(models)
            .servers
            .into_iter()
            .find(|server| server.provider.id == "ollama")
            .unwrap()
    }

    fn offered_models(models: &ModelsAndKeys, provider: &str) -> Vec<String> {
        view(models)
            .providers
            .into_iter()
            .find(|option| option.provider.id == provider)
            .unwrap()
            .models
            .into_iter()
            .map(|model| model.id)
            .collect()
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

    fn in_the_environment(variable: &str) -> Option<String> {
        (variable == "ANTHROPIC_API_KEY")
            .then(|| "sk-ant-api03-from-the-environment-zzzz-9z9z".to_owned())
    }

    #[tokio::test]
    async fn a_key_in_the_environment_is_only_offered_until_the_player_says_to_use_it() {
        let models = loaded(
            &MemoryKeychain::default(),
            no_file(),
            in_the_environment,
            &[],
        );

        assert_eq!(
            anthropic(&models).key,
            KeyState::Offered {
                masked: "sk-ant-…9z9z".to_owned(),
                variable: "ANTHROPIC_API_KEY".to_owned(),
            }
        );
        let question = LlmRequest::new("claude-haiku-5-5", 16, vec![Message::user("OK?")]);
        assert_eq!(
            models.ask(Provider::Anthropic, question, None).await,
            Err(LlmError::NoKey {
                provider: "Anthropic".to_owned()
            })
        );

        assert_eq!(
            models.use_environment_key(Provider::Anthropic, true),
            [Provider::Anthropic]
        );
        assert!(matches!(
            anthropic(&models).key,
            KeyState::Stored {
                place: KeyPlace::Environment { .. },
                ..
            }
        ));

        assert_eq!(models.use_environment_key(Provider::Anthropic, false), []);
        assert!(matches!(anthropic(&models).key, KeyState::Offered { .. }));
    }

    #[test]
    fn a_key_only_offered_shows_masked_in_the_settings_window() {
        let models = loaded(
            &MemoryKeychain::default(),
            no_file(),
            |variable: &str| (variable == "ANTHROPIC_API_KEY").then(|| KEY.to_owned()),
            &[],
        );

        let window = serde_json::to_string(&view(&models)).unwrap();

        assert!(window.contains("sk-ant-…a1b2"), "{window}");
        assert!(!window.contains(KEY), "{window}");
    }

    #[test]
    fn a_key_in_the_environment_the_player_accepted_is_used_until_they_give_their_own() {
        let models = loaded(
            &MemoryKeychain::default(),
            no_file(),
            in_the_environment,
            &[Provider::Anthropic],
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
    fn each_provider_with_a_key_reads_its_own_environment_variable() {
        let models = loaded(
            &MemoryKeychain::default(),
            no_file(),
            |variable: &str| (variable == "OPENROUTER_API_KEY").then(|| KEY.to_owned()),
            &[],
        );

        let states: Vec<(String, bool)> = view(&models)
            .keys
            .into_iter()
            .map(|key| (key.provider.id, matches!(key.key, KeyState::Offered { .. })))
            .collect();
        assert_eq!(
            states,
            [
                ("anthropic".to_owned(), false),
                ("openai".to_owned(), false),
                ("openrouter".to_owned(), true),
            ]
        );
    }

    #[test]
    fn opting_in_stores_a_session_key_in_a_file_only_the_player_can_read() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("keys.json");
        let keychain = MemoryKeychain::unusable();
        let first = with_file(&keychain, &path);
        first.save_key(Provider::Anthropic, KEY).unwrap();
        assert!(
            !path.exists(),
            "nothing is written before the player opts in"
        );

        first
            .store_key_unencrypted(Provider::Anthropic, true)
            .unwrap();

        assert_eq!(anthropic(&first).key, stored_in(KeyPlace::File));
        assert!(std::fs::read_to_string(&path).unwrap().contains(KEY));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600);
        }
        let restarted = with_file(&keychain, &path);
        assert_eq!(anthropic(&restarted).key, stored_in(KeyPlace::File));
    }

    #[test]
    fn opting_out_takes_the_key_off_the_disk_and_holds_it_for_the_session() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("keys.json");
        let models = with_file(&MemoryKeychain::unusable(), &path);
        models.save_key(Provider::Anthropic, KEY).unwrap();
        models
            .store_key_unencrypted(Provider::Anthropic, true)
            .unwrap();

        models
            .store_key_unencrypted(Provider::Anthropic, false)
            .unwrap();

        assert!(!path.exists());
        assert_eq!(
            anthropic(&models).key,
            stored_in(KeyPlace::Session {
                reason: "no Secret Service is running".to_owned()
            })
        );
    }

    #[test]
    fn replacing_an_unencrypted_key_replaces_it_in_the_file_and_removing_it_deletes_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("keys.json");
        let models = with_file(&MemoryKeychain::unusable(), &path);
        models
            .save_key(
                Provider::Anthropic,
                "sk-ant-the-first-key-0000-0000-0000-a1b2",
            )
            .unwrap();
        models
            .store_key_unencrypted(Provider::Anthropic, true)
            .unwrap();

        models.save_key(Provider::Anthropic, KEY).unwrap();

        let written = std::fs::read_to_string(&path).unwrap();
        assert!(written.contains(KEY) && !written.contains("the-first-key"));
        assert_eq!(anthropic(&models).key, stored_in(KeyPlace::File));

        models.remove_key(Provider::Anthropic).unwrap();

        assert!(!path.exists());
        assert!(matches!(anthropic(&models).key, KeyState::Missing { .. }));
    }

    #[test]
    fn a_key_the_keychain_can_take_again_leaves_no_copy_in_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("keys.json");
        let without = with_file(&MemoryKeychain::unusable(), &path);
        without.save_key(Provider::Anthropic, KEY).unwrap();
        without
            .store_key_unencrypted(Provider::Anthropic, true)
            .unwrap();

        // The next launch finds a keychain that works, and the key still in the file.
        let with = with_file(&MemoryKeychain::default(), &path);
        assert_eq!(anthropic(&with).key, stored_in(KeyPlace::File));
        with.save_key(Provider::Anthropic, KEY).unwrap();

        assert_eq!(anthropic(&with).key, stored_in(KeyPlace::Keychain));
        assert!(!path.exists());
    }

    #[test]
    fn a_key_the_keychain_holds_takes_any_plaintext_copy_off_the_disk_at_launch() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("keys.json");
        let without = with_file(&MemoryKeychain::unusable(), &path);
        without.save_key(Provider::Anthropic, KEY).unwrap();
        without
            .store_key_unencrypted(Provider::Anthropic, true)
            .unwrap();
        let keychain = MemoryKeychain::default();
        keychain
            .write(Provider::Anthropic, &ApiKey::new(KEY))
            .unwrap();

        let models = with_file(&keychain, &path);

        assert_eq!(anthropic(&models).key, stored_in(KeyPlace::Keychain));
        assert!(!path.exists());
    }

    #[test]
    fn a_key_in_the_keychain_cant_be_moved_to_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("keys.json");
        let models = with_file(&MemoryKeychain::default(), &path);
        models.save_key(Provider::Anthropic, KEY).unwrap();

        assert_eq!(
            models.store_key_unencrypted(Provider::Anthropic, true),
            Err(KeyError::NotInSession)
        );
        assert!(!path.exists());
    }

    #[test]
    fn a_broken_key_file_is_logged_without_quoting_what_it_holds() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("keys.json");
        std::fs::write(&path, format!("{{\"anthropic\": [\"{KEY}\"]}}")).unwrap();

        let models = with_file(&MemoryKeychain::default(), &path);

        assert!(matches!(anthropic(&models).key, KeyState::Missing { .. }));
        for line in crate::log::tests::logged() {
            assert!(!line.contains(KEY), "the key reached: {line}");
        }
    }

    #[test]
    fn local_servers_take_no_key_and_are_labelled_as_local() {
        let view = view(&local(detecting(&[]), replaying(WORKS)));

        let keyed: Vec<&str> = view
            .keys
            .iter()
            .map(|key| key.provider.id.as_str())
            .collect();
        assert_eq!(keyed, ["anthropic", "openai", "openrouter"]);
        let servers: Vec<(&str, &str, bool)> = view
            .servers
            .iter()
            .map(|server| {
                (
                    server.provider.id.as_str(),
                    server.address.as_str(),
                    server.provider.local,
                )
            })
            .collect();
        assert_eq!(
            servers,
            [
                ("ollama", "http://localhost:11434", true),
                ("llama_server", "http://localhost:9931", true),
            ]
        );
    }

    #[tokio::test]
    async fn a_local_model_is_called_with_no_key_at_the_servers_address() {
        let models = local(detecting(&[]), echoing());
        models
            .set_address(Provider::Ollama, "http://192.168.1.20:11434/")
            .unwrap();
        let question = LlmRequest::new("qwen3:8b", 16, vec![Message::user("OK?")]);

        let answer = models.ask(Provider::Ollama, question, None).await.unwrap();

        assert_eq!(
            answer.model,
            r#"Ollama { address: ServerAddress("http://192.168.1.20:11434") }"#
        );
    }

    #[tokio::test]
    async fn the_models_a_running_server_has_are_offered_to_the_slots() {
        let models = local(detecting(&["qwen3:8b", "llama3.2:3b"]), replaying(WORKS));
        assert_eq!(ollama(&models).detection, Detection::Unchecked);
        assert!(offered_models(&models, "ollama").is_empty());

        models.detecting(Provider::Ollama);
        assert_eq!(ollama(&models).detection, Detection::Checking);
        models.detect_models(Provider::Ollama).await;

        assert_eq!(
            offered_models(&models, "ollama"),
            ["qwen3:8b", "llama3.2:3b"]
        );
        assert!(offered_models(&models, "llama_server").is_empty());
    }

    #[tokio::test]
    async fn a_server_that_isnt_running_says_so_and_a_new_address_clears_what_was_found() {
        let down = local(nothing_running(), replaying(WORKS));
        down.detect_models(Provider::Ollama).await;
        assert_eq!(
            ollama(&down).detection,
            Detection::Unreachable {
                message: "the provider couldn't be reached: connection refused".to_owned()
            }
        );

        let up = local(detecting(&["qwen3:8b"]), replaying(WORKS));
        up.detect_models(Provider::Ollama).await;
        up.set_address(Provider::Ollama, "http://localhost:11500")
            .unwrap();
        assert_eq!(ollama(&up).detection, Detection::Unchecked);
        assert!(offered_models(&up, "ollama").is_empty());
    }

    #[test]
    fn a_servers_address_must_be_callable_and_an_empty_one_is_its_default() {
        let models = local(detecting(&[]), replaying(WORKS));

        assert!(matches!(
            models.set_address(Provider::Ollama, "localhost:11434"),
            Err(AddressError::Invalid(_))
        ));
        assert_eq!(
            models.set_address(Provider::OpenAi, "http://x"),
            Err(AddressError::NotAServer(Provider::OpenAi))
        );
        assert_eq!(
            models.set_address(Provider::Ollama, "  ").unwrap().as_str(),
            "http://localhost:11434"
        );
    }

    #[test]
    fn a_slot_shows_a_model_the_app_doesnt_list_and_the_thinking_it_may_use() {
        let models = local(detecting(&[]), replaying(WORKS));
        let quick = ModelChoice::new(Provider::Ollama, "qwen3:8b", Thinking::Off).unwrap();
        let deep = ModelChoice::new(
            Provider::OpenRouter,
            "mistralai/some-new-model",
            Thinking::Adaptive(Effort::Low),
        )
        .unwrap();

        let view = models.view(&quick, &deep);

        assert_eq!(view.quick.model.name, "qwen3:8b");
        assert_eq!(view.quick.model.thinking, [ThinkingOption::Off]);
        assert_eq!(view.deep.model.id, "mistralai/some-new-model");
        assert_eq!(view.deep.model.thinking.len(), 4);
    }

    #[tokio::test]
    async fn an_openai_key_is_tested_with_openais_cheapest_listed_model() {
        let models = models(&MemoryKeychain::default(), echoing());
        models.save_key(Provider::OpenAi, KEY).unwrap();

        models.test_key(Provider::OpenAi, None).await;

        let test = view(&models).keys.remove(1).test;
        assert!(
            matches!(&test, KeyTest::Works { model, .. } if model == "GPT-6 Luna"),
            "{test:?}"
        );
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
    fn a_provider_without_models_has_no_key_test_rather_than_one_with_an_empty_model() {
        assert_eq!(key_test_request(&[]), None);
    }

    #[test]
    fn each_slot_offers_its_providers_models_with_the_thinking_each_accepts() {
        let models = models(&MemoryKeychain::default(), replaying(WORKS));
        let quick = ModelChoice::default_for(ModelSlot::Quick);
        let deep = ModelChoice::default_for(ModelSlot::Deep);

        let view = models.view(&quick, &deep);

        insta::assert_json_snapshot!((view.quick, view.deep, view.providers));
    }
}
