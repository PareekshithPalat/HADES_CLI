use std::fs;

use async_trait::async_trait;
use serde_json::json;

use crate::context::ToolContext;
use crate::definition::{RiskLevel, Tool, ToolDefinition, ToolResult};
use crate::security::path::PathSecurity;

/// Tool for reading text files with line range selection and binary content protection.
pub struct FileSystemReadTool;

#[async_trait]
impl Tool for FileSystemReadTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "filesystem.read",
            "Reads content from a specified text file with optional line-range slicing and output bounds.",
            json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "The file path to read (relative to workspace or absolute)."
                    },
                    "start_line": {
                        "type": "integer",
                        "description": "Optional 1-based start line index to begin reading from."
                    },
                    "end_line": {
                        "type": "integer",
                        "description": "Optional 1-based end line index to read up to (inclusive)."
                    },
                    "max_bytes": {
                        "type": "integer",
                        "description": "Maximum bytes to read (default: 65536, max: 262144)."
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
                    "filesystem.read",
                    "Missing required 'path' parameter",
                )
            }
        };

        let resolved_path = match PathSecurity::resolve_path(&context.working_directory, path_str) {
            Ok(p) => p,
            Err(e) => return ToolResult::failure(call_id, "filesystem.read", e.to_string()),
        };

        if !resolved_path.exists() {
            return ToolResult::failure(
                call_id,
                "filesystem.read",
                format!("File does not exist: {}", resolved_path.display()),
            );
        }

        if resolved_path.is_dir() {
            return ToolResult::failure(
                call_id,
                "filesystem.read",
                format!(
                    "Path is a directory, not a file: {}. Use filesystem.list instead.",
                    resolved_path.display()
                ),
            );
        }

        // Read raw bytes to check for binary content and enforce max_bytes
        let raw_bytes = match fs::read(&resolved_path) {
            Ok(b) => b,
            Err(e) => {
                return ToolResult::failure(
                    call_id,
                    "filesystem.read",
                    format!("Failed to read file '{}': {e}", resolved_path.display()),
                )
            }
        };

        // Binary check: scan first 1024 bytes for null byte
        let check_len = raw_bytes.len().min(1024);
        if raw_bytes[..check_len].contains(&0) {
            return ToolResult::failure(
                call_id,
                "filesystem.read",
                format!(
                    "Binary file detected at '{}'. Binary inspection is unsupported.",
                    resolved_path.display()
                ),
            );
        }

        let content_str = match String::from_utf8(raw_bytes) {
            Ok(s) => s,
            Err(e) => {
                return ToolResult::failure(
                    call_id,
                    "filesystem.read",
                    format!("File contains non-UTF8 encoding: {e}"),
                )
            }
        };

        let start_line = input
            .get("start_line")
            .and_then(|v| v.as_u64())
            .unwrap_or(1)
            .max(1) as usize;
        let end_line = input
            .get("end_line")
            .and_then(|v| v.as_u64())
            .map(|v| v as usize);
        let max_bytes = input
            .get("max_bytes")
            .and_then(|v| v.as_u64())
            .unwrap_or(65536)
            .min(262144) as usize;

        let all_lines: Vec<&str> = content_str.lines().collect();
        let total_lines = all_lines.len();

        let start_idx = (start_line - 1).min(total_lines);
        let end_idx = match end_line {
            Some(el) => el.min(total_lines),
            None => total_lines,
        };

        if start_idx >= total_lines && total_lines > 0 {
            return ToolResult::failure(
                call_id,
                "filesystem.read",
                format!("start_line ({start_line}) exceeds total lines in file ({total_lines})"),
            );
        }

        let slice = if total_lines == 0 {
            &[][..]
        } else {
            &all_lines[start_idx..end_idx.max(start_idx)]
        };

        let mut output = String::new();
        let mut byte_count = 0;
        let mut lines_included = 0;
        let mut is_truncated = false;

        for (offset, line) in slice.iter().enumerate() {
            let line_num = start_idx + offset + 1;
            let formatted = format!("{line_num:>5} | {line}\n");
            if byte_count + formatted.len() > max_bytes && lines_included > 0 {
                is_truncated = true;
                output.push_str(&format!(
                    "\n[... Output truncated at {byte_count} bytes / {} remaining lines omitted ...]\n",
                    slice.len() - lines_included
                ));
                break;
            }
            byte_count += formatted.len();
            lines_included += 1;
            output.push_str(&formatted);
        }

        let mut result = ToolResult::success(call_id, "filesystem.read", output);
        result = result
            .with_metadata(json!({
                "path": resolved_path.display().to_string(),
                "total_lines": total_lines,
                "lines_read": lines_included,
                "start_line": start_line,
                "end_line": end_idx,
                "is_truncated": is_truncated,
            }))
            .with_truncation(is_truncated);

        result
    }
}
