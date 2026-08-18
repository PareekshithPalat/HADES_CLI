use std::fs;

use async_trait::async_trait;
use serde_json::json;

use crate::context::ToolContext;
use crate::definition::{RiskLevel, Tool, ToolDefinition, ToolResult};
use crate::security::path::PathSecurity;

/// Tool for writing or overwriting a file atomically via temporary swap.
pub struct FileSystemWriteTool;

#[async_trait]
impl Tool for FileSystemWriteTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "filesystem.write",
            "Writes or overwrites a file atomically using a temporary file rename.",
            json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "The destination file path (relative to workspace or absolute)."
                    },
                    "content": {
                        "type": "string",
                        "description": "The full text content to write to the file."
                    },
                    "overwrite": {
                        "type": "boolean",
                        "description": "Whether overwriting an existing file is permitted (default: true)."
                    }
                },
                "required": ["path", "content"]
            }),
            RiskLevel::Medium,
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
                    "filesystem.write",
                    "Missing required 'path' parameter",
                )
            }
        };

        let content = match input.get("content").and_then(|v| v.as_str()) {
            Some(c) => c,
            None => {
                return ToolResult::invalid_input(
                    call_id,
                    "filesystem.write",
                    "Missing required 'content' parameter",
                )
            }
        };

        let overwrite = input
            .get("overwrite")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let resolved_path = match PathSecurity::resolve_path(&context.working_directory, path_str) {
            Ok(p) => p,
            Err(e) => return ToolResult::failure(call_id, "filesystem.write", e.to_string()),
        };

        if resolved_path.exists() && resolved_path.is_dir() {
            return ToolResult::failure(
                call_id,
                "filesystem.write",
                format!(
                    "Cannot write to directory path '{}'",
                    resolved_path.display()
                ),
            );
        }

        if resolved_path.exists() && !overwrite {
            return ToolResult::failure(
                call_id,
                "filesystem.write",
                format!(
                    "File already exists at '{}' and overwrite=false",
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
                        "filesystem.write",
                        format!(
                            "Failed to create parent directory '{}': {e}",
                            parent.display()
                        ),
                    );
                }
            }
        }

        // Atomic write via temporary file
        let tmp_file = resolved_path.with_extension(format!("tmp.{}", uuid::Uuid::new_v4()));
        if let Err(e) = fs::write(&tmp_file, content) {
            return ToolResult::failure(
                call_id,
                "filesystem.write",
                format!(
                    "Failed to write temporary file '{}': {e}",
                    tmp_file.display()
                ),
            );
        }

        if let Err(e) = fs::rename(&tmp_file, &resolved_path) {
            let _ = fs::remove_file(&tmp_file);
            return ToolResult::failure(
                call_id,
                "filesystem.write",
                format!(
                    "Failed to atomically commit file '{}': {e}",
                    resolved_path.display()
                ),
            );
        }

        let output = format!(
            "Successfully wrote {} bytes to '{}'.",
            content.len(),
            resolved_path.display()
        );

        let mut result = ToolResult::success(call_id, "filesystem.write", output);
        result = result.with_metadata(json!({
            "path": resolved_path.display().to_string(),
            "bytes_written": content.len(),
        }));

        result
    }
}
