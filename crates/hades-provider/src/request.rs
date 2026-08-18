use serde::{Deserialize, Serialize};

/// Role of a message participant in a chat completion prompt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    System,
    User,
    Assistant,
}

/// Normalized chat message structure.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: MessageRole,
    pub content: String,
}

impl ChatMessage {
    /// Creates a new user prompt message.
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::User,
            content: content.into(),
        }
    }

    /// Creates a new system instruction message.
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::System,
            content: content.into(),
        }
    }

    /// Creates a new assistant response message.
    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::Assistant,
            content: content.into(),
        }
    }
}

/// Normalized request sent to an AI model provider.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompletionRequest {
    /// Target model identifier.
    pub model: String,

    /// Chronological list of chat messages.
    pub messages: Vec<ChatMessage>,

    /// Sampling temperature (0.0 to 2.0).
    pub temperature: Option<f32>,

    /// Maximum token generation budget.
    pub max_tokens: Option<u32>,

    /// Whether to request server-sent event streaming.
    pub stream: bool,
}

impl CompletionRequest {
    /// Constructs a single-turn user prompt completion request.
    pub fn single_prompt(model: impl Into<String>, prompt: impl Into<String>) -> Self {
        Self {
            model: model.into(),
            messages: vec![ChatMessage::user(prompt)],
            temperature: None,
            max_tokens: None,
            stream: false,
        }
    }

    /// Sets streaming flag.
    pub fn with_stream(mut self, stream: bool) -> Self {
        self.stream = stream;
        self
    }
}

/// Reasons why a model completed token generation.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FinishReason {
    Stop,
    Length,
    ToolCalls,
    ContentFilter,
    Unknown(String),
}

/// Token accounting metrics for a model interaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Usage {
    pub input_tokens: Option<u32>,
    pub output_tokens: Option<u32>,
    pub total_tokens: Option<u32>,
}

impl Usage {
    /// Constructs a `Usage` object with specified token quantities.
    pub fn new(input: Option<u32>, output: Option<u32>, total: Option<u32>) -> Self {
        let calculated_total = total.or_else(|| match (input, output) {
            (Some(i), Some(o)) => Some(i + o),
            _ => None,
        });
        Self {
            input_tokens: input,
            output_tokens: output,
            total_tokens: calculated_total,
        }
    }
}

/// Normalized non-streaming completion response from a provider.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompletionResponse {
    /// Unique provider response ID.
    pub id: String,

    /// Responding model identifier.
    pub model: String,

    /// Generated text content.
    pub content: String,

    /// Completion finish reason.
    pub finish_reason: Option<FinishReason>,

    /// Token utilization metadata, if returned by provider.
    pub usage: Option<Usage>,
}
