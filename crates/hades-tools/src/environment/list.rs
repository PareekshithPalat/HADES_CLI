use std::collections::BTreeMap;

use async_trait::async_trait;
use serde_json::json;

use crate::context::ToolContext;
use crate::definition::{RiskLevel, Tool, ToolDefinition, ToolResult};
use crate::security::redaction::SecretRedactor;

/// Tool for listing active environment variables with comprehensive secret redaction.
pub struct EnvironmentListTool;

#[async_trait]
impl Tool for EnvironmentListTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "environment.list",
            "Lists current environment variables with secret credentials and tokens automatically redacted.",
            json!({
                "type": "object",
                "properties": {
                    "prefix": {
                        "type": "string",
                        "description": "Optional prefix filter (e.g. 'RUST_', 'CARGO_', 'PATH')."
                    }
                }
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
        let prefix_filter = input
            .get("prefix")
            .and_then(|v| v.as_str())
            .map(|s| s.to_uppercase());

        let mut env_map = BTreeMap::new();

        // 1. Host environment variables
        for (k, v) in std::env::vars() {
            env_map.insert(k, v);
        }

        // 2. Session overrides
        for (k, v) in &context.env_overrides {
            env_map.insert(k.clone(), v.clone());
        }

        let mut output = String::new();
        let mut count = 0;

        for (k, v) in env_map {
            if let Some(ref p) = prefix_filter {
                if !k.to_uppercase().starts_with(p) {
                    continue;
                }
            }

            let redacted_val = SecretRedactor::redact_env_var(&k, &v);
            output.push_str(&format!("{k}={redacted_val}\n"));
            count += 1;
        }

        let header = format!("Environment variables ({} entries):\n\n", count);
        let final_output = format!("{header}{output}");

        let mut result = ToolResult::success(call_id, "environment.list", final_output);
        result = result.with_metadata(json!({
            "variable_count": count,
        }));

        result
    }
}
