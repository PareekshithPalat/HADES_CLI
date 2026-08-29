use std::fmt;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tracing::{debug, info};

use crate::error::McpError;
use crate::protocol::{
    CallToolParams, CallToolResult, ClientCapabilities, GetPromptParams, GetPromptResult,
    ImplementationInfo, InitializeParams, InitializeResult, JsonRpcNotification, JsonRpcRequest,
    ListPromptsResult, ListResourcesResult, ListToolsResult, McpPrompt, McpResource,
    McpToolDefinition, ReadResourceParams, ReadResourceResult, ServerCapabilities,
    LATEST_PROTOCOL_VERSION,
};
use crate::transport::McpTransport;

/// Lifecycle connection state of an MCP server connection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum McpServerState {
    Configured,
    Starting,
    Connected,
    Ready,
    Disconnected,
    Failed(String),
    Stopping,
    Stopped,
}

impl fmt::Display for McpServerState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Configured => write!(f, "CONFIGURED"),
            Self::Starting => write!(f, "STARTING"),
            Self::Connected => write!(f, "CONNECTED"),
            Self::Ready => write!(f, "READY"),
            Self::Disconnected => write!(f, "DISCONNECTED"),
            Self::Failed(err) => write!(f, "FAILED: {err}"),
            Self::Stopping => write!(f, "STOPPING"),
            Self::Stopped => write!(f, "STOPPED"),
        }
    }
}

/// High-level client managing an individual MCP server connection and protocol interactions.
pub struct McpClient {
    name: String,
    transport: Arc<dyn McpTransport>,
    state: Arc<RwLock<McpServerState>>,
    server_info: Arc<RwLock<Option<ImplementationInfo>>>,
    server_capabilities: Arc<RwLock<Option<ServerCapabilities>>>,
    timeout: Duration,
}

impl McpClient {
    /// Creates a new MCP client with the provided transport.
    pub fn new(
        name: impl Into<String>,
        transport: Arc<dyn McpTransport>,
        timeout: Duration,
    ) -> Self {
        Self {
            name: name.into(),
            transport,
            state: Arc::new(RwLock::new(McpServerState::Configured)),
            server_info: Arc::new(RwLock::new(None)),
            server_capabilities: Arc::new(RwLock::new(None)),
            timeout,
        }
    }

    /// Returns the name of the MCP server.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns current connection lifecycle state.
    pub async fn state(&self) -> McpServerState {
        self.state.read().await.clone()
    }

    /// Returns server implementation metadata if initialized.
    pub async fn server_info(&self) -> Option<ImplementationInfo> {
        self.server_info.read().await.clone()
    }

    /// Returns server declared capabilities if initialized.
    pub async fn capabilities(&self) -> Option<ServerCapabilities> {
        self.server_capabilities.read().await.clone()
    }

