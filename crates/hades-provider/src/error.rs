use crate::capability::Capability;
use thiserror::Error;

/// Normalized errors produced by AI model providers.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ProviderError {
    #[error("Authentication failed for provider '{provider}': {message}")]
    AuthenticationFailed { provider: String, message: String },

    #[error("Model '{model}' not found on provider '{provider}': {message}")]
    ModelNotFound {
        provider: String,
        model: String,
        message: String,
    },

    #[error("Rate limit exceeded for provider '{provider}' (retry after: {retry_after_secs:?}s)")]
    RateLimitExceeded {
        provider: String,
        retry_after_secs: Option<u64>,
    },

    #[error("Network connection error communicating with provider '{provider}': {message}")]
    NetworkError { provider: String, message: String },

    #[error("Provider '{provider}' returned server error HTTP {status_code}: {message}")]
    ServerUnavailable {
        provider: String,
        status_code: u16,
        message: String,
    },

    #[error("Invalid request parameters for provider '{provider}': {message}")]
    InvalidRequest { provider: String, message: String },

    #[error("Streaming error from provider '{provider}': {message}")]
    StreamError { provider: String, message: String },

    #[error("Failed to parse response payload from provider '{provider}': {message}")]
    Serialization { provider: String, message: String },

    #[error(
        "Capability '{capability}' is not supported by model '{model}' on provider '{provider}'"
    )]
    CapabilityNotSupported {
        provider: String,
        model: String,
        capability: Capability,
    },

    #[error("Credential error for provider '{provider}': {message}")]
    CredentialError { provider: String, message: String },

    #[error("Error from provider '{provider}': {message}")]
    Other { provider: String, message: String },
}
