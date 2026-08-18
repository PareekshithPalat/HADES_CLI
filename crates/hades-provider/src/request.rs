use serde::{Deserialize, Serialize};

/// Role of a message participant in a chat completion prompt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    System,
    User,
    Assistant,
    Tool,
}

impl std::fmt::Display for MessageRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::System => write!(f, "system"),
            Self::User => write!(f, "user"),
            Self::Assistant => write!(f, "assistant"),
            Self::Tool => write!(f, "tool"),
        }
    }
}

/// Function invocation payload inside a tool call.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolCallFunction {
    /// Name of the target tool/function to invoke.
    pub name: String,
    /// JSON string of arguments for the tool.
    pub arguments: String,
}

/// Structured tool invocation produced by a model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderToolCall {
    /// Unique identifier for this specific tool call invocation.
    pub id: String,
    /// Invocation type (defaults to "function").
    #[serde(rename = "type", default = "default_tool_type")]
    pub call_type: String,
    /// Function name and arguments.
    pub function: ToolCallFunction,
}

impl ProviderToolCall {
    /// Constructs a new tool call with function type.
    pub fn function(
        id: impl Into<String>,
        name: impl Into<String>,
        arguments: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            call_type: "function".to_string(),
            function: ToolCallFunction {
                name: name.into(),
                arguments: arguments.into(),
            },
        }
    }
}

fn default_tool_type() -> String {
    "function".to_string()
}

/// Schema definition for a single tool function exposed to the AI model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolFunctionDefinition {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

/// Provider-facing tool definition payload matching the standard OpenAI function tool schema.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolDefinitionPayload {
    #[serde(rename = "type")]
    pub tool_type: String,
    pub function: ToolFunctionDefinition,
}

impl ToolDefinitionPayload {
    /// Constructs a function tool definition from name, description, and JSON parameter schema.
    pub fn function(
        name: impl Into<String>,
        description: impl Into<String>,
        parameters: serde_json::Value,
    ) -> Self {
        Self {
            tool_type: "function".to_string(),
            function: ToolFunctionDefinition {
                name: name.into(),
                description: description.into(),
                parameters,
            },
        }
    }
}

/// Normalized chat message structure supporting system, user, assistant, and tool roles.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: MessageRole,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ProviderToolCall>>,
}

impl ChatMessage {
    /// Creates a new user prompt message.
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::User,
            content: Some(content.into()),
            name: None,
            tool_call_id: None,
            tool_calls: None,
        }
    }

    /// Creates a new system instruction message.
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::System,
            content: Some(content.into()),
            name: None,
            tool_call_id: None,
            tool_calls: None,
        }
    }

    /// Creates a standard assistant text response message.
    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::Assistant,
            content: Some(content.into()),
            name: None,
            tool_call_id: None,
            tool_calls: None,
        }
    }

    /// Creates an assistant message containing structured tool calls.
    pub fn assistant_with_tools(
        content: Option<String>,
        tool_calls: Vec<ProviderToolCall>,
    ) -> Self {
        Self {
            role: MessageRole::Assistant,
            content,
            name: None,
            tool_call_id: None,
            tool_calls: if tool_calls.is_empty() {
                None
            } else {
                Some(tool_calls)
            },
        }
    }

    /// Creates a tool execution result message responding to a specific `tool_call_id`.
    pub fn tool_result(tool_call_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::Tool,
            content: Some(content.into()),
            name: None,
            tool_call_id: Some(tool_call_id.into()),
            tool_calls: None,
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

    /// Optional tool definitions available for the model to invoke.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<ToolDefinitionPayload>>,

    /// Optional tool choice constraint (e.g. "auto", "none", "required").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<serde_json::Value>,

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
            tools: None,
            tool_choice: None,
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

    /// Attaches tool definitions to the completion request.
    pub fn with_tools(mut self, tools: Vec<ToolDefinitionPayload>) -> Self {
        if !tools.is_empty() {
            self.tools = Some(tools);
        }
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

    /// Structured tool calls requested by the model, if any.
    pub tool_calls: Vec<ProviderToolCall>,

    /// Completion finish reason.
    pub finish_reason: Option<FinishReason>,

    /// Token utilization metadata, if returned by provider.
    pub usage: Option<Usage>,
}
