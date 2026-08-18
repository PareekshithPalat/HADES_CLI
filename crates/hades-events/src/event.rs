use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// High-level application events representing state transitions and operational lifecycle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum HadesEvent {
    /// Application successfully started.
    ApplicationStarted {
        timestamp: DateTime<Utc>,
        version: String,
    },

    /// Application initiated shutdown.
    ApplicationShutdown {
        timestamp: DateTime<Utc>,
        reason: Option<String>,
    },

    /// A command string was submitted by the user.
    CommandEntered {
        timestamp: DateTime<Utc>,
        raw_input: String,
    },

    /// A command finished execution.
    CommandExecuted {
        timestamp: DateTime<Utc>,
        command: String,
        success: bool,
    },

    /// Application configuration was loaded.
    ConfigLoaded {
        timestamp: DateTime<Utc>,
        path: PathBuf,
    },

    /// Application configuration was saved.
    ConfigSaved {
        timestamp: DateTime<Utc>,
        path: PathBuf,
    },

    /// An AI provider was selected in the workflow.
    ProviderSelected {
        timestamp: DateTime<Utc>,
        provider_id: String,
    },

    /// An AI model was selected in the workflow.
    ModelSelected {
        timestamp: DateTime<Utc>,
        provider_id: String,
        model_id: String,
    },

    /// Credential configuration began for a provider.
    CredentialSetupStarted {
        timestamp: DateTime<Utc>,
        provider_id: String,
    },

    /// Credential verification commenced.
    CredentialVerificationStarted {
        timestamp: DateTime<Utc>,
        provider_id: String,
    },

    /// Credential and model verification succeeded.
    CredentialVerificationSucceeded {
        timestamp: DateTime<Utc>,
        provider_id: String,
        model_id: String,
    },

    /// Credential verification failed.
    CredentialVerificationFailed {
        timestamp: DateTime<Utc>,
        provider_id: String,
        error: String,
    },

    /// Active model and provider successfully loaded into runtime.
    ModelLoaded {
        timestamp: DateTime<Utc>,
        provider_id: String,
        model_id: String,
    },

    /// An AI prompt request was submitted to the model provider.
    ModelRequestStarted {
        timestamp: DateTime<Utc>,
        provider_id: String,
        model_id: String,
    },

    /// First token or response chunk received from the model.
    ModelResponseStarted {
        timestamp: DateTime<Utc>,
        provider_id: String,
        model_id: String,
    },

    /// Model response completed generation.
    ModelResponseCompleted {
        timestamp: DateTime<Utc>,
        provider_id: String,
        model_id: String,
        total_tokens: Option<u32>,
    },

    /// An error occurred in the provider subsystem.
    ProviderErrorOccurred {
        timestamp: DateTime<Utc>,
        provider_id: String,
        error: String,
    },

    /// An application-level error occurred.
    ErrorOccurred {
        timestamp: DateTime<Utc>,
        error: String,
    },
}

impl HadesEvent {
    /// Creates an `ApplicationStarted` event at the current time.
    pub fn app_started(version: impl Into<String>) -> Self {
        Self::ApplicationStarted {
            timestamp: Utc::now(),
            version: version.into(),
        }
    }

    /// Creates an `ApplicationShutdown` event at the current time.
    pub fn app_shutdown(reason: Option<String>) -> Self {
        Self::ApplicationShutdown {
            timestamp: Utc::now(),
            reason,
        }
    }

    /// Creates a `CommandEntered` event at the current time.
    pub fn command_entered(raw_input: impl Into<String>) -> Self {
        Self::CommandEntered {
            timestamp: Utc::now(),
            raw_input: raw_input.into(),
        }
    }

    /// Creates a `CommandExecuted` event at the current time.
    pub fn command_executed(command: impl Into<String>, success: bool) -> Self {
        Self::CommandExecuted {
            timestamp: Utc::now(),
            command: command.into(),
            success,
        }
    }

    /// Creates a `ConfigLoaded` event at the current time.
    pub fn config_loaded(path: impl Into<PathBuf>) -> Self {
        Self::ConfigLoaded {
            timestamp: Utc::now(),
            path: path.into(),
        }
    }

    /// Creates a `ConfigSaved` event at the current time.
    pub fn config_saved(path: impl Into<PathBuf>) -> Self {
        Self::ConfigSaved {
            timestamp: Utc::now(),
            path: path.into(),
        }
    }

    /// Creates a `ProviderSelected` event.
    pub fn provider_selected(provider_id: impl Into<String>) -> Self {
        Self::ProviderSelected {
            timestamp: Utc::now(),
            provider_id: provider_id.into(),
        }
    }

    /// Creates a `ModelSelected` event.
    pub fn model_selected(provider_id: impl Into<String>, model_id: impl Into<String>) -> Self {
        Self::ModelSelected {
            timestamp: Utc::now(),
            provider_id: provider_id.into(),
            model_id: model_id.into(),
        }
    }

    /// Creates a `ModelLoaded` event.
    pub fn model_loaded(provider_id: impl Into<String>, model_id: impl Into<String>) -> Self {
        Self::ModelLoaded {
            timestamp: Utc::now(),
            provider_id: provider_id.into(),
            model_id: model_id.into(),
        }
    }

    /// Creates an `ErrorOccurred` event at the current time.
    pub fn error_occurred(error: impl Into<String>) -> Self {
        Self::ErrorOccurred {
            timestamp: Utc::now(),
            error: error.into(),
        }
    }

    /// Returns the timestamp of the event.
    pub fn timestamp(&self) -> DateTime<Utc> {
        match self {
            Self::ApplicationStarted { timestamp, .. } => *timestamp,
            Self::ApplicationShutdown { timestamp, .. } => *timestamp,
            Self::CommandEntered { timestamp, .. } => *timestamp,
            Self::CommandExecuted { timestamp, .. } => *timestamp,
            Self::ConfigLoaded { timestamp, .. } => *timestamp,
            Self::ConfigSaved { timestamp, .. } => *timestamp,
            Self::ProviderSelected { timestamp, .. } => *timestamp,
            Self::ModelSelected { timestamp, .. } => *timestamp,
            Self::CredentialSetupStarted { timestamp, .. } => *timestamp,
            Self::CredentialVerificationStarted { timestamp, .. } => *timestamp,
            Self::CredentialVerificationSucceeded { timestamp, .. } => *timestamp,
            Self::CredentialVerificationFailed { timestamp, .. } => *timestamp,
            Self::ModelLoaded { timestamp, .. } => *timestamp,
            Self::ModelRequestStarted { timestamp, .. } => *timestamp,
            Self::ModelResponseStarted { timestamp, .. } => *timestamp,
            Self::ModelResponseCompleted { timestamp, .. } => *timestamp,
            Self::ProviderErrorOccurred { timestamp, .. } => *timestamp,
            Self::ErrorOccurred { timestamp, .. } => *timestamp,
        }
    }
}
