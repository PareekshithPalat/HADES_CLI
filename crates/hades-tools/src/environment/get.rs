use async_trait::async_trait;
use serde_json::json;

use crate::context::ToolContext;
use crate::definition::{RiskLevel, Tool, ToolDefinition, ToolResult};
use crate::security::redaction::SecretRedactor;

/// Tool for safely inspecting a single environment variable with secret redaction.
pub struct EnvironmentGetTool;

#[async_trait]
impl Tool for EnvironmentGetTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "environment.get",
            "Retrieves the value of a specific environment variable with automatic secret redaction.",
            json!({
                "type": "object",
                "properties": {
                    "key": {
                        "type": "string",
                        "description": "The environment variable name (e.g. 'PATH', 'RUST_LOG')."
                    }
                },
                "required": ["key"]
            }),
            RiskLevel::Safe,
            false,
        )
    }

    async fn execute(
        &self,
        call_id: &str,
        input: serde_json::Value,
        context: &ToolContext,
    ) -> ToolResult {
        let key = match input.get("key").and_then(|v| v.as_str()) {
            Some(k) => k.trim(),
            None => {
                return ToolResult::invalid_input(
                    call_id,
                    "environment.get",
                    "Missing required 'key' parameter",
                )
            }
        };

        if key.is_empty() {
            return ToolResult::invalid_input(
                call_id,
                "environment.get",
                "Environment variable key cannot be empty",
            );
        }

        let val_opt = context
            .env_overrides
            .get(key)
            .cloned()
            .or_else(|| std::env::var(key).ok());

        match val_opt {
            Some(raw_val) => {
                let redacted = SecretRedactor::redact_env_var(key, &raw_val);
                let output = format!("{key}={redacted}");
                let mut result = ToolResult::success(call_id, "environment.get", output);
                result = result.with_metadata(json!({
                    "key": key,
                    "exists": true,
                }));
                result
            }
            None => {
                let output = format!("Environment variable '{key}' is not set.");
                let mut result = ToolResult::success(call_id, "environment.get", output);
                result = result.with_metadata(json!({
                    "key": key,
                    "exists": false,
                }));
                result
            }
        }
    }
}
