use std::fmt;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::context::ToolContext;

/// Categorical risk level of a tool execution for safety evaluation and approval policies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RiskLevel {
    /// Read-only operations within the current workspace that cannot alter system state.
    Safe = 0,
    /// Low-impact mutating operations like creating a new non-sensitive file in workspace.
    Low = 1,
    /// Non-trivial mutations like modifying source code, running build checks.
    Medium = 2,
    /// High-impact operations like file deletion, process execution, environment changes.
    High = 3,
    /// Critical operations like recursive deletion, sensitive-file access, system paths.
    Critical = 4,
}

impl fmt::Display for RiskLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Safe => write!(f, "SAFE"),
            Self::Low => write!(f, "LOW"),
            Self::Medium => write!(f, "MEDIUM"),
            Self::High => write!(f, "HIGH"),
            Self::Critical => write!(f, "CRITICAL"),
        }
    }
}

/// Execution status of a tool invocation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ToolStatus {
    Success,
    Failure,
    Cancelled,
    TimedOut,
    PermissionDenied,
    InvalidInput,
}

impl fmt::Display for ToolStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Success => write!(f, "SUCCESS"),
            Self::Failure => write!(f, "FAILURE"),
            Self::Cancelled => write!(f, "CANCELLED"),
            Self::TimedOut => write!(f, "TIMED_OUT"),
            Self::PermissionDenied => write!(f, "PERMISSION_DENIED"),
            Self::InvalidInput => write!(f, "INVALID_INPUT"),
        }
    }
}

/// Structural metadata defining a Hades tool's interface, capabilities, and safety bounds.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    /// Stable namespaced identifier (e.g., "filesystem.read", "shell.execute").
    pub name: String,
    /// Human and LLM-readable description of what the tool accomplishes.
    pub description: String,
    /// JSON schema describing the expected input parameters.
    pub parameters_schema: serde_json::Value,
    /// Inherent baseline risk level of this tool.
    pub risk_level: RiskLevel,
    /// Whether this tool mutates persistent state.
    pub is_mutating: bool,
    /// Default execution timeout.
    pub timeout: Duration,
}

impl ToolDefinition {
    /// Creates a new tool definition with default parameters.
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        parameters_schema: serde_json::Value,
        risk_level: RiskLevel,
        is_mutating: bool,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            parameters_schema,
            risk_level,
            is_mutating,
            timeout: Duration::from_secs(30),
        }
    }

    /// Sets custom timeout.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }
}

/// A structured tool request issued by the model or runtime.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolCall {
    /// Unique execution identifier for tracking, audit, and result matching.
    pub id: String,
    /// Target tool name to execute.
    pub tool_name: String,
    /// Structured arguments provided for execution.
    pub arguments: serde_json::Value,
}

impl ToolCall {
    /// Creates a new tool call with generated or specified ID.
    pub fn new(
        id: impl Into<String>,
        tool_name: impl Into<String>,
        arguments: serde_json::Value,
    ) -> Self {
        Self {
            id: id.into(),
            tool_name: tool_name.into(),
            arguments,
        }
    }
}

/// Result returned from a tool execution, sanitized and bounded for model context and UI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolResult {
    /// The matching tool call ID.
    pub call_id: String,
    /// The tool name executed.
    pub tool_name: String,
    /// High-level execution status.
    pub status: ToolStatus,
    /// Primary output text (sanitized, bounded).
    pub output: String,
    /// Optional error message if the tool failed.
    pub error: Option<String>,
    /// Additional structured metadata (e.g. exit code, lines modified, bytes read).
    pub metadata: serde_json::Value,
    /// Whether the output was truncated due to length/byte limits.
    pub is_truncated: bool,
    /// Optional artifact ID if a full unabridged log was saved to disk.
    pub artifact_id: Option<String>,
}

impl ToolResult {
    /// Creates a successful tool result.
    pub fn success(
        call_id: impl Into<String>,
        tool_name: impl Into<String>,
        output: impl Into<String>,
    ) -> Self {
        Self {
            call_id: call_id.into(),
            tool_name: tool_name.into(),
            status: ToolStatus::Success,
            output: output.into(),
            error: None,
            metadata: serde_json::json!({}),
            is_truncated: false,
            artifact_id: None,
        }
    }

    /// Creates a failed tool result with descriptive error.
    pub fn failure(
        call_id: impl Into<String>,
        tool_name: impl Into<String>,
        error: impl Into<String>,
    ) -> Self {
        let err_str = error.into();
        Self {
            call_id: call_id.into(),
            tool_name: tool_name.into(),
            status: ToolStatus::Failure,
            output: String::new(),
            error: Some(err_str),
            metadata: serde_json::json!({}),
            is_truncated: false,
            artifact_id: None,
        }
    }

    /// Creates a permission-denied tool result.
    pub fn permission_denied(
        call_id: impl Into<String>,
        tool_name: impl Into<String>,
        reason: impl Into<String>,
    ) -> Self {
        let reason_str = reason.into();
        Self {
            call_id: call_id.into(),
            tool_name: tool_name.into(),
            status: ToolStatus::PermissionDenied,
            output: String::new(),
            error: Some(format!("Permission denied: {}", reason_str)),
            metadata: serde_json::json!({ "reason": reason_str }),
            is_truncated: false,
            artifact_id: None,
        }
    }

    /// Creates an invalid-input tool result.
    pub fn invalid_input(
        call_id: impl Into<String>,
        tool_name: impl Into<String>,
        reason: impl Into<String>,
    ) -> Self {
        let reason_str = reason.into();
        Self {
            call_id: call_id.into(),
            tool_name: tool_name.into(),
            status: ToolStatus::InvalidInput,
            output: String::new(),
            error: Some(format!("Invalid input: {}", reason_str)),
            metadata: serde_json::json!({ "reason": reason_str }),
            is_truncated: false,
            artifact_id: None,
        }
    }

    /// Sets metadata payload.
    pub fn with_metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = metadata;
        self
    }

    /// Sets truncation flag.
    pub fn with_truncation(mut self, is_truncated: bool) -> Self {
        self.is_truncated = is_truncated;
        self
    }

    /// Sets associated artifact ID.
    pub fn with_artifact(mut self, artifact_id: impl Into<String>) -> Self {
        self.artifact_id = Some(artifact_id.into());
        self
    }
}

/// Asynchronous trait implemented by all Hades tool executors.
#[async_trait]
pub trait Tool: Send + Sync {
    /// Returns the static definition and schema of this tool.
    fn definition(&self) -> ToolDefinition;

    /// Executes the tool with the provided arguments and context.
    async fn execute(
        &self,
        call_id: &str,
        input: serde_json::Value,
        context: &ToolContext,
    ) -> ToolResult;
}

pub type DynTool = Arc<dyn Tool>;
