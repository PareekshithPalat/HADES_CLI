use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

use async_trait::async_trait;
use serde_json::json;

use crate::context::ToolContext;
use crate::definition::{RiskLevel, Tool, ToolDefinition, ToolResult};
use crate::security::redaction::SecretRedactor;

/// Tool locating an executable binary on the system PATH.
pub struct SystemRuntimeWhichTool;

#[async_trait]
impl Tool for SystemRuntimeWhichTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "system.runtime.which",
            "Finds the absolute path of an executable binary (e.g. 'cargo', 'python', 'java', 'node', 'git') by searching directories in PATH.",
            json!({
                "type": "object",
                "properties": {
                    "binary": {
                        "type": "string",
                        "description": "Name of the executable binary to locate (e.g. 'java', 'python3', 'rustc')"
                    }
                },
                "required": ["binary"],
                "additionalProperties": false
            }),
            RiskLevel::Safe,
            false,
        )
    }

    async fn execute(
        &self,
        call_id: &str,
        input: serde_json::Value,
        _context: &ToolContext,
    ) -> ToolResult {
        let binary_name = match input.get("binary").and_then(|v| v.as_str()) {
            Some(b) if !b.trim().is_empty() => b.trim(),
            _ => {
                return ToolResult::invalid_input(
                    call_id,
                    "system.runtime.which",
                    "Missing required parameter 'binary'",
                );
            }
        };

        match find_in_path(binary_name) {
            Some(path) => {
                let output = format!("Executable '{binary_name}' found at:\n{}", path.display());
                ToolResult::success(call_id, "system.runtime.which", output)
            }
            None => {
                let path_env = std::env::var("PATH").unwrap_or_default();
                let dir_count = std::env::split_paths(&path_env).count();
                let output = format!(
                    "Executable '{binary_name}' was NOT found in system PATH (searched {dir_count} directories).\n\
                     Ensure '{binary_name}' is installed and its containing directory is added to your PATH environment variable."
                );
                ToolResult::success(call_id, "system.runtime.which", output)
            }
        }
    }
}

/// Tool checking the installed version of a command/runtime safely.
pub struct SystemRuntimeVersionTool;

#[async_trait]
impl Tool for SystemRuntimeVersionTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "system.runtime.version",
            "Executes an installed tool or runtime with version arguments (defaults to '--version') to inspect its installed release version.",
            json!({
                "type": "object",
                "properties": {
                    "binary": {
                        "type": "string",
                        "description": "Executable name (e.g. 'java', 'python', 'cargo', 'node')"
                    },
                    "args": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Arguments to pass to retrieve version (default: ['--version'])"
                    }
                },
                "required": ["binary"],
                "additionalProperties": false
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
        let binary = match input.get("binary").and_then(|v| v.as_str()) {
            Some(b) if !b.trim().is_empty() => b.trim(),
            _ => {
                return ToolResult::invalid_input(
                    call_id,
                    "system.runtime.version",
                    "Missing required parameter 'binary'",
                );
            }
        };

        // Determine executable path
        let resolved_path = match find_in_path(binary) {
            Some(p) => p,
            None => {
                return ToolResult::failure(
                    call_id,
                    "system.runtime.version",
                    format!(
                        "Executable '{binary}' is not installed or not present on system PATH."
                    ),
                );
            }
        };

        let args: Vec<String> = input
            .get("args")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|s| s.as_str().map(|str_val| str_val.to_string()))
                    .collect()
            })
            .unwrap_or_else(|| {
                // Java special case: `java -version` vs `java --version`
                if binary.eq_ignore_ascii_case("java") {
                    vec!["-version".to_string()]
                } else {
                    vec!["--version".to_string()]
                }
            });

        // Run command with timeout
        let working_dir = context.working_directory.clone();
        let cmd_res = tokio::time::timeout(
            Duration::from_secs(5),
            tokio::task::spawn_blocking(move || {
                Command::new(&resolved_path)
                    .args(&args)
                    .current_dir(&working_dir)
                    .output()
            }),
        )
        .await;

        match cmd_res {
            Ok(Ok(Ok(output))) => {
                let stdout_text = String::from_utf8_lossy(&output.stdout);
                let stderr_text = String::from_utf8_lossy(&output.stderr);
                let combined = if !stdout_text.trim().is_empty() {
                    stdout_text.trim().to_string()
                } else if !stderr_text.trim().is_empty() {
                    // Some tools (e.g. java -version) output to stderr
                    stderr_text.trim().to_string()
                } else {
                    format!("Process exited with status {}", output.status)
                };

                let redacted = SecretRedactor::redact_text(&combined);
                ToolResult::success(
                    call_id,
                    "system.runtime.version",
                    format!("Version info for '{binary}':\n{redacted}"),
                )
            }
            Ok(Ok(Err(e))) => ToolResult::failure(
                call_id,
                "system.runtime.version",
                format!("Failed to execute '{binary}': {e}"),
            ),
            Ok(Err(join_err)) => ToolResult::failure(
                call_id,
                "system.runtime.version",
                format!("Internal error spawning process: {join_err}"),
            ),
            Err(_) => ToolResult::timed_out(
                call_id,
                "system.runtime.version",
                format!("Execution of '{binary}' timed out after 5 seconds."),
            ),
        }
    }
}

/// Helper looking up a binary name in system PATH with platform extension resolution.
pub fn find_in_path(binary: &str) -> Option<PathBuf> {
    let path_buf = PathBuf::from(binary);
    if path_buf.is_absolute() && path_buf.exists() {
        return Some(path_buf);
    }

    let path_env = std::env::var("PATH").ok()?;
    let paths = std::env::split_paths(&path_env);

    #[cfg(target_os = "windows")]
    let extensions = ["", ".exe", ".cmd", ".bat", ".ps1"];
    #[cfg(not(target_os = "windows"))]
    let extensions = [""];

    for dir in paths {
        for ext in &extensions {
            let candidate = if ext.is_empty() || binary.ends_with(ext) {
                dir.join(binary)
            } else {
                dir.join(format!("{binary}{ext}"))
            };

            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_system_runtime_which_cargo() {
        let ctx = ToolContext::new("test-session", ".", ".");
        let tool = SystemRuntimeWhichTool;

        let res = tool.execute("w1", json!({ "binary": "cargo" }), &ctx).await;
        assert_eq!(res.status, crate::definition::ToolStatus::Success);
        assert!(res.output.contains("cargo"));
    }

    #[tokio::test]
    async fn test_system_runtime_which_nonexistent() {
        let ctx = ToolContext::new("test-session", ".", ".");
        let tool = SystemRuntimeWhichTool;

        let res = tool
            .execute(
                "w2",
                json!({ "binary": "nonexistent_binary_xyz_123" }),
                &ctx,
            )
            .await;
        assert_eq!(res.status, crate::definition::ToolStatus::Success);
        assert!(res.output.contains("was NOT found in system PATH"));
    }

    #[tokio::test]
    async fn test_system_runtime_version_cargo() {
        let ctx = ToolContext::new("test-session", ".", ".");
        let tool = SystemRuntimeVersionTool;

        let res = tool.execute("v1", json!({ "binary": "cargo" }), &ctx).await;
        assert_eq!(res.status, crate::definition::ToolStatus::Success);
        assert!(res.output.contains("cargo"));
    }
}
