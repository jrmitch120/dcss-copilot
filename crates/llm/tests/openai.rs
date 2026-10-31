//! The OpenAI-compatible chat completions adapter and its OpenAI, OpenRouter and llama-server
//! presets, against a fake endpoint that answers with Server-Sent Events written from each
//! provider's documentation. Asserted: what the adapter sends, and what the caller gets back.

// Helpers outside #[test] functions panic on a broken setup too.
#![allow(clippy::unwrap_used)]

mod support;

use copilot_llm::{
    ApiKey, CacheHint, Effort, LlmError, LlmProvider, LlmRequest, Message, OpenAiCompatible,
    OutputSchema, Part, Role, StopReason, StreamEvent, SystemBlock, Thinking, ToolDef, Usage,
    complete,
};
use futures_util::StreamExt;
use serde_json::json;
use support::{Canned, FakeEndpoint};

const TEXT_ANSWER: &str = include_str!("fixtures/openai/text-answer.sse");
const TOOL_USE: &str = include_str!("fixtures/openai/tool-use.sse");
const KEY: &str = "sk-proj-test-secret-0123456789-abcdefgh-a1b2";

fn openai(endpoint: &FakeEndpoint) -> OpenAiCompatible {
    OpenAiCompatible::openai(ApiKey::new(KEY)).with_base_url(&endpoint.url)
}

fn openrouter(endpoint: &FakeEndpoint) -> OpenAiCompatible {
    OpenAiCompatible::openrouter(ApiKey::new(KEY)).with_base_url(&endpoint.url)
}

fn llama_server(endpoint: &FakeEndpoint) -> OpenAiCompatible {
    OpenAiCompatible::llama_server(&endpoint.url)
}

fn question() -> LlmRequest {
    LlmRequest::new(
        "gpt-6-luna",
        200,
        vec![Message::user("Two orc warriors and I'm at 9 HP. What now?")],
    )
}

/// A Coach-style call: a cached static prefix, an uncached moment tail, an engine tool and a
/// structured answer.
fn coach_call(thinking: Thinking) -> LlmRequest {
    LlmRequest {
        system: vec![SystemBlock::cached(
            "You are the Advisor. Choose only among the Candidate Actions.",
            CacheHint::OneHour,
        )],
        messages: vec![Message {
            role: Role::User,
            content: vec![
                Part::cached_text("Build: Minotaur Fighter, XL 9.", CacheHint::FiveMinutes),
                Part::text("Threat Level: DANGEROUS. Candidate Actions: fight, upstairs."),
            ],
        }],
        tools: vec![ToolDef {
            name: "lookup".to_owned(),
            description: "Game Knowledge facts about a monster, item or spell.".to_owned(),
            input_schema: json!({
                "type": "object",
                "properties": { "name": { "type": "string" } },
                "required": ["name"],
                "additionalProperties": false
            }),
        }],
        output: Some(OutputSchema(json!({
            "type": "object",
            "properties": { "pick": { "type": "string" }, "reason": { "type": "string" } },
            "required": ["pick", "reason"],
            "additionalProperties": false
        }))),
        thinking,
        ..LlmRequest::new("gpt-6-luna", 800, Vec::new())
    }
}

#[tokio::test]
async fn a_text_answer_streams_as_it_arrives_and_ends_with_its_usage() {
    let endpoint = FakeEndpoint::start(Canned::events(TEXT_ANSWER)).await;

    let events: Vec<StreamEvent> = openai(&endpoint)
        .stream(question())
        .map(Result::unwrap)
        .collect()
        .await;

    let deltas: Vec<&str> = events
        .iter()
        .filter_map(|event| match event {
            StreamEvent::Text(text) => Some(text.as_str()),
            StreamEvent::Finished(_) => None,
        })
        .collect();
    assert_eq!(deltas, ["Read the scroll", " of teleportation."]);
    let Some(StreamEvent::Finished(response)) = events.last() else {
        panic!("the stream ended without its response: {events:?}");
    };
    assert_eq!(
        response.content,
        [Part::text("Read the scroll of teleportation.")]
    );
    assert_eq!(response.stop, StopReason::EndTurn);
    assert_eq!(response.model, "gpt-6-luna");
    // Cached tokens are counted inside `prompt_tokens`, but billed at their own rate.
    assert_eq!(
        response.usage,
        Usage {
            input_tokens: 18,
            output_tokens: 9,
            cache_read_tokens: 2048,
            cache_write_tokens: 0,
        }
    );
}

