use async_trait::async_trait;
use futures::StreamExt;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::Duration;
use tracing::debug;

use crate::capability::{Capability, CapabilityState, ModelCapabilities};
use crate::credential::Credential;
use crate::error::ProviderError;
use crate::model::Model;
use crate::provider::{Provider, ProviderMetadata};
use crate::request::{ChatMessage, CompletionRequest, CompletionResponse, FinishReason, Usage};
use crate::stream::{StreamEvent, StreamResult};

/// Adapter for OpenAI-compatible REST APIs (OpenAI, Groq, Ollama, DeepSeek, Local vLLM, LM Studio, etc.).
#[derive(Debug, Clone)]
pub struct OpenAiProvider {
    metadata: ProviderMetadata,
    client: Client,
}

impl OpenAiProvider {
    /// Creates a new `OpenAiProvider` with custom metadata.
    pub fn new(metadata: ProviderMetadata) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(60))
            .build()
            .unwrap_or_default();
        Self { metadata, client }
    }

    /// Factory constructor for OpenAI standard cloud provider.
    pub fn openai() -> Self {
        Self::new(ProviderMetadata {
            id: "openai".to_string(),
            name: "OpenAI".to_string(),
            description: "Industry standard frontier model suite (GPT-4o, o1, GPT-4o-mini)."
                .to_string(),
            default_endpoint: Some("https://api.openai.com/v1".to_string()),
            supports_dynamic_model_discovery: true,
            requires_api_key: true,
        })
    }

    /// Factory constructor for Groq Cloud high-speed inference.
    pub fn groq() -> Self {
        Self::new(ProviderMetadata {
            id: "groq".to_string(),
            name: "Groq".to_string(),
            description: "Ultra-low latency LPU inference (Llama-3.3-70B, Mixtral, Gemma)."
                .to_string(),
            default_endpoint: Some("https://api.groq.com/openai/v1".to_string()),
            supports_dynamic_model_discovery: true,
            requires_api_key: true,
        })
    }

    /// Factory constructor for local Ollama OpenAI-compatible server.
    pub fn ollama() -> Self {
        Self::new(ProviderMetadata {
            id: "ollama".to_string(),
            name: "Ollama (Local)".to_string(),
            description:
                "Locally running self-hosted models via Ollama OpenAI compatibility endpoint."
                    .to_string(),
            default_endpoint: Some("http://localhost:11434/v1".to_string()),
            supports_dynamic_model_discovery: true,
            requires_api_key: false,
        })
    }

    /// Factory constructor for arbitrary custom OpenAI-compatible endpoints.
    pub fn custom() -> Self {
        Self::new(ProviderMetadata {
            id: "custom".to_string(),
            name: "Custom OpenAI-compatible".to_string(),
            description: "Custom self-hosted or proxy REST server implementing the OpenAI chat completions protocol.".to_string(),
            default_endpoint: None,
            supports_dynamic_model_discovery: true,
            requires_api_key: false,
        })
    }

    fn resolve_endpoint(&self, credential: &Credential) -> String {
        let base = credential
            .endpoint
            .as_deref()
            .or(self.metadata.default_endpoint.as_deref())
            .unwrap_or("https://api.openai.com/v1");
        base.trim_end_matches('/').to_string()
    }

    fn build_headers(&self, credential: &Credential) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

        if let Some(ref api_key) = credential.api_key {
            let key_str = api_key.expose_secret();
            if !key_str.is_empty() {
                if let Ok(val) = HeaderValue::from_str(&format!("Bearer {key_str}")) {
                    headers.insert(AUTHORIZATION, val);
                }
            }
        }

        for (name, secret) in &credential.custom_headers {
            if let (Ok(hname), Ok(hval)) = (
                HeaderName::from_bytes(name.as_bytes()),
                HeaderValue::from_str(secret.expose_secret()),
            ) {
                headers.insert(hname, hval);
            }
        }

        headers
    }

    fn parse_error_response(
        provider_id: &str,
        status: reqwest::StatusCode,
        body: &str,
    ) -> ProviderError {
        let message = if let Ok(val) = serde_json::from_str::<Value>(body) {
            val.get("error")
                .and_then(|e| e.get("message"))
                .and_then(|m| m.as_str())
                .unwrap_or(body)
                .to_string()
        } else {
            body.to_string()
        };

        match status.as_u16() {
            401 | 403 => ProviderError::AuthenticationFailed {
                provider: provider_id.to_string(),
                message,
            },
            404 => ProviderError::ModelNotFound {
                provider: provider_id.to_string(),
                model: "requested".to_string(),
                message,
            },
            429 => ProviderError::RateLimitExceeded {
                provider: provider_id.to_string(),
                retry_after_secs: None,
            },
            500..=599 => ProviderError::ServerUnavailable {
                provider: provider_id.to_string(),
                status_code: status.as_u16(),
                message,
            },
            400..=499 => ProviderError::InvalidRequest {
                provider: provider_id.to_string(),
                message,
            },
            _ => ProviderError::Other {
                provider: provider_id.to_string(),
                message,
            },
        }
    }

    fn map_model_capabilities(&self, model_id: &str) -> ModelCapabilities {
        let mut caps = ModelCapabilities::standard_text();
        let lower = model_id.to_lowercase();

        if lower.contains("o1")
            || lower.contains("o3")
            || lower.contains("reasoning")
            || lower.contains("r1")
        {
            caps.set(Capability::Reasoning, CapabilityState::Supported);
        }
        if lower.contains("4o") || lower.contains("vision") || lower.contains("vl") {
            caps.set(Capability::Vision, CapabilityState::Supported);
        }
        if lower.contains("128k")
            || lower.contains("200k")
            || lower.contains("1m")
            || lower.contains("4o")
            || lower.contains("llama-3")
        {
            caps.set(Capability::LongContext, CapabilityState::Supported);
        }
        caps.set(Capability::ToolCalling, CapabilityState::Supported);
        caps
    }
}

