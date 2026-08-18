use async_trait::async_trait;
use serde_json::json;

use crate::context::ToolContext;
use crate::definition::{RiskLevel, Tool, ToolDefinition, ToolResult};

/// Tool for setting session-scoped environment variables safely.
pub struct EnvironmentSetTool;

#[async_trait]
impl Tool for EnvironmentSetTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "environment.set",
            "Sets an environment variable for the duration of the current session.",
            json!({
                "type": "object",
                "properties": {
                    "key": {
                        "type": "string",
                        "description": "The environment variable name to set."
                    },
                    "value": {
                        "type": "string",
                        "description": "The value to assign."
                    }
                },
                "required": ["key", "value"]
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
                    "environment.set",
                    "Missing required 'key' parameter",
                )
            }
        };

        let value = match input.get("value").and_then(|v| v.as_str()) {
            Some(v) => v,
            None => {
                return ToolResult::invalid_input(
                    call_id,
                    "environment.set",
                    "Missing required 'value' parameter",
                )
            }
        };

        if key.is_empty() {
            return ToolResult::invalid_input(
                call_id,
                "environment.set",
                "Environment variable key cannot be empty",
            );
        }

        let output = format!("Environment variable '{key}' set successfully for the session.");
        let mut result = ToolResult::success(call_id, "environment.set", output);
        result = result.with_metadata(json!({
            "key": key,
            "value": value,
        }));

        result
    }
}
