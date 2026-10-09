use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{debug, info};

use crate::credential::Credential;
use crate::error::ProviderError;
use crate::model::Model;
use crate::provider::{Provider, ProviderMetadata};
use crate::request::{CompletionRequest, CompletionResponse};
use crate::stream::StreamResult;

/// Result of a successful provider and model verification.
#[derive(Debug, Clone, PartialEq)]
pub struct ProviderVerification {
    /// The verified model.
    pub model: Model,
    /// Round-trip time of the provider authentication request.
    pub latency: Duration,
}

impl ProviderVerification {
    /// Round-trip latency in whole milliseconds.
    pub fn latency_ms(&self) -> u128 {
        self.latency.as_millis()
    }
}

/// Central coordinator for AI providers and model lifecycle management.
#[derive(Default)]
pub struct ModelManager {
    providers: HashMap<String, Arc<dyn Provider>>,
    provider_order: Vec<String>,
    active_provider_id: Option<String>,
    active_model_id: Option<String>,
    cached_models: HashMap<String, Vec<Model>>,
}

impl ModelManager {
    /// Creates a new `ModelManager` with no registered providers.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers an AI provider with the manager.
    pub fn register_provider(&mut self, provider: Arc<dyn Provider>) {
        let id = provider.id().to_string();
        debug!(provider_id = %id, "Registering AI provider");
        if !self.providers.contains_key(&id) {
            self.provider_order.push(id.clone());
        }
        self.providers.insert(id, provider);
    }

    /// Looks up a registered provider by its unique identifier.
    pub fn get_provider(&self, provider_id: &str) -> Option<Arc<dyn Provider>> {
        self.providers.get(provider_id).cloned()
    }

    /// Returns a list of metadata for all registered providers in registration order.
    pub fn list_providers(&self) -> Vec<ProviderMetadata> {
        self.provider_order
            .iter()
            .filter_map(|id| self.providers.get(id))
            .map(|p| p.metadata().clone())
            .collect()
    }

    /// Returns the currently active provider identifier, if set.
    pub fn active_provider_id(&self) -> Option<&str> {
        self.active_provider_id.as_deref()
    }

    /// Returns the currently active model identifier, if set.
    pub fn active_model_id(&self) -> Option<&str> {
        self.active_model_id.as_deref()
    }

    /// Sets the active provider and model.
    pub fn set_active(&mut self, provider_id: impl Into<String>, model_id: impl Into<String>) {
        let p = provider_id.into();
        let m = model_id.into();
        info!(provider = %p, model = %m, "Activating AI model");
        self.active_provider_id = Some(p);
        self.active_model_id = Some(m);
    }

    /// Clears active model configuration.
    pub fn clear_active(&mut self) {
        self.active_provider_id = None;
        self.active_model_id = None;
    }

    /// Dynamically discovers available models for a provider, using cache if available.
    pub async fn discover_models(
        &mut self,
        provider_id: &str,
        credential: &Credential,
    ) -> Result<Vec<Model>, ProviderError> {
        let provider = self
            .get_provider(provider_id)
            .ok_or_else(|| ProviderError::Other {
                provider: provider_id.to_string(),
                message: format!("Provider '{provider_id}' is not registered"),
            })?;

        debug!(provider = %provider_id, "Discovering models");
        let models = provider.list_models(credential).await?;
        self.cached_models
            .insert(provider_id.to_string(), models.clone());
        Ok(models)
    }

    /// Retrieves cached models for a provider, or returns empty slice.
    pub fn get_cached_models(&self, provider_id: &str) -> Option<&[Model]> {
        self.cached_models.get(provider_id).map(|v| v.as_slice())
    }

    /// Verifies authentication with provider and availability of the chosen model.
    pub async fn verify_provider_and_model(
        &self,
        provider_id: &str,
        model_id: &str,
        credential: &Credential,
    ) -> Result<Model, ProviderError> {
        let provider = self
            .get_provider(provider_id)
            .ok_or_else(|| ProviderError::Other {
                provider: provider_id.to_string(),
                message: format!("Provider '{provider_id}' is not registered"),
            })?;

        info!(provider = %provider_id, model = %model_id, "Verifying provider and model access");
        provider.authenticate(credential).await?;
        let model = provider.get_model(model_id, credential).await?;
        Ok(model)
    }

    /// Verifies provider access and the model like [`Self::verify_provider_and_model`],
    /// additionally measuring the round-trip latency of the authentication request.
    pub async fn verify_with_latency(
        &self,
        provider_id: &str,
        model_id: &str,
        credential: &Credential,
    ) -> Result<ProviderVerification, ProviderError> {
        let provider = self
            .get_provider(provider_id)
            .ok_or_else(|| ProviderError::Other {
                provider: provider_id.to_string(),
                message: format!("Provider '{provider_id}' is not registered"),
            })?;

        info!(provider = %provider_id, model = %model_id, "Verifying provider and model access");
        let started = Instant::now();
        provider.authenticate(credential).await?;
        let latency = started.elapsed();
        let model = provider.get_model(model_id, credential).await?;
        info!(provider = %provider_id, latency_ms = latency.as_millis() as u64, "Provider verified");
        Ok(ProviderVerification { model, latency })
    }

    /// Executes a standard completion request using the active model or an explicitly specified model.
    pub async fn complete(
        &self,
        request: CompletionRequest,
        credential: &Credential,
    ) -> Result<CompletionResponse, ProviderError> {
        let provider_id =
            self.active_provider_id
                .as_deref()
                .ok_or_else(|| ProviderError::Other {
                    provider: "none".to_string(),
                    message: "No active AI provider configured".to_string(),
                })?;

        let provider = self
            .get_provider(provider_id)
            .ok_or_else(|| ProviderError::Other {
                provider: provider_id.to_string(),
                message: format!("Active provider '{provider_id}' is not registered"),
            })?;

        provider.complete(request, credential).await
    }

    /// Executes a streaming completion request using the active model.
    pub async fn complete_stream(
        &self,
        request: CompletionRequest,
        credential: &Credential,
    ) -> Result<StreamResult, ProviderError> {
        let provider_id =
            self.active_provider_id
                .as_deref()
                .ok_or_else(|| ProviderError::Other {
                    provider: "none".to_string(),
                    message: "No active AI provider configured".to_string(),
                })?;

        let provider = self
            .get_provider(provider_id)
            .ok_or_else(|| ProviderError::Other {
                provider: provider_id.to_string(),
                message: format!("Active provider '{provider_id}' is not registered"),
            })?;

        provider.complete_stream(request, credential).await
    }
}
