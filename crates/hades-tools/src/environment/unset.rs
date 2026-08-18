use async_trait::async_trait;
use serde_json::json;

use crate::context::ToolContext;
use crate::definition::{RiskLevel, Tool, ToolDefinition, ToolResult};

/// Tool for unsetting session-scoped environment variables.
pub struct EnvironmentUnsetTool;

#[async_trait]
impl Tool for EnvironmentUnsetTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "environment.unset",
            "Unsets an environment variable for the duration of the current session.",
            json!({
                "type": "object",
                "properties": {
                    "key": {
                        "type": "string",
                        "description": "The environment variable name to unset."
                    }
                },
                "required": ["key"]
            }),
            RiskLevel::Medium,
            true,
        )
    }

    async fn execute(
        &self,
        call_id: &str,
        input: serde_json::Value,
        _context: &ToolContext,
    ) -> ToolResult {
        let key = match input.get("key").and_then(|v| v.as_str()) {
            Some(k) => k.trim(),
            None => {
                return ToolResult::invalid_input(
                    call_id,
                    "environment.unset",
                    "Missing required 'key' parameter",
                )
            }
        };

        if key.is_empty() {
            return ToolResult::invalid_input(
                call_id,
                "environment.unset",
                "Environment variable key cannot be empty",
            );
        }

        let output = format!("Environment variable '{key}' unset for the session.");
        let mut result = ToolResult::success(call_id, "environment.unset", output);
        result = result.with_metadata(json!({
            "key": key,
        }));

        result
    }
}
