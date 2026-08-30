use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::result::AgentResult;
use crate::role::AgentRole;

/// Formal configuration and capability constraints defining a specialized subagent instance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentDefinition {
    /// Unique agent definition identifier.
    pub id: String,
    /// Human-readable name (e.g. "Rust Code Reviewer").
    pub name: String,
    /// Logical role assigned to this agent.
    pub role: AgentRole,
    /// Functional description of the agent's responsibilities.
    pub description: String,
    /// Specialized system prompt template.
    pub system_instruction: String,
    /// Whitelist of tool name patterns this agent may invoke (e.g. `["filesystem.read", "workspace.*"]`).
    pub tool_patterns: Vec<String>,
    /// Optional upper bound on token consumption.
    pub token_budget: Option<usize>,
    /// Execution timeout duration in seconds.
    pub timeout_secs: u64,
    /// Optional parent agent identifier if spawned through delegation.
    pub parent_agent: Option<String>,
    /// Current recursion delegation depth (0 = primary orchestrator, 1 = subagent, 2 = nested subagent).
    pub delegation_depth: usize,
}

impl AgentDefinition {
    /// Creates a new `AgentDefinition` from a given role and delegation depth.
    pub fn for_role(role: AgentRole, delegation_depth: usize) -> Self {
        let name = role.name().to_string();
        let description = role.description().to_string();
        let system_instruction = role.system_instruction().to_string();
        let tool_patterns = role
            .allowed_tool_patterns()
            .into_iter()
            .map(String::from)
            .collect();
        let timeout_secs = role.default_timeout_secs();

        Self {
            id: format!(
                "{}-{}",
                role.name().to_lowercase().replace(' ', "-"),
                uuid::Uuid::new_v4()
            ),
            name,
            role,
            description,
            system_instruction,
            tool_patterns,
            token_budget: None,
            timeout_secs,
            parent_agent: None,
            delegation_depth,
        }
    }

    /// Checks if a tool call with the given name is permitted under this agent's tool whitelist.
    pub fn is_tool_allowed(&self, tool_name: &str) -> bool {
        for pattern in &self.tool_patterns {
            if pattern == "*" {
                return true;
            }
            if let Some(prefix) = pattern.strip_suffix(".*") {
                if tool_name.starts_with(prefix) {
                    return true;
                }
            } else if pattern == tool_name {
                return true;
            }
        }
        false
    }
}

/// Operational lifecycle state of an active subagent execution run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

impl std::fmt::Display for AgentStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending => write!(f, "Pending"),
            Self::Running => write!(f, "Running"),
            Self::Completed => write!(f, "Completed"),
            Self::Failed => write!(f, "Failed"),
            Self::Cancelled => write!(f, "Cancelled"),
        }
    }
}

/// Historical and telemetry record representing a concrete subagent execution pass.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentExecution {
    /// Unique execution identifier.
    pub execution_id: String,
    /// Identifier of the executing agent definition.
    pub agent_id: String,
    /// Task assigned for execution.
    pub task_id: String,
    /// Current lifecycle status.
    pub status: AgentStatus,
    /// UTC timestamp when execution began.
    pub started_at: Option<DateTime<Utc>>,
    /// UTC timestamp when execution finished.
    pub completed_at: Option<DateTime<Utc>>,
    /// Final structured result output if successful.
    pub result: Option<AgentResult>,
    /// Error message if execution failed.
    pub error: Option<String>,
    /// Token usage accrued during execution.
    pub token_usage: Option<hades_provider::Usage>,
    /// Total number of tool invocations performed.
    pub tool_calls_count: usize,
    /// Number of retries performed for this execution.
    pub retries: usize,
}

impl AgentExecution {
    /// Creates a new pending `AgentExecution` instance.
    pub fn new(agent_id: impl Into<String>, task_id: impl Into<String>) -> Self {
        Self {
            execution_id: uuid::Uuid::new_v4().to_string(),
            agent_id: agent_id.into(),
            task_id: task_id.into(),
            status: AgentStatus::Pending,
            started_at: None,
            completed_at: None,
            result: None,
            error: None,
            token_usage: None,
            tool_calls_count: 0,
            retries: 0,
        }
    }

    /// Marks execution as started.
    pub fn mark_started(&mut self) {
        self.status = AgentStatus::Running;
        self.started_at = Some(Utc::now());
    }

    /// Marks execution as completed with a structured result.
    pub fn mark_completed(&mut self, result: AgentResult) {
        self.status = AgentStatus::Completed;
        self.completed_at = Some(Utc::now());
        self.token_usage = result.token_usage;
        self.tool_calls_count = result.tool_calls_count;
        self.result = Some(result);
    }

    /// Marks execution as failed.
    pub fn mark_failed(&mut self, error: impl Into<String>) {
        self.status = AgentStatus::Failed;
        self.completed_at = Some(Utc::now());
        self.error = Some(error.into());
    }

    /// Marks execution as cancelled.
    pub fn mark_cancelled(&mut self) {
        self.status = AgentStatus::Cancelled;
        self.completed_at = Some(Utc::now());
    }
}
