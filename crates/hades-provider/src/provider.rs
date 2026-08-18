use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::capability::ModelCapabilities;
use crate::credential::Credential;
use crate::error::ProviderError;
use crate::model::Model;
use crate::request::{CompletionRequest, CompletionResponse};
use crate::stream::StreamResult;

/// Metadata describing an AI provider implementation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderMetadata {
    /// Unique provider identifier (e.g. "openai", "groq", "ollama", "custom").
    pub id: String,

    /// Human-readable provider title (e.g. "OpenAI", "Groq Cloud", "Ollama").
    pub name: String,

    /// Concise summary of provider scope and offerings.
    pub description: String,

    /// Default public endpoint or base URL, if standard.
    pub default_endpoint: Option<String>,

    /// Whether this provider exposes an API endpoint to discover models dynamically.
    pub supports_dynamic_model_discovery: bool,

    /// Whether authentication credentials (e.g. API keys) are required.
    pub requires_api_key: bool,
}

/// Core abstraction for AI model service providers.
#[async_trait]
pub trait Provider: Send + Sync {
    /// Returns the unique machine identifier for this provider.
    fn id(&self) -> &str;

    /// Returns static metadata describing this provider.
    fn metadata(&self) -> &ProviderMetadata;

    /// Verifies that the provided credentials authenticate successfully with the provider service.
    async fn authenticate(&self, credential: &Credential) -> Result<(), ProviderError>;

    /// Lists models discovered from or supported by this provider.
    async fn list_models(&self, credential: &Credential) -> Result<Vec<Model>, ProviderError>;

    /// Retrieves detailed model metadata for a specific model ID.
    async fn get_model(
        &self,
        model_id: &str,
        credential: &Credential,
    ) -> Result<Model, ProviderError>;

    /// Returns capability support flags for a designated model under this provider.
    fn capabilities(&self, model_id: &str) -> ModelCapabilities;

    /// Executes a standard (non-streaming) chat completion request.
    async fn complete(
        &self,
        request: CompletionRequest,
        credential: &Credential,
    ) -> Result<CompletionResponse, ProviderError>;

    /// Executes a streaming chat completion request, returning a normalized `StreamResult`.
    async fn complete_stream(
        &self,
        request: CompletionRequest,
        credential: &Credential,
    ) -> Result<StreamResult, ProviderError>;
}
