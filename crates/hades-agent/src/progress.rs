use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::definition::AgentStatus;
use crate::role::AgentRole;
use crate::strategy::OrchestrationStrategy;

/// Real-time progress update event emitted by an executing subagent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentProgressUpdate {
    /// Active orchestration run identifier.
    pub orchestration_id: String,
    /// Logical role of the reporting agent.
    pub agent_role: AgentRole,
    /// Human-readable agent name.
    pub agent_name: String,
    /// Title of the task currently being executed.
    pub task_title: String,
    /// Current activity description (e.g. "Reading src/config.rs...").
    pub activity: String,
    /// Update timestamp in UTC.
    pub timestamp: DateTime<Utc>,
}

/// Snapshot representation of a single subagent's live execution state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentLiveState {
    pub agent_role: AgentRole,
    pub agent_name: String,
    pub task_title: String,
    pub status: AgentStatus,
    pub current_activity: Option<String>,
}

/// High-level orchestration progress aggregate for live TUI rendering and status reports.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrchestrationProgress {
    /// Active orchestration identifier.
    pub orchestration_id: String,
    /// Selected execution strategy.
    pub strategy: OrchestrationStrategy,
    /// Live state of all assigned subagents.
    pub agents: Vec<AgentLiveState>,
    /// Whether multi-agent orchestration is actively executing.
    pub is_active: bool,
    /// Final executive summary if completed.
    pub summary: Option<String>,
}

impl OrchestrationProgress {
    /// Constructs a new `OrchestrationProgress` instance.
    pub fn new(orchestration_id: impl Into<String>, strategy: OrchestrationStrategy) -> Self {
        Self {
            orchestration_id: orchestration_id.into(),
            strategy,
            agents: Vec::new(),
            is_active: true,
            summary: None,
        }
    }

    /// Registers a subagent in the progress tracker.
    pub fn register_agent(
        &mut self,
        role: AgentRole,
        name: impl Into<String>,
        task_title: impl Into<String>,
    ) {
        self.agents.push(AgentLiveState {
            agent_role: role,
            agent_name: name.into(),
            task_title: task_title.into(),
            status: AgentStatus::Pending,
            current_activity: None,
        });
    }

    /// Updates an agent's live status and activity string.
    pub fn update_agent(
        &mut self,
        role: &AgentRole,
        status: AgentStatus,
        activity: Option<String>,
    ) {
        if let Some(agent) = self.agents.iter_mut().find(|a| &a.agent_role == role) {
            agent.status = status;
            agent.current_activity = activity;
        }
    }

    /// Marks the orchestration run as finished.
    pub fn finish(&mut self, summary: impl Into<String>) {
        self.is_active = false;
        self.summary = Some(summary.into());
    }

    /// Generates a compact formatted status line suitable for TUI activity display.
    pub fn compact_display(&self) -> String {
        let running_agents: Vec<&str> = self
            .agents
            .iter()
            .filter(|a| a.status == AgentStatus::Running)
            .map(|a| a.agent_role.name())
            .collect();

        if !running_agents.is_empty() {
            format!(
                "HADES AGENTS: {} actively executing",
                running_agents.join(", ")
            )
        } else if self.is_active {
            format!("HADES: Orchestrating tasks ({})", self.strategy)
        } else {
            "HADES: Multi-agent orchestration complete".to_string()
        }
    }
}
