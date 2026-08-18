pub mod adapters;
pub mod capability;
pub mod credential;
pub mod error;
pub mod manager;
pub mod model;
pub mod provider;
pub mod request;
pub mod stream;

pub use adapters::OpenAiProvider;
pub use capability::{Capability, CapabilityState, ModelCapabilities};
pub use credential::{
    Credential, CredentialBackend, CredentialError, CredentialSecret, FileCredentialBackend,
};
pub use error::ProviderError;
pub use manager::ModelManager;
pub use model::{Model, PricingMetadata};
pub use provider::{Provider, ProviderMetadata};
pub use request::{
    ChatMessage, CompletionRequest, CompletionResponse, FinishReason, MessageRole, Usage,
};
pub use stream::{StreamEvent, StreamResult};

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use std::sync::Arc;
    use tempfile::tempdir;

    #[test]
    fn test_capability_support_and_states() {
        let mut caps = ModelCapabilities::new();
        assert_eq!(
            caps.get(Capability::TextGeneration),
            CapabilityState::Unknown
        );
        assert!(!caps.supports(Capability::TextGeneration));

        caps.set(Capability::TextGeneration, CapabilityState::Supported);
        assert_eq!(
            caps.get(Capability::TextGeneration),
            CapabilityState::Supported
        );
        assert!(caps.supports(Capability::TextGeneration));

        caps.set(Capability::Vision, CapabilityState::Unsupported);
        assert_eq!(caps.get(Capability::Vision), CapabilityState::Unsupported);
        assert!(!caps.supports(Capability::Vision));

        let standard = ModelCapabilities::standard_text();
        assert!(standard.supports(Capability::TextGeneration));
        assert!(standard.supports(Capability::Streaming));
        assert_eq!(
            standard.get(Capability::Vision),
            CapabilityState::Unsupported
        );
    }

    #[test]
    fn test_model_creation_and_context_formatting() {
        let mut model = Model::new("gpt-4o", "openai", "GPT-4o Frontier");
        assert_eq!(model.id, "gpt-4o");
        assert_eq!(model.provider_id, "openai");
        assert_eq!(model.display_name, "GPT-4o Frontier");
        assert_eq!(model.context_window_display(), "Unknown");

        model.context_window = Some(128_000);
        assert_eq!(model.context_window_display(), "128K");

        model.context_window = Some(1_000_000);
        assert_eq!(model.context_window_display(), "1M");

        model.context_window = Some(500);
        assert_eq!(model.context_window_display(), "500 tokens");
    }

    #[test]
    fn test_credential_secret_redaction() {
        let secret = CredentialSecret::new("sk-real-secret-key-12345");
        assert_eq!(secret.expose_secret(), "sk-real-secret-key-12345");

        let debug_str = format!("{:?}", secret);
        assert_eq!(debug_str, "[REDACTED]");
        assert!(!debug_str.contains("real-secret-key"));

        let display_str = format!("{}", secret);
        assert_eq!(display_str, "[REDACTED]");
        assert!(!display_str.contains("real-secret-key"));

        let cred = Credential::with_api_key("openai", "sk-secret-123");
        let cred_debug = format!("{:?}", cred);
        assert!(!cred_debug.contains("sk-secret-123"));
        assert!(cred_debug.contains("[REDACTED]"));
    }

    #[tokio::test]
    async fn test_file_credential_backend_roundtrip() {
        let dir = tempdir().expect("create temp dir");
        let path = dir.path().join("credentials.json");
        let backend = FileCredentialBackend::with_path(&path);

        assert_eq!(backend.get_credential("openai").await.unwrap(), None);

        let cred = Credential::with_api_key("openai", "test-api-key-abc");
        backend
            .store_credential(&cred)
            .await
            .expect("store credential");

        let loaded = backend
            .get_credential("openai")
            .await
            .unwrap()
            .expect("found credential");
        assert_eq!(loaded.provider_id, "openai");
        assert_eq!(
            loaded.api_key.as_ref().unwrap().expose_secret(),
            "test-api-key-abc"
        );

        let removed = backend.delete_credential("openai").await.unwrap();
        assert!(removed);
        assert_eq!(backend.get_credential("openai").await.unwrap(), None);
    }

    #[test]
    fn test_model_manager_registration_and_active_state() {
        let mut manager = ModelManager::new();
        assert_eq!(manager.list_providers().len(), 0);

        let openai_provider = Arc::new(OpenAiProvider::openai());
        let groq_provider = Arc::new(OpenAiProvider::groq());

        manager.register_provider(openai_provider);
        manager.register_provider(groq_provider);

        let providers = manager.list_providers();
        assert_eq!(providers.len(), 2);
        assert_eq!(providers[0].id, "openai");
        assert_eq!(providers[1].id, "groq");

        assert_eq!(manager.active_provider_id(), None);
        assert_eq!(manager.active_model_id(), None);

        manager.set_active("groq", "llama-3.3-70b-versatile");
        assert_eq!(manager.active_provider_id(), Some("groq"));
        assert_eq!(manager.active_model_id(), Some("llama-3.3-70b-versatile"));

        manager.clear_active();
        assert_eq!(manager.active_provider_id(), None);
    }

    #[test]
    fn test_usage_calculation_and_finish_reasons() {
        let usage = Usage::new(Some(10), Some(20), None);
        assert_eq!(usage.input_tokens, Some(10));
        assert_eq!(usage.output_tokens, Some(20));
        assert_eq!(usage.total_tokens, Some(30));

        let response = CompletionResponse {
            id: "chat-123".to_string(),
            model: "gpt-4o".to_string(),
            content: "Hello user!".to_string(),
            finish_reason: Some(FinishReason::Stop),
            usage: Some(usage),
        };
        assert_eq!(response.content, "Hello user!");
        assert_eq!(response.finish_reason, Some(FinishReason::Stop));
    }

    // Mock Provider for deterministic unit testing of verification, discovery, and execution
    struct MockProvider {
        metadata: ProviderMetadata,
        should_fail_auth: bool,
        models: Vec<Model>,
        mock_response: String,
    }

    #[async_trait]
    impl Provider for MockProvider {
        fn id(&self) -> &str {
            &self.metadata.id
        }

        fn metadata(&self) -> &ProviderMetadata {
            &self.metadata
        }

        async fn authenticate(&self, _cred: &Credential) -> Result<(), ProviderError> {
            if self.should_fail_auth {
                Err(ProviderError::AuthenticationFailed {
                    provider: self.id().to_string(),
                    message: "Invalid API key".to_string(),
                })
            } else {
                Ok(())
            }
        }

        async fn list_models(&self, _cred: &Credential) -> Result<Vec<Model>, ProviderError> {
            Ok(self.models.clone())
        }

        async fn get_model(
            &self,
            model_id: &str,
            _cred: &Credential,
        ) -> Result<Model, ProviderError> {
            self.models
                .iter()
                .find(|m| m.id == model_id)
                .cloned()
                .ok_or_else(|| ProviderError::ModelNotFound {
                    provider: self.id().to_string(),
                    model: model_id.to_string(),
                    message: "Model not found".to_string(),
                })
        }

        fn capabilities(&self, _model_id: &str) -> ModelCapabilities {
            ModelCapabilities::standard_text()
        }

        async fn complete(
            &self,
            request: CompletionRequest,
            _cred: &Credential,
        ) -> Result<CompletionResponse, ProviderError> {
            Ok(CompletionResponse {
                id: "mock-123".to_string(),
                model: request.model,
                content: self.mock_response.clone(),
                finish_reason: Some(FinishReason::Stop),
                usage: Some(Usage::new(Some(5), Some(10), Some(15))),
            })
        }

        async fn complete_stream(
            &self,
            _request: CompletionRequest,
            _cred: &Credential,
        ) -> Result<StreamResult, ProviderError> {
            let chunks = vec![
                Ok(StreamEvent::Started),
                Ok(StreamEvent::Delta("Mock ".to_string())),
                Ok(StreamEvent::Delta("response".to_string())),
                Ok(StreamEvent::Usage(Usage::new(Some(5), Some(10), Some(15)))),
                Ok(StreamEvent::Finished(FinishReason::Stop)),
            ];
            Ok(Box::pin(futures::stream::iter(chunks)))
        }
    }

    #[tokio::test]
    async fn test_mock_provider_verification_success_and_failure() {
        let success_provider = Arc::new(MockProvider {
            metadata: ProviderMetadata {
                id: "mock".to_string(),
                name: "Mock Provider".to_string(),
                description: "Mock".to_string(),
                default_endpoint: None,
                supports_dynamic_model_discovery: true,
                requires_api_key: true,
            },
            should_fail_auth: false,
            models: vec![Model::new("mock-model-1", "mock", "Mock Model 1")],
            mock_response: "Hello from mock!".to_string(),
        });

        let mut manager = ModelManager::new();
        manager.register_provider(success_provider);

        let cred = Credential::with_api_key("mock", "valid-key");
        let verified = manager
            .verify_provider_and_model("mock", "mock-model-1", &cred)
            .await;
        assert!(verified.is_ok());
        assert_eq!(verified.unwrap().id, "mock-model-1");

        let fail_provider = Arc::new(MockProvider {
            metadata: ProviderMetadata {
                id: "mock-fail".to_string(),
                name: "Mock Fail".to_string(),
                description: "Mock".to_string(),
                default_endpoint: None,
                supports_dynamic_model_discovery: true,
                requires_api_key: true,
            },
            should_fail_auth: true,
            models: vec![],
            mock_response: String::new(),
        });
        manager.register_provider(fail_provider);

        let failed = manager
            .verify_provider_and_model("mock-fail", "any-model", &cred)
            .await;
        assert!(failed.is_err());
        match failed {
            Err(ProviderError::AuthenticationFailed { provider, message }) => {
                assert_eq!(provider, "mock-fail");
                assert_eq!(message, "Invalid API key");
            }
            _ => panic!("Expected AuthenticationFailed error"),
        }
    }

    #[tokio::test]
    async fn test_mock_provider_complete_and_streaming() {
        use futures::StreamExt;

        let provider = Arc::new(MockProvider {
            metadata: ProviderMetadata {
                id: "mock".to_string(),
                name: "Mock".to_string(),
                description: "Mock".to_string(),
                default_endpoint: None,
                supports_dynamic_model_discovery: true,
                requires_api_key: false,
            },
            should_fail_auth: false,
            models: vec![Model::new("mock-model", "mock", "Mock Model")],
            mock_response: "Calculated answer: 42".to_string(),
        });

        let mut manager = ModelManager::new();
        manager.register_provider(provider);
        manager.set_active("mock", "mock-model");

        let cred = Credential::with_api_key("mock", "");
        let req = CompletionRequest::single_prompt("mock-model", "What is the answer?");
        let resp = manager
            .complete(req.clone(), &cred)
            .await
            .expect("complete");
        assert_eq!(resp.content, "Calculated answer: 42");
        assert_eq!(resp.usage.unwrap().total_tokens, Some(15));

        let mut stream = manager.complete_stream(req, &cred).await.expect("stream");
        let mut full_text = String::new();
        while let Some(event) = stream.next().await {
            match event.unwrap() {
                StreamEvent::Delta(text) => full_text.push_str(&text),
                StreamEvent::Finished(reason) => assert_eq!(reason, FinishReason::Stop),
                _ => {}
            }
        }
        assert_eq!(full_text, "Mock response");
    }
}
