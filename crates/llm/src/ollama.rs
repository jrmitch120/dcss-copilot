//! The native Ollama adapter, hand-written over `reqwest`, streaming JSON Lines.
//!
//! Ollama has an OpenAI-compatible endpoint too, but it can't set `num_ctx`: its context stays at
//! the 4096-token default, which silently cuts the Advisor's prompts off. Only `/api/chat` can.

use serde_json::{Value, json};

use crate::openai::{turn_text, wire_tool};
use crate::provider::{LlmError, LlmProvider, LlmStream};
use crate::request::{LlmRequest, Message, OutputSchema, Part, Role, Thinking};
use crate::response::{LlmResponse, StopReason, StreamEvent, Usage};
use crate::stream::{self, Frames, api_error, said};

/// The context each call asks for: room for the static prefix, the largest moment tail and the
/// ask box's tool results (#24, Tail budgets; `round2.html` on `prototype/setup-settings`).
const NUM_CTX: u32 = 16_384;

/// Calls the Ollama at a local address. It takes no key.
#[derive(Debug, Clone)]
pub struct OllamaProvider {
    http: reqwest::Client,
    url: String,
}

impl OllamaProvider {
    /// Calls the Ollama at `url`.
    #[must_use]
    pub fn new(url: &str) -> Self {
        Self {
            http: reqwest::Client::new(),
            url: url.trim_end_matches('/').to_owned(),
        }
    }

    /// The names of the models this Ollama has, any of which it loads when called.
    ///
    /// # Errors
    /// If it can't be reached, refuses, or answers with something else.
    pub async fn models(&self) -> Result<Vec<String>, LlmError> {
        let listed = stream::listing(self.http.get(format!("{}/api/tags", self.url))).await?;
        let models = listed["models"]
            .as_array()
            .ok_or_else(|| LlmError::Malformed("Ollama listed no models".to_owned()))?;
        Ok(models
            .iter()
            .filter_map(|model| model["name"].as_str().map(str::to_owned))
            .collect())
    }
}

impl LlmProvider for OllamaProvider {
    fn stream(&self, request: LlmRequest) -> LlmStream {
        let call = self
            .http
            .post(format!("{}/api/chat", self.url))
            .header("content-type", "application/json")
            .body(wire_request(&request).to_string());
        stream::answer_stream::<Answer>(call)
    }
}

/// The request body, in `/api/chat`'s shape.
fn wire_request(request: &LlmRequest) -> Value {
    let mut body = json!({
        "model": request.model,
        "stream": true,
        "messages": wire_messages(request),
        "think": request.thinking != Thinking::Off,
        "options": { "num_ctx": NUM_CTX, "num_predict": request.max_tokens },
    });
    if !request.tools.is_empty() {
        body["tools"] = request.tools.iter().map(wire_tool).collect();
    }
    if let Some(OutputSchema(schema)) = &request.output {
        body["format"] = schema.clone();
    }
    body
}

fn wire_messages(request: &LlmRequest) -> Vec<Value> {
    let mut messages = Vec::new();
    if !request.system.is_empty() {
        let system: Vec<&str> = request
            .system
            .iter()
            .map(|block| block.text.as_str())
            .collect();
        messages.push(json!({ "role": "system", "content": system.join("\n\n") }));
    }
    for message in &request.messages {
        match message.role {
            Role::User => push_user_turn(&mut messages, message, &request.messages),
            Role::Assistant => messages.push(wire_assistant_turn(message)),
        }
    }
    messages
}

/// A turn from the Advisor's side, its tool results first and each a message of its own.
fn push_user_turn(messages: &mut Vec<Value>, turn: &Message, conversation: &[Message]) {
    for part in &turn.content {
        if let Part::ToolResult {
            tool_use_id,
            content,
            ..
        } = part
        {
            messages.push(json!({
                "role": "tool",
                "tool_name": tool_called(conversation, tool_use_id),
                "tool_call_id": tool_use_id,
                "content": content,
            }));
        }
    }
    let text = turn_text(turn);
    if !text.is_empty() {
        messages.push(json!({ "role": "user", "content": text }));
    }
}

