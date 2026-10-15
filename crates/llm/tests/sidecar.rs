//! The LLM sidecar: calls recorded in dev replay in tests and CI without a live model.

// Helpers outside #[test] functions panic on a broken setup too.
#![allow(clippy::unwrap_used)]

mod support;

use copilot_llm::{
    AnthropicProvider, ApiKey, LlmError, LlmProvider, LlmRequest, Message, SidecarError,
    SidecarRecorder, SidecarReplayer, StreamEvent, complete, request_hash,
};
use futures_util::StreamExt;
use serde_json::json;
use support::{Canned, FakeEndpoint};

const TEXT_ANSWER: &str = include_str!("fixtures/anthropic/text-answer.sse");
const KEY: &str = "sk-ant-api03-test-secret-0123456789-a1b2";
const TACTICAL: &[u8] = include_bytes!("sidecars/tactical-two-orcs.llm.jsonl");

fn question(text: &str) -> LlmRequest {
    LlmRequest::new("claude-haiku-5-5", 200, vec![Message::user(text)])
}

async fn events(
    provider: &dyn LlmProvider,
    request: LlmRequest,
) -> Vec<Result<StreamEvent, LlmError>> {
    provider.stream(request).collect().await
}

/// Records `requests` against a fake Anthropic endpoint answering `canned`, into a sidecar
/// in `dir`, and returns what each call streamed.
async fn record(
    dir: &std::path::Path,
    canned: Canned,
    requests: &[LlmRequest],
) -> (std::path::PathBuf, Vec<Vec<Result<StreamEvent, LlmError>>>) {
    let endpoint = FakeEndpoint::start(canned).await;
    let live = AnthropicProvider::new(ApiKey::new(KEY)).with_base_url(&endpoint.url);
    let path = dir.join("game.llm.jsonl");
    let recorder = SidecarRecorder::create(live, &path).unwrap();
    let mut streamed = Vec::new();
    for request in requests {
        streamed.push(events(&recorder, request.clone()).await);
    }
    (path, streamed)
}

#[tokio::test]
async fn a_recorded_call_replays_exactly_without_a_provider() {
    let dir = tempfile::tempdir().unwrap();
    let request = question("Two orc warriors and I'm at 9 HP. What now?");
    let (path, streamed) = record(
        dir.path(),
        Canned::events(TEXT_ANSWER),
        std::slice::from_ref(&request),
    )
    .await;

    let replayer = SidecarReplayer::open(&path).unwrap();

    assert_eq!(events(&replayer, request).await, streamed[0]);
}

#[tokio::test]
async fn a_failed_call_replays_as_the_same_failure() {
    let dir = tempfile::tempdir().unwrap();
    let request = question("Two orc warriors and I'm at 9 HP. What now?");
    let (path, streamed) = record(
        dir.path(),
        Canned::error(
            529,
            r#"{"type":"error","error":{"type":"overloaded_error","message":"Overloaded"}}"#,
        ),
        std::slice::from_ref(&request),
    )
    .await;

    let replayer = SidecarReplayer::open(&path).unwrap();

    let replayed = events(&replayer, request).await;
    assert_eq!(replayed, streamed[0]);
    assert!(matches!(
        replayed[..],
        [Err(LlmError::Unavailable { status: 529, .. })]
    ));
}

#[tokio::test]
async fn a_call_that_was_never_recorded_fails_naming_its_request_hash() {
    let dir = tempfile::tempdir().unwrap();
    let (path, _) = record(
        dir.path(),
        Canned::events(TEXT_ANSWER),
        &[question("Two orc warriors and I'm at 9 HP. What now?")],
    )
    .await;
    let replayer = SidecarReplayer::open(&path).unwrap();
    let changed = question("Two orc warriors and I'm at 8 HP. What now?");

    let answer = complete(&replayer, changed.clone()).await;

    assert_eq!(
        answer,
        Err(LlmError::NotRecorded {
            hash: request_hash(&changed).to_string()
        })
    );
}

#[tokio::test]
async fn the_sidecar_never_holds_the_key() {
    let dir = tempfile::tempdir().unwrap();
    let (path, _) = record(
        dir.path(),
        Canned::events(TEXT_ANSWER),
        &[question("Two orc warriors and I'm at 9 HP. What now?")],
    )
    .await;

    let written = std::fs::read_to_string(path).unwrap();

    assert!(!written.contains(KEY));
    assert!(!written.contains("sk-ant-"));
}

#[tokio::test]
async fn calls_are_appended_one_line_each_so_a_crash_keeps_those_before_it() {
    let dir = tempfile::tempdir().unwrap();
    let (path, _) = record(
        dir.path(),
        Canned::events(TEXT_ANSWER),
        &[question("first"), question("second")],
    )
    .await;

    let written = std::fs::read_to_string(path).unwrap();

    let hashes: Vec<String> = written
        .lines()
        .map(|line| {
            serde_json::from_str::<serde_json::Value>(line).unwrap()["hash"]
                .as_str()
                .unwrap()
                .to_owned()
        })
        .collect();
    assert_eq!(
        hashes,
        [
            request_hash(&question("first")).to_string(),
            request_hash(&question("second")).to_string()
        ]
    );
}

#[test]
fn a_line_whose_request_no_longer_matches_its_hash_is_refused() {
    let edited = String::from_utf8(TACTICAL.to_vec())
        .unwrap()
        .replace("Pick one.", "Pick one, quickly.");

    let read = SidecarReplayer::read(edited.as_bytes());

    assert!(
        matches!(read, Err(SidecarError::HashMismatch { line: 1 })),
        "{:?}",
        read.err()
    );
}

#[tokio::test]
async fn a_committed_sidecar_replays_a_structured_tactical_answer() {
    let replayer = SidecarReplayer::read(TACTICAL).unwrap();
    let request = LlmRequest {
        output: Some(copilot_llm::OutputSchema(json!({
            "type": "object",
            "properties": { "pick": { "type": "string" }, "reason": { "type": "string" } },
            "required": ["pick", "reason"],
            "additionalProperties": false
        }))),
        ..question("Threat Level: DANGEROUS. Candidate Actions: fight, upstairs. Pick one.")
    };

    let response = complete(&replayer, request).await.unwrap();

    assert_eq!(
        response.output_json(),
        Ok(
            json!({ "pick": "upstairs", "reason": "Two orcs can act before you; the stairs are 2 steps away." })
        )
    );
    assert_eq!(response.usage.output_tokens, 24);
}
