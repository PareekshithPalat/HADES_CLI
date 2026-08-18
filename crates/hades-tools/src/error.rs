use std::path::PathBuf;
use thiserror::Error;

/// Error types occurring within the Hades tool execution, filesystem, and security subsystem.
#[derive(Debug, Error)]
pub enum ToolError {
    #[error("Tool '{0}' not found in registry")]
    ToolNotFound(String),

    #[error("Invalid tool arguments for '{tool_name}': {reason}")]
    InvalidArguments { tool_name: String, reason: String },

    #[error("Permission denied for '{tool_name}': {reason}")]
    PermissionDenied { tool_name: String, reason: String },

    #[error("Security violation: Path '{path}' escapes workspace boundary '{boundary}'")]
    PathEscapesWorkspace { path: PathBuf, boundary: PathBuf },

    #[error("Security violation: Path traversal or invalid path: '{0}'")]
    InvalidPath(String),

    #[error("Security violation: Access to sensitive path '{0}' is restricted")]
    SensitivePathRestricted(PathBuf),

    #[error("File operation failed at '{path}': {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("File already exists at '{0}'")]
    FileAlreadyExists(PathBuf),

    #[error("File not found at '{0}'")]
    FileNotFound(PathBuf),

    #[error("Target is a directory, not a file: '{0}'")]
    IsADirectory(PathBuf),

    #[error("Target is a file, not a directory: '{0}'")]
    NotADirectory(PathBuf),

    #[error("Edit conflict in '{path}': Expected snippet not found or matched multiple regions")]
    EditConflict { path: PathBuf, details: String },

    #[error("Binary file detected at '{0}'; text operations are unsupported")]
    BinaryFileDetected(PathBuf),

    #[error("Execution timed out after {0} seconds")]
    TimedOut(u64),

    #[error("Tool execution was cancelled")]
    Cancelled,

    #[error("Process execution error: {0}")]
    Process(String),

    #[error("Serialization error: {0}")]
    Serialization(String),
}
