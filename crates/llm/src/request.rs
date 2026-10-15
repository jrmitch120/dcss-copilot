//! What the Advisor asks a model: the provider-neutral shape of one LLM call.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// One call to a model. Providers translate it to their own wire format; the LLM sidecar keys
/// recorded answers by its hash, so everything that changes the answer lives here.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LlmRequest {
    /// The provider's model id, such as `claude-haiku-5-5`.
    pub model: String,
    /// The most the answer may spend on output, thinking included.
    pub max_tokens: u32,
    /// The system prompt, in blocks so a cache hint can end the static prefix.
    pub system: Vec<SystemBlock>,
    /// The conversation so far, starting with the Advisor's side.
    pub messages: Vec<Message>,
    /// Engine tools the model may call, such as lookup or simulate.
    pub tools: Vec<ToolDef>,
    /// The JSON Schema the answer's text must follow, for structured output.
    pub output: Option<OutputSchema>,
    /// Whether the model thinks before answering.
    pub thinking: Thinking,
}

impl LlmRequest {
    /// A plain call to `model`: no system prompt, tools or schema, and no thinking.
    #[must_use]
    pub fn new(model: &str, max_tokens: u32, messages: Vec<Message>) -> Self {
        Self {
            model: model.to_owned(),
            max_tokens,
            system: Vec::new(),
            messages,
            tools: Vec::new(),
            output: None,
            thinking: Thinking::Off,
        }
    }
}

/// A hint that everything up to and including this block is a prefix worth caching, because
/// later calls will repeat it byte for byte. Providers without explicit caching ignore it.
///
/// Anthropic allows at most four per request, and an hour-long hint can't follow a
/// five-minute one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CacheHint {
    /// Kept for five minutes after its last use: the static prefix during play.
    FiveMinutes,
    /// Kept for an hour, at a higher write price: worth it for a game block reused across pauses.
    OneHour,
}

/// One block of the system prompt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SystemBlock {
    /// The prompt text.
    pub text: String,
    /// Whether the prompt up to here should be cached.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache: Option<CacheHint>,
}

impl SystemBlock {
    /// A block of system prompt that ends a cached prefix.
    #[must_use]
    pub fn cached(text: &str, cache: CacheHint) -> Self {
        Self {
            text: text.to_owned(),
            cache: Some(cache),
        }
    }
}

/// One turn of the conversation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Message {
    /// Who said it.
    pub role: Role,
    /// What was said, in order.
    pub content: Vec<Part>,
}

impl Message {
    /// A turn from the Advisor's side holding one piece of text.
    #[must_use]
    pub fn user(text: &str) -> Self {
        Self {
            role: Role::User,
            content: vec![Part::text(text)],
        }
    }
}

/// Which side of the conversation a turn is from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    /// The Advisor's side: prompts, the player's questions and tool results.
    User,
    /// The model's side.
    Assistant,
}

/// One piece of a turn, in a request or an answer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Part {
    /// Plain text.
    Text {
        /// The text.
        text: String,
        /// Whether the prompt up to here should be cached. Answers never carry one.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        cache: Option<CacheHint>,
    },
    /// The model's reasoning, which goes back unchanged when a tool call continues the turn.
    Thinking {
        /// The reasoning, or its summary; empty when the provider withholds it.
        thinking: String,
        /// The provider's proof that the reasoning is unaltered.
        signature: String,
    },
    /// Reasoning the provider returned only in encrypted form, to be sent back as it is.
    RedactedThinking {
        /// The encrypted reasoning.
        data: String,
    },
    /// The model calling one of the request's tools.
    ToolUse {
        /// The call's id, which its result refers to.
        id: String,
        /// The tool called.
        name: String,
        /// Its arguments, as JSON.
        input: Value,
    },
    /// The engine's answer to a tool call.
    ToolResult {
        /// The id of the call this answers.
        tool_use_id: String,
        /// The result, rendered as text.
        content: String,
        /// Whether the tool failed, with `content` saying why.
        is_error: bool,
    },
}

impl Part {
    /// A piece of plain text.
    #[must_use]
    pub fn text(text: &str) -> Self {
        Self::Text {
            text: text.to_owned(),
            cache: None,
        }
    }

    /// A piece of text that ends a cached prefix.
    #[must_use]
    pub fn cached_text(text: &str, cache: CacheHint) -> Self {
        Self::Text {
            text: text.to_owned(),
            cache: Some(cache),
        }
    }
}

/// An engine tool the model may call.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolDef {
    /// The tool's name, such as `lookup`.
    pub name: String,
    /// What the tool does, for the model.
    pub description: String,
    /// The JSON Schema of its arguments.
    pub input_schema: Value,
}

/// The JSON Schema an answer must follow. Its objects must close with
/// `"additionalProperties": false`, which providers with strict structured output require.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OutputSchema(pub Value);

/// Whether a model thinks before it answers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Thinking {
    /// It answers straight away: fast and cheap, for Tactical calls.
    Off,
    /// It decides how much to think, up to the effort given.
    Adaptive(Effort),
}

/// How hard a thinking model works on an answer, trading cost and time for quality.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Effort {
    /// Brief.
    Low,
    /// Balanced.
    Medium,
    /// Thorough.
    High,
}
