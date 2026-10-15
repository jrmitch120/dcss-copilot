//! The native Anthropic Messages adapter, hand-written over `reqwest`, streaming with
//! Server-Sent Events.

use std::collections::VecDeque;

use futures_util::StreamExt;
use futures_util::stream::{self, BoxStream};
use serde_json::{Value, json};

use crate::key::ApiKey;
use crate::provider::{LlmError, LlmProvider, LlmStream};
use crate::request::{CacheHint, Effort, LlmRequest, Message, OutputSchema, Part, Role, Thinking};
use crate::response::{LlmResponse, StopReason, StreamEvent, Usage};

/// Where Anthropic's API lives.
const ANTHROPIC_URL: &str = "https://api.anthropic.com";
/// The Messages API version this adapter speaks.
const API_VERSION: &str = "2023-06-01";

/// Calls Anthropic's Messages API with the player's key.
#[derive(Debug, Clone)]
pub struct AnthropicProvider {
    http: reqwest::Client,
    base_url: String,
    key: ApiKey,
}

impl AnthropicProvider {
    /// Calls Anthropic with `key`.
    #[must_use]
    pub fn new(key: ApiKey) -> Self {
        Self {
            http: reqwest::Client::new(),
            base_url: ANTHROPIC_URL.to_owned(),
            key,
        }
    }

    /// Calls the API at `base_url` instead, such as a test's fake endpoint.
    #[must_use]
    pub fn with_base_url(mut self, base_url: &str) -> Self {
        base_url
            .trim_end_matches('/')
            .clone_into(&mut self.base_url);
        self
    }
}

impl LlmProvider for AnthropicProvider {
    fn stream(&self, request: LlmRequest) -> LlmStream {
        let call = self
            .http
            .post(format!("{}/v1/messages", self.base_url))
            .header("x-api-key", self.key.expose())
            .header("anthropic-version", API_VERSION)
            .header("content-type", "application/json")
            .body(wire_request(&request).to_string());
        stream::unfold(Call::Start(call), Call::next).boxed()
    }
}

/// The request body, in the Messages API's shape.
fn wire_request(request: &LlmRequest) -> Value {
    let mut body = json!({
        "model": request.model,
        "max_tokens": request.max_tokens,
        "stream": true,
        "messages": request.messages.iter().map(wire_message).collect::<Vec<_>>(),
    });
    if !request.system.is_empty() {
        body["system"] = request
            .system
            .iter()
            .map(|block| with_cache(json!({ "type": "text", "text": block.text }), block.cache))
            .collect();
    }
    if !request.tools.is_empty() {
        body["tools"] = request
            .tools
            .iter()
            .map(|tool| {
                json!({
                    "name": tool.name,
                    "description": tool.description,
                    "input_schema": tool.input_schema,
                })
            })
            .collect();
    }
    let mut output_config = serde_json::Map::new();
    match request.thinking {
        Thinking::Off => body["thinking"] = json!({ "type": "disabled" }),
        Thinking::Adaptive(effort) => {
            body["thinking"] = json!({ "type": "adaptive" });
            let effort = match effort {
                Effort::Low => "low",
                Effort::Medium => "medium",
                Effort::High => "high",
            };
            output_config.insert("effort".to_owned(), json!(effort));
        }
    }
    if let Some(OutputSchema(schema)) = &request.output {
        output_config.insert(
            "format".to_owned(),
            json!({ "type": "json_schema", "schema": schema }),
        );
    }
    if !output_config.is_empty() {
        body["output_config"] = Value::Object(output_config);
    }
    body
}

/// Adds Anthropic's explicit `cache_control` breakpoint for a cache hint.
fn with_cache(mut block: Value, cache: Option<CacheHint>) -> Value {
    match cache {
        Some(CacheHint::FiveMinutes) => block["cache_control"] = json!({ "type": "ephemeral" }),
        Some(CacheHint::OneHour) => {
            block["cache_control"] = json!({ "type": "ephemeral", "ttl": "1h" });
        }
        None => {}
    }
    block
}

fn wire_message(message: &Message) -> Value {
    let role = match message.role {
        Role::User => "user",
        Role::Assistant => "assistant",
    };
    json!({
        "role": role,
        "content": message.content.iter().map(wire_part).collect::<Vec<_>>(),
    })
}

fn wire_part(part: &Part) -> Value {
    match part {
        Part::Text { text, cache } => with_cache(json!({ "type": "text", "text": text }), *cache),
        Part::Thinking {
            thinking,
            signature,
        } => json!({ "type": "thinking", "thinking": thinking, "signature": signature }),
        Part::RedactedThinking { data } => json!({ "type": "redacted_thinking", "data": data }),
        Part::ToolUse { id, name, input } => {
            json!({ "type": "tool_use", "id": id, "name": name, "input": input })
        }
        Part::ToolResult {
            tool_use_id,
            content,
            is_error,
        } => json!({
            "type": "tool_result",
            "tool_use_id": tool_use_id,
            "content": content,
            "is_error": is_error,
        }),
    }
}

