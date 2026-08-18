use std::collections::HashMap;
use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use tokio::io::AsyncReadExt;
use tokio::process::Command;

use crate::context::ToolContext;
use crate::error::ToolError;
use crate::security::redaction::SecretRedactor;

/// Results of a process execution with bounded output capture.
#[derive(Debug, Clone)]
pub struct ProcessOutput {
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub is_truncated: bool,
}

/// Cross-platform process executor supporting timeouts, cancellations, and bounded output capture.
pub struct ProcessExecutor;

impl ProcessExecutor {
    /// Maximum byte limit per stream (stdout/stderr) before truncation.
    pub const MAX_STREAM_BYTES: usize = 65536;

    /// Runs a structured executable command with arguments within the given context.
    pub async fn run(
        executable: &str,
        args: &[String],
        working_dir: &Path,
        env_vars: Option<&HashMap<String, String>>,
        timeout_duration: Duration,
        context: &ToolContext,
    ) -> Result<ProcessOutput, ToolError> {
        let mut cmd = Command::new(executable);
        cmd.args(args);
        cmd.current_dir(working_dir);
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        // Apply session environment overrides
        for (k, v) in &context.env_overrides {
            cmd.env(k, v);
        }

        // Apply custom call environment variables
        if let Some(envs) = env_vars {
            for (k, v) in envs {
                cmd.env(k, v);
            }
        }

        let mut child = cmd.spawn().map_err(|e| {
            ToolError::Process(format!("Failed to spawn executable '{executable}': {e}"))
        })?;

        let cancel_flag = context.is_cancelled.clone();
        let stdout_handle = child.stdout.take();
        let stderr_handle = child.stderr.take();

        let mut stdout_raw = Vec::new();
        let mut stderr_raw = Vec::new();

        // Wrap execution in cancellation watcher and output capture
        let execution_fut = async {
            tokio::select! {
                status_res = child.wait() => {
                    let status = status_res.map_err(|e| ToolError::Process(format!("Process execution error: {e}")))?;

                    if let Some(mut s) = stdout_handle {
                        let _ = s.read_to_end(&mut stdout_raw).await;
                    }
                    if let Some(mut s) = stderr_handle {
                        let _ = s.read_to_end(&mut stderr_raw).await;
                    }

                    let mut is_truncated = false;

                    // Bounded stdout
                    if stdout_raw.len() > Self::MAX_STREAM_BYTES {
                        stdout_raw.truncate(Self::MAX_STREAM_BYTES);
                        is_truncated = true;
                    }
                    let stdout_str = String::from_utf8_lossy(&stdout_raw).to_string();
                    let stdout_redacted = SecretRedactor::redact_text(&stdout_str);

                    // Bounded stderr
                    if stderr_raw.len() > Self::MAX_STREAM_BYTES {
                        stderr_raw.truncate(Self::MAX_STREAM_BYTES);
                        is_truncated = true;
                    }
                    let stderr_str = String::from_utf8_lossy(&stderr_raw).to_string();
                    let stderr_redacted = SecretRedactor::redact_text(&stderr_str);

                    Ok(ProcessOutput {
                        exit_code: status.code(),
                        stdout: stdout_redacted,
                        stderr: stderr_redacted,
                        is_truncated,
                    })
                }
                _ = async {
                    while !cancel_flag.load(std::sync::atomic::Ordering::Relaxed) {
                        tokio::time::sleep(Duration::from_millis(50)).await;
                    }
                } => {
                    let _ = child.kill().await;
                    Err(ToolError::Cancelled)
                }
            }
        };

        match tokio::time::timeout(timeout_duration, execution_fut).await {
            Ok(result) => result,
            Err(_) => {
                let _ = child.kill().await;
                Err(ToolError::TimedOut(timeout_duration.as_secs()))
            }
        }
    }
}
