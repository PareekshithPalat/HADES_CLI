use thiserror::Error;

/// Errors arising from the Model Context Protocol (MCP) subsystem.
#[derive(Debug, Error)]
pub enum McpError {
    #[error("I/O error during MCP communication: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON serialization error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("HTTP transport error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("Protocol error: {0}")]
    Protocol(String),

    #[error("JSON-RPC error [{code}]: {message}")]
    JsonRpc { code: i64, message: String },

    #[error("Transport error: {0}")]
    Transport(String),

    #[error("Server '{0}' is not running or disconnected")]
    NotConnected(String),

    #[error("Server '{0}' failed to start: {1}")]
    StartupFailed(String, String),

    #[error("Server '{0}' timed out after {1:?}")]
    Timeout(String, std::time::Duration),

    #[error("Tool '{0}' not found on MCP server '{1}'")]
    ToolNotFound(String, String),

    #[error("Resource '{0}' not found on MCP server '{1}'")]
    ResourceNotFound(String, String),

    #[error("Prompt '{0}' not found on MCP server '{1}'")]
    PromptNotFound(String, String),

    #[error("Invalid MCP server configuration: {0}")]
    Configuration(String),

    #[error("MCP server process terminated unexpectedly: {0}")]
    ProcessTerminated(String),

    #[error("MCP execution cancelled")]
    Cancelled,
}
