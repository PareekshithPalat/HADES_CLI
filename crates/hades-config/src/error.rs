use std::path::PathBuf;
use thiserror::Error;

/// Errors that can occur within the configuration subsystem.
#[derive(Debug, Error)]
pub enum ConfigError {
    /// An I/O error occurred while reading or writing configuration files.
    #[error("I/O error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    /// Failed to deserialize configuration from file content.
    #[error("Failed to parse configuration file at {path}: {source}")]
    Parse {
        path: PathBuf,
        #[source]
        source: Box<toml::de::Error>,
    },

    /// Failed to serialize configuration to string/file.
    #[error("Failed to serialize configuration: {source}")]
    Serialize {
        #[from]
        source: toml::ser::Error,
    },

    /// Configuration validation failed.
    #[error("Configuration validation error: {0}")]
    Validation(String),

    /// Unable to resolve the platform-specific configuration directory.
    #[error("Unable to determine default user configuration directory")]
    HomeDirectoryNotFound,
}
