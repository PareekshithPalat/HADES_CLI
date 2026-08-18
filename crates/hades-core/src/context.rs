use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{debug, warn};

use crate::error::CoreError;
use hades_provider::ChatMessage;
use hades_storage::{Message, MessageRole as StorageRole};

/// Distinguishes exact provider-reported token usage from local estimations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UsageKind {
    Exact,
    Estimated,
}

/// Token accounting and truncation diagnostic report for a constructed context.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextReport {
    /// Total number of historical messages in the session.
    pub total_messages: usize,
    /// Number of messages selected and included in the model payload.
    pub included_messages: usize,
    /// Estimated total input tokens for the constructed request.
    pub estimated_input_tokens: usize,
    /// Effective context limit for the active model.
    pub context_limit: usize,
    /// Number of tokens reserved for model output generation.
    pub output_reserve: usize,
    /// Whether older conversation history was truncated to fit context budget.
    pub was_truncated: bool,
}

/// Fast and robust heuristic token estimator.
pub struct TokenEstimator;

impl TokenEstimator {
    /// Estimates token count for a text string (~4 characters per token + word adjustment).
    pub fn estimate_tokens(text: &str) -> usize {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return 0;
        }

        // Standard token heuristic: 1 token ≈ 4 characters or ~0.75 words
        let char_count = trimmed.chars().count();
        let word_count = trimmed.split_whitespace().count();

        // Balanced estimate combining char length and word boundary density
        let by_chars = char_count.div_ceil(4);
        let by_words = (word_count * 4).div_ceil(3);

        by_chars.max(by_words).max(1)
    }

    /// Estimates total tokens for a single structured chat message including framing overhead.
    pub fn estimate_message_tokens(role: StorageRole, content: &str) -> usize {
        // Message framing overhead in chat completion payloads (role + delimiters ≈ 4 tokens)
        let framing_overhead = 4;
        let role_tokens = match role {
            StorageRole::System => 2,
            StorageRole::User => 1,
            StorageRole::Assistant => 1,
            StorageRole::Tool => 2,
            StorageRole::Error => 2,
        };
        framing_overhead + role_tokens + Self::estimate_tokens(content)
    }
}

/// Model-aware conversation context builder with budget reservation and safe truncation.
pub struct ContextManager {
    known_limits: HashMap<String, usize>,
    fallback_limit: usize,
    default_output_reserve: usize,
}

impl Default for ContextManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ContextManager {
    /// Creates a new `ContextManager` pre-populated with standard model context capabilities.
    pub fn new() -> Self {
        let mut known_limits = HashMap::new();

        // Groq / Meta models
        known_limits.insert("llama-3.3-70b-versatile".to_string(), 131_072);
        known_limits.insert("llama-3.1-8b-instant".to_string(), 131_072);
        known_limits.insert("mixtral-8x7b-32768".to_string(), 32_768);

        // OpenAI models
        known_limits.insert("gpt-4o".to_string(), 128_000);
        known_limits.insert("gpt-4o-mini".to_string(), 128_000);
        known_limits.insert("o1".to_string(), 128_000);

        // Ollama / Local models
        known_limits.insert("llama3.2".to_string(), 8_192);
        known_limits.insert("qwen2.5-coder".to_string(), 32_768);
        known_limits.insert("mistral".to_string(), 8_192);

        Self {
            known_limits,
            fallback_limit: 32_768,
            default_output_reserve: 4_096,
        }
    }

    /// Registers or overrides context window limit for a model identifier.
    pub fn register_model_limit(&mut self, model_id: impl Into<String>, limit: usize) {
        self.known_limits.insert(model_id.into(), limit);
    }

    /// Resolves the effective context limit for a model identifier, using fallback if unknown.
    pub fn resolve_context_limit(&self, model_id: &str) -> usize {
        let normalized = model_id.to_lowercase();
        for (k, v) in &self.known_limits {
            if normalized.contains(k) || k.contains(&normalized) {
                return *v;
            }
        }
        self.fallback_limit
    }

    /// Resolves output reserve budget tailored to the model's total capacity.
    pub fn resolve_output_reserve(&self, context_limit: usize) -> usize {
        // Reserve default 4096 tokens or up to 25% of total window for smaller models
        let max_reserve = context_limit / 4;
        self.default_output_reserve.min(max_reserve)
    }

