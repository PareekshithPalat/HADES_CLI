use std::fs;

use async_trait::async_trait;
use serde_json::json;

use crate::context::ToolContext;
use crate::definition::{RiskLevel, Tool, ToolDefinition, ToolResult};
use crate::security::path::PathSecurity;

/// Tool for performing structured find-and-replace edits with conflict detection.
pub struct FileSystemEditTool;

#[async_trait]
impl Tool for FileSystemEditTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "filesystem.edit",
            "Performs a precise structured edit on a file by replacing an exact old_content snippet with new_content.",
            json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "The file path to edit (relative to workspace or absolute)."
                    },
                    "old_content": {
                        "type": "string",
                        "description": "The exact existing text chunk to replace."
                    },
                    "new_content": {
                        "type": "string",
                        "description": "The replacement text to insert in place of old_content."
                    }
                },
                "required": ["path", "old_content", "new_content"]
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
                    "filesystem.edit",
                    "Missing required 'path' parameter",
                )
            }
        };

        let old_content = match input.get("old_content").and_then(|v| v.as_str()) {
            Some(c) => c,
            None => {
                return ToolResult::invalid_input(
                    call_id,
                    "filesystem.edit",
                    "Missing required 'old_content' parameter",
                )
            }
        };

        let new_content = match input.get("new_content").and_then(|v| v.as_str()) {
            Some(c) => c,
            None => {
                return ToolResult::invalid_input(
                    call_id,
                    "filesystem.edit",
                    "Missing required 'new_content' parameter",
                )
            }
        };

        let resolved_path = match PathSecurity::resolve_path(&context.working_directory, path_str) {
            Ok(p) => p,
            Err(e) => return ToolResult::failure(call_id, "filesystem.edit", e.to_string()),
        };

        if !resolved_path.exists() {
            return ToolResult::failure(
                call_id,
                "filesystem.edit",
                format!("File does not exist: {}", resolved_path.display()),
            );
        }

        if resolved_path.is_dir() {
            return ToolResult::failure(
                call_id,
                "filesystem.edit",
                format!("Cannot edit directory '{}'", resolved_path.display()),
            );
        }

        let existing_content = match fs::read_to_string(&resolved_path) {
            Ok(s) => s,
            Err(e) => {
                return ToolResult::failure(
                    call_id,
                    "filesystem.edit",
                    format!("Failed to read file '{}': {e}", resolved_path.display()),
                )
            }
        };

        // Match verification
        let matches: Vec<usize> = existing_content
            .match_indices(old_content)
            .map(|(idx, _)| idx)
            .collect();

        if matches.is_empty() {
            return ToolResult::failure(
                call_id,
                "filesystem.edit",
                format!(
                    "Edit conflict in '{}': Expected old_content snippet was not found in the file.",
                    resolved_path.display()
                ),
            );
        }

        if matches.len() > 1 {
            return ToolResult::failure(
                call_id,
                "filesystem.edit",
                format!(
                    "Edit conflict in '{}': Found {} matching occurrences of old_content. Provide more surrounding context to disambiguate.",
                    resolved_path.display(),
                    matches.len()
                ),
            );
        }

        let match_idx = matches[0];
        let mut updated_content = String::with_capacity(
            existing_content.len() + new_content.len().saturating_sub(old_content.len()),
        );
        updated_content.push_str(&existing_content[..match_idx]);
        updated_content.push_str(new_content);
        updated_content.push_str(&existing_content[match_idx + old_content.len()..]);

        // Atomic write
        let tmp_file = resolved_path.with_extension(format!("tmp.{}", uuid::Uuid::new_v4()));
        if let Err(e) = fs::write(&tmp_file, &updated_content) {
            return ToolResult::failure(
                call_id,
                "filesystem.edit",
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
                "filesystem.edit",
                format!(
                    "Failed to atomically commit edited file '{}': {e}",
                    resolved_path.display()
                ),
            );
        }

        let output = format!(
            "Successfully edited '{}'. Replaced {} bytes with {} bytes.",
            resolved_path.display(),
            old_content.len(),
            new_content.len()
        );

        let mut result = ToolResult::success(call_id, "filesystem.edit", output);
        result = result.with_metadata(json!({
            "path": resolved_path.display().to_string(),
            "bytes_removed": old_content.len(),
            "bytes_added": new_content.len(),
        }));

        result
    }
}
