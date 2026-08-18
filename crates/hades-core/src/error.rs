use thiserror::Error;

use hades_config::ConfigError;
use hades_provider::{CredentialError, ProviderError};
use hades_storage::StorageError;

/// Core application runtime errors.
#[derive(Debug, Error)]
pub enum CoreError {
    /// An error occurred in the configuration subsystem.
    #[error("Configuration error: {0}")]
    Config(#[from] ConfigError),

    /// An error occurred in the storage subsystem.
    #[error("Storage error: {0}")]
    Storage(#[from] StorageError),

    /// An error occurred in the AI model/provider subsystem.
    #[error("Provider error: {0}")]
    Provider(#[from] ProviderError),

    /// An error occurred in the credential subsystem.
    #[error("Credential error: {0}")]
    Credential(#[from] CredentialError),

    /// An invalid application state transition was attempted.
    #[error("Invalid state transition from {from:?} to {to:?}")]
    InvalidStateTransition {
        from: crate::state::AppState,
        to: crate::state::AppState,
    },

    /// Command subsystem error.
    #[error("Command error: {0}")]
    Command(#[from] CommandError),

    /// General application initialization or runtime error.
    #[error("Runtime error: {0}")]
    Runtime(String),
}

/// Errors originating from command lookup or execution.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum CommandError {
    /// Command is not recognized.
    #[error("Unknown command: {0}")]
    UnknownCommand(String),

    /// Empty command input provided.
    #[error("Command input cannot be empty")]
    EmptyInput,

    /// Execution failure during command processing.
    #[error("Execution error: {0}")]
    ExecutionFailed(String),
}
