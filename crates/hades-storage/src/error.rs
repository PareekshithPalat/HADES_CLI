use std::path::PathBuf;
use thiserror::Error;

/// Errors that can occur within the storage subsystem.
#[derive(Debug, Error)]
pub enum StorageError {
    /// An I/O error occurred while performing file operations.
    #[error("Storage I/O error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    /// Failed to serialize data for storage.
    #[error("Failed to serialize value: {0}")]
    Serialization(String),

    /// Failed to deserialize data from storage.
    #[error("Failed to deserialize value: {0}")]
    Deserialization(String),

    /// Invalid storage key specified (e.g. contains illegal path characters).
    #[error("Invalid storage key '{0}': must be alphanumeric with dashes or underscores")]
    InvalidKey(String),

    /// Storage subsystem initialization failed.
    #[error("Storage initialization failed at {path}: {message}")]
    InitializationFailed { path: PathBuf, message: String },

    /// Unable to determine home directory.
    #[error("Unable to determine default user storage directory")]
    HomeDirectoryNotFound,
}