#[tokio::test]
async fn openai_is_called_with_the_key_and_asked_to_report_usage_at_the_end_of_the_stream() {
    let endpoint = FakeEndpoint::start(Canned::events(TEXT_ANSWER)).await;

    complete(&openai(&endpoint), question()).await.unwrap();

    let received = endpoint.received();
    let [request] = received.as_slice() else {
        panic!("expected one request, got {received:?}");
    };
    assert_eq!(request.method, "POST");
    assert_eq!(request.path, "/chat/completions");
    assert_eq!(request.headers["authorization"], format!("Bearer {KEY}"));
    assert_eq!(request.body["stream"], true);
    assert_eq!(
        request.body["stream_options"],
        json!({ "include_usage": true })
    );
}

#[tokio::test]
async fn the_system_prompt_the_tools_the_schema_and_thinking_reach_openai_in_its_shape() {
    let endpoint = FakeEndpoint::start(Canned::events(TEXT_ANSWER)).await;

    complete(
        &openai(&endpoint),
        coach_call(Thinking::Adaptive(Effort::Medium)),
    )
    .await
    .unwrap();

    insta::assert_json_snapshot!(endpoint.received()[0].body);
}

#[tokio::test]
async fn thinking_off_is_asked_for_explicitly_because_openais_models_may_think_by_default() {
    let endpoint = FakeEndpoint::start(Canned::events(TEXT_ANSWER)).await;

    complete(&openai(&endpoint), coach_call(Thinking::Off))
        .await
        .unwrap();

    assert_eq!(endpoint.received()[0].body["reasoning_effort"], "none");
}

#[tokio::test]
async fn openrouter_only_routes_to_a_provider_honouring_the_request_and_nests_its_effort() {
    let endpoint = FakeEndpoint::start(Canned::events(TEXT_ANSWER)).await;

    complete(
        &openrouter(&endpoint),
        coach_call(Thinking::Adaptive(Effort::High)),
    )
    .await
    .unwrap();

    let body = &endpoint.received()[0].body;
    assert_eq!(body["provider"], json!({ "require_parameters": true }));
    assert_eq!(body["reasoning"], json!({ "effort": "high" }));
    assert_eq!(body["max_tokens"], 800);
    assert_eq!(body.get("reasoning_effort"), None);
    assert_eq!(body.get("max_completion_tokens"), None);
}

#[tokio::test]
async fn openrouter_with_thinking_off_doesnt_exclude_models_that_cant_reason() {
    let endpoint = FakeEndpoint::start(Canned::events(TEXT_ANSWER)).await;

    complete(&openrouter(&endpoint), coach_call(Thinking::Off))
        .await
        .unwrap();

    assert_eq!(endpoint.received()[0].body.get("reasoning"), None);
}

#[tokio::test]
async fn llama_server_is_called_on_its_own_address_with_no_key_and_thinking_off() {
    let endpoint = FakeEndpoint::start(Canned::events(TEXT_ANSWER)).await;

    complete(&llama_server(&endpoint), coach_call(Thinking::Off))
        .await
        .unwrap();

    let request = &endpoint.received()[0];
    assert_eq!(request.path, "/v1/chat/completions");
    assert!(!request.headers.contains_key("authorization"));
    assert_eq!(request.body["max_tokens"], 800);
    assert_eq!(request.body["reasoning_effort"], "none");
}

