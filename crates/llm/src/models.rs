//! The models a player can choose for the Quick and Deep slots, and the thinking each accepts.

use serde::{Deserialize, Serialize};

use crate::request::{Effort, Thinking};

/// An LLM provider the Advisor can call.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Provider {
    /// Anthropic's Messages API.
    #[serde(rename = "anthropic")]
    Anthropic,
    /// OpenAI's chat completions.
    #[serde(rename = "openai")]
    OpenAi,
    /// OpenRouter's chat completions: one key, many providers.
    #[serde(rename = "openrouter")]
    OpenRouter,
    /// Ollama, on a server the player runs.
    #[serde(rename = "ollama")]
    Ollama,
    /// llama.cpp's server, on a server the player runs.
    #[serde(rename = "llama_server")]
    LlamaServer,
}

/// What it takes to call a provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Access {
    /// The player's API key.
    Key {
        /// The environment variable a player may already keep it in.
        variable: &'static str,
    },
    /// The address of a server the player runs, which takes no key.
    Address {
        /// Where the server listens unless the player started it elsewhere.
        default: &'static str,
    },
}

impl Provider {
    /// Every provider, in the order Settings lists them.
    pub const ALL: [Self; 5] = [
        Self::Anthropic,
        Self::OpenAi,
        Self::OpenRouter,
        Self::Ollama,
        Self::LlamaServer,
    ];

    /// The provider's id in settings, the keychain and the webview, such as `anthropic`.
    #[must_use]
    pub fn id(self) -> &'static str {
        match self {
            Self::Anthropic => "anthropic",
            Self::OpenAi => "openai",
            Self::OpenRouter => "openrouter",
            Self::Ollama => "ollama",
            Self::LlamaServer => "llama_server",
        }
    }

    /// The provider with this id, if any.
    #[must_use]
    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|provider| provider.id() == id)
    }

    /// The provider's name, for the player.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Anthropic => "Anthropic",
            Self::OpenAi => "OpenAI",
            Self::OpenRouter => "OpenRouter",
            Self::Ollama => "Ollama",
            Self::LlamaServer => "llama-server",
        }
    }

    /// What it takes to call this provider.
    #[must_use]
    pub fn access(self) -> Access {
        match self {
            Self::Anthropic => Access::Key {
                variable: "ANTHROPIC_API_KEY",
            },
            Self::OpenAi => Access::Key {
                variable: "OPENAI_API_KEY",
            },
            Self::OpenRouter => Access::Key {
                variable: "OPENROUTER_API_KEY",
            },
            Self::Ollama => Access::Address {
                default: "http://localhost:11434",
            },
            // Per llama-server's README; `--port` moves it.
            Self::LlamaServer => Access::Address {
                default: "http://localhost:9931",
            },
        }
    }

    /// Where this provider's server listens unless the player moved it, if it has one.
    #[must_use]
    pub fn default_address(self) -> Option<ServerAddress> {
        match self.access() {
            Access::Address { default } => Some(ServerAddress(default.to_owned())),
            Access::Key { .. } => None,
        }
    }

    /// Whether its models run on the player's own server: a Local Model, experimental and $0.
    #[must_use]
    pub fn is_local(self) -> bool {
        matches!(self.access(), Access::Address { .. })
    }

    /// The models to offer from this provider. A local server's are the ones it says it has.
    #[must_use]
    pub fn models(self) -> &'static [ModelInfo] {
        match self {
            Self::Anthropic => ANTHROPIC_MODELS,
            Self::OpenAi => OPENAI_MODELS,
            Self::OpenRouter => OPENROUTER_MODELS,
            Self::Ollama | Self::LlamaServer => &[],
        }
    }

    /// The thinking a model not in [`Provider::models`] may use, or `None` if only those can be
    /// chosen. A provider's catalogue changes faster than the app, so most stay open.
    #[must_use]
    pub fn unlisted_model_thinking(self) -> Option<&'static [Thinking]> {
        match self {
            Self::Anthropic => None,
            Self::OpenAi | Self::OpenRouter => Some(ANY_THINKING),
            // Slow enough without it, and each model takes its own settings.
            Self::Ollama | Self::LlamaServer => Some(&[Thinking::Off]),
        }
    }

    fn model(self, id: &str) -> Option<&'static ModelInfo> {
        self.models().iter().find(|model| model.id == id)
    }
}

