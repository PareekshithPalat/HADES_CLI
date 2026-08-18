use async_trait::async_trait;
use serde_json::json;

use crate::context::ToolContext;
use crate::definition::{RiskLevel, Tool, ToolDefinition, ToolResult};
use crate::workspace::WorkspaceDetector;

/// Tool for retrieving structured metadata and overview of the active project workspace.
pub struct WorkspaceInspectTool;

#[async_trait]
impl Tool for WorkspaceInspectTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "workspace.inspect",
            "Returns structured metadata about the active workspace, project type, Git status, and directory layout.",
            json!({
                "type": "object",
                "properties": {}
            }),
            RiskLevel::Safe,
            false,
        )
    }

    async fn execute(
        &self,
        call_id: &str,
        _input: serde_json::Value,
        context: &ToolContext,
    ) -> ToolResult {
        let meta = WorkspaceDetector::detect(&context.workspace_root);

        let mut output = String::new();
        output.push_str(&format!("Workspace: {}\n", meta.name()));
        output.push_str(&format!("Root Path: {}\n", meta.root.display()));
        output.push_str(&format!(
            "Working Directory: {}\n",
            meta.current_dir.display()
        ));
        output.push_str(&format!("Project Type: {}\n", meta.project_type));

        if meta.has_git {
            let branch_str = meta.git_branch.as_deref().unwrap_or("unknown");
            output.push_str(&format!(
                "Git Repository: Initialized (branch: {branch_str})\n"
            ));
        } else {
            output.push_str("Git Repository: Not detected\n");
        }

        output.push_str(&format!(
            "Languages: {}\n",
            meta.detected_languages.join(", ")
        ));

        output.push_str("\nTop-level layout:\n");
        for entry in &meta.top_level_entries {
            output.push_str(&format!("  - {entry}\n"));
        }

        let mut result = ToolResult::success(call_id, "workspace.inspect", output);
        result = result.with_metadata(json!({
            "root": meta.root.display().to_string(),
            "project_type": meta.project_type.to_string(),
            "has_git": meta.has_git,
            "git_branch": meta.git_branch,
            "languages": meta.detected_languages,
        }));

        result
    }
}
