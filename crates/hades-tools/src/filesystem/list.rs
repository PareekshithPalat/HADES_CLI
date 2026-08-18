use std::fs;
use std::path::Path;

use async_trait::async_trait;
use serde_json::json;

use crate::context::ToolContext;
use crate::definition::{RiskLevel, Tool, ToolDefinition, ToolResult};
use crate::security::path::PathSecurity;

/// Tool for inspecting directory contents safely with depth and entry bounds.
pub struct FileSystemListTool;

#[async_trait]
impl Tool for FileSystemListTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "filesystem.list",
            "Lists files and directories in the specified path with bounded depth and entry limits.",
            json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "The directory path to list (relative to workspace or absolute)."
                    },
                    "recursive": {
                        "type": "boolean",
                        "description": "Whether to list subdirectories recursively (default: false)."
                    },
                    "max_depth": {
                        "type": "integer",
                        "description": "Maximum recursion depth when recursive is true (default: 2, max: 5)."
                    },
                    "show_hidden": {
                        "type": "boolean",
                        "description": "Whether to include hidden files starting with '.' (default: false)."
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum number of entries to return (default: 100, max: 500)."
                    }
                },
                "required": ["path"]
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
        let path_str = match input.get("path").and_then(|v| v.as_str()) {
            Some(p) => p,
            None => {
                return ToolResult::invalid_input(
                    call_id,
                    "filesystem.list",
                    "Missing required 'path' parameter",
                )
            }
        };

        let resolved_path = match PathSecurity::resolve_path(&context.working_directory, path_str) {
            Ok(p) => p,
            Err(e) => return ToolResult::failure(call_id, "filesystem.list", e.to_string()),
        };

        if !resolved_path.exists() {
            return ToolResult::failure(
                call_id,
                "filesystem.list",
                format!("Directory does not exist: {}", resolved_path.display()),
            );
        }

        if !resolved_path.is_dir() {
            return ToolResult::failure(
                call_id,
                "filesystem.list",
                format!("Path is not a directory: {}", resolved_path.display()),
            );
        }

        let recursive = input
            .get("recursive")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let max_depth = input
            .get("max_depth")
            .and_then(|v| v.as_u64())
            .unwrap_or(2)
            .min(5) as usize;
        let show_hidden = input
            .get("show_hidden")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let limit = input
            .get("limit")
            .and_then(|v| v.as_u64())
            .unwrap_or(100)
            .min(500) as usize;

        let mut entries = Vec::new();
        let mut truncated = false;

        Self::collect_entries(
            &resolved_path,
            &resolved_path,
            recursive,
            0,
            max_depth,
            show_hidden,
            limit,
            &mut entries,
            &mut truncated,
        );

        let mut output = format!("Directory listing for: {}\n", resolved_path.display());
        output.push_str(&format!("Found {} entries", entries.len()));
        if truncated {
            output.push_str(&format!(" (truncated at limit of {limit})"));
        }
        output.push_str(":\n\n");

        for entry in &entries {
            output.push_str(entry);
            output.push('\n');
        }

        let mut result = ToolResult::success(call_id, "filesystem.list", output);
        result = result
            .with_metadata(json!({
                "entry_count": entries.len(),
                "is_truncated": truncated,
                "path": resolved_path.display().to_string(),
            }))
            .with_truncation(truncated);

        result
    }
}

impl FileSystemListTool {
    #[allow(clippy::too_many_arguments)]
    fn collect_entries(
        base: &Path,
        current: &Path,
        recursive: bool,
        current_depth: usize,
        max_depth: usize,
        show_hidden: bool,
        limit: usize,
        entries: &mut Vec<String>,
        truncated: &mut bool,
    ) {
        if entries.len() >= limit {
            *truncated = true;
            return;
        }

        let read_dir = match fs::read_dir(current) {
            Ok(rd) => rd,
            Err(_) => return,
        };

        let mut dir_items = Vec::new();
        for item in read_dir.flatten() {
            let name = item.file_name().to_string_lossy().to_string();
            if !show_hidden && name.starts_with('.') {
                continue;
            }
            dir_items.push(item);
        }

        // Sort alphabetically
        dir_items.sort_by_key(|a| a.file_name());

        for item in dir_items {
            if entries.len() >= limit {
                *truncated = true;
                return;
            }

            let path = item.path();
            let is_dir = path.is_dir();
            let relative = path
                .strip_prefix(base)
                .unwrap_or(&path)
                .to_string_lossy()
                .to_string();

            if is_dir {
                entries.push(format!("[DIR]  {relative}/"));
                if recursive && current_depth < max_depth {
                    Self::collect_entries(
                        base,
                        &path,
                        recursive,
                        current_depth + 1,
                        max_depth,
                        show_hidden,
                        limit,
                        entries,
                        truncated,
                    );
                }
            } else {
                let size_str = match item.metadata() {
                    Ok(m) => format!("{:>8} B", m.len()),
                    Err(_) => "       ? B".to_string(),
                };
                entries.push(format!("[FILE] {size_str}  {relative}"));
            }
        }
    }
}
