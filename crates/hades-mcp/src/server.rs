use std::path::PathBuf;

use hades_tools::{ToolContext, ToolRegistry, ToolStatus};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tracing::{debug, info};

use crate::error::McpError;
use crate::protocol::{
    CallToolParams, CallToolResult, ImplementationInfo, InitializeResult, JsonRpcError,
    JsonRpcNotification, JsonRpcRequest, JsonRpcResponse, ListToolsResult, McpContent,
    McpToolDefinition, ServerCapabilities, ToolsCapability, LATEST_PROTOCOL_VERSION,
};

/// Exposes Hades workspace and diagnostic capabilities as a standard Model Context Protocol (MCP) server over STDIO.
pub struct HadesMcpServer {
    workspace_root: PathBuf,
    tool_registry: ToolRegistry,
}

impl HadesMcpServer {
    /// Creates a new `HadesMcpServer` for the specified workspace root.
    pub fn new(workspace_root: impl Into<PathBuf>) -> Self {
        let mut reg = ToolRegistry::new();

        // Expose safe, read-only inspection tools for external MCP clients
        reg.register(hades_tools::WorkspaceInspectTool);
        reg.register(hades_tools::WorkspaceDetectTool);
        reg.register(hades_tools::FileSystemListTool);
        reg.register(hades_tools::FileSystemReadTool);
        reg.register(hades_tools::SystemInfoTool);
        reg.register(hades_tools::SystemPlatformTool);
        reg.register(hades_tools::SystemArchitectureTool);
        reg.register(hades_tools::SystemUptimeTool);
        reg.register(hades_tools::SystemRuntimeWhichTool);
        reg.register(hades_tools::SystemRuntimeVersionTool);

        Self {
            workspace_root: workspace_root.into(),
            tool_registry: reg,
        }
    }

    /// Runs the server reading requests from stdin and writing responses to stdout.
    pub async fn run_stdio(&self) -> Result<(), McpError> {
        info!("Starting Hades MCP Server mode on STDIO");
        let stdin = tokio::io::stdin();
        let mut stdout = tokio::io::stdout();

        let reader = BufReader::new(stdin);
        let mut lines = reader.lines();

        while let Ok(Some(line)) = lines.next_line().await {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            debug!(request = %trimmed, "Hades MCP server received frame");

            if let Ok(req) = serde_json::from_str::<JsonRpcRequest>(trimmed) {
                let response = self.handle_request(req).await;
                let mut out_str = serde_json::to_string(&response)?;
                out_str.push('\n');
                stdout.write_all(out_str.as_bytes()).await?;
                stdout.flush().await?;
            } else if let Ok(notif) = serde_json::from_str::<JsonRpcNotification>(trimmed) {
                debug!(method = %notif.method, "Hades MCP server received notification");
            }
        }

        info!("Hades MCP Server STDIO connection closed");
        Ok(())
    }

    /// Handles an individual JSON-RPC request and produces a valid response.
    pub async fn handle_request(&self, req: JsonRpcRequest) -> JsonRpcResponse {
        let req_id = req.id.clone();
        match req.method.as_str() {
            "initialize" => {
                let init_result = InitializeResult {
                    protocol_version: LATEST_PROTOCOL_VERSION.to_string(),
                    capabilities: ServerCapabilities {
                        tools: Some(ToolsCapability {
                            list_changed: Some(false),
                        }),
                        resources: None,
                        prompts: None,
                        logging: None,
                        experimental: None,
                    },
                    server_info: ImplementationInfo {
                        name: "hades-mcp-server".to_string(),
                        version: env!("CARGO_PKG_VERSION").to_string(),
                    },
                    instructions: Some(
                        "Hades Universal AI Agent MCP Server providing safe workspace inspection and diagnostic tools."
                            .to_string(),
                    ),
                };

                JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: req_id,
                    result: Some(serde_json::to_value(init_result).unwrap_or_default()),
                    error: None,
                }
            }
            "ping" => JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: req_id,
                result: Some(serde_json::json!({})),
                error: None,
            },
            "tools/list" => {
                let tools: Vec<McpToolDefinition> = self
                    .tool_registry
                    .list()
                    .into_iter()
                    .map(|def| McpToolDefinition {
                        name: def.name,
                        description: Some(def.description),
                        input_schema: def.parameters_schema,
                    })
                    .collect();

                let result = ListToolsResult {
                    tools,
                    next_cursor: None,
                };

                JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: req_id,
                    result: Some(serde_json::to_value(result).unwrap_or_default()),
                    error: None,
                }
            }
            "tools/call" => {
                let params: Result<CallToolParams, _> = match req.params {
                    Some(p) => serde_json::from_value(p),
                    None => Err(serde_json::Error::io(
                        std::io::ErrorKind::InvalidInput.into(),
                    )),
                };

                match params {
                    Ok(call_params) => {
                        let tool = match self.tool_registry.get(&call_params.name) {
                            Some(t) => t,
                            None => {
                                return JsonRpcResponse {
                                    jsonrpc: "2.0".to_string(),
                                    id: req_id,
                                    result: None,
                                    error: Some(JsonRpcError {
                                        code: -32601,
                                        message: format!("Tool '{}' not found", call_params.name),
                                        data: None,
                                    }),
                                };
                            }
                        };

                        let context = ToolContext::new(
                            "mcp-server",
                            &self.workspace_root,
                            &self.workspace_root,
                        );
                        let tool_result = tool
                            .execute("mcp-call", call_params.arguments, &context)
                            .await;

                        let is_error = tool_result.status != ToolStatus::Success;
                        let output_text = if !tool_result.output.is_empty() {
                            tool_result.output
                        } else {
                            tool_result.error.unwrap_or_default()
                        };

                        let call_result = CallToolResult {
                            content: vec![McpContent::Text { text: output_text }],
                            is_error: if is_error { Some(true) } else { None },
                        };

                        JsonRpcResponse {
                            jsonrpc: "2.0".to_string(),
                            id: req_id,
                            result: Some(serde_json::to_value(call_result).unwrap_or_default()),
                            error: None,
                        }
                    }
                    Err(e) => JsonRpcResponse {
                        jsonrpc: "2.0".to_string(),
                        id: req_id,
                        result: None,
                        error: Some(JsonRpcError {
                            code: -32602,
                            message: format!("Invalid params: {e}"),
                            data: None,
                        }),
                    },
                }
            }
            other => JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: req_id,
                result: None,
                error: Some(JsonRpcError {
                    code: -32601,
                    message: format!("Method '{other}' not implemented"),
                    data: None,
                }),
            },
        }
    }
}
