use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use hades_tools::{RiskLevel, Tool, ToolContext, ToolDefinition, ToolResult, ToolStatus};
use tracing::{debug, warn};

use crate::client::McpClient;
use crate::protocol::McpToolDefinition;

/// Adapts an external MCP tool into a first-class Hades `Tool` implementing standard execution and permission bounds.
pub struct McpToolAdapter {
    server_name: String,
    raw_tool_name: String,
    namespaced_name: String,
    definition: ToolDefinition,
    client: Arc<McpClient>,
}

impl McpToolAdapter {
    /// Constructs a new `McpToolAdapter` wrapping an MCP tool definition.
    pub fn new(
        server_name: impl Into<String>,
        mcp_tool: McpToolDefinition,
        client: Arc<McpClient>,
    ) -> Self {
        let s_name = server_name.into();
        let raw_name = mcp_tool.name.clone();
        let namespaced = format!("{s_name}.{raw_name}");

        let (risk_level, is_mutating) =
            Self::infer_risk_and_mutation(&raw_name, mcp_tool.description.as_deref());

        let description = mcp_tool
            .description
            .unwrap_or_else(|| format!("MCP Tool '{raw_name}' provided by server '{s_name}'."));

        let definition = ToolDefinition::new(
            namespaced.clone(),
            description,
            mcp_tool.input_schema,
            risk_level,
            is_mutating,
        )
        .with_timeout(Duration::from_secs(60));

        Self {
            server_name: s_name,
            raw_tool_name: raw_name,
            namespaced_name: namespaced,
            definition,
            client,
        }
    }

    /// Returns the originating MCP server name.
    pub fn server_name(&self) -> &str {
        &self.server_name
    }

    /// Returns the original unnamespaced tool name on the MCP server.
    pub fn raw_tool_name(&self) -> &str {
        &self.raw_tool_name
    }

    /// Automatically classifies safety risk and mutation behavior based on semantics and keywords.
    fn infer_risk_and_mutation(name: &str, description: Option<&str>) -> (RiskLevel, bool) {
        let text = format!("{} {}", name, description.unwrap_or("")).to_lowercase();

        // Critical destructive keywords
        if text.contains("delete")
            || text.contains("remove")
            || text.contains("drop")
            || text.contains("terminate")
            || text.contains("destroy")
            || text.contains("format")
            || text.contains("kill")
            || text.contains("purge")
        {
            return (RiskLevel::Critical, true);
        }

        // High mutating keywords
        if text.contains("create")
            || text.contains("write")
            || text.contains("update")
            || text.contains("modify")
            || text.contains("insert")
            || text.contains("post")
            || text.contains("patch")
            || text.contains("put")
            || text.contains("send")
            || text.contains("publish")
            || text.contains("exec")
        {
            return (RiskLevel::High, true);
        }

        // Read-only / inspection keywords
        if text.contains("read")
            || text.contains("get")
            || text.contains("list")
            || text.contains("search")
            || text.contains("find")
            || text.contains("inspect")
            || text.contains("fetch")
            || text.contains("query")
            || text.contains("check")
            || text.contains("count")
        {
            return (RiskLevel::Low, false);
        }

        // Default conservative baseline for external MCP tools
        (RiskLevel::Medium, false)
    }
}

#[async_trait]
impl Tool for McpToolAdapter {
    fn definition(&self) -> ToolDefinition {
        self.definition.clone()
    }

    async fn execute(
        &self,
        call_id: &str,
        input: serde_json::Value,
        _context: &ToolContext,
    ) -> ToolResult {
        debug!(
            call_id = %call_id,
            server = %self.server_name,
            tool = %self.raw_tool_name,
            "Executing MCP tool via adapter"
        );

        match self.client.call_tool(&self.raw_tool_name, input).await {
            Ok(call_result) => {
                let is_error = call_result.is_error.unwrap_or(false);
                let text_output = call_result.combined_text();

                if is_error {
                    ToolResult {
                        call_id: call_id.to_string(),
                        tool_name: self.namespaced_name.clone(),
                        status: ToolStatus::Failure,
                        output: text_output.clone(),
                        error: Some(if text_output.is_empty() {
                            "MCP tool reported execution failure".to_string()
                        } else {
                            text_output
                        }),
                        metadata: serde_json::json!({
                            "source": "mcp",
                            "server": self.server_name,
                            "raw_name": self.raw_tool_name,
                        }),
                        is_truncated: false,
                        artifact_id: None,
                    }
                } else {
                    ToolResult::success(call_id, &self.namespaced_name, text_output).with_metadata(
                        serde_json::json!({
                            "source": "mcp",
                            "server": self.server_name,
                            "raw_name": self.raw_tool_name,
                        }),
                    )
                }
            }
            Err(err) => {
                warn!(
                    call_id = %call_id,
                    server = %self.server_name,
                    tool = %self.raw_tool_name,
                    error = %err,
                    "MCP tool execution failed"
                );
                ToolResult::failure(
                    call_id,
                    &self.namespaced_name,
                    format!("MCP Server '{}' execution error: {err}", self.server_name),
                )
                .with_metadata(serde_json::json!({
                    "source": "mcp",
                    "server": self.server_name,
                    "error": err.to_string(),
                }))
            }
        }
    }
}