#[derive(Serialize)]
struct OpenAiChatRequest<'a> {
    model: &'a str,
    messages: &'a [ChatMessage],
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    stream_options: Option<StreamOptions>,
}

#[derive(Serialize)]
struct StreamOptions {
    include_usage: bool,
}

#[derive(Deserialize)]
struct OpenAiModelListResponse {
    data: Vec<OpenAiModelItem>,
}

#[derive(Deserialize)]
struct OpenAiModelItem {
    id: String,
}

#[derive(Deserialize)]
struct OpenAiChatResponse {
    id: Option<String>,
    model: Option<String>,
    choices: Vec<OpenAiChoice>,
    usage: Option<OpenAiUsage>,
}

#[derive(Deserialize)]
struct OpenAiChoice {
    message: Option<OpenAiMessage>,
    finish_reason: Option<String>,
}

#[derive(Deserialize)]
struct OpenAiMessage {
    content: Option<String>,
}

#[derive(Deserialize)]
struct OpenAiUsage {
    prompt_tokens: Option<u32>,
    completion_tokens: Option<u32>,
    total_tokens: Option<u32>,
}

#[derive(Deserialize)]
struct OpenAiStreamChunk {
    choices: Vec<OpenAiStreamChoice>,
    usage: Option<OpenAiUsage>,
}

#[derive(Deserialize)]
struct OpenAiStreamChoice {
    delta: Option<OpenAiStreamDelta>,
    finish_reason: Option<String>,
}

#[derive(Deserialize)]
struct OpenAiStreamDelta {
    content: Option<String>,
}

#[async_trait]
impl Provider for OpenAiProvider {
    fn id(&self) -> &str {
        &self.metadata.id
    }

    fn metadata(&self) -> &ProviderMetadata {
        &self.metadata
    }

    async fn authenticate(&self, credential: &Credential) -> Result<(), ProviderError> {
        let base = self.resolve_endpoint(credential);
        let url = format!("{base}/models");
        let headers = self.build_headers(credential);

        debug!(provider = %self.id(), url = %url, "Authenticating with provider");
        let resp = self
            .client
            .get(&url)
            .headers(headers)
            .send()
            .await
            .map_err(|e| ProviderError::NetworkError {
                provider: self.id().to_string(),
                message: e.to_string(),
            })?;

        let status = resp.status();
        if status.is_success() {
            Ok(())
        } else {
            let body = resp.text().await.unwrap_or_default();
            Err(Self::parse_error_response(self.id(), status, &body))
        }
    }

