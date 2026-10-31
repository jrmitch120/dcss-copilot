//! The native Ollama adapter, against a fake endpoint that answers with JSON Lines written from
//! Ollama's documentation. Asserted: what the adapter sends, and what the caller gets back.

// Helpers outside #[test] functions panic on a broken setup too.
#![allow(clippy::unwrap_used)]

mod support;

use copilot_llm::{
    LlmError, LlmProvider, LlmRequest, Message, OllamaProvider, OutputSchema, Part,
    ProviderConnection, Role, ServerAddress, StopReason, StreamEvent, SystemBlock, ToolDef, Usage,
    complete,
};
use futures_util::StreamExt;
use serde_json::json;
use support::{Canned, FakeEndpoint};

const TEXT_ANSWER: &str = include_str!("fixtures/ollama/text-answer.ndjson");
const TOOL_USE: &str = include_str!("fixtures/ollama/tool-use.ndjson");

fn ollama(endpoint: &FakeEndpoint) -> OllamaProvider {
    OllamaProvider::new(&endpoint.url)
}

fn question() -> LlmRequest {
    LlmRequest::new(
        "qwen3:8b",
        200,
        vec![Message::user("Two orc warriors and I'm at 9 HP. What now?")],
    )
}

#[tokio::test]
async fn a_text_answer_streams_as_it_arrives_and_ends_with_its_usage() {
    let endpoint = FakeEndpoint::start(Canned::lines(TEXT_ANSWER)).await;

    let events: Vec<StreamEvent> = ollama(&endpoint)
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
    assert_eq!(response.model, "qwen3:8b");
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
async fn every_call_sets_a_context_large_enough_that_prompts_arent_silently_cut_off() {
    let endpoint = FakeEndpoint::start(Canned::lines(TEXT_ANSWER)).await;

    complete(&ollama(&endpoint), question()).await.unwrap();

    let request = &endpoint.received()[0];
    assert_eq!(request.path, "/api/chat");
    assert!(!request.headers.contains_key("authorization"));
    assert_eq!(
        request.body["options"],
        json!({ "num_ctx": 16384, "num_predict": 200 })
    );
    assert_eq!(request.body["think"], false);
}

#[tokio::test]
async fn the_system_prompt_the_tools_and_the_schema_reach_ollama_in_its_shape() {
    let endpoint = FakeEndpoint::start(Canned::lines(TEXT_ANSWER)).await;
    let schema = json!({
        "type": "object",
        "properties": { "pick": { "type": "string" } },
        "required": ["pick"],
        "additionalProperties": false
    });
    let call = LlmRequest {
        system: vec![SystemBlock {
            text: "You are the Advisor.".to_owned(),
            cache: None,
        }],
        tools: vec![ToolDef {
            name: "lookup".to_owned(),
            description: "Game Knowledge facts.".to_owned(),
            input_schema: json!({ "type": "object" }),
        }],
        output: Some(OutputSchema(schema.clone())),
        ..question()
    };

    complete(&ollama(&endpoint), call).await.unwrap();

    let body = &endpoint.received()[0].body;
    assert_eq!(body["format"], schema);
    assert_eq!(
        body["messages"][0],
        json!({ "role": "system", "content": "You are the Advisor." })
    );
    assert_eq!(
        body["tools"],
        json!([{
            "type": "function",
            "function": {
                "name": "lookup",
                "description": "Game Knowledge facts.",
                "parameters": { "type": "object" }
            }
        }])
    );
}

#[tokio::test]
async fn a_tool_call_gets_an_id_of_its_own_because_ollama_may_not_give_one() {
    let endpoint = FakeEndpoint::start(Canned::lines(TOOL_USE)).await;

    let response = complete(&ollama(&endpoint), question()).await.unwrap();

    assert_eq!(response.stop, StopReason::ToolUse);
    assert_eq!(
        response.content,
        [Part::ToolUse {
            id: "call_0".to_owned(),
            name: "lookup".to_owned(),
            input: json!({ "name": "Sigmund" }),
        }]
    );
    // A prompt with nothing cached reports no cached count.
    assert_eq!(response.usage.input_tokens, 1800);
    assert_eq!(response.usage.cache_read_tokens, 0);
}

#[tokio::test]
async fn a_tool_result_goes_back_naming_the_tool_it_answers() {
    let endpoint = FakeEndpoint::start(Canned::lines(TEXT_ANSWER)).await;
    let call = LlmRequest {
        messages: vec![
            Message::user("Who is Sigmund?"),
            Message {
                role: Role::Assistant,
                content: vec![Part::ToolUse {
                    id: "call_0".to_owned(),
                    name: "lookup".to_owned(),
                    input: json!({ "name": "Sigmund" }),
                }],
            },
            Message {
                role: Role::User,
                content: vec![Part::ToolResult {
                    tool_use_id: "call_0".to_owned(),
                    content: "Sigmund: unique human.".to_owned(),
                    is_error: false,
                }],
            },
        ],
        ..question()
    };

    complete(&ollama(&endpoint), call).await.unwrap();

    assert_eq!(
        endpoint.received()[0].body["messages"],
        json!([
            { "role": "user", "content": "Who is Sigmund?" },
            {
                "role": "assistant",
                "content": "",
                "tool_calls": [{
                    "id": "call_0",
                    "function": { "name": "lookup", "arguments": { "name": "Sigmund" } }
                }]
            },
            {
                "role": "tool",
                "tool_name": "lookup",
                "tool_call_id": "call_0",
                "content": "Sigmund: unique human."
            }
        ])
    );
}

#[tokio::test]
async fn an_answer_cut_short_says_so() {
    let endpoint = FakeEndpoint::start(Canned::lines(
        &TEXT_ANSWER.replace(r#""done_reason":"stop""#, r#""done_reason":"length""#),
    ))
    .await;

    let response = complete(&ollama(&endpoint), question()).await.unwrap();

    assert_eq!(response.stop, StopReason::MaxTokens);
}

#[tokio::test]
async fn an_answer_without_a_count_of_what_it_generated_fails_rather_than_cost_nothing() {
    let endpoint = FakeEndpoint::start(Canned::lines(
        &TEXT_ANSWER.replace(r#","eval_count":9"#, ""),
    ))
    .await;

    let answer = complete(&ollama(&endpoint), question()).await;

    assert_eq!(answer, Err(LlmError::NoUsage));
}

#[tokio::test]
async fn a_model_ollama_doesnt_have_is_told_apart_and_says_why() {
    let endpoint = FakeEndpoint::start(Canned::error(
        404,
        r#"{"error":"model 'qwen3:8b' not found"}"#,
    ))
    .await;

    let error = complete(&ollama(&endpoint), question()).await.unwrap_err();

    assert_eq!(
        error,
        LlmError::BadRequest {
            status: 404,
            message: "model 'qwen3:8b' not found".to_owned()
        }
    );
}

#[tokio::test]
async fn an_error_mid_stream_ends_the_call_with_that_error() {
    let failed = format!(
        "{}{{\"error\":\"the model runner stopped\"}}\n",
        TEXT_ANSWER.lines().next().unwrap().to_owned() + "\n"
    );
    let endpoint = FakeEndpoint::start(Canned::lines(&failed)).await;

    let error = complete(&ollama(&endpoint), question()).await.unwrap_err();

    assert_eq!(
        error,
        LlmError::Unavailable {
            status: 500,
            message: "the model runner stopped".to_owned()
        }
    );
}

#[tokio::test]
async fn a_running_ollama_says_which_models_it_has() {
    let endpoint = FakeEndpoint::start(Canned::json(
        r#"{"models":[{"name":"qwen3:8b","model":"qwen3:8b","size":1},{"name":"llama3.2:3b","model":"llama3.2:3b","size":2}]}"#,
    ))
    .await;

    let models = ollama(&endpoint).models().await.unwrap();

    assert_eq!(models, ["qwen3:8b", "llama3.2:3b"]);
    let request = &endpoint.received()[0];
    assert_eq!(
        (request.method.as_str(), request.path.as_str()),
        ("GET", "/api/tags")
    );
}

#[tokio::test]
async fn an_ollama_that_isnt_running_is_told_apart_from_one_with_no_models() {
    let endpoint = FakeEndpoint::start(Canned::json(r#"{"models":[]}"#)).await;
    assert_eq!(ollama(&endpoint).models().await, Ok(Vec::new()));

    // Nothing listens on port 9, the discard port.
    let nothing = OllamaProvider::new("http://127.0.0.1:9").models().await;
    assert!(
        matches!(nothing, Err(LlmError::Unreachable(_))),
        "{nothing:?}"
    );
}

#[tokio::test]
async fn a_connection_reaches_the_server_at_its_address_without_a_key() {
    let endpoint = FakeEndpoint::start(Canned::lines(TEXT_ANSWER)).await;
    let connection = ProviderConnection::Ollama {
        address: ServerAddress::new(&endpoint.url).unwrap(),
    };
    assert_eq!(connection.key(), None);

    complete(connection.open().as_ref(), question())
        .await
        .unwrap();

    assert_eq!(endpoint.received()[0].path, "/api/chat");
}
