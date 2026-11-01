//! The OpenAI-compatible chat completions adapter, hand-written over `reqwest`, streaming with
//! Server-Sent Events, and the presets that differ in what they accept.

use serde_json::{Value, json};

use crate::chat::{self, turn_text, wire_tool};
use crate::key::ApiKey;
use crate::provider::{LlmError, LlmProvider, LlmStream};
use crate::request::{Effort, LlmRequest, Message, OutputSchema, Part, Thinking};
use crate::response::{LlmResponse, StopReason, StreamEvent, Usage};
use crate::stream::{self, SseFrames, api_error, error_message};

/// Where OpenAI's API lives.
const OPENAI_URL: &str = "https://api.openai.com/v1";
/// Where OpenRouter's API lives.
const OPENROUTER_URL: &str = "https://openrouter.ai/api/v1";

/// What a request says differently to each preset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Preset {
    OpenAi,
    OpenRouter,
    LlamaServer,
}

/// Calls an OpenAI-compatible chat completions API. Cache hints are ignored, since these
/// providers cache by themselves and report what they read in the usage.
#[derive(Debug, Clone)]
pub struct OpenAiCompatible {
    http: reqwest::Client,
    preset: Preset,
    base_url: String,
    key: Option<ApiKey>,
}

impl OpenAiCompatible {
    fn new(preset: Preset, base_url: &str, key: Option<ApiKey>) -> Self {
        Self {
            http: reqwest::Client::new(),
            preset,
            base_url: base_url.trim_end_matches('/').to_owned(),
            key,
        }
    }

    /// Calls OpenAI with `key`.
    #[must_use]
    pub fn openai(key: ApiKey) -> Self {
        Self::new(Preset::OpenAi, OPENAI_URL, Some(key))
    }

    /// Calls OpenRouter with `key`.
    #[must_use]
    pub fn openrouter(key: ApiKey) -> Self {
        Self::new(Preset::OpenRouter, OPENROUTER_URL, Some(key))
    }

    /// Calls the llama-server at `url`, which needs no key.
    #[must_use]
    pub fn llama_server(url: &str) -> Self {
        let url = format!("{}/v1", url.trim_end_matches('/'));
        Self::new(Preset::LlamaServer, &url, None)
    }

    /// Calls the API at `base_url` instead, such as a test's fake endpoint.
    #[must_use]
    pub fn with_base_url(mut self, base_url: &str) -> Self {
        base_url
            .trim_end_matches('/')
            .clone_into(&mut self.base_url);
        self
    }

    /// The ids of the models the server says it serves.
    ///
    /// # Errors
    /// If the server can't be reached, refuses, or answers with something else.
    pub async fn models(&self) -> Result<Vec<String>, LlmError> {
        let listed = stream::get_json(self.http.get(format!("{}/models", self.base_url))).await?;
        let models = listed["data"]
            .as_array()
            .ok_or_else(|| LlmError::Malformed("the server listed no models".to_owned()))?;
        Ok(models
            .iter()
            .filter_map(|model| model["id"].as_str().map(str::to_owned))
            .collect())
    }
}

impl LlmProvider for OpenAiCompatible {
    fn stream(&self, request: LlmRequest) -> LlmStream {
        let mut call = self
            .http
            .post(format!("{}/chat/completions", self.base_url))
            .header("content-type", "application/json")
            .body(wire_request(self.preset, &request).to_string());
        if let Some(key) = &self.key {
            call = call.bearer_auth(key.expose());
        }
        stream::answer_stream::<Answer>(call)
    }
}

/// The request body, in the chat completions shape as `preset` accepts it.
fn wire_request(preset: Preset, request: &LlmRequest) -> Value {
    let mut body = json!({
        "model": request.model,
        "stream": true,
        "messages": chat::wire_messages(request, wire_tool_result, wire_assistant_turn),
    });
    match preset {
        // OpenAI's current models refuse `max_tokens`.
        Preset::OpenAi => body["max_completion_tokens"] = json!(request.max_tokens),
        Preset::OpenRouter | Preset::LlamaServer => body["max_tokens"] = json!(request.max_tokens),
    }
    match preset {
        Preset::OpenAi | Preset::LlamaServer => {
            body["stream_options"] = json!({ "include_usage": true });
        }
        // It always reports usage; requiring what's sent stops a provider silently ignoring it.
        Preset::OpenRouter => {
            body["provider"] = json!({ "require_parameters": true });
        }
    }
    if !request.tools.is_empty() {
        body["tools"] = request.tools.iter().map(wire_tool).collect();
    }
    if let Some(OutputSchema(schema)) = &request.output {
        body["response_format"] = json!({
            "type": "json_schema",
            "json_schema": { "name": "answer", "strict": true, "schema": schema },
        });
    }
    match (preset, request.thinking) {
        // Requiring parameters would rule out every model that can't reason, for asking it not to.
        (Preset::OpenRouter, Thinking::Off) => {}
        (Preset::OpenRouter, Thinking::Adaptive(effort)) => {
            body["reasoning"] = json!({ "effort": effort_name(effort) });
        }
        // Some models think unless told not to.
        (Preset::OpenAi | Preset::LlamaServer, Thinking::Off) => {
            body["reasoning_effort"] = json!("none");
        }
        (Preset::OpenAi | Preset::LlamaServer, Thinking::Adaptive(effort)) => {
            body["reasoning_effort"] = json!(effort_name(effort));
        }
    }
    body
}

fn effort_name(effort: Effort) -> &'static str {
    match effort {
        Effort::Low => "low",
        Effort::Medium => "medium",
        Effort::High => "high",
    }
}

fn wire_tool_result(tool_use_id: &str, content: &str) -> Value {
    json!({ "role": "tool", "tool_call_id": tool_use_id, "content": content })
}