    /// Builds a provider-compliant message payload from session history, preserving current prompt and system context.
    pub fn build_context(
        &self,
        history: &[Message],
        active_model: &str,
        system_prompt: Option<&str>,
        current_prompt: &str,
    ) -> Result<(Vec<ChatMessage>, ContextReport), CoreError> {
        let context_limit = self.resolve_context_limit(active_model);
        let output_reserve = self.resolve_output_reserve(context_limit);
        let max_input_budget = context_limit.saturating_sub(output_reserve);

        // 1. Calculate system prompt cost (highest priority)
        let system_tokens = system_prompt
            .map(|s| TokenEstimator::estimate_message_tokens(StorageRole::System, s))
            .unwrap_or(0);

        // 2. Calculate current user prompt cost (must always fit)
        let current_prompt_tokens =
            TokenEstimator::estimate_message_tokens(StorageRole::User, current_prompt);

        if system_tokens + current_prompt_tokens > max_input_budget {
            warn!(
                prompt_tokens = current_prompt_tokens,
                budget = max_input_budget,
                "Current prompt exceeds maximum input context budget"
            );
            return Err(CoreError::Runtime(format!(
                "Prompt exceeds model input context limit ({current_prompt_tokens} tokens > {max_input_budget} budget)."
            )));
        }

        let mut available_history_budget =
            max_input_budget.saturating_sub(system_tokens + current_prompt_tokens);

        // 3. Scan historical messages from newest to oldest
        let mut selected_history: Vec<&Message> = Vec::new();
        let mut total_history_tokens = 0;
        let mut was_truncated = false;

        for msg in history.iter().rev() {
            // Skip error records from historical model payload
            if msg.role == StorageRole::Error {
                continue;
            }

            let msg_tokens = TokenEstimator::estimate_message_tokens(msg.role, &msg.content);
            if msg_tokens <= available_history_budget {
                selected_history.push(msg);
                available_history_budget -= msg_tokens;
                total_history_tokens += msg_tokens;
            } else {
                was_truncated = true;
                debug!(
                    msg_id = %msg.id,
                    msg_tokens = msg_tokens,
                    remaining_budget = available_history_budget,
                    "Truncated older message from active context"
                );
            }
        }

        // Restore chronological order (oldest to newest)
        selected_history.reverse();

        // 4. Construct final Provider ChatMessage vector
        let mut provider_messages = Vec::new();

        if let Some(sys) = system_prompt {
            provider_messages.push(ChatMessage::system(sys));
        }

        for msg in &selected_history {
            match msg.role {
                StorageRole::System => {
                    provider_messages.push(ChatMessage::system(&msg.content));
                }
                StorageRole::User => {
                    provider_messages.push(ChatMessage::user(&msg.content));
                }
                StorageRole::Assistant => {
                    if let Some(ref tc_json) = msg.metadata.tool_calls {
                        if let Ok(tcs) =
                            serde_json::from_str::<Vec<hades_provider::ProviderToolCall>>(tc_json)
                        {
                            let content = if msg.content.is_empty() {
                                None
                            } else {
                                Some(msg.content.clone())
                            };
                            provider_messages.push(ChatMessage::assistant_with_tools(content, tcs));
                            continue;
                        }
                    }
                    provider_messages.push(ChatMessage::assistant(&msg.content));
                }
                StorageRole::Tool => {
                    let tool_call_id = msg
                        .metadata
                        .tool_call_id
                        .clone()
                        .unwrap_or_else(|| "call_0".to_string());
                    provider_messages.push(ChatMessage::tool_result(tool_call_id, &msg.content));
                }
                StorageRole::Error => continue,
            }
        }

        // Only append current_prompt if it is non-empty AND not already the trailing message in selected_history
        let last_is_current = selected_history
            .last()
            .map(|m| m.role == StorageRole::User && m.content == current_prompt)
            .unwrap_or(false);

        if !current_prompt.is_empty() && !last_is_current {
            provider_messages.push(ChatMessage::user(current_prompt));
        }

        let total_input_tokens = system_tokens + total_history_tokens + current_prompt_tokens;

        let report = ContextReport {
            total_messages: history.len()
                + if !current_prompt.is_empty() && !last_is_current {
                    1
                } else {
                    0
                },
            included_messages: provider_messages.len(),
            estimated_input_tokens: total_input_tokens,
            context_limit,
            output_reserve,
            was_truncated,
        };

        debug!(
            model = %active_model,
            tokens = total_input_tokens,
            limit = context_limit,
            included = report.included_messages,
            truncated = was_truncated,
            "Context successfully constructed"
        );

        Ok((provider_messages, report))
    }
}
