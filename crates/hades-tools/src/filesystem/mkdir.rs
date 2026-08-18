use std::fs;

use async_trait::async_trait;
use serde_json::json;

use crate::context::ToolContext;
use crate::definition::{RiskLevel, Tool, ToolDefinition, ToolResult};
use crate::security::path::PathSecurity;

/// Tool for creating directories safely.
pub struct FileSystemMkdirTool;

#[async_trait]
impl Tool for FileSystemMkdirTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "filesystem.mkdir",
            "Creates a new directory (and any necessary parent directories).",
            json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "The directory path to create (relative to workspace or absolute)."
                    },
                    "recursive": {
                        "type": "boolean",
                        "description": "Whether to create parent directories if they do not exist (default: true)."
                    }
                },
                "required": ["path"]
            }),
            RiskLevel::Low,
            true,
        )
    }

    async fn execute(
        &self,
        call_id: &str,
        input: serde_json::Value,
        context: &ToolContext,
    ) -> ToolResult {
        let path_str = match input.get("path").and_then(|v| v.as_str()) {
            Some(p) => p,
            None => {
                return ToolResult::invalid_input(
                    call_id,
                    "filesystem.mkdir",
                    "Missing required 'path' parameter",
                )
            }
        };

        let recursive = input
            .get("recursive")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let resolved_path = match PathSecurity::resolve_path(&context.working_directory, path_str) {
            Ok(p) => p,
            Err(e) => return ToolResult::failure(call_id, "filesystem.mkdir", e.to_string()),
        };

        if resolved_path.exists() {
            if resolved_path.is_dir() {
                return ToolResult::success(
                    call_id,
                    "filesystem.mkdir",
                    format!("Directory already exists at '{}'.", resolved_path.display()),
                );
            } else {
                return ToolResult::failure(
                    call_id,
                    "filesystem.mkdir",
                    format!(
                        "A file already exists at '{}'; cannot create directory.",
                        resolved_path.display()
                    ),
                );
            }
        }

        let res = if recursive {
            fs::create_dir_all(&resolved_path)
        } else {
            fs::create_dir(&resolved_path)
        };

        if let Err(e) = res {
            return ToolResult::failure(
                call_id,
                "filesystem.mkdir",
                format!(
                    "Failed to create directory '{}': {e}",
                    resolved_path.display()
                ),
            );
        }

        let output = format!(
            "Successfully created directory '{}'.",
            resolved_path.display()
        );

        let mut result = ToolResult::success(call_id, "filesystem.mkdir", output);
        result = result.with_metadata(json!({
            "path": resolved_path.display().to_string(),
        }));

        result
    }
}
