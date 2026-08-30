use serde::{Deserialize, Serialize};

use crate::role::AgentRole;

/// Structured output produced by a subagent execution containing synthesized findings and telemetry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentResult {
    /// Unique execution run identifier.
    pub execution_id: String,
    /// Identifier of the task that produced this result.
    pub task_id: String,
    /// Logical role of the executing agent.
    pub agent_role: AgentRole,
    /// Execution status string (e.g. "SUCCESS", "FAILED", "PARTIAL").
    pub status: String,
    /// Concise, executive summary of findings or actions performed.
    pub summary: String,
    /// Key detailed findings, code discoveries, or review comments.
    pub detailed_findings: Vec<String>,
    /// Paths or identifiers of created/referenced artifacts.
    pub artifacts: Vec<String>,
    /// List of file paths modified or created during this execution.
    pub changed_files: Vec<String>,
    /// Shell commands or scripts run by this agent.
    pub commands_executed: Vec<String>,
    /// Test identifiers or descriptions executed.
    pub tests_run: Vec<String>,
    /// Total number of tool invocations performed.
    pub tool_calls_count: usize,
    /// Warnings or potential risks flagged during execution.
    pub warnings: Vec<String>,
    /// Error messages encountered during execution.
    pub errors: Vec<String>,
    /// Recommended next steps or follow-up tasks for subsequent agents.
    pub suggested_next_actions: Vec<String>,
    /// Token usage metrics for this execution.
    pub token_usage: Option<hades_provider::Usage>,
}

impl AgentResult {
    /// Constructs a successful `AgentResult` with standard fields.
    pub fn success(
        execution_id: impl Into<String>,
        task_id: impl Into<String>,
        agent_role: AgentRole,
        summary: impl Into<String>,
    ) -> Self {
        Self {
            execution_id: execution_id.into(),
            task_id: task_id.into(),
            agent_role,
            status: "SUCCESS".to_string(),
            summary: summary.into(),
            detailed_findings: Vec::new(),
            artifacts: Vec::new(),
            changed_files: Vec::new(),
            commands_executed: Vec::new(),
            tests_run: Vec::new(),
            tool_calls_count: 0,
            warnings: Vec::new(),
            errors: Vec::new(),
            suggested_next_actions: Vec::new(),
            token_usage: None,
        }
    }

    /// Constructs a failure `AgentResult`.
    pub fn failure(
        execution_id: impl Into<String>,
        task_id: impl Into<String>,
        agent_role: AgentRole,
        error: impl Into<String>,
    ) -> Self {
        let err_str = error.into();
        Self {
            execution_id: execution_id.into(),
            task_id: task_id.into(),
            agent_role,
            status: "FAILED".to_string(),
            summary: format!("Task failed: {err_str}"),
            detailed_findings: Vec::new(),
            artifacts: Vec::new(),
            changed_files: Vec::new(),
            commands_executed: Vec::new(),
            tests_run: Vec::new(),
            tool_calls_count: 0,
            warnings: Vec::new(),
            errors: vec![err_str],
            suggested_next_actions: Vec::new(),
            token_usage: None,
        }
    }

    /// Adds a detailed finding.
    pub fn with_finding(mut self, finding: impl Into<String>) -> Self {
        self.detailed_findings.push(finding.into());
        self
    }

    /// Adds a changed file.
    pub fn with_changed_file(mut self, path: impl Into<String>) -> Self {
        self.changed_files.push(path.into());
        self
    }

    /// Sets token usage.
    pub fn with_usage(mut self, usage: Option<hades_provider::Usage>) -> Self {
        self.token_usage = usage;
        self
    }

    /// Returns a condensed Markdown formatted report suitable for context injection into dependent agents.
    pub fn to_condensed_markdown(&self) -> String {
        let mut out = format!("### {} Result ({})\n", self.agent_role.name(), self.status);
        out.push_str(&format!("**Summary**: {}\n", self.summary));

        if !self.detailed_findings.is_empty() {
            out.push_str("**Findings**:\n");
            for f in &self.detailed_findings {
                out.push_str(&format!("- {f}\n"));
            }
        }

        if !self.changed_files.is_empty() {
            out.push_str("**Changed Files**:\n");
            for cf in &self.changed_files {
                out.push_str(&format!("- `{cf}`\n"));
            }
        }

        if !self.errors.is_empty() {
            out.push_str("**Errors**:\n");
            for e in &self.errors {
                out.push_str(&format!("- ⚠️ {e}\n"));
            }
        }

        out
    }
}