/// The name of the tool the call `id` called, which Ollama wants beside its result.
fn tool_called<'a>(conversation: &'a [Message], id: &str) -> &'a str {
    conversation
        .iter()
        .flat_map(|message| &message.content)
        .find_map(|part| match part {
            Part::ToolUse {
                id: called, name, ..
            } if called == id => Some(name.as_str()),
            _ => None,
        })
        .unwrap_or_default()
}

fn wire_assistant_turn(turn: &Message) -> Value {
    let calls: Vec<Value> = turn
        .content
        .iter()
        .filter_map(|part| match part {
            Part::ToolUse { id, name, input } => Some(json!({
                "id": id,
                "function": { "name": name, "arguments": input },
            })),
            _ => None,
        })
        .collect();
    let mut wire = json!({ "role": "assistant", "content": turn_text(turn) });
    if !calls.is_empty() {
        wire["tool_calls"] = Value::Array(calls);
    }
    wire
}

/// JSON Lines: each complete line of the body, however it was chunked.
#[derive(Default)]
struct JsonLines {
    pending: Vec<u8>,
}

impl Frames for JsonLines {
    fn push(&mut self, chunk: &[u8]) -> Vec<String> {
        self.pending.extend_from_slice(chunk);
        let mut lines = Vec::new();
        while let Some(end) = self.pending.iter().position(|&byte| byte == b'\n') {
            let line: Vec<u8> = self.pending.drain(..=end).collect();
            let line = String::from_utf8_lossy(&line);
            let line = line.trim();
            if !line.is_empty() {
                lines.push(line.to_owned());
            }
        }
        lines
    }
}

/// The answer as its lines arrive.
#[derive(Default)]
struct Answer {
    text: String,
    calls: Vec<Part>,
}

impl stream::Answer for Answer {
    type Frames = JsonLines;

    fn apply(&mut self, data: &str) -> Result<Vec<StreamEvent>, LlmError> {
        let line: Value =
            serde_json::from_str(data).map_err(|error| LlmError::Malformed(error.to_string()))?;
        if let Some(message) = said(&line["error"]) {
            return Err(api_error(500, message));
        }
        let message = &line["message"];
        for call in message["tool_calls"].as_array().into_iter().flatten() {
            self.read_tool_call(call);
        }
        let mut events = Vec::new();
        if let Some(text) = message["content"].as_str().filter(|text| !text.is_empty()) {
            self.text.push_str(text);
            events.push(StreamEvent::Text(text.to_owned()));
        }
        if line["done"].as_bool() == Some(true) {
            events.push(StreamEvent::Finished(self.finish(&line)?));
        }
        Ok(events)
    }
}

impl Answer {
    fn read_tool_call(&mut self, call: &Value) {
        let function = &call["function"];
        self.calls.push(Part::ToolUse {
            id: call["id"]
                .as_str()
                .map_or_else(|| format!("call_{}", self.calls.len()), str::to_owned),
            name: function["name"].as_str().unwrap_or_default().to_owned(),
            input: function["arguments"].clone(),
        });
    }

    fn finish(&mut self, last: &Value) -> Result<LlmResponse, LlmError> {
        let output_tokens = last["eval_count"].as_u64().ok_or(LlmError::NoUsage)?;
        // Ollama leaves out a count of nothing, as when the whole prompt came from its cache.
        let prompt_tokens = last["prompt_eval_count"].as_u64().unwrap_or(0);
        let cache_read_tokens = last["prompt_eval_cached_count"].as_u64().unwrap_or(0);
        let mut content = Vec::new();
        if !self.text.is_empty() {
            content.push(Part::text(&self.text));
        }
        content.append(&mut self.calls);
        let called = content
            .iter()
            .any(|part| matches!(part, Part::ToolUse { .. }));
        Ok(LlmResponse {
            model: last["model"].as_str().unwrap_or_default().to_owned(),
            content,
            stop: match last["done_reason"].as_str() {
                // It says `stop` when it called a tool as well.
                Some("stop") if called => StopReason::ToolUse,
                Some("stop") => StopReason::EndTurn,
                Some("length") => StopReason::MaxTokens,
                _ => StopReason::Other,
            },
            usage: Usage {
                input_tokens: prompt_tokens.saturating_sub(cache_read_tokens),
                output_tokens,
                cache_read_tokens,
                cache_write_tokens: 0,
            },
        })
    }
}
