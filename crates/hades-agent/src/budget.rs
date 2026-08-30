use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::error::AgentError;

/// Runtime orchestration budget controller governing concurrency, delegation limits, and token allocations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentBudget {
    /// Maximum allowable estimated tokens for the complete orchestration request.
    pub max_total_tokens: usize,
    /// Cumulative token usage observed so far.
    pub used_tokens: usize,
    /// Maximum concurrent subagents executing at any given instant (default: 4).
    pub max_concurrent_agents: usize,
    /// Maximum total subagents that may be spawned across the entire request (default: 8).
    pub max_total_agents: usize,
    /// Maximum nested delegation depth (default: 1, hard ceiling: 2).
    pub max_delegation_depth: usize,
    /// Maximum allowed retries per failed task (default: 2).
    pub max_retries: usize,
    /// Maximum orchestration loop iterations.
    pub max_iterations: usize,
    /// Total agents spawned so far in this request.
    pub spawned_agents_count: usize,
    /// Token allocations assigned to individual subagents.
    pub allocations: HashMap<String, usize>,
}

impl Default for AgentBudget {
    fn default() -> Self {
        Self {
            max_total_tokens: 128_000,
            used_tokens: 0,
            max_concurrent_agents: 4,
            max_total_agents: 8,
            max_delegation_depth: 1,
            max_retries: 2,
            max_iterations: 15,
            spawned_agents_count: 0,
            allocations: HashMap::new(),
        }
    }
}

impl AgentBudget {
    /// Constructs a new `AgentBudget` with standard defaults.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets custom token ceiling.
    pub fn with_max_tokens(mut self, max_tokens: usize) -> Self {
        self.max_total_tokens = max_tokens;
        self
    }

    /// Sets maximum concurrency limit.
    pub fn with_max_concurrent(mut self, max_concurrent: usize) -> Self {
        self.max_concurrent_agents = max_concurrent.clamp(1, 16);
        self
    }

    /// Sets maximum total agents allowed.
    pub fn with_max_total_agents(mut self, max_total: usize) -> Self {
        self.max_total_agents = max_total.clamp(1, 32);
        self
    }

    /// Validates whether a new agent can be spawned at the given delegation depth.
    pub fn validate_agent_spawn(&self, depth: usize) -> Result<(), AgentError> {
        if depth > self.max_delegation_depth || depth > 2 {
            return Err(AgentError::MaxDelegationDepthExceeded(depth));
        }
        if self.spawned_agents_count >= self.max_total_agents {
            return Err(AgentError::Execution(format!(
                "Cannot spawn agent: maximum total agent limit ({}) reached",
                self.max_total_agents
            )));
        }
        Ok(())
    }

    /// Records the successful spawn of a subagent.
    pub fn record_agent_spawned(&mut self) {
        self.spawned_agents_count = self.spawned_agents_count.saturating_add(1);
    }

    /// Records token usage from an agent execution.
    pub fn record_usage(&mut self, agent_id: &str, tokens: usize) -> Result<(), AgentError> {
        let new_total = self.used_tokens.saturating_add(tokens);
        if new_total > self.max_total_tokens {
            return Err(AgentError::BudgetExceeded {
                limit: self.max_total_tokens,
                requested: new_total,
            });
        }
        self.used_tokens = new_total;
        *self.allocations.entry(agent_id.to_string()).or_insert(0) += tokens;
        Ok(())
    }

    /// Returns remaining available token capacity.
    pub fn remaining_tokens(&self) -> usize {
        self.max_total_tokens.saturating_sub(self.used_tokens)
    }

    /// Checks if budget capacity is nearly exhausted (e.g. < 5% remaining).
    pub fn is_exhausted(&self) -> bool {
        self.used_tokens >= self.max_total_tokens
    }
}