/// A turn from the model's side. Its reasoning stays behind: only Anthropic reads it back.
fn wire_assistant_turn(turn: &Message) -> Value {
    let calls: Vec<Value> = turn
        .content
        .iter()
        .filter_map(|part| match part {
            Part::ToolUse { id, name, input } => Some(json!({
                "id": id,
                "type": "function",
                "function": { "name": name, "arguments": input.to_string() },
            })),
            _ => None,
        })
        .collect();
    let text = turn_text(turn);
    let mut wire = json!({
        "role": "assistant",
        "content": if text.is_empty() { Value::Null } else { json!(text) },
    });
    if !calls.is_empty() {
        wire["tool_calls"] = Value::Array(calls);
    }
    wire
}

/// The answer as its chunks arrive.
#[derive(Default)]
struct Answer {
    model: String,
    text: String,
    calls: Vec<ToolCall>,
    finish: Option<StopReason>,
    refused: bool,
    usage: Option<Usage>,
}

/// One tool call of the answer, while it streams.
#[derive(Default)]
struct ToolCall {
    id: String,
    name: String,
    /// The arguments' JSON, which streams in pieces.
    json: String,
}

impl stream::Answer for Answer {
    type Frames = SseFrames;

    fn apply(&mut self, data: &str) -> Result<Vec<StreamEvent>, LlmError> {
        if data == "[DONE]" {
            return Ok(vec![StreamEvent::Finished(self.finish()?)]);
        }
        let chunk: Value =
            serde_json::from_str(data).map_err(|error| LlmError::Malformed(error.to_string()))?;
        let error = &chunk["error"];
        if !error.is_null() {
            let status = error["code"]
                .as_u64()
                .and_then(|code| u16::try_from(code).ok())
                .unwrap_or(500);
            return Err(api_error(status, error_message(error).unwrap_or_default()));
        }
        if let Some(model) = chunk["model"].as_str() {
            model.clone_into(&mut self.model);
        }
        self.read_usage(&chunk["usage"]);
        let choice = &chunk["choices"][0];
        if let Some(reason) = choice["finish_reason"].as_str() {
            self.finish = Some(stop_reason(reason));
        }
        let delta = &choice["delta"];
        self.read_tool_calls(delta);
        if delta["refusal"]
            .as_str()
            .is_some_and(|text| !text.is_empty())
        {
            self.refused = true;
        }
        match delta["content"].as_str() {
            Some(text) if !text.is_empty() => {
                self.text.push_str(text);
                Ok(vec![StreamEvent::Text(text.to_owned())])
            }
            _ => Ok(Vec::new()),
        }
    }
}

impl Answer {
    fn read_tool_calls(&mut self, delta: &Value) {
        let Some(calls) = delta["tool_calls"].as_array() else {
            return;
        };
        for call in calls {
            let index = call["index"]
                .as_u64()
                .and_then(|index| usize::try_from(index).ok())
                .unwrap_or(self.calls.len());
            if self.calls.len() <= index {
                self.calls.resize_with(index + 1, ToolCall::default);
            }
            let read = &mut self.calls[index];
            if let Some(id) = call["id"].as_str() {
                id.clone_into(&mut read.id);
            }
            if let Some(name) = call["function"]["name"].as_str() {
                name.clone_into(&mut read.name);
            }
            if let Some(arguments) = call["function"]["arguments"].as_str() {
                read.json.push_str(arguments);
            }
        }
    }

    fn read_usage(&mut self, usage: &Value) {
        let (Some(prompt), Some(output_tokens)) = (
            usage["prompt_tokens"].as_u64(),
            usage["completion_tokens"].as_u64(),
        ) else {
            return;
        };
        let details = &usage["prompt_tokens_details"];
        let cache_read_tokens = details["cached_tokens"].as_u64().unwrap_or(0);
        let cache_write_tokens = details["cache_write_tokens"].as_u64().unwrap_or(0);
        self.usage = Some(Usage {
            // The cache's tokens are counted in the prompt's, but billed at their own rates.
            input_tokens: prompt
                .saturating_sub(cache_read_tokens)
                .saturating_sub(cache_write_tokens),
            output_tokens,
            cache_read_tokens,
            cache_write_tokens,
        });
    }

    fn finish(&mut self) -> Result<LlmResponse, LlmError> {
        let mut content = Vec::new();
        if !self.text.is_empty() {
            content.push(Part::text(&self.text));
        }
        for call in std::mem::take(&mut self.calls) {
            // A call with no arguments streams no JSON at all.
            let input = if call.json.is_empty() {
                json!({})
            } else {
                serde_json::from_str(&call.json).map_err(|error| {
                    LlmError::Malformed(format!("tool `{}`: {error}", call.name))
                })?
            };
            content.push(Part::ToolUse {
                id: call.id,
                name: call.name,
                input,
            });
        }
        let stop = if self.refused {
            StopReason::Refusal
        } else {
            match self.finish.unwrap_or(StopReason::Other) {
                // Not every server says `tool_calls` when it called a tool.
                StopReason::EndTurn
                    if content
                        .iter()
                        .any(|part| matches!(part, Part::ToolUse { .. })) =>
                {
                    StopReason::ToolUse
                }
                stop => stop,
            }
        };
        let usage = self.usage.ok_or(LlmError::NoUsage)?;
        Ok(LlmResponse {
            model: std::mem::take(&mut self.model),
            content,
            stop,
            usage,
        })
    }
}

fn stop_reason(reason: &str) -> StopReason {
    match reason {
        "stop" => StopReason::EndTurn,
        "length" => StopReason::MaxTokens,
        "tool_calls" => StopReason::ToolUse,
        "content_filter" => StopReason::Refusal,
        _ => StopReason::Other,
    }
}