/// Where a server the player runs listens: always an `http` or `https` address the app can call,
/// typed or read back from their settings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ServerAddress(String);

/// An address that isn't one the app can call.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("\"{0}\" isn't an address: it starts with http:// or https://")]
pub struct ServerAddressError(String);

impl ServerAddress {
    /// `address`, without the whitespace and trailing slash a paste often brings along.
    ///
    /// # Errors
    /// If it isn't an `http` or `https` address with a host.
    pub fn new(address: &str) -> Result<Self, ServerAddressError> {
        let address = address.trim().trim_end_matches('/');
        let host = address
            .strip_prefix("http://")
            .or_else(|| address.strip_prefix("https://"));
        match host {
            Some(host) if !host.is_empty() => Ok(Self(address.to_owned())),
            _ => Err(ServerAddressError(address.to_owned())),
        }
    }

    /// The address.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for ServerAddress {
    type Error = ServerAddressError;

    fn try_from(address: String) -> Result<Self, Self::Error> {
        Self::new(&address)
    }
}

impl From<ServerAddress> for String {
    fn from(address: ServerAddress) -> Self {
        address.0
    }
}

/// A model a player can choose.
#[derive(Debug, PartialEq, Eq)]
pub struct ModelInfo {
    /// The provider's id for it.
    pub id: &'static str,
    /// Its name, for the player.
    pub name: &'static str,
    /// The thinking settings it accepts, from least to most.
    pub thinking: &'static [Thinking],
    /// The one of them to suggest when the player picks this model.
    pub suggested: Thinking,
}

/// Anthropic's current models. Sonnet 5.5 and Opus 5.5 always think, so they can't be turned
/// off; Haiku 5.5 can, which keeps Tactical calls fast and cheap.
const ANTHROPIC_MODELS: &[ModelInfo] = &[
    ModelInfo {
        id: "claude-haiku-5-5",
        name: "Claude Haiku 5.5",
        thinking: &[
            Thinking::Off,
            Thinking::Adaptive(Effort::Low),
            Thinking::Adaptive(Effort::Medium),
        ],
        suggested: Thinking::Off,
    },
    ModelInfo {
        id: "claude-sonnet-5-5",
        name: "Claude Sonnet 5.5",
        thinking: ADAPTIVE,
        suggested: Thinking::Adaptive(Effort::Medium),
    },
    ModelInfo {
        id: "claude-opus-5-5",
        name: "Claude Opus 5.5",
        thinking: ADAPTIVE,
        suggested: Thinking::Adaptive(Effort::Medium),
    },
];

const ADAPTIVE: &[Thinking] = &[
    Thinking::Adaptive(Effort::Low),
    Thinking::Adaptive(Effort::Medium),
    Thinking::Adaptive(Effort::High),
];

const ANY_THINKING: &[Thinking] = &[
    Thinking::Off,
    Thinking::Adaptive(Effort::Low),
    Thinking::Adaptive(Effort::Medium),
    Thinking::Adaptive(Effort::High),
];

/// OpenAI's models for this kind of work, cheapest first so a key test costs least. GPT-6 Astra
/// and GPT-6.1 Sol can't turn thinking off (OpenAI's model list, 2026-10-30).
const OPENAI_MODELS: &[ModelInfo] = &[
    ModelInfo {
        id: "gpt-6-luna",
        name: "GPT-6 Luna",
        thinking: ANY_THINKING,
        suggested: Thinking::Off,
    },
    ModelInfo {
        id: "gpt-6.1-terra",
        name: "GPT-6.1 Terra",
        thinking: ANY_THINKING,
        suggested: Thinking::Adaptive(Effort::Medium),
    },
    ModelInfo {
        id: "gpt-6.1-sol",
        name: "GPT-6.1 Sol",
        thinking: ADAPTIVE,
        suggested: Thinking::Adaptive(Effort::Medium),
    },
    ModelInfo {
        id: "gpt-6-astra",
        name: "GPT-6 Astra",
        thinking: ADAPTIVE,
        suggested: Thinking::Adaptive(Effort::Medium),
    },
];

