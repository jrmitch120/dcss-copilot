//! The native Anthropic Messages adapter, against a fake endpoint that answers with recorded
//! Server-Sent Events. Asserted: what the adapter sends, and what the caller gets back.

// Helpers outside #[test] functions panic on a broken setup too.
#![allow(clippy::unwrap_used)]

mod support;

use copilot_llm::{
    AnthropicProvider, ApiKey, CacheHint, Effort, LlmError, LlmProvider, LlmRequest, Message,
    OutputSchema, Part, Role, StopReason, StreamEvent, SystemBlock, Thinking, ToolDef, Usage,
    complete,
};
use futures_util::StreamExt;
use serde_json::json;
use support::{Canned, FakeEndpoint};

const TEXT_ANSWER: &str = include_str!("fixtures/anthropic/text-answer.sse");
const TOOL_USE: &str = include_str!("fixtures/anthropic/tool-use.sse");
const KEY: &str = "sk-ant-api03-test-secret-0123456789-a1b2";

fn provider(endpoint: &FakeEndpoint) -> AnthropicProvider {
    AnthropicProvider::new(ApiKey::new(KEY)).with_base_url(&endpoint.url)
}

fn question() -> LlmRequest {
    LlmRequest::new(
        "claude-haiku-5-5",
        200,
        vec![Message::user("Two orc warriors and I'm at 9 HP. What now?")],
    )
}

#[tokio::test]
async fn a_text_answer_streams_as_it_arrives_and_ends_with_its_usage() {
    let endpoint = FakeEndpoint::start(Canned::events(TEXT_ANSWER)).await;

    let events: Vec<StreamEvent> = provider(&endpoint)
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
    assert_eq!(response.model, "claude-haiku-5-5");
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
async fn the_request_carries_the_key_the_api_version_and_asks_for_a_stream() {
    let endpoint = FakeEndpoint::start(Canned::events(TEXT_ANSWER)).await;

    complete(&provider(&endpoint), question()).await.unwrap();

    let received = endpoint.received();
    let [request] = received.as_slice() else {
        panic!("expected one request, got {received:?}");
    };
    assert_eq!(request.method, "POST");
    assert_eq!(request.path, "/v1/messages");
    assert_eq!(request.headers["x-api-key"], KEY);
    assert_eq!(request.headers["anthropic-version"], "2023-06-01");
    assert_eq!(request.body["stream"], true);
}

/// A Coach-style call: a cached static prefix, a cached game block, an uncached moment tail,
/// an engine tool and a structured answer.
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
        ..LlmRequest::new("claude-sonnet-5-5", 800, Vec::new())
    }
}

#[tokio::test]
async fn cache_hints_tools_the_output_schema_and_thinking_reach_the_wire() {
    let endpoint = FakeEndpoint::start(Canned::events(TEXT_ANSWER)).await;

    complete(
        &provider(&endpoint),
        coach_call(Thinking::Adaptive(Effort::Medium)),
    )
    .await
    .unwrap();

    insta::assert_json_snapshot!(endpoint.received()[0].body);
}

#[tokio::test]
async fn thinking_off_is_asked_for_explicitly() {
    let endpoint = FakeEndpoint::start(Canned::events(TEXT_ANSWER)).await;

    complete(&provider(&endpoint), coach_call(Thinking::Off))
        .await
        .unwrap();

    let body = &endpoint.received()[0].body;
    assert_eq!(body["thinking"], json!({ "type": "disabled" }));
    assert_eq!(body["output_config"].get("effort"), None);
}

#[tokio::test]
async fn a_tool_call_arrives_whole_with_the_thinking_to_send_back() {
    let endpoint = FakeEndpoint::start(Canned::events(TOOL_USE)).await;

    let response = complete(
        &provider(&endpoint),
        coach_call(Thinking::Adaptive(Effort::Medium)),
    )
    .await
    .unwrap();

    assert_eq!(response.stop, StopReason::ToolUse);
    assert_eq!(
        response.content,
        [
            Part::Thinking {
                thinking: "The player asks about Sigmund.".to_owned(),
                signature: "EqQBCgIYAhIM".to_owned(),
            },
            Part::text("Let me look him up."),
            Part::ToolUse {
                id: "toolu_01".to_owned(),
                name: "lookup".to_owned(),
                input: json!({ "name": "Sigmund" }),
            },
        ]
    );
    assert_eq!(response.usage.cache_write_tokens, 1730);
}

