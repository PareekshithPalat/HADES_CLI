use std::fs;

use async_trait::async_trait;
use serde_json::json;

use crate::context::ToolContext;
use crate::definition::{RiskLevel, Tool, ToolDefinition, ToolResult};
use crate::security::path::PathSecurity;

/// Tool for deleting files or directories safely.
pub struct FileSystemDeleteTool;

#[async_trait]
impl Tool for FileSystemDeleteTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "filesystem.delete",
            "Deletes a file or directory. Recursive directory deletion requires recursive=true.",
            json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "The file or directory path to delete (relative to workspace or absolute)."
                    },
                    "recursive": {
                        "type": "boolean",
                        "description": "Whether to delete directory contents recursively (default: false)."
                    }
                },
                "required": ["path"]
            }),
            RiskLevel::High,
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
                    "filesystem.delete",
                    "Missing required 'path' parameter",
                )
            }
        };

        let recursive = input
            .get("recursive")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let resolved_path = match PathSecurity::resolve_path(&context.working_directory, path_str) {
            Ok(p) => p,
            Err(e) => return ToolResult::failure(call_id, "filesystem.delete", e.to_string()),
        };

        if !resolved_path.exists() {
            return ToolResult::failure(
                call_id,
                "filesystem.delete",
                format!("Target does not exist: {}", resolved_path.display()),
            );
        }

        let is_dir = resolved_path.is_dir();

        if is_dir {
            if recursive {
                if let Err(e) = fs::remove_dir_all(&resolved_path) {
                    return ToolResult::failure(
                        call_id,
                        "filesystem.delete",
                        format!(
                            "Failed to recursively delete directory '{}': {e}",
                            resolved_path.display()
                        ),
                    );
                }
            } else if let Err(e) = fs::remove_dir(&resolved_path) {
                return ToolResult::failure(
                    call_id,
                    "filesystem.delete",
                    format!(
                        "Failed to delete directory '{}' (it may not be empty, use recursive=true): {e}",
                        resolved_path.display()
                    ),
                );
            }
        } else if let Err(e) = fs::remove_file(&resolved_path) {
            return ToolResult::failure(
                call_id,
                "filesystem.delete",
                format!("Failed to delete file '{}': {e}", resolved_path.display()),
            );
        }

        // Verify target no longer exists
        if resolved_path.exists() {
            return ToolResult::failure(
                call_id,
                "filesystem.delete",
                format!(
                    "Verification failed: Target '{}' still exists after deletion attempt.",
                    resolved_path.display()
                ),
            );
        }

        let output = format!(
            "Successfully deleted {} '{}'.",
            if is_dir { "directory" } else { "file" },
            resolved_path.display()
        );

        let mut result = ToolResult::success(call_id, "filesystem.delete", output);
        result = result.with_metadata(json!({
            "path": resolved_path.display().to_string(),
            "was_directory": is_dir,
            "was_recursive": recursive,
        }));

        result
    }
}
