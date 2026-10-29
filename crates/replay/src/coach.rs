//! A Coach question as an LLM call, and its answer read back: the provider-neutral request the
//! app and the replayer send, and the structured output the core takes.

use copilot_core::{QuestionId, TacticalAnswer, TacticalQuestion};
use copilot_llm::{
    CacheHint, LlmError, LlmRequest, LlmResponse, Message, ModelChoice, OutputSchema, SystemBlock,
    Thinking,
};
use serde_json::json;

/// Tokens to a word of English, about: a reason of `n` words costs about `n × 4 / 3` tokens.
const TOKENS_PER_WORD_NUMERATOR: u32 = 4;
/// See [`TOKENS_PER_WORD_NUMERATOR`].
const TOKENS_PER_WORD_DENOMINATOR: u32 = 3;

/// The `max_tokens` cap over what the word limit costs: about 2×, from the spec (#24, "Length").
const CAP_OVER_WORD_LIMIT: u32 = 2;

/// The tokens the answer's JSON costs around its reason: the braces, keys and the pick's id.
const ANSWER_JSON_TOKENS: u32 = 16;

/// What a Quick Model that thinks may spend thinking, besides its answer: its thinking counts
/// against `max_tokens`, and an answer cut short is no answer.
const THINKING_TOKENS: u32 = 2_048;

/// A call to the Coach: the question it asks, and the request that asks it.
#[derive(Debug, Clone, PartialEq)]
pub struct CoachCall {
    /// Which question it asks, which its answer is handed back with.
    pub question: QuestionId,
    /// The request to make.
    pub request: LlmRequest,
}

impl CoachCall {
    /// `question` asked of `model`: the static prefix as a cached system prompt, the moment tail
    /// as the one user turn, structured output, and `max_tokens` at about twice the word limit.
    #[must_use]
    pub fn new(question: &TacticalQuestion, model: &ModelChoice) -> Self {
        let reason_tokens =
            question.words.saturating_mul(TOKENS_PER_WORD_NUMERATOR) / TOKENS_PER_WORD_DENOMINATOR;
        let mut max_tokens = reason_tokens.saturating_mul(CAP_OVER_WORD_LIMIT) + ANSWER_JSON_TOKENS;
        if model.thinking() != Thinking::Off {
            max_tokens += THINKING_TOKENS;
        }
        Self {
            question: question.id,
            request: LlmRequest {
                system: vec![SystemBlock::cached(
                    &question.prompt.prefix,
                    CacheHint::FiveMinutes,
                )],
                output: Some(tactical_schema()),
                thinking: model.thinking(),
                ..LlmRequest::new(
                    model.model(),
                    max_tokens,
                    vec![Message::user(&question.prompt.moment)],
                )
            },
        }
    }
}

/// The answer a Tactical call asks for: a Candidate Action's id, and why.
fn tactical_schema() -> OutputSchema {
    OutputSchema(json!({
        "type": "object",
        "properties": {
            "pick": { "type": "string" },
            "reason": { "type": "string" }
        },
        "required": ["pick", "reason"],
        "additionalProperties": false
    }))
}

/// What the model answered a Tactical call, or why there is no answer: the call failed, or its
/// text isn't the JSON asked for, such as an answer cut short at `max_tokens`.
///
/// # Errors
/// As said, in words for the timeline.
pub fn tactical_answer(response: Result<LlmResponse, LlmError>) -> Result<TacticalAnswer, String> {
    let json = response
        .and_then(|response| response.output_json())
        .map_err(|error| error.to_string())?;
    serde_json::from_value(json)
        .map_err(|error| format!("the answer isn't a pick and a reason: {error}"))
}
