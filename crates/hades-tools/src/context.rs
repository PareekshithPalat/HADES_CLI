use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// Execution context provided to a tool during invocation.
#[derive(Debug, Clone)]
pub struct ToolContext {
    /// Active conversation session ID.
    pub session_id: String,
    /// Root directory of the active workspace.
    pub workspace_root: PathBuf,
    /// Current working directory.
    pub working_directory: PathBuf,
    /// Execution tracking ID for audit and events.
    pub execution_id: String,
    /// Max execution timeout duration.
    pub timeout: Duration,
    /// Optional cancellation flag.
    pub is_cancelled: Arc<AtomicBool>,
    /// Session-scoped environment variable overrides.
    pub env_overrides: HashMap<String, String>,
}

impl ToolContext {
    /// Creates a new tool execution context with workspace bounds.
    pub fn new(
        session_id: impl Into<String>,
        workspace_root: impl Into<PathBuf>,
        working_directory: impl Into<PathBuf>,
    ) -> Self {
        Self {
            session_id: session_id.into(),
            workspace_root: workspace_root.into(),
            working_directory: working_directory.into(),
            execution_id: uuid::Uuid::new_v4().to_string(),
            timeout: Duration::from_secs(30),
            is_cancelled: Arc::new(AtomicBool::new(false)),
            env_overrides: HashMap::new(),
        }
    }

    /// Sets custom timeout for this context.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Sets environment overrides.
    pub fn with_env_overrides(mut self, env: HashMap<String, String>) -> Self {
        self.env_overrides = env;
        self
    }

    /// Checks if cancellation has been requested.
    pub fn is_cancelled(&self) -> bool {
        self.is_cancelled.load(Ordering::Relaxed)
    }

    /// Signals cancellation to the running tool.
    pub fn cancel(&self) {
        self.is_cancelled.store(true, Ordering::Relaxed);
    }

    /// Resolves a path against the working directory or workspace root.
    pub fn resolve_path(&self, relative_or_absolute: &Path) -> PathBuf {
        if relative_or_absolute.is_absolute() {
            relative_or_absolute.to_path_buf()
        } else {
            self.working_directory.join(relative_or_absolute)
        }
    }
}