/// A few of OpenRouter's models, cheapest first so a key test costs least. Each one offered with
/// thinking off doesn't think by default, since the adapter can't tell OpenRouter not to (see
/// `wire_request` in `openai.rs`); ids from OpenRouter's model list, 2026-10-30.
const OPENROUTER_MODELS: &[ModelInfo] = &[
    ModelInfo {
        id: "anthropic/claude-haiku-5.5",
        name: "Claude Haiku 5.5",
        thinking: &[
            Thinking::Off,
            Thinking::Adaptive(Effort::Low),
            Thinking::Adaptive(Effort::Medium),
        ],
        suggested: Thinking::Off,
    },
    ModelInfo {
        id: "google/gemini-2.5-flash-lite",
        name: "Gemini 2.5 Flash-Lite",
        thinking: &[Thinking::Off],
        suggested: Thinking::Off,
    },
    ModelInfo {
        id: "anthropic/claude-sonnet-5.5",
        name: "Claude Sonnet 5.5",
        thinking: ADAPTIVE,
        suggested: Thinking::Adaptive(Effort::Medium),
    },
];

/// The two models a player chooses: one for everyday advice, one for thinking harder.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum ModelSlot {
    /// The Quick Model: Alerts, Decision Point nudges, Coach advice and ordinary questions.
    Quick,
    /// The Deep Model: questions the player asks the Advisor to think harder about.
    Deep,
}

/// What a slot calls: a provider, one of its models and a thinking setting that model accepts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(into = "UncheckedChoice", try_from = "UncheckedChoice")]
pub struct ModelChoice {
    provider: Provider,
    model: String,
    thinking: Thinking,
}

/// Why a model choice was refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ModelChoiceError {
    /// No model was named.
    #[error("name a model from {}", provider.name())]
    NoModel {
        /// The provider.
        provider: Provider,
    },
    /// The provider doesn't offer the model.
    #[error("{} doesn't offer {model}", provider.name())]
    UnknownModel {
        /// The provider.
        provider: Provider,
        /// The model asked for.
        model: String,
    },
    /// The model doesn't accept the thinking setting.
    #[error("{model} doesn't accept that thinking setting")]
    Thinking {
        /// The model.
        model: String,
        /// The setting asked for.
        thinking: Thinking,
    },
}

impl ModelChoice {
    /// `model` from `provider`, thinking as given.
    ///
    /// # Errors
    /// If the provider doesn't offer the model, or the model doesn't accept the thinking.
    pub fn new(
        provider: Provider,
        model: &str,
        thinking: Thinking,
    ) -> Result<Self, ModelChoiceError> {
        let model = model.trim();
        if model.is_empty() {
            return Err(ModelChoiceError::NoModel { provider });
        }
        let accepted = provider
            .model(model)
            .map(|info| info.thinking)
            .or_else(|| provider.unlisted_model_thinking())
            .ok_or_else(|| ModelChoiceError::UnknownModel {
                provider,
                model: model.to_owned(),
            })?;
        if !accepted.contains(&thinking) {
            return Err(ModelChoiceError::Thinking {
                model: model.to_owned(),
                thinking,
            });
        }
        Ok(Self {
            provider,
            model: model.to_owned(),
            thinking,
        })
    }

    /// What a slot calls until the player chooses otherwise.
    #[must_use]
    pub fn default_for(slot: ModelSlot) -> Self {
        let (model, thinking) = match slot {
            ModelSlot::Quick => ("claude-haiku-5-5", Thinking::Off),
            ModelSlot::Deep => ("claude-sonnet-5-5", Thinking::Adaptive(Effort::Medium)),
        };
        Self {
            provider: Provider::Anthropic,
            model: model.to_owned(),
            thinking,
        }
    }

    /// The provider called.
    #[must_use]
    pub fn provider(&self) -> Provider {
        self.provider
    }

    /// The provider's id for the model.
    #[must_use]
    pub fn model(&self) -> &str {
        &self.model
    }

    /// How the model thinks.
    #[must_use]
    pub fn thinking(&self) -> Thinking {
        self.thinking
    }
}

/// A model choice as saved, checked when it loads.
#[derive(Serialize, Deserialize)]
struct UncheckedChoice {
    provider: Provider,
    model: String,
    thinking: Thinking,
}

impl TryFrom<UncheckedChoice> for ModelChoice {
    type Error = ModelChoiceError;

    fn try_from(saved: UncheckedChoice) -> Result<Self, Self::Error> {
        Self::new(saved.provider, &saved.model, saved.thinking)
    }
}

impl From<ModelChoice> for UncheckedChoice {
    fn from(choice: ModelChoice) -> Self {
        Self {
            provider: choice.provider,
            model: choice.model,
            thinking: choice.thinking,
        }
    }
}
