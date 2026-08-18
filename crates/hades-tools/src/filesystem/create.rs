use std::fs;

use async_trait::async_trait;
use serde_json::json;

use crate::context::ToolContext;
use crate::definition::{RiskLevel, Tool, ToolDefinition, ToolResult};
use crate::security::path::PathSecurity;

/// Tool for creating a new file safely without overwriting existing files.
pub struct FileSystemCreateTool;

#[async_trait]
impl Tool for FileSystemCreateTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "filesystem.create",
            "Creates a new file with specified content. Fails if the file already exists.",
            json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "The destination file path to create (relative to workspace or absolute)."
                    },
                    "content": {
                        "type": "string",
                        "description": "The text content to populate in the newly created file (default: empty)."
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
                    "filesystem.create",
                    "Missing required 'path' parameter",
                )
            }
        };

        let resolved_path = match PathSecurity::resolve_path(&context.working_directory, path_str) {
            Ok(p) => p,
            Err(e) => return ToolResult::failure(call_id, "filesystem.create", e.to_string()),
        };

        if resolved_path.exists() {
            return ToolResult::failure(
                call_id,
                "filesystem.create",
                format!(
                    "File already exists at '{}'. Use filesystem.write or filesystem.edit instead.",
                    resolved_path.display()
                ),
            );
        }

        // Ensure parent directory exists
        if let Some(parent) = resolved_path.parent() {
            if !parent.exists() {
                if let Err(e) = fs::create_dir_all(parent) {
                    return ToolResult::failure(
                        call_id,
                        "filesystem.create",
                        format!(
                            "Failed to create parent directory '{}': {e}",
                            parent.display()
                        ),
                    );
                }
            }
        }

        let content = input.get("content").and_then(|v| v.as_str()).unwrap_or("");

        if let Err(e) = fs::write(&resolved_path, content) {
            return ToolResult::failure(
                call_id,
                "filesystem.create",
                format!(
                    "Failed to write new file '{}': {e}",
                    resolved_path.display()
                ),
            );
        }

        let output = format!(
            "Successfully created file '{}' ({} bytes written).",
            resolved_path.display(),
            content.len()
        );

        let mut result = ToolResult::success(call_id, "filesystem.create", output);
        result = result.with_metadata(json!({
            "path": resolved_path.display().to_string(),
            "bytes_written": content.len(),
        }));

        result
    }
}