    async fn list_models(&self, credential: &Credential) -> Result<Vec<Model>, ProviderError> {
        let base = self.resolve_endpoint(credential);
        let url = format!("{base}/models");
        let headers = self.build_headers(credential);

        debug!(provider = %self.id(), url = %url, "Discovering models from provider");
        let resp = self
            .client
            .get(&url)
            .headers(headers)
            .send()
            .await
            .map_err(|e| ProviderError::NetworkError {
                provider: self.id().to_string(),
                message: e.to_string(),
            })?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(Self::parse_error_response(self.id(), status, &body));
        }

        let body = resp.text().await.map_err(|e| ProviderError::NetworkError {
            provider: self.id().to_string(),
            message: e.to_string(),
        })?;

        let list_resp: OpenAiModelListResponse =
            serde_json::from_str(&body).map_err(|e| ProviderError::Serialization {
                provider: self.id().to_string(),
                message: format!("Failed to parse models payload: {e}"),
            })?;

        let models: Vec<Model> = list_resp
            .data
            .into_iter()
            .map(|item| {
                let mut m = Model::new(&item.id, self.id(), &item.id);
                m.capabilities = self.map_model_capabilities(&item.id);
                m
            })
            .collect();

        Ok(models)
    }

    async fn get_model(
        &self,
        model_id: &str,
        credential: &Credential,
    ) -> Result<Model, ProviderError> {
        let models = self.list_models(credential).await?;
        if let Some(found) = models.into_iter().find(|m| m.id == model_id) {
            Ok(found)
        } else {
            // If not found in dynamic list, synthesize model representation
            let mut m = Model::new(model_id, self.id(), model_id);
            m.capabilities = self.map_model_capabilities(model_id);
            Ok(m)
        }
    }

    fn capabilities(&self, model_id: &str) -> ModelCapabilities {
        self.map_model_capabilities(model_id)
    }

    async fn complete(
        &self,
        request: CompletionRequest,
        credential: &Credential,
    ) -> Result<CompletionResponse, ProviderError> {
        let base = self.resolve_endpoint(credential);
        let url = format!("{base}/chat/completions");
        let headers = self.build_headers(credential);

        let payload = OpenAiChatRequest {
            model: &request.model,
            messages: &request.messages,
            temperature: request.temperature,
            max_tokens: request.max_tokens,
            stream: false,
            stream_options: None,
        };

        debug!(provider = %self.id(), model = %request.model, "Sending completion request");
        let resp = self
            .client
            .post(&url)
            .headers(headers)
            .json(&payload)
            .send()
            .await
            .map_err(|e| ProviderError::NetworkError {
                provider: self.id().to_string(),
                message: e.to_string(),
            })?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(Self::parse_error_response(self.id(), status, &body));
        }

        let chat_resp: OpenAiChatResponse =
            resp.json()
                .await
                .map_err(|e| ProviderError::Serialization {
                    provider: self.id().to_string(),
                    message: format!("Failed to parse chat response: {e}"),
                })?;

        let choice = chat_resp.choices.into_iter().next();
        let content = choice
            .as_ref()
            .and_then(|c| c.message.as_ref())
            .and_then(|m| m.content.clone())
            .unwrap_or_default();

        let finish_reason = choice
            .and_then(|c| c.finish_reason)
            .map(|r| match r.as_str() {
                "stop" => FinishReason::Stop,
                "length" => FinishReason::Length,
                "tool_calls" => FinishReason::ToolCalls,
                "content_filter" => FinishReason::ContentFilter,
                other => FinishReason::Unknown(other.to_string()),
            });

        let usage = chat_resp
            .usage
            .map(|u| Usage::new(u.prompt_tokens, u.completion_tokens, u.total_tokens));

        Ok(CompletionResponse {
            id: chat_resp.id.unwrap_or_else(|| "response".to_string()),
            model: chat_resp.model.unwrap_or(request.model),
            content,
            finish_reason,
            usage,
        })
    }

    async fn complete_stream(
        &self,
        request: CompletionRequest,
        credential: &Credential,
    ) -> Result<StreamResult, ProviderError> {
        let base = self.resolve_endpoint(credential);
        let url = format!("{base}/chat/completions");
        let headers = self.build_headers(credential);

        let payload = OpenAiChatRequest {
            model: &request.model,
            messages: &request.messages,
            temperature: request.temperature,
            max_tokens: request.max_tokens,
            stream: true,
            stream_options: Some(StreamOptions {
                include_usage: true,
            }),
        };

        debug!(provider = %self.id(), model = %request.model, "Initiating streaming completion");
        let resp = self
            .client
            .post(&url)
            .headers(headers)
            .json(&payload)
            .send()
            .await
            .map_err(|e| ProviderError::NetworkError {
                provider: self.id().to_string(),
                message: e.to_string(),
            })?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(Self::parse_error_response(self.id(), status, &body));
        }

        let provider_id = self.id().to_string();
        let byte_stream = resp.bytes_stream();

        // State machine parsing incoming SSE byte stream into lines & JSON objects
        struct StreamState<S> {
            stream: S,
            buffer: String,
            provider_id: String,
            sent_started: bool,
            finished: bool,
        }

        let initial_state = StreamState {
            stream: byte_stream,
            buffer: String::new(),
            provider_id,
            sent_started: false,
            finished: false,
        };

        let stream = futures::stream::unfold(initial_state, |mut state| async move {
            if state.finished {
                return None;
            }

            if !state.sent_started {
                state.sent_started = true;
                return Some((Ok(StreamEvent::Started), state));
            }

            loop {
                // Check if buffer contains a complete line
                if let Some(newline_pos) = state.buffer.find('\n') {
                    let line = state.buffer[..newline_pos].trim().to_string();
                    state.buffer = state.buffer[newline_pos + 1..].to_string();

                    if line.is_empty() || line.starts_with(':') {
                        continue;
                    }

                    if let Some(data) = line.strip_prefix("data: ") {
                        let data = data.trim();
                        if data == "[DONE]" {
                            return None;
                        }

                        if let Ok(chunk) = serde_json::from_str::<OpenAiStreamChunk>(data) {
                            if let Some(usage) = chunk.usage {
                                return Some((
                                    Ok(StreamEvent::Usage(Usage::new(
                                        usage.prompt_tokens,
                                        usage.completion_tokens,
                                        usage.total_tokens,
                                    ))),
                                    state,
                                ));
                            }

                            if let Some(choice) = chunk.choices.into_iter().next() {
                                if let Some(delta) = choice.delta {
                                    if let Some(text) = delta.content {
                                        if !text.is_empty() {
                                            return Some((Ok(StreamEvent::Delta(text)), state));
                                        }
                                    }
                                }

                                if let Some(reason) = choice.finish_reason {
                                    let finish_reason = match reason.as_str() {
                                        "stop" => FinishReason::Stop,
                                        "length" => FinishReason::Length,
                                        "tool_calls" => FinishReason::ToolCalls,
                                        "content_filter" => FinishReason::ContentFilter,
                                        other => FinishReason::Unknown(other.to_string()),
                                    };
                                    return Some((Ok(StreamEvent::Finished(finish_reason)), state));
                                }
                            }
                        }
                    }
                    continue;
                }

                // Need more bytes from network
                match state.stream.next().await {
                    Some(Ok(bytes)) => {
                        if let Ok(text) = std::str::from_utf8(&bytes) {
                            state.buffer.push_str(text);
                        }
                    }
                    Some(Err(e)) => {
                        state.finished = true;
                        return Some((
                            Err(ProviderError::StreamError {
                                provider: state.provider_id.clone(),
                                message: e.to_string(),
                            }),
                            state,
                        ));
                    }
                    None => {
                        return None;
                    }
                }
            }
        });

        Ok(Box::pin(stream))
    }
}
