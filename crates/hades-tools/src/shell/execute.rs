use std::collections::HashMap;
use std::time::Duration;

use async_trait::async_trait;
use serde_json::json;

use crate::context::ToolContext;
use crate::definition::{RiskLevel, Tool, ToolDefinition, ToolResult, ToolStatus};
use crate::process::ProcessExecutor;
use crate::security::path::PathSecurity;

/// Tool for executing structured command line processes safely with timeout and bounded outputs.
pub struct ShellExecuteTool;

#[async_trait]
impl Tool for ShellExecuteTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "shell.execute",
            "Executes a command-line executable with structured arguments, timeout safety, and bounded output.",
            json!({
                "type": "object",
                "properties": {
                    "executable": {
                        "type": "string",
                        "description": "The command or binary to execute (e.g. 'cargo', 'git', 'npm', 'pytest')."
                    },
                    "arguments": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Array of command arguments (e.g. ['test', '--workspace'])."
                    },
                    "working_directory": {
                        "type": "string",
                        "description": "Optional working directory path (defaults to current workspace directory)."
                    },
                    "env": {
                        "type": "object",
                        "additionalProperties": { "type": "string" },
                        "description": "Optional environment variable overrides for this command execution."
                    },
                    "timeout_seconds": {
                        "type": "integer",
                        "description": "Maximum execution time in seconds before terminating (default: 60, max: 600)."
                    }
                },
                "required": ["executable"]
            }),
            RiskLevel::High,
            true,
        )
        .with_timeout(Duration::from_secs(60))
    }

    async fn execute(
        &self,
        call_id: &str,
        input: serde_json::Value,
        context: &ToolContext,
    ) -> ToolResult {
        let executable = match input.get("executable").and_then(|v| v.as_str()) {
            Some(e) => e.trim(),
            None => {
                return ToolResult::invalid_input(
                    call_id,
                    "shell.execute",
                    "Missing required 'executable' parameter",
                )
            }
        };

        if executable.is_empty() {
            return ToolResult::invalid_input(
                call_id,
                "shell.execute",
                "Executable name cannot be empty",
            );
        }

        let arguments: Vec<String> = input
            .get("arguments")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .map(|v| v.as_str().unwrap_or_default().to_string())
                    .collect()
            })
            .unwrap_or_default();

        let working_dir =
            if let Some(dir_str) = input.get("working_directory").and_then(|v| v.as_str()) {
                match PathSecurity::resolve_path(&context.working_directory, dir_str) {
                    Ok(p) => p,
                    Err(e) => {
                        return ToolResult::failure(
                            call_id,
                            "shell.execute",
                            format!("Invalid working directory '{dir_str}': {e}"),
                        )
                    }
                }
            } else {
                context.working_directory.clone()
            };

        if !working_dir.exists() || !working_dir.is_dir() {
            return ToolResult::failure(
                call_id,
                "shell.execute",
                format!(
                    "Working directory does not exist or is not a directory: {}",
                    working_dir.display()
                ),
            );
        }

        let custom_env: Option<HashMap<String, String>> =
            input.get("env").and_then(|v| v.as_object()).map(|obj| {
                obj.iter()
                    .map(|(k, v)| (k.clone(), v.as_str().unwrap_or_default().to_string()))
                    .collect()
            });

        let timeout_secs = input
            .get("timeout_seconds")
            .and_then(|v| v.as_u64())
            .unwrap_or(60)
            .min(600);
        let timeout_duration = Duration::from_secs(timeout_secs);

        let process_output = match ProcessExecutor::run(
            executable,
            &arguments,
            &working_dir,
            custom_env.as_ref(),
            timeout_duration,
            context,
        )
        .await
        {
            Ok(out) => out,
            Err(e) => {
                return ToolResult::failure(call_id, "shell.execute", e.to_string());
            }
        };

        let exit_code = process_output.exit_code.unwrap_or(-1);
        let is_success = exit_code == 0;

        let mut output = String::new();
        if !process_output.stdout.is_empty() {
            output.push_str(&process_output.stdout);
        }
        if !process_output.stderr.is_empty() {
            if !output.is_empty() && !output.ends_with('\n') {
                output.push('\n');
            }
            output.push_str("--- stderr ---\n");
            output.push_str(&process_output.stderr);
        }

        let status = if is_success {
            ToolStatus::Success
        } else {
            ToolStatus::Failure
        };

        let mut result = ToolResult {
            call_id: call_id.to_string(),
            tool_name: "shell.execute".to_string(),
            status,
            output,
            error: if is_success {
                None
            } else {
                Some(format!(
                    "Command exited with non-zero status code: {exit_code}"
                ))
            },
            metadata: json!({
                "executable": executable,
                "arguments": arguments,
                "exit_code": exit_code,
                "is_truncated": process_output.is_truncated,
                "working_directory": working_dir.display().to_string(),
            }),
            is_truncated: process_output.is_truncated,
            artifact_id: None,
        };

        result = result.with_truncation(process_output.is_truncated);
        result
    }
}
