use serde::{Deserialize, Serialize};

/// High-level execution strategy governing how subagents are scheduled, executed, and synthesized.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrchestrationStrategy {
    /// Primary agent handles the request directly without spawning subagents.
    Direct,
    /// Subtasks execute sequentially one after another in strict dependency order.
    Sequential,
    /// Independent subtasks execute concurrently up to the configured concurrency limit.
    Parallel,
    /// Planner agent first generates a plan, tasks execute, and primary agent synthesizes.
    PlanAndExecute,
    /// Implementation is performed, followed by independent Reviewer audit and optional refinement.
    ReviewAndRefine,
}

impl OrchestrationStrategy {
    /// Returns the human-readable display name of this strategy.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Direct => "Direct",
            Self::Sequential => "Sequential Pipeline",
            Self::Parallel => "Parallel Execution",
            Self::PlanAndExecute => "Plan & Execute",
            Self::ReviewAndRefine => "Review & Refine",
        }
    }

    /// Returns a concise description of how this strategy orchestrates execution.
    pub fn description(&self) -> &'static str {
        match self {
            Self::Direct => "Executes directly in the primary agent context without subagent delegation.",
            Self::Sequential => "Executes decomposed subtasks one after another in dependency order.",
            Self::Parallel => "Executes independent subtasks concurrently to optimize latency.",
            Self::PlanAndExecute => "Formulates an upfront plan, executes assigned subtasks, and synthesizes findings.",
            Self::ReviewAndRefine => "Executes implementation tasks, conducts an independent review, and applies refinements.",
        }
    }

    /// Returns whether this strategy delegates work to subagents.
    pub fn is_delegated(&self) -> bool {
        !matches!(self, Self::Direct)
    }
}

impl std::fmt::Display for OrchestrationStrategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}
