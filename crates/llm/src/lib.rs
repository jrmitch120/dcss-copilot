//! The LlmProvider trait, its adapters, and LLM sidecar record and replay.
//!
//! Everything the Advisor asks a model goes through [`LlmProvider`]: a provider-neutral
//! [`LlmRequest`] in, a streamed [`LlmResponse`] with its required [`Usage`] out.

mod anthropic;
mod key;
mod models;
mod provider;
mod request;
mod response;
mod sidecar;

pub use anthropic::AnthropicProvider;
pub use key::ApiKey;
pub use models::{ModelChoice, ModelChoiceError, ModelInfo, ModelSlot, Provider};
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