    /// Performs MCP handshake initialization and capability negotiation.
    pub async fn initialize(&self) -> Result<InitializeResult, McpError> {
        {
            let mut state = self.state.write().await;
            *state = McpServerState::Starting;
        }

        let init_params = InitializeParams {
            protocol_version: LATEST_PROTOCOL_VERSION.to_string(),
            capabilities: ClientCapabilities::default(),
            client_info: ImplementationInfo {
                name: "hades-mcp-client".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
        };

        let req = JsonRpcRequest::new("1", "initialize", Some(serde_json::to_value(init_params)?));

        let resp = match self.transport.send_request(req, self.timeout).await {
            Ok(r) => r,
            Err(e) => {
                let mut state = self.state.write().await;
                *state = McpServerState::Failed(e.to_string());
                return Err(e);
            }
        };

        if let Some(err) = resp.error {
            let err_msg = format!("Init error [{}]: {}", err.code, err.message);
            let mut state = self.state.write().await;
            *state = McpServerState::Failed(err_msg.clone());
            return Err(McpError::JsonRpc {
                code: err.code,
                message: err.message,
            });
        }

        let result_val = resp.result.ok_or_else(|| {
            McpError::Protocol("Missing result in initialize response".to_string())
        })?;

        let init_result: InitializeResult = serde_json::from_value(result_val)?;

        // Send notifications/initialized
        let notif = JsonRpcNotification::new("notifications/initialized", None);
        let _ = self.transport.send_notification(notif).await;

        {
            let mut s_info = self.server_info.write().await;
            *s_info = Some(init_result.server_info.clone());

            let mut s_caps = self.server_capabilities.write().await;
            *s_caps = Some(init_result.capabilities.clone());

            let mut state = self.state.write().await;
            *state = McpServerState::Ready;
        }

        info!(
            server = %self.name,
            version = %init_result.protocol_version,
            server_name = %init_result.server_info.name,
            "MCP server initialized successfully"
        );

        Ok(init_result)
    }

    /// Pings the server and measures round-trip latency.
    pub async fn ping(&self) -> Result<Duration, McpError> {
        let start = Instant::now();
        let req = JsonRpcRequest::new(serde_json::Value::Null, "ping", None);
        let resp = self
            .transport
            .send_request(req, Duration::from_secs(5))
            .await?;

        if let Some(err) = resp.error {
            return Err(McpError::JsonRpc {
                code: err.code,
                message: err.message,
            });
        }

        Ok(start.elapsed())
    }

    /// Discovers all available tools on the server via `tools/list`.
    pub async fn list_tools(&self) -> Result<Vec<McpToolDefinition>, McpError> {
        let req = JsonRpcRequest::new(serde_json::Value::Null, "tools/list", None);
        let resp = self.transport.send_request(req, self.timeout).await?;

        if let Some(err) = resp.error {
            return Err(McpError::JsonRpc {
                code: err.code,
                message: err.message,
            });
        }

        let result_val = resp.result.unwrap_or(serde_json::json!({ "tools": [] }));
        let list_result: ListToolsResult = serde_json::from_value(result_val)?;

        Ok(list_result.tools)
    }

    /// Executes an MCP tool via `tools/call`.
    pub async fn call_tool(
        &self,
        name: &str,
        arguments: serde_json::Value,
    ) -> Result<CallToolResult, McpError> {
        debug!(server = %self.name, tool = %name, "Calling MCP tool");
        let params = CallToolParams {
            name: name.to_string(),
            arguments,
        };

        let req = JsonRpcRequest::new(
            serde_json::Value::Null,
            "tools/call",
            Some(serde_json::to_value(params)?),
        );

        let resp = self.transport.send_request(req, self.timeout).await?;

        if let Some(err) = resp.error {
            return Err(McpError::JsonRpc {
                code: err.code,
                message: err.message,
            });
        }

        let result_val = resp.result.unwrap_or_default();
        let tool_result: CallToolResult = serde_json::from_value(result_val)?;

        Ok(tool_result)
    }

    /// Discovers all available resources via `resources/list`.
    pub async fn list_resources(&self) -> Result<Vec<McpResource>, McpError> {
        let req = JsonRpcRequest::new(serde_json::Value::Null, "resources/list", None);
        let resp = self.transport.send_request(req, self.timeout).await?;

        if let Some(err) = resp.error {
            return Err(McpError::JsonRpc {
                code: err.code,
                message: err.message,
            });
        }

        let result_val = resp
            .result
            .unwrap_or(serde_json::json!({ "resources": [] }));
        let list_result: ListResourcesResult = serde_json::from_value(result_val)?;

        Ok(list_result.resources)
    }

    /// Reads resource contents via `resources/read`.
    pub async fn read_resource(&self, uri: &str) -> Result<ReadResourceResult, McpError> {
        let params = ReadResourceParams {
            uri: uri.to_string(),
        };

        let req = JsonRpcRequest::new(
            serde_json::Value::Null,
            "resources/read",
            Some(serde_json::to_value(params)?),
        );

        let resp = self.transport.send_request(req, self.timeout).await?;

        if let Some(err) = resp.error {
            return Err(McpError::JsonRpc {
                code: err.code,
                message: err.message,
            });
        }

        let result_val = resp.result.unwrap_or_default();
        let read_result: ReadResourceResult = serde_json::from_value(result_val)?;

        Ok(read_result)
    }

    /// Discovers all available prompts via `prompts/list`.
    pub async fn list_prompts(&self) -> Result<Vec<McpPrompt>, McpError> {
        let req = JsonRpcRequest::new(serde_json::Value::Null, "prompts/list", None);
        let resp = self.transport.send_request(req, self.timeout).await?;

        if let Some(err) = resp.error {
            return Err(McpError::JsonRpc {
                code: err.code,
                message: err.message,
            });
        }

        let result_val = resp.result.unwrap_or(serde_json::json!({ "prompts": [] }));
        let list_result: ListPromptsResult = serde_json::from_value(result_val)?;

        Ok(list_result.prompts)
    }

    /// Retrieves a rendered prompt template via `prompts/get`.
    pub async fn get_prompt(
        &self,
        name: &str,
        arguments: Option<serde_json::Value>,
    ) -> Result<GetPromptResult, McpError> {
        let params = GetPromptParams {
            name: name.to_string(),
            arguments,
        };

        let req = JsonRpcRequest::new(
            serde_json::Value::Null,
            "prompts/get",
            Some(serde_json::to_value(params)?),
        );

        let resp = self.transport.send_request(req, self.timeout).await?;

        if let Some(err) = resp.error {
            return Err(McpError::JsonRpc {
                code: err.code,
                message: err.message,
            });
        }

        let result_val = resp.result.unwrap_or_default();
        let get_result: GetPromptResult = serde_json::from_value(result_val)?;

        Ok(get_result)
    }

    /// Disconnects and shuts down the MCP server.
    pub async fn disconnect(&self) -> Result<(), McpError> {
        {
            let mut state = self.state.write().await;
            *state = McpServerState::Stopping;
        }

        let res = self.transport.close().await;

        {
            let mut state = self.state.write().await;
            *state = McpServerState::Stopped;
        }

        res
    }
}