#[tokio::test]
async fn a_tool_result_goes_back_after_the_models_turn() {
    let endpoint = FakeEndpoint::start(Canned::events(TEXT_ANSWER)).await;
    let mut call = coach_call(Thinking::Adaptive(Effort::Medium));
    call.messages.push(Message {
        role: Role::Assistant,
        content: vec![
            Part::Thinking {
                thinking: String::new(),
                signature: "EqQBCgIYAhIM".to_owned(),
            },
            Part::ToolUse {
                id: "toolu_01".to_owned(),
                name: "lookup".to_owned(),
                input: json!({ "name": "Sigmund" }),
            },
        ],
    });
    call.messages.push(Message {
        role: Role::User,
        content: vec![Part::ToolResult {
            tool_use_id: "toolu_01".to_owned(),
            content: "Sigmund: unique human, casts Confuse and Haste.".to_owned(),
            is_error: false,
        }],
    });

    complete(&provider(&endpoint), call).await.unwrap();

    let messages = &endpoint.received()[0].body["messages"];
    assert_eq!(
        messages[1]["content"],
        json!([
            { "type": "thinking", "thinking": "", "signature": "EqQBCgIYAhIM" },
            { "type": "tool_use", "id": "toolu_01", "name": "lookup", "input": { "name": "Sigmund" } }
        ])
    );
    assert_eq!(
        messages[2]["content"],
        json!([{
            "type": "tool_result",
            "tool_use_id": "toolu_01",
            "content": "Sigmund: unique human, casts Confuse and Haste.",
            "is_error": false
        }])
    );
}

#[tokio::test]
async fn a_structured_answer_reads_as_the_json_its_schema_asked_for() {
    let structured = TEXT_ANSWER
        .replace("Read the scroll", r#"{\"pick\": \"upstairs\", "#)
        .replace(" of teleportation.", r#"\"reason\": \"Two orcs, 9 HP.\"}"#);
    let endpoint = FakeEndpoint::start(Canned::events(&structured)).await;

    let response = complete(&provider(&endpoint), coach_call(Thinking::Off))
        .await
        .unwrap();

    assert_eq!(
        response.output_json(),
        Ok(json!({ "pick": "upstairs", "reason": "Two orcs, 9 HP." }))
    );
}

#[tokio::test]
async fn an_answer_without_a_usage_report_fails_rather_than_cost_nothing() {
    let unreported = TEXT_ANSWER
        .replace(
            r#","usage":{"input_tokens":18,"cache_creation_input_tokens":0,"cache_read_input_tokens":2048,"output_tokens":1}"#,
            "",
        )
        .replace(r#","usage":{"output_tokens":9}"#, "");
    let endpoint = FakeEndpoint::start(Canned::events(&unreported)).await;

    let answer = complete(&provider(&endpoint), question()).await;

    assert_eq!(answer, Err(LlmError::NoUsage));
}

#[tokio::test]
async fn a_rejected_key_is_reported_as_such_without_repeating_the_key() {
    let endpoint = FakeEndpoint::start(Canned::error(
        401,
        r#"{"type":"error","error":{"type":"authentication_error","message":"invalid x-api-key"}}"#,
    ))
    .await;

    let error = complete(&provider(&endpoint), question())
        .await
        .unwrap_err();

    assert_eq!(
        error,
        LlmError::KeyRejected {
            status: 401,
            message: "invalid x-api-key".to_owned()
        }
    );
    assert!(!format!("{error} {error:?}").contains(KEY));
}

#[tokio::test]
async fn overload_and_rate_limits_are_told_apart_from_bad_requests() {
    let message = || "slow down".to_owned();
    let cases = [
        (429, LlmError::RateLimited { message: message() }),
        (
            529,
            LlmError::Unavailable {
                status: 529,
                message: message(),
            },
        ),
        (
            400,
            LlmError::BadRequest {
                status: 400,
                message: message(),
            },
        ),
    ];
    for (status, expected) in cases {
        let endpoint = FakeEndpoint::start(Canned::error(
            status,
            r#"{"type":"error","error":{"type":"some_error","message":"slow down"}}"#,
        ))
        .await;

        let error = complete(&provider(&endpoint), question())
            .await
            .unwrap_err();

        assert_eq!(error, expected);
    }
}

#[tokio::test]
async fn an_error_event_mid_stream_ends_the_call_with_that_error() {
    let overloaded = TEXT_ANSWER.replace(
        "event: message_delta",
        "event: error\ndata: {\"type\":\"error\",\"error\":{\"type\":\"overloaded_error\",\"message\":\"Overloaded\"}}\n\nevent: message_delta",
    );
    let endpoint = FakeEndpoint::start(Canned::events(&overloaded)).await;

    let error = complete(&provider(&endpoint), question())
        .await
        .unwrap_err();

    assert_eq!(
        error,
        LlmError::Unavailable {
            status: 529,
            message: "Overloaded".to_owned()
        }
    );
}