#[tokio::test]
async fn tool_calls_arrive_whole_however_their_arguments_were_split() {
    let endpoint = FakeEndpoint::start(Canned::events(TOOL_USE)).await;

    let response = complete(&openrouter(&endpoint), coach_call(Thinking::Off))
        .await
        .unwrap();

    assert_eq!(response.stop, StopReason::ToolUse);
    assert_eq!(
        response.content,
        [
            Part::text("Let me look him up."),
            Part::ToolUse {
                id: "call_01".to_owned(),
                name: "lookup".to_owned(),
                input: json!({ "name": "Sigmund" }),
            },
            Part::ToolUse {
                id: "call_02".to_owned(),
                name: "recall".to_owned(),
                input: json!({}),
            },
        ]
    );
    assert_eq!(
        response.usage,
        Usage {
            input_tokens: 70,
            output_tokens: 30,
            cache_read_tokens: 0,
            cache_write_tokens: 1730,
        }
    );
}

#[tokio::test]
async fn a_server_that_says_it_stopped_when_it_called_a_tool_still_called_a_tool() {
    let stopped = TOOL_USE.replace(
        r#""finish_reason":"tool_calls""#,
        r#""finish_reason":"stop""#,
    );
    let endpoint = FakeEndpoint::start(Canned::events(&stopped)).await;

    let response = complete(&llama_server(&endpoint), coach_call(Thinking::Off))
        .await
        .unwrap();

    assert_eq!(response.stop, StopReason::ToolUse);
}

#[tokio::test]
async fn a_tool_result_follows_the_models_turn_as_a_message_of_its_own() {
    let endpoint = FakeEndpoint::start(Canned::events(TEXT_ANSWER)).await;
    let mut call = coach_call(Thinking::Off);
    call.messages = vec![
        Message::user("Who is Sigmund?"),
        Message {
            role: Role::Assistant,
            content: vec![
                Part::Thinking {
                    thinking: "Only Anthropic can read this back.".to_owned(),
                    signature: "EqQBCgIYAhIM".to_owned(),
                },
                Part::text("Let me look him up."),
                Part::ToolUse {
                    id: "call_01".to_owned(),
                    name: "lookup".to_owned(),
                    input: json!({ "name": "Sigmund" }),
                },
            ],
        },
        Message {
            role: Role::User,
            content: vec![
                Part::ToolResult {
                    tool_use_id: "call_01".to_owned(),
                    content: "Sigmund: unique human, casts Confuse and Haste.".to_owned(),
                    is_error: false,
                },
                Part::text("Keep it short."),
            ],
        },
    ];

    complete(&openai(&endpoint), call).await.unwrap();

    assert_eq!(
        endpoint.received()[0].body["messages"],
        json!([
            {
                "role": "system",
                "content": "You are the Advisor. Choose only among the Candidate Actions."
            },
            { "role": "user", "content": "Who is Sigmund?" },
            {
                "role": "assistant",
                "content": "Let me look him up.",
                "tool_calls": [{
                    "id": "call_01",
                    "type": "function",
                    "function": { "name": "lookup", "arguments": "{\"name\":\"Sigmund\"}" }
                }]
            },
            {
                "role": "tool",
                "tool_call_id": "call_01",
                "content": "Sigmund: unique human, casts Confuse and Haste."
            },
            { "role": "user", "content": "Keep it short." }
        ])
    );
}

