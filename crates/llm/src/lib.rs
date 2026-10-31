//! The LlmProvider trait, its adapters, and LLM sidecar record and replay.
//!
//! Everything the Advisor asks a model goes through [`LlmProvider`]: a provider-neutral
//! [`LlmRequest`] in, a streamed [`LlmResponse`] with its required [`Usage`] out.

mod anthropic;
mod chat;
mod connection;
mod key;
mod models;
mod ollama;
mod openai;
mod provider;
mod request;
mod response;
mod sidecar;
mod stream;

pub use anthropic::AnthropicProvider;
pub use connection::ProviderConnection;
pub use key::ApiKey;
pub use models::{
    Access, ModelChoice, ModelChoiceError, ModelInfo, ModelSlot, Provider, ServerAddress,
    ServerAddressError,
};
pub use ollama::OllamaProvider;
pub use openai::OpenAiCompatible;
pub use provider::{LlmError, LlmProvider, LlmStream, complete};
pub use request::{
    CacheHint, Effort, LlmRequest, Message, OutputSchema, Part, Role, SystemBlock, Thinking,
    ToolDef,
};
pub use response::{LlmResponse, StopReason, StreamEvent, Usage};
pub use sidecar::{
    RequestHash, SIDECAR_EXTENSION, SidecarError, SidecarRecorder, SidecarReplayer, request_hash,
    sidecar_path,
};
