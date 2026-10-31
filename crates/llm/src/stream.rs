//! What every provider adapter shares: sending a call, reading its body as frames, and telling
//! the errors an API answers with apart.

use std::collections::VecDeque;

use futures_util::StreamExt;
use futures_util::stream::{self, BoxStream};
use serde_json::Value;

use crate::provider::{LlmError, LlmStream};
use crate::response::StreamEvent;

/// How a provider frames its streamed body.
pub(crate) trait Frames: Default + Send + 'static {
    /// Takes in a chunk of the body and returns the data of every frame it completed.
    fn push(&mut self, chunk: &[u8]) -> Vec<String>;
}

/// One answer as its frames arrive, in a provider's own format.
pub(crate) trait Answer: Default + Send + 'static {
    /// How this provider frames its body.
    type Frames: Frames;

    /// Applies one frame's data and returns what the caller should see of it.
    ///
    /// # Errors
    /// If the frame can't be read, or says the call failed.
    fn apply(&mut self, data: &str) -> Result<Vec<StreamEvent>, LlmError>;
}

/// Sends `call` and streams its answer, read as `A`.
pub(crate) fn answer_stream<A: Answer>(call: reqwest::RequestBuilder) -> LlmStream {
    stream::unfold(Call::<A>::Start(Box::new(call)), Call::next).boxed()
}

/// One call, from sending the request to the end of its stream.
enum Call<A: Answer> {
    Start(Box<reqwest::RequestBuilder>),
    Reading {
        body: BoxStream<'static, Result<Vec<u8>, String>>,
        frames: A::Frames,
        answer: A,
        ready: VecDeque<StreamEvent>,
    },
    Done,
}

impl<A: Answer> Call<A> {
    /// The next item of the stream, and what's left of the call.
    async fn next(mut self) -> Option<(Result<StreamEvent, LlmError>, Self)> {
        loop {
            match self {
                Self::Done => return None,
                Self::Start(call) => match start(*call).await {
                    Ok(body) => {
                        self = Self::Reading {
                            body,
                            frames: A::Frames::default(),
                            answer: A::default(),
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
        return Err(refusal(status, response).await);
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

/// The JSON a server answers a `GET` with, such as its list of models.
///
/// # Errors
/// If the server can't be reached, refuses, or answers with something else.
pub(crate) async fn listing(call: reqwest::RequestBuilder) -> Result<Value, LlmError> {
    let response = call
        .send()
        .await
        .map_err(|error| LlmError::Unreachable(error.to_string()))?;
    let status = response.status().as_u16();
    if !response.status().is_success() {
        return Err(refusal(status, response).await);
    }
    let body = response
        .text()
        .await
        .map_err(|error| LlmError::Unreachable(error.to_string()))?;
    serde_json::from_str(&body).map_err(|error| LlmError::Malformed(error.to_string()))
}

/// The error an unsuccessful answer with this `status` stands for.
async fn refusal(status: u16, response: reqwest::Response) -> LlmError {
    let body = response.text().await.unwrap_or_default();
    let message = serde_json::from_str::<Value>(&body)
        .ok()
        .and_then(|error| said(&error["error"]))
        .unwrap_or(body);
    api_error(status, message)
}

/// What an API's `error` field says: most nest a message in it, and Ollama's is the message.
pub(crate) fn said(error: &Value) -> Option<String> {
    error["message"]
        .as_str()
        .or_else(|| error.as_str())
        .map(str::to_owned)
}

/// An error the API answered with, by its HTTP status.
pub(crate) fn api_error(status: u16, message: String) -> LlmError {
    match status {
        401 | 403 => LlmError::KeyRejected { status, message },
        429 => LlmError::RateLimited { message },
        500.. => LlmError::Unavailable { status, message },
        _ => LlmError::BadRequest { status, message },
    }
}

/// Server-Sent Events: the `data` of each event, however the body was chunked.
#[derive(Default)]
pub(crate) struct SseFrames {
    pending: Vec<u8>,
}

impl Frames for SseFrames {
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
