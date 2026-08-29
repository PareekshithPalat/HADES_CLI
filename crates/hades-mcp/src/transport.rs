use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, Command};
use tokio::sync::{oneshot, Mutex, RwLock};
use tracing::{debug, info, trace};

use crate::error::McpError;
use crate::protocol::{JsonRpcNotification, JsonRpcRequest, JsonRpcResponse};

/// Core abstraction for MCP message transport.
#[async_trait]
pub trait McpTransport: Send + Sync {
    /// Sends a JSON-RPC request and awaits the corresponding response.
    async fn send_request(
        &self,
        request: JsonRpcRequest,
        timeout: Duration,
    ) -> Result<JsonRpcResponse, McpError>;

    /// Sends a fire-and-forget JSON-RPC notification.
    async fn send_notification(&self, notification: JsonRpcNotification) -> Result<(), McpError>;

    /// Returns whether the transport connection is currently active and healthy.
    fn is_alive(&self) -> bool;

    /// Gracefully closes and cleans up the transport connection.
    async fn close(&self) -> Result<(), McpError>;
}

/// Standard I/O (STDIO) process transport for local MCP servers.
pub struct StdioTransport {
    server_name: String,
    stdin_writer: Arc<Mutex<Option<ChildStdin>>>,
    pending_requests: Arc<RwLock<HashMap<String, oneshot::Sender<JsonRpcResponse>>>>,
    is_running: Arc<AtomicBool>,
    child_process: Arc<Mutex<Option<Child>>>,
    request_counter: AtomicU64,
}

impl StdioTransport {
    /// Spawns a child process and initializes the bidirectional STDIO transport.
    pub async fn spawn(
        server_name: impl Into<String>,
        command: &str,
        args: &[String],
        env: &HashMap<String, String>,
        working_dir: Option<&PathBuf>,
    ) -> Result<Self, McpError> {
        let name = server_name.into();
        info!(server = %name, cmd = %command, "Spawning STDIO MCP server process");

        let mut cmd = Command::new(command);
        cmd.args(args);
        cmd.stdin(Stdio::piped());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        if let Some(dir) = working_dir {
            cmd.current_dir(dir);
        }

        for (k, v) in env {
            cmd.env(k, v);
        }

        let mut child = cmd.spawn().map_err(|e| {
            McpError::StartupFailed(
                name.clone(),
                format!("Failed to execute command '{command}': {e}"),
            )
        })?;

        let stdin = child.stdin.take().ok_or_else(|| {
            McpError::StartupFailed(name.clone(), "Failed to capture stdin pipe".to_string())
        })?;

        let stdout = child.stdout.take().ok_or_else(|| {
            McpError::StartupFailed(name.clone(), "Failed to capture stdout pipe".to_string())
        })?;

        let stderr = child.stderr.take();

        let pending_requests: Arc<RwLock<HashMap<String, oneshot::Sender<JsonRpcResponse>>>> =
            Arc::new(RwLock::new(HashMap::new()));
        let is_running = Arc::new(AtomicBool::new(true));

        // Background reader loop for stdout
        let pending_clone = pending_requests.clone();
        let running_clone = is_running.clone();
        let name_clone = name.clone();

        tokio::spawn(async move {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();

            while let Ok(Some(line)) = lines.next_line().await {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }

                trace!(server = %name_clone, raw_line = %trimmed, "STDIO frame received");

                // Parse as JSON-RPC response
                if let Ok(response) = serde_json::from_str::<JsonRpcResponse>(trimmed) {
                    let req_id_str = match &response.id {
                        serde_json::Value::String(s) => s.clone(),
                        serde_json::Value::Number(n) => n.to_string(),
                        other => other.to_string(),
                    };

                    let mut pending = pending_clone.write().await;
                    if let Some(tx) = pending.remove(&req_id_str) {
                        let _ = tx.send(response);
                    } else {
                        debug!(server = %name_clone, id = %req_id_str, "Unmatched response ID");
                    }
                }
            }

            running_clone.store(false, Ordering::SeqCst);
            debug!(server = %name_clone, "STDIO stdout reader exited");
        });

        // Background reader loop for stderr (logs/diagnostics)
        if let Some(stderr_pipe) = stderr {
            let name_err = name.clone();
            tokio::spawn(async move {
                let reader = BufReader::new(stderr_pipe);
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    let trimmed = line.trim();
                    if !trimmed.is_empty() {
                        debug!(server = %name_err, stderr = %trimmed, "MCP server stderr");
                    }
                }
            });
        }

        Ok(Self {
            server_name: name,
            stdin_writer: Arc::new(Mutex::new(Some(stdin))),
            pending_requests,
            is_running,
            child_process: Arc::new(Mutex::new(Some(child))),
            request_counter: AtomicU64::new(1),
        })
    }

    /// Allocates an incremental request ID.
    pub fn next_request_id(&self) -> String {
        self.request_counter
            .fetch_add(1, Ordering::SeqCst)
            .to_string()
    }
}

