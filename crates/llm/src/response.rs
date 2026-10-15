//! What comes back from a model: streamed text, then the whole answer with its usage.

use serde::{Deserialize, Serialize};

use crate::provider::LlmError;
use crate::request::Part;

/// One step of a streamed answer. A stream that succeeds ends with exactly one `Finished`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "event", content = "data", rename_all = "snake_case")]
pub enum StreamEvent {
    /// More of the answer's text, to show as it arrives.
    Text(String),
    /// The whole answer, once the model has stopped.
    Finished(LlmResponse),
}

/// A model's whole answer to one call.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LlmResponse {
    /// The model that answered, as the provider names it.
    pub model: String,
    /// What the model said, in order.
    pub content: Vec<Part>,
    /// Why the model stopped.
    pub stop: StopReason,
    /// What the call cost in tokens. Every answer has one: a provider that doesn't report usage
    /// fails the call, so the Game Budget never under-counts.
    pub usage: Usage,
}

impl LlmResponse {
    /// The answer's text, all of it in order.
    #[must_use]
    pub fn text(&self) -> String {
        self.content
            .iter()
            .filter_map(|part| match part {
                Part::Text { text, .. } => Some(text.as_str()),
                _ => None,
            })
            .collect()
    }

    /// A structured answer: its text read as the JSON the request's schema asked for.
    ///
    /// # Errors
    /// If the text isn't JSON, such as an answer cut short at `max_tokens` or a refusal.
    pub fn output_json(&self) -> Result<serde_json::Value, LlmError> {
        serde_json::from_str(&self.text()).map_err(|error| {
            LlmError::Malformed(format!("the answer isn't the JSON asked for: {error}"))
        })
    }
}

/// Why a model stopped answering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
    /// It finished.
    EndTurn,
    /// It hit the request's `max_tokens`, so the answer may be cut short.
    MaxTokens,
    /// It wants a tool's result before going on.
    ToolUse,
    /// It declined to answer.
    Refusal,
    /// A reason this adapter doesn't know, kept so it isn't mistaken for a finished answer.
    Other,
}

/// The tokens one call used, as the provider bills them.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Usage {
    /// Input tokens billed at the full rate.
    pub input_tokens: u64,
    /// Output tokens, thinking included.
    pub output_tokens: u64,
    /// Input tokens read from the prompt cache.
    pub cache_read_tokens: u64,
    /// Input tokens written to the prompt cache.
    pub cache_write_tokens: u64,
}