/// One call, from sending the request to the end of its stream.
enum Call {
    Start(reqwest::RequestBuilder),
    Reading {
        body: BoxStream<'static, Result<Vec<u8>, String>>,
        frames: SseFrames,
        answer: Answer,
        ready: VecDeque<StreamEvent>,
    },
    Done,
}

impl Call {
    /// The next item of the stream, and what's left of the call.
    async fn next(mut self) -> Option<(Result<StreamEvent, LlmError>, Self)> {
        loop {
            match self {
                Self::Done => return None,
                Self::Start(call) => match start(call).await {
                    Ok(body) => {
                        self = Self::Reading {
                            body,
                            frames: SseFrames::default(),
                            answer: Answer::default(),
                            ready: VecDeque::new(),
                        };
                    }
                    Err(error) => return Some((Err(error), Self::Done)),
                },
                Self::Reading {
                    mut body,
                    mut frames,
                    mut answer,
                    mut ready,
                } => {
                    if let Some(event) = ready.pop_front() {
                        let rest = if matches!(event, StreamEvent::Finished(_)) {
                            Self::Done
                        } else {
                            Self::Reading {
                                body,
                                frames,
                                answer,
                                ready,
                            }
                        };
                        return Some((Ok(event), rest));
                    }
                    let chunk = match body.next().await {
                        Some(Ok(chunk)) => chunk,
                        Some(Err(error)) => {
                            return Some((Err(LlmError::Unreachable(error)), Self::Done));
                        }
                        None => return Some((Err(LlmError::Incomplete), Self::Done)),
                    };
                    for data in frames.push(&chunk) {
                        match answer.apply(&data) {
                            Ok(events) => ready.extend(events),
                            Err(error) => return Some((Err(error), Self::Done)),
                        }
                    }
                    self = Self::Reading {
                        body,
                        frames,
                        answer,
                        ready,
                    };
                }
            }
        }
    }
}

/// Sends the request and hands back the body of a successful answer.
async fn start(
    call: reqwest::RequestBuilder,
) -> Result<BoxStream<'static, Result<Vec<u8>, String>>, LlmError> {
    let response = call
        .send()
        .await
        .map_err(|error| LlmError::Unreachable(error.to_string()))?;
    let status = response.status().as_u16();
    if !response.status().is_success() {
        let body = response.text().await.unwrap_or_default();
        let message = serde_json::from_str::<Value>(&body)
            .ok()
            .and_then(|error| error["error"]["message"].as_str().map(str::to_owned))
            .unwrap_or(body);
        return Err(api_error(status, message));
    }
    Ok(response
        .bytes_stream()
        .map(|chunk| {
            chunk
                .map(|bytes| bytes.to_vec())
                .map_err(|error| error.to_string())
        })
        .boxed())
}

/// Splits a Server-Sent Events body into the `data` of each event, however it was chunked.
#[derive(Default)]
struct SseFrames {
    pending: Vec<u8>,
}

impl SseFrames {
    /// Takes in a chunk of the body and returns the data of every event it completed.
    fn push(&mut self, chunk: &[u8]) -> Vec<String> {
        self.pending
            .extend(chunk.iter().copied().filter(|&byte| byte != b'\r'));
        let mut data = Vec::new();
        while let Some(end) = self.pending.windows(2).position(|pair| pair == b"\n\n") {
            let frame: Vec<u8> = self.pending.drain(..end + 2).collect();
            let frame = String::from_utf8_lossy(&frame);
            let lines: Vec<&str> = frame
                .lines()
                .filter_map(|line| line.strip_prefix("data:"))
                .map(str::trim_start)
                .collect();
            if !lines.is_empty() {
                data.push(lines.join("\n"));
            }
        }
        data
    }
}

/// The answer as its events arrive.
#[derive(Default)]
struct Answer {
    model: String,
    blocks: Vec<Block>,
    stop: Option<StopReason>,
    input_tokens: Option<u64>,
    output_tokens: Option<u64>,
    cache_read_tokens: u64,
    cache_write_tokens: u64,
}

/// One content block of the answer, while it streams.
enum Block {
    Text(String),
    Thinking {
        thinking: String,
        signature: String,
    },
    RedactedThinking(String),
    ToolUse {
        id: String,
        name: String,
        /// The arguments' JSON, which streams in pieces.
        json: String,
    },
    /// A kind of block this adapter doesn't know.
    Unknown,
}

impl Block {
    fn start(block: &Value) -> Self {
        let text = |field: &str| block[field].as_str().unwrap_or_default().to_owned();
        match block["type"].as_str() {
            Some("text") => Self::Text(text("text")),
            Some("thinking") => Self::Thinking {
                thinking: text("thinking"),
                signature: text("signature"),
            },
            Some("redacted_thinking") => Self::RedactedThinking(text("data")),
            Some("tool_use") => Self::ToolUse {
                id: text("id"),
                name: text("name"),
                json: String::new(),
            },
            _ => Self::Unknown,
        }
    }

