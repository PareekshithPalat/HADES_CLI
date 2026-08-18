use thiserror::Error;

use hades_core::CoreError;

/// Errors originating in the TUI subsystem.
#[derive(Debug, Error)]
pub enum TuiError {
    /// An I/O error occurred during terminal operations.
    #[error("Terminal I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// A core application error occurred.
    #[error("Core runtime error: {0}")]
    Core(#[from] CoreError),

    /// A general rendering or event processing error.
    #[error("TUI error: {0}")]
    Generic(String),
}
