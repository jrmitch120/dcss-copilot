//! What it takes to reach each provider, and the adapter that does.

use futures_util::FutureExt;
use futures_util::future::BoxFuture;

use crate::anthropic::AnthropicProvider;
use crate::key::ApiKey;
use crate::models::ServerAddress;
use crate::ollama::OllamaProvider;
use crate::openai::OpenAiCompatible;
use crate::provider::{LlmError, LlmProvider};

/// One provider, with what calling it takes: a key for a cloud provider, an address for a server
/// the player runs. A provider can't be reached without its own.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderConnection {
    /// Anthropic, with the player's key.
    Anthropic(ApiKey),
    /// OpenAI, with the player's key.
    OpenAi(ApiKey),
    /// OpenRouter, with the player's key.
    OpenRouter(ApiKey),
    /// The Ollama at an address.
    Ollama {
        /// Where it listens.
        address: ServerAddress,
    },
    /// The llama-server at an address.
    LlamaServer {
        /// Where it listens.
        address: ServerAddress,
    },
}

impl ProviderConnection {
    /// The adapter that calls this provider.
    #[must_use]
    pub fn open(self) -> Box<dyn LlmProvider> {
        match self {
            Self::Anthropic(key) => Box::new(AnthropicProvider::new(key)),
            Self::OpenAi(key) => Box::new(OpenAiCompatible::openai(key)),
            Self::OpenRouter(key) => Box::new(OpenAiCompatible::openrouter(key)),
            Self::Ollama { address } => Box::new(OllamaProvider::new(address.as_str())),
            Self::LlamaServer { address } => {
                Box::new(OpenAiCompatible::llama_server(address.as_str()))
            }
        }
    }

    /// The key this provider is called with, if it takes one.
    #[must_use]
    pub fn key(&self) -> Option<&ApiKey> {
        match self {
            Self::Anthropic(key) | Self::OpenAi(key) | Self::OpenRouter(key) => Some(key),
            Self::Ollama { .. } | Self::LlamaServer { .. } => None,
        }
    }

    /// The models a local server says it has. A cloud provider's are listed in the app, so it says
    /// it has none.
    ///
    /// # Errors
    /// If the server can't be reached, refuses, or answers with something else.
    pub fn local_models(self) -> BoxFuture<'static, Result<Vec<String>, LlmError>> {
        async move {
            match self {
                Self::Ollama { address } => OllamaProvider::new(address.as_str()).models().await,
                Self::LlamaServer { address } => {
                    OpenAiCompatible::llama_server(address.as_str())
                        .models()
                        .await
                }
                Self::Anthropic(_) | Self::OpenAi(_) | Self::OpenRouter(_) => Ok(Vec::new()),
            }
        }
        .boxed()
    }
}
