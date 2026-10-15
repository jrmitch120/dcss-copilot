//! The models a player can choose for the Quick and Deep slots, and the thinking each accepts.

use serde::{Deserialize, Serialize};

use crate::request::{Effort, Thinking};

/// An LLM provider the Advisor can call.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Provider {
    /// Anthropic's Messages API.
    Anthropic,
}

impl Provider {
    /// Every provider, in the order Settings lists them.
    pub const ALL: [Self; 1] = [Self::Anthropic];

    /// The provider's id in settings, the keychain and the webview, such as `anthropic`.
    #[must_use]
    pub fn id(self) -> &'static str {
        match self {
            Self::Anthropic => "anthropic",
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
        }
    }

    /// The environment variable a player may already keep this provider's key in.
    #[must_use]
    pub fn key_variable(self) -> &'static str {
        match self {
            Self::Anthropic => "ANTHROPIC_API_KEY",
        }
    }

    /// The models a player can choose from this provider.
    #[must_use]
    pub fn models(self) -> &'static [ModelInfo] {
        match self {
            Self::Anthropic => ANTHROPIC_MODELS,
        }
    }

    fn model(self, id: &str) -> Option<&'static ModelInfo> {
        self.models().iter().find(|model| model.id == id)
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(into = "UncheckedChoice")]
pub struct ModelChoice {
    provider: Provider,
    model: &'static str,
    thinking: Thinking,
}

/// Why a model choice was refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ModelChoiceError {
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
        let info = provider
            .model(model)
            .ok_or_else(|| ModelChoiceError::UnknownModel {
                provider,
                model: model.to_owned(),
            })?;
        if !info.thinking.contains(&thinking) {
            return Err(ModelChoiceError::Thinking {
                model: model.to_owned(),
                thinking,
            });
        }
        Ok(Self {
            provider,
            model: info.id,
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
            model,
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
    pub fn model(&self) -> &'static str {
        self.model
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

// Written out because the derive can't take `try_from` with a `&'static str` field.
impl<'de> Deserialize<'de> for ModelChoice {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let saved = UncheckedChoice::deserialize(deserializer)?;
        Self::new(saved.provider, &saved.model, saved.thinking).map_err(serde::de::Error::custom)
    }
}

impl From<ModelChoice> for UncheckedChoice {
    fn from(choice: ModelChoice) -> Self {
        Self {
            provider: choice.provider,
            model: choice.model.to_owned(),
            thinking: choice.thinking,
        }
    }
}
