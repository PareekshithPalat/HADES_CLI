use futures::Stream;
use serde::{Deserialize, Serialize};
use std::pin::Pin;

use crate::error::ProviderError;
use crate::request::{FinishReason, ProviderToolCall, Usage};

/// Normalized asynchronous stream chunk emitted during incremental token generation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum StreamEvent {
    /// Token generation has commenced.
    Started,

    /// Text chunk / delta produced by the model.
    Delta(String),

    /// Streaming fragment of a structured tool call.
    ToolCallChunk {
        index: usize,
        id: Option<String>,
        name: Option<String>,
        arguments_chunk: String,
    },

    /// Fully accumulated list of structured tool calls ready for evaluation/execution.
    ToolCallsReady(Vec<ProviderToolCall>),

    /// Token consumption accounting update.
    Usage(Usage),

    /// Generation completed normally with a termination reason.
    Finished(FinishReason),

    /// An error occurred mid-stream.
    Error(String),
}

/// Type alias for a pinned, thread-safe asynchronous stream of `StreamEvent` results.
pub type StreamResult = Pin<Box<dyn Stream<Item = Result<StreamEvent, ProviderError>> + Send>>;
