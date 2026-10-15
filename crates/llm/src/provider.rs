//! The seam between the Advisor and any model: one trait every provider adapter implements.

use futures_util::StreamExt;
use futures_util::stream::BoxStream;
use serde::{Deserialize, Serialize};

use crate::request::LlmRequest;
use crate::response::{LlmResponse, StreamEvent};

/// A streamed answer: text as it arrives, then the whole answer.
pub type LlmStream = BoxStream<'static, Result<StreamEvent, LlmError>>;

/// Something that can answer an [`LlmRequest`]: a provider adapter, or the LLM sidecar that
/// records and replays one.
pub trait LlmProvider: Send + Sync {
    /// Starts one call. The stream yields the answer's text as it arrives and ends with
    /// [`StreamEvent::Finished`], or with an error.
    fn stream(&self, request: LlmRequest) -> LlmStream;
}

/// A boxed provider is a provider, so wrappers such as the sidecar recorder can own any.
impl<P: LlmProvider + ?Sized> LlmProvider for Box<P> {
    fn stream(&self, request: LlmRequest) -> LlmStream {
        (**self).stream(request)
    }
}

/// Makes one call and waits for the whole answer.
///
/// # Errors
/// If the call fails, or the stream ends without an answer.
pub async fn complete(
    provider: &dyn LlmProvider,
    request: LlmRequest,
) -> Result<LlmResponse, LlmError> {
    let mut stream = provider.stream(request);
    while let Some(event) = stream.next().await {
        if let StreamEvent::Finished(response) = event? {
            return Ok(response);
        }
    }
    Err(LlmError::Incomplete)
}

/// Why a call to a model failed. Messages are for the player, and never hold a key.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize, Deserialize)]
#[serde(tag = "error", rename_all = "snake_case")]
pub enum LlmError {
    /// The provider refused the key.
    #[error("the provider rejected the key ({status}): {message}")]
    KeyRejected {
        /// The HTTP status.
        status: u16,
        /// What the provider said.
        message: String,
    },
    /// The provider is limiting how fast calls can be made.
    #[error("the provider is rate limiting calls: {message}")]
    RateLimited {
        /// What the provider said.
        message: String,
    },
    /// The provider couldn't answer just now.
    #[error("the provider is unavailable ({status}): {message}")]
    Unavailable {
        /// The HTTP status.
        status: u16,
        /// What the provider said.
        message: String,
    },
    /// The provider refused the request itself.
    #[error("the provider refused the request ({status}): {message}")]
    BadRequest {
        /// The HTTP status.
        status: u16,
        /// What the provider said.
        message: String,
    },
    /// The provider couldn't be reached, or the connection broke.
    #[error("the provider couldn't be reached: {0}")]
    Unreachable(String),
    /// The provider answered with something this adapter can't read.
    #[error("the provider's answer couldn't be read: {0}")]
    Malformed(String),
    /// The provider didn't say what the call cost, so it can't be counted against the Game Budget.
    #[error("the provider didn't report the call's usage")]
    NoUsage,
    /// The stream ended before the answer was complete.
    #[error("the answer ended before it was complete")]
    Incomplete,
    /// A replay asked for a call its LLM sidecar never recorded, such as after a prompt changed.
    #[error("the LLM sidecar has no answer for request {hash}; record it again")]
    NotRecorded {
        /// The request's hash.
        hash: String,
    },
    /// A dev build couldn't record the call in its LLM sidecar.
    #[error("the call couldn't be recorded in the LLM sidecar: {0}")]
    Sidecar(String),
}