#[async_trait]
impl McpTransport for StdioTransport {
    async fn send_request(
        &self,
        mut request: JsonRpcRequest,
        timeout_dur: Duration,
    ) -> Result<JsonRpcResponse, McpError> {
        if !self.is_alive() {
            return Err(McpError::NotConnected(self.server_name.clone()));
        }

        let id_str = match &request.id {
            serde_json::Value::Null => {
                let generated = self.next_request_id();
                request.id = serde_json::Value::String(generated.clone());
                generated
            }
            serde_json::Value::String(s) => s.clone(),
            serde_json::Value::Number(n) => n.to_string(),
            other => other.to_string(),
        };

        let (tx, rx) = oneshot::channel();
        {
            let mut pending = self.pending_requests.write().await;
            pending.insert(id_str.clone(), tx);
        }

        let mut payload = serde_json::to_string(&request)?;
        payload.push('\n');

        {
            let mut stdin_guard = self.stdin_writer.lock().await;
            if let Some(ref mut stdin) = *stdin_guard {
                if let Err(e) = stdin.write_all(payload.as_bytes()).await {
                    self.is_running.store(false, Ordering::SeqCst);
                    let mut pending = self.pending_requests.write().await;
                    pending.remove(&id_str);
                    return Err(McpError::Transport(format!("Write to stdin failed: {e}")));
                }
                let _ = stdin.flush().await;
            } else {
                let mut pending = self.pending_requests.write().await;
                pending.remove(&id_str);
                return Err(McpError::NotConnected(self.server_name.clone()));
            }
        }

        match tokio::time::timeout(timeout_dur, rx).await {
            Ok(Ok(response)) => Ok(response),
            Ok(Err(_)) => {
                let mut pending = self.pending_requests.write().await;
                pending.remove(&id_str);
                Err(McpError::ProcessTerminated(format!(
                    "Server '{}' terminated while waiting for response",
                    self.server_name
                )))
            }
            Err(_) => {
                let mut pending = self.pending_requests.write().await;
                pending.remove(&id_str);
                Err(McpError::Timeout(self.server_name.clone(), timeout_dur))
            }
        }
    }

    async fn send_notification(&self, notification: JsonRpcNotification) -> Result<(), McpError> {
        if !self.is_alive() {
            return Err(McpError::NotConnected(self.server_name.clone()));
        }

        let mut payload = serde_json::to_string(&notification)?;
        payload.push('\n');

        let mut stdin_guard = self.stdin_writer.lock().await;
        if let Some(ref mut stdin) = *stdin_guard {
            stdin
                .write_all(payload.as_bytes())
                .await
                .map_err(|e| McpError::Transport(format!("Notification write failed: {e}")))?;
            let _ = stdin.flush().await;
            Ok(())
        } else {
            Err(McpError::NotConnected(self.server_name.clone()))
        }
    }

    fn is_alive(&self) -> bool {
        self.is_running.load(Ordering::SeqCst)
    }

    async fn close(&self) -> Result<(), McpError> {
        self.is_running.store(false, Ordering::SeqCst);

        // Close stdin to signal EOF to child
        {
            let mut stdin_guard = self.stdin_writer.lock().await;
            *stdin_guard = None;
        }

        // Cleanly terminate child process
        let mut child_guard = self.child_process.lock().await;
        if let Some(mut child) = child_guard.take() {
            info!(server = %self.server_name, "Terminating STDIO MCP server process");
            let _ = child.kill().await;
            let _ = child.wait().await;
        }

        Ok(())
    }
}

impl Drop for StdioTransport {
    fn drop(&mut self) {
        self.is_running.store(false, Ordering::SeqCst);
    }
}

/// HTTP JSON-RPC 2.0 Transport for remote MCP servers.
pub struct HttpTransport {
    server_name: String,
    endpoint_url: String,
    client: reqwest::Client,
    headers: HashMap<String, String>,
    is_running: Arc<AtomicBool>,
    request_counter: AtomicU64,
}

impl HttpTransport {
    pub fn new(
        server_name: impl Into<String>,
        endpoint_url: impl Into<String>,
        headers: HashMap<String, String>,
    ) -> Self {
        Self {
            server_name: server_name.into(),
            endpoint_url: endpoint_url.into(),
            client: reqwest::Client::builder().build().unwrap_or_default(),
            headers,
            is_running: Arc::new(AtomicBool::new(true)),
            request_counter: AtomicU64::new(1),
        }
    }

    pub fn next_request_id(&self) -> String {
        self.request_counter
            .fetch_add(1, Ordering::SeqCst)
            .to_string()
    }
}

#[async_trait]
impl McpTransport for HttpTransport {
    async fn send_request(
        &self,
        mut request: JsonRpcRequest,
        timeout_dur: Duration,
    ) -> Result<JsonRpcResponse, McpError> {
        if !self.is_alive() {
            return Err(McpError::NotConnected(self.server_name.clone()));
        }

        if request.id.is_null() {
            request.id = serde_json::Value::String(self.next_request_id());
        }

        let mut req_builder = self
            .client
            .post(&self.endpoint_url)
            .timeout(timeout_dur)
            .json(&request);

        for (k, v) in &self.headers {
            req_builder = req_builder.header(k, v);
        }

        let resp = req_builder
            .send()
            .await
            .map_err(|e| McpError::Transport(format!("HTTP request failed: {e}")))?;

        if !resp.status().is_success() {
            return Err(McpError::Transport(format!(
                "HTTP server returned status {}",
                resp.status()
            )));
        }

        let json_rpc_resp: JsonRpcResponse = resp.json().await.map_err(|e| {
            McpError::Transport(format!("Failed to parse HTTP JSON-RPC response: {e}"))
        })?;

        Ok(json_rpc_resp)
    }

    async fn send_notification(&self, notification: JsonRpcNotification) -> Result<(), McpError> {
        let mut req_builder = self.client.post(&self.endpoint_url).json(&notification);

        for (k, v) in &self.headers {
            req_builder = req_builder.header(k, v);
        }

        let _ = req_builder.send().await;
        Ok(())
    }

    fn is_alive(&self) -> bool {
        self.is_running.load(Ordering::SeqCst)
    }

    async fn close(&self) -> Result<(), McpError> {
        self.is_running.store(false, Ordering::SeqCst);
        Ok(())
    }
}