    /// Applies a delta and returns any text the caller should see now.
    fn apply(&mut self, delta: &Value) -> Option<String> {
        let more = |field: &str| delta[field].as_str().unwrap_or_default();
        match (self, delta["type"].as_str()) {
            (Self::Text(text), Some("text_delta")) => {
                text.push_str(more("text"));
                return Some(more("text").to_owned());
            }
            (Self::Thinking { thinking, .. }, Some("thinking_delta")) => {
                thinking.push_str(more("thinking"));
            }
            (Self::Thinking { signature, .. }, Some("signature_delta")) => {
                signature.push_str(more("signature"));
            }
            (Self::ToolUse { json, .. }, Some("input_json_delta")) => {
                json.push_str(more("partial_json"));
            }
            _ => {}
        }
        None
    }

    fn finish(self) -> Result<Option<Part>, LlmError> {
        Ok(Some(match self {
            Self::Text(text) => Part::Text { text, cache: None },
            Self::Thinking {
                thinking,
                signature,
            } => Part::Thinking {
                thinking,
                signature,
            },
            Self::RedactedThinking(data) => Part::RedactedThinking { data },
            Self::ToolUse { id, name, json } => {
                // A call with no arguments streams no JSON at all.
                let input = if json.is_empty() {
                    json!({})
                } else {
                    serde_json::from_str(&json)
                        .map_err(|error| LlmError::Malformed(format!("tool `{name}`: {error}")))?
                };
                Part::ToolUse { id, name, input }
            }
            Self::Unknown => return Ok(None),
        }))
    }
}

impl Answer {
    /// Applies one event's data and returns what the caller should see of it.
    fn apply(&mut self, data: &str) -> Result<Vec<StreamEvent>, LlmError> {
        let event: Value =
            serde_json::from_str(data).map_err(|error| LlmError::Malformed(error.to_string()))?;
        match event["type"].as_str() {
            Some("message_start") => {
                let message = &event["message"];
                message["model"]
                    .as_str()
                    .unwrap_or_default()
                    .clone_into(&mut self.model);
                self.read_usage(&message["usage"]);
            }
            Some("content_block_start") => self.blocks.push(Block::start(&event["content_block"])),
            Some("content_block_delta") => {
                if let Some(text) = self
                    .blocks
                    .last_mut()
                    .and_then(|block| block.apply(&event["delta"]))
                {
                    return Ok(vec![StreamEvent::Text(text)]);
                }
            }
            Some("message_delta") => {
                self.stop = Some(stop_reason(event["delta"]["stop_reason"].as_str()));
                self.read_usage(&event["usage"]);
            }
            Some("message_stop") => return Ok(vec![StreamEvent::Finished(self.finish()?)]),
            Some("error") => {
                let error = &event["error"];
                let message = error["message"].as_str().unwrap_or_default().to_owned();
                return Err(api_error(error_status(error["type"].as_str()), message));
            }
            _ => {}
        }
        Ok(Vec::new())
    }

    fn read_usage(&mut self, usage: &Value) {
        if let Some(tokens) = usage["input_tokens"].as_u64() {
            self.input_tokens = Some(tokens);
        }
        // Output tokens are a running total, so the last report wins.
        if let Some(tokens) = usage["output_tokens"].as_u64() {
            self.output_tokens = Some(tokens);
        }
        if let Some(tokens) = usage["cache_read_input_tokens"].as_u64() {
            self.cache_read_tokens = tokens;
        }
        if let Some(tokens) = usage["cache_creation_input_tokens"].as_u64() {
            self.cache_write_tokens = tokens;
        }
    }

    fn finish(&mut self) -> Result<LlmResponse, LlmError> {
        let mut content = Vec::new();
        for block in std::mem::take(&mut self.blocks) {
            content.extend(block.finish()?);
        }
        let (Some(input_tokens), Some(output_tokens)) = (self.input_tokens, self.output_tokens)
        else {
            return Err(LlmError::NoUsage);
        };
        Ok(LlmResponse {
            model: std::mem::take(&mut self.model),
            content,
            stop: self.stop.unwrap_or(StopReason::Other),
            usage: Usage {
                input_tokens,
                output_tokens,
                cache_read_tokens: self.cache_read_tokens,
                cache_write_tokens: self.cache_write_tokens,
            },
        })
    }
}

/// An error the API answered with, by its HTTP status.
fn api_error(status: u16, message: String) -> LlmError {
    match status {
        401 | 403 => LlmError::KeyRejected { status, message },
        429 => LlmError::RateLimited { message },
        500.. => LlmError::Unavailable { status, message },
        _ => LlmError::BadRequest { status, message },
    }
}

/// The HTTP status the API gives an error of this type, for errors that arrive mid-stream.
fn error_status(kind: Option<&str>) -> u16 {
    match kind {
        Some("authentication_error") => 401,
        Some("permission_error") => 403,
        Some("rate_limit_error") => 429,
        Some("overloaded_error") => 529,
        Some("invalid_request_error") => 400,
        _ => 500,
    }
}

fn stop_reason(reason: Option<&str>) -> StopReason {
    match reason {
        Some("end_turn" | "stop_sequence") => StopReason::EndTurn,
        Some("max_tokens") => StopReason::MaxTokens,
        Some("tool_use") => StopReason::ToolUse,
        Some("refusal") => StopReason::Refusal,
        _ => StopReason::Other,
    }
}
