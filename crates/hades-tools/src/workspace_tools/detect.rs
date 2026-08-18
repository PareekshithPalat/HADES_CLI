use async_trait::async_trait;
use serde_json::json;

use crate::context::ToolContext;
use crate::definition::{RiskLevel, Tool, ToolDefinition, ToolResult};
use crate::security::path::PathSecurity;
use crate::workspace::WorkspaceDetector;

/// Tool for discovering and detecting project root markers starting from any directory path.
pub struct WorkspaceDetectTool;

#[async_trait]
impl Tool for WorkspaceDetectTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "workspace.detect",
            "Detects project root and ecosystem markers starting from a given directory.",
            json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Optional starting directory path to search from (defaults to working directory)."
                    }
                }
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
        let start_dir = if let Some(path_str) = input.get("path").and_then(|v| v.as_str()) {
            match PathSecurity::resolve_path(&context.working_directory, path_str) {
                Ok(p) => p,
                Err(e) => return ToolResult::failure(call_id, "workspace.detect", e.to_string()),
            }
        } else {
            context.working_directory.clone()
        };

        let meta = WorkspaceDetector::detect(&start_dir);

        let output = format!(
            "Detected workspace root at '{}' ({})",
            meta.root.display(),
            meta.project_type
        );

        let mut result = ToolResult::success(call_id, "workspace.detect", output);
        result = result.with_metadata(json!({
            "root": meta.root.display().to_string(),
            "project_type": meta.project_type.to_string(),
            "has_git": meta.has_git,
            "git_branch": meta.git_branch,
        }));

        result
    }
}