#[tokio::test]
async fn a_structured_answer_reads_as_the_json_its_schema_asked_for() {
    let structured = TEXT_ANSWER
        .replace("Read the scroll", r#"{\"pick\": \"upstairs\", "#)
        .replace(" of teleportation.", r#"\"reason\": \"Two orcs, 9 HP.\"}"#);
    let endpoint = FakeEndpoint::start(Canned::events(&structured)).await;

    let response = complete(&openai(&endpoint), coach_call(Thinking::Off))
        .await
        .unwrap();

    assert_eq!(
        response.output_json(),
        Ok(json!({ "pick": "upstairs", "reason": "Two orcs, 9 HP." }))
    );
}

#[tokio::test]
async fn an_answer_cut_short_or_refused_says_so() {
    for (from, to, expected) in [
        (
            r#""finish_reason":"stop""#,
            r#""finish_reason":"length""#,
            StopReason::MaxTokens,
        ),
        (
            r#""finish_reason":"stop""#,
            r#""finish_reason":"content_filter""#,
            StopReason::Refusal,
        ),
        (
            r#""delta":{"content":" of teleportation."}"#,
            r#""delta":{"refusal":"I can't help with that."}"#,
            StopReason::Refusal,
        ),
    ] {
        let endpoint = FakeEndpoint::start(Canned::events(&TEXT_ANSWER.replace(from, to))).await;

        let response = complete(&openai(&endpoint), question()).await.unwrap();

        assert_eq!(response.stop, expected, "{to}");
    }
}

#[tokio::test]
async fn an_answer_without_a_usage_report_fails_rather_than_cost_nothing() {
    let unreported: String = TEXT_ANSWER
        .split_inclusive("\n\n")
        .filter(|event| !event.contains("prompt_tokens"))
        .collect();
    let endpoint = FakeEndpoint::start(Canned::events(&unreported)).await;

    let answer = complete(&openai(&endpoint), question()).await;

    assert_eq!(answer, Err(LlmError::NoUsage));
}

#[tokio::test]
async fn a_stream_that_ends_without_finishing_is_incomplete() {
    let endpoint =
        FakeEndpoint::start(Canned::events(&TEXT_ANSWER.replace("data: [DONE]\n\n", ""))).await;

    let answer = complete(&openai(&endpoint), question()).await;

    assert_eq!(answer, Err(LlmError::Incomplete));
}

#[tokio::test]
async fn a_rejected_key_is_reported_as_such_without_repeating_the_key() {
    let endpoint = FakeEndpoint::start(Canned::error(
        401,
        r#"{"error":{"message":"Incorrect API key provided.","type":"invalid_request_error","code":"invalid_api_key"}}"#,
    ))
    .await;

    let error = complete(&openai(&endpoint), question()).await.unwrap_err();

    assert_eq!(
        error,
        LlmError::KeyRejected {
            status: 401,
            message: "Incorrect API key provided.".to_owned()
        }
    );
    assert!(!format!("{error} {error:?} {:?}", openai(&endpoint)).contains(KEY));
}

#[tokio::test]
async fn an_error_mid_stream_ends_the_call_with_that_error() {
    let limited = TEXT_ANSWER.replace(
        r#"data: {"id":"chatcmpl-1","object":"chat.completion.chunk","model":"gpt-6-luna","choices":[{"index":0,"delta":{},"finish_reason":"stop"}],"usage":null}"#,
        r#"data: {"id":"gen-1","error":{"code":429,"message":"Rate limit exceeded"},"choices":[{"index":0,"delta":{"content":""},"finish_reason":"error"}]}"#,
    );
    let endpoint = FakeEndpoint::start(Canned::events(&limited)).await;

    let error = complete(&openrouter(&endpoint), question())
        .await
        .unwrap_err();

    assert_eq!(
        error,
        LlmError::RateLimited {
            message: "Rate limit exceeded".to_owned()
        }
    );
}

#[tokio::test]
async fn an_error_mid_stream_with_no_status_counts_as_the_provider_being_unavailable() {
    let failed = TEXT_ANSWER.replace(
        r#"data: {"id":"chatcmpl-1","object":"chat.completion.chunk","model":"gpt-6-luna","choices":[{"index":0,"delta":{},"finish_reason":"stop"}],"usage":null}"#,
        r#"data: {"error":{"message":"The server had an error","type":"server_error","code":null}}"#,
    );
    let endpoint = FakeEndpoint::start(Canned::events(&failed)).await;

    let error = complete(&openai(&endpoint), question()).await.unwrap_err();

    assert_eq!(
        error,
        LlmError::Unavailable {
            status: 500,
            message: "The server had an error".to_owned()
        }
    );
}

#[tokio::test]
async fn llama_server_says_which_model_it_serves() {
    let endpoint = FakeEndpoint::start(Canned::json(
        r#"{"object":"list","data":[{"id":"qwen3-8b-q4.gguf","object":"model"}]}"#,
    ))
    .await;

    let models = llama_server(&endpoint).models().await.unwrap();

    assert_eq!(models, ["qwen3-8b-q4.gguf"]);
    assert_eq!(endpoint.received()[0].path, "/v1/models");
}
