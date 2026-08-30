use thiserror::Error;

/// Error domain for Multi-Agent Orchestration and Collaborative Subagent execution.
#[derive(Debug, Error)]
pub enum AgentError {
    #[error("Execution error: {0}")]
    Execution(String),

    #[error("Task '{0}' not found in task plan")]
    TaskNotFound(String),

    #[error("Task '{task_id}' cannot start: dependency '{dependency_id}' failed")]
    DependencyFailed {
        task_id: String,
        dependency_id: String,
    },

    #[error("Task '{task_id}' timed out after {duration_ms} ms")]
    Timeout { task_id: String, duration_ms: u64 },

    #[error("Orchestration or subagent operation was cancelled: {0}")]
    Cancelled(String),

    #[error(
        "Token budget exceeded: limit is {limit} tokens, but {requested} tokens were requested"
    )]
    BudgetExceeded { limit: usize, requested: usize },

    #[error("Resource conflict: resource '{resource}' is currently locked by agent '{holder}'")]
    ResourceConflict { resource: String, holder: String },

    #[error("Maximum delegation depth {0} exceeded (hard limit is 2)")]
    MaxDelegationDepthExceeded(usize),

    #[error("Tool execution permission denied for agent '{agent_role}': {reason}")]
    PermissionDenied { agent_role: String, reason: String },

    #[error("Provider error during subagent execution: {0}")]
    Provider(String),

    #[error("Tool error during subagent execution: {0}")]
    Tool(String),

    #[error("Serialization / parsing error: {0}")]
    Serialization(String),

    #[error("Circular dependency detected among tasks: {0}")]
    CircularDependency(String),
}

impl From<serde_json::Error> for AgentError {
    fn from(err: serde_json::Error) -> Self {
        Self::Serialization(err.to_string())
    }
}

impl From<hades_provider::ProviderError> for AgentError {
    fn from(err: hades_provider::ProviderError) -> Self {
        Self::Provider(err.to_string())
    }
}

impl From<hades_tools::ToolError> for AgentError {
    fn from(err: hades_tools::ToolError) -> Self {
        Self::Tool(err.to_string())
    }
}
