use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

/// Operational status indicator for storage subsystem.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum StorageStatus {
    Ready,
    Degraded(String),
    Unhealthy(String),
}

/// Diagnostic health report for persistent storage.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StorageHealth {
    pub status: StorageStatus,
    pub root_dir: PathBuf,
    pub writable: bool,
}

/// Role of a message in a conversation session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    System,
    User,
    Assistant,
    Tool,
    Error,
}

impl std::fmt::Display for MessageRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MessageRole::System => write!(f, "system"),
            MessageRole::User => write!(f, "user"),
            MessageRole::Assistant => write!(f, "assistant"),
            MessageRole::Tool => write!(f, "tool"),
            MessageRole::Error => write!(f, "error"),
        }
    }
}

/// Extensible metadata attached to a message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MessageMetadata {
    pub provider: Option<String>,
    pub model: Option<String>,
    pub input_tokens: Option<u32>,
    pub output_tokens: Option<u32>,
    pub total_tokens: Option<u32>,
    pub finish_reason: Option<String>,
    pub latency_ms: Option<u64>,
    pub streaming_complete: bool,
    pub is_interrupted: bool,
    pub tool_call_id: Option<String>,
    pub tool_calls: Option<String>,
}

impl Default for MessageMetadata {
    fn default() -> Self {
        Self {
            provider: None,
            model: None,
            input_tokens: None,
            output_tokens: None,
            total_tokens: None,
            finish_reason: None,
            latency_ms: None,
            streaming_complete: true,
            is_interrupted: false,
            tool_call_id: None,
            tool_calls: None,
        }
    }
}

/// Canonical structured message model belonging to a Hades session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Message {
    pub id: String,
    pub session_id: String,
    pub role: MessageRole,
    pub content: String,
    pub created_at: DateTime<Utc>,
    pub metadata: MessageMetadata,
}

impl Message {
    /// Creates a new user message.
    pub fn user(session_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            session_id: session_id.into(),
            role: MessageRole::User,
            content: content.into(),
            created_at: Utc::now(),
            metadata: MessageMetadata::default(),
        }
    }

    /// Creates a new assistant message with provider and model metadata.
    pub fn assistant(
        session_id: impl Into<String>,
        content: impl Into<String>,
        provider: Option<String>,
        model: Option<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            session_id: session_id.into(),
            role: MessageRole::Assistant,
            content: content.into(),
            created_at: Utc::now(),
            metadata: MessageMetadata {
                provider,
                model,
                streaming_complete: true,
                ..Default::default()
            },
        }
    }

    /// Creates a new assistant message with tool calls.
    pub fn assistant_with_tools(
        session_id: impl Into<String>,
        content: impl Into<String>,
        tool_calls_json: impl Into<String>,
        provider: Option<String>,
        model: Option<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            session_id: session_id.into(),
            role: MessageRole::Assistant,
            content: content.into(),
            created_at: Utc::now(),
            metadata: MessageMetadata {
                provider,
                model,
                streaming_complete: true,
                tool_calls: Some(tool_calls_json.into()),
                ..Default::default()
            },
        }
    }

    /// Creates a new system instruction message.
    pub fn system(session_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            session_id: session_id.into(),
            role: MessageRole::System,
            content: content.into(),
            created_at: Utc::now(),
            metadata: MessageMetadata::default(),
        }
    }

    /// Creates a new tool execution result message.
    pub fn tool(session_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            session_id: session_id.into(),
            role: MessageRole::Tool,
            content: content.into(),
            created_at: Utc::now(),
            metadata: MessageMetadata::default(),
        }
    }

    /// Creates a new tool execution result message with matching tool_call_id.
    pub fn tool_result(
        session_id: impl Into<String>,
        tool_call_id: impl Into<String>,
        content: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            session_id: session_id.into(),
            role: MessageRole::Tool,
            content: content.into(),
            created_at: Utc::now(),
            metadata: MessageMetadata {
                tool_call_id: Some(tool_call_id.into()),
                ..Default::default()
            },
        }
    }

    /// Creates a new error message.
    pub fn error(session_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            session_id: session_id.into(),
            role: MessageRole::Error,
            content: content.into(),
            created_at: Utc::now(),
            metadata: MessageMetadata::default(),
        }
    }
}

/// Metadata describing a conversation session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionMetadata {
    pub id: String,
    pub title: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub active_provider: Option<String>,
    pub active_model: Option<String>,
    pub message_count: usize,
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
    pub total_tokens: u64,
    pub last_message_at: Option<DateTime<Utc>>,
    pub is_archived: bool,
}

impl SessionMetadata {
    /// Constructs initial session metadata.
    pub fn new(
        id: impl Into<String>,
        title: impl Into<String>,
        active_provider: Option<String>,
        active_model: Option<String>,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: id.into(),
            title: title.into(),
            created_at: now,
            updated_at: now,
            active_provider,
            active_model,
            message_count: 0,
            total_input_tokens: 0,
            total_output_tokens: 0,
            total_tokens: 0,
            last_message_at: None,
            is_archived: false,
        }
    }
}

/// Versioned persistent representation of a complete conversation session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionRecord {
    /// Schema version for future migration capability (current: 1).
    pub schema_version: u32,
    /// Summary session metadata.
    pub metadata: SessionMetadata,
    /// Chronological list of structured messages.
    pub messages: Vec<Message>,
}

impl SessionRecord {
    pub const CURRENT_SCHEMA_VERSION: u32 = 1;

    /// Creates a new empty session record.
    pub fn new(
        title: Option<String>,
        active_provider: Option<String>,
        active_model: Option<String>,
    ) -> Self {
        let id = Uuid::new_v4().to_string();
        let title = title.unwrap_or_else(|| "New Session".to_string());
        let metadata = SessionMetadata::new(id, title, active_provider, active_model);

        Self {
            schema_version: Self::CURRENT_SCHEMA_VERSION,
            metadata,
            messages: Vec::new(),
        }
    }

    /// Appends a message to the session and recalculates metadata metrics.
    pub fn add_message(&mut self, message: Message) {
        self.metadata.message_count += 1;
        self.metadata.last_message_at = Some(message.created_at);
        self.metadata.updated_at = Utc::now();

        if let Some(in_tok) = message.metadata.input_tokens {
            self.metadata.total_input_tokens += in_tok as u64;
        }
        if let Some(out_tok) = message.metadata.output_tokens {
            self.metadata.total_output_tokens += out_tok as u64;
        }
        if let Some(tot_tok) = message.metadata.total_tokens {
            self.metadata.total_tokens += tot_tok as u64;
        }

        // Generate title from first user prompt if still default
        if self.metadata.title == "New Session" && message.role == MessageRole::User {
            self.metadata.title = generate_session_title(&message.content);
        }

        self.messages.push(message);
    }
}

/// Generates a clean human-readable title from the initial user prompt.
pub fn generate_session_title(prompt: &str) -> String {
    let cleaned = prompt.trim();
    if cleaned.is_empty() {
        return "New Session".to_string();
    }

    // Take first line up to 50 chars
    let first_line = cleaned.lines().next().unwrap_or(cleaned).trim();
    let mut title = String::new();
    for word in first_line.split_whitespace() {
        if title.is_empty() {
            title.push_str(word);
        } else if title.len() + 1 + word.len() <= 40 {
            title.push(' ');
            title.push_str(word);
        } else {
            break;
        }
    }

    if title.is_empty() {
        "New Session".to_string()
    } else {
        // Capitalize first letter
        let mut chars = title.chars();
        match chars.next() {
            None => String::new(),
            Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        }
    }
}
