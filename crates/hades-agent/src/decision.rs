use serde::{Deserialize, Serialize};

use crate::role::AgentRole;
use crate::strategy::OrchestrationStrategy;
use crate::task::{Task, TaskPlan};

/// Evaluation verdict determining whether and how to delegate a user objective to specialized subagents.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrchestrationDecision {
    /// Whether delegation to specialized subagents should occur.
    pub should_delegate: bool,
    /// Selected execution strategy.
    pub strategy: OrchestrationStrategy,
    /// Rationale behind the delegation decision.
    pub reason: String,
    /// Proposed agent roles to involve.
    pub proposed_roles: Vec<AgentRole>,
    /// Estimated relative cost profile.
    pub estimated_cost: String,
}

impl OrchestrationDecision {
    /// Constructs a direct execution decision (no subagents).
    pub fn direct(reason: impl Into<String>) -> Self {
        Self {
            should_delegate: false,
            strategy: OrchestrationStrategy::Direct,
            reason: reason.into(),
            proposed_roles: Vec::new(),
            estimated_cost: "Low (1x)".to_string(),
        }
    }

    /// Constructs a delegated orchestration decision.
    pub fn delegate(
        strategy: OrchestrationStrategy,
        reason: impl Into<String>,
        roles: Vec<AgentRole>,
        cost: impl Into<String>,
    ) -> Self {
        Self {
            should_delegate: true,
            strategy,
            reason: reason.into(),
            proposed_roles: roles,
            estimated_cost: cost.into(),
        }
    }
}

/// Adaptive decision engine determining optimal task decomposition and delegation strategies.
pub struct DecisionEngine;

impl DecisionEngine {
    /// Evaluates a user prompt and decides whether to orchestrate subagents.
    pub fn evaluate(prompt: &str, is_workspace_available: bool) -> OrchestrationDecision {
        let text = prompt.trim().to_lowercase();

        // 1. Explicit user override: No subagents requested
        if text.contains("without subagent")
            || text.contains("don't use subagent")
            || text.contains("no subagent")
            || text.contains("do this yourself")
            || text.contains("single agent")
        {
            return OrchestrationDecision::direct(
                "User explicitly requested direct single-agent execution.",
            );
        }

        // 2. Explicit user override: Subagents requested
        if text.contains("use subagent")
            || text.contains("delegate to")
            || text.contains("collaborative agent")
            || text.contains("multi-agent")
            || text.contains("use a team")
        {
            return OrchestrationDecision::delegate(
                OrchestrationStrategy::PlanAndExecute,
                "User explicitly requested multi-agent delegation.",
                vec![
                    AgentRole::Planner,
                    AgentRole::Explorer,
                    AgentRole::Implementer,
                    AgentRole::Reviewer,
                ],
                "Medium (3x)",
            );
        }

        // 3. Simple queries, conversational statements, or single-line lookups -> Direct
        if prompt.lines().count() <= 1 && prompt.len() < 80 {
            let is_complex_intent = text.contains("audit")
                || text.contains("security")
                || text.contains("refactor")
                || text.contains("investigate")
                || text.contains("implement");
            if !is_complex_intent {
                return OrchestrationDecision::direct(
                    "Simple query best handled directly by primary agent.",
                );
            }
        }

        // 4. Complex development & multi-step audit workflows
        if is_workspace_available {
            let has_security_audit = text.contains("security")
                && (text.contains("audit") || text.contains("review") || text.contains("scan"));
            let has_refactor_and_test = (text.contains("refactor") || text.contains("implement"))
                && (text.contains("test") || text.contains("verify"));
            let has_deep_investigation =
                text.contains("investigate") && text.contains("fix") && text.contains("review");

            if has_security_audit {
                return OrchestrationDecision::delegate(
                    OrchestrationStrategy::PlanAndExecute,
                    "Security audit and vulnerability remediation detected.",
                    vec![
                        AgentRole::Planner,
                        AgentRole::SecurityReviewer,
                        AgentRole::Implementer,
                        AgentRole::Reviewer,
                    ],
                    "Medium (3x)",
                );
            }

            if has_refactor_and_test {
                return OrchestrationDecision::delegate(
                    OrchestrationStrategy::ReviewAndRefine,
                    "Complex implementation and test verification workflow detected.",
                    vec![
                        AgentRole::Explorer,
                        AgentRole::Implementer,
                        AgentRole::Tester,
                        AgentRole::Reviewer,
                    ],
                    "High (4x)",
                );
            }

            if has_deep_investigation {
                return OrchestrationDecision::delegate(
                    OrchestrationStrategy::PlanAndExecute,
                    "Multi-objective bug investigation, fix, and review workflow detected.",
                    vec![
                        AgentRole::Debugger,
                        AgentRole::Implementer,
                        AgentRole::Tester,
                        AgentRole::Reviewer,
                    ],
                    "High (4x)",
                );
            }

            let has_web_testing = text.contains("browser test")
                || (text.contains("web") && text.contains("e2e"))
                || (text.contains("test") && text.contains("frontend") && text.contains("ui"));

            if has_web_testing {
                return OrchestrationDecision::delegate(
                    OrchestrationStrategy::PlanAndExecute,
                    "Web application end-to-end testing workflow detected.",
                    vec![
                        AgentRole::Planner,
                        AgentRole::BrowserAgent,
                        AgentRole::WebTestingAgent,
                        AgentRole::Tester,
                    ],
                    "Medium (3x)",
                );
            }

            if prompt.len() > 200
                && (text.contains("and then") || text.contains("also") || text.contains("finally"))
            {
                return OrchestrationDecision::delegate(
                    OrchestrationStrategy::Sequential,
                    "Multi-stage task decomposition detected in prompt.",
                    vec![
                        AgentRole::Explorer,
                        AgentRole::Implementer,
                        AgentRole::Reviewer,
                    ],
                    "Medium (3x)",
                );
            }
        }

        // Default: Direct execution
        OrchestrationDecision::direct("Standard task handled directly by primary agent.")
    }

    /// Generates a concrete `TaskPlan` based on the user prompt and decision.
    pub fn build_plan(prompt: &str, decision: &OrchestrationDecision) -> Option<TaskPlan> {
        if !decision.should_delegate {
            return None;
        }

        let mut tasks = Vec::new();
        match decision.strategy {
            OrchestrationStrategy::PlanAndExecute => {
                let t1 = Task::new(
                    "task-1-explore",
                    "Workspace & Code Exploration",
                    format!("Map project structure and discover files relevant to: {prompt}"),
                    AgentRole::Explorer,
                );
                let t2 = Task::new(
                    "task-2-implement",
                    "Implementation & Remediation",
                    format!("Execute necessary modifications and fixes for: {prompt}"),
                    AgentRole::Implementer,
                )
                .with_dependency("task-1-explore");
                let t3 = Task::new(
                    "task-3-review",
                    "Code Review & Quality Check",
                    "Verify correctness, logic, and absence of regressions in the modifications.",
                    AgentRole::Reviewer,
                )
                .with_dependency("task-2-implement");
                tasks.push(t1);
                tasks.push(t2);
                tasks.push(t3);
            }
            OrchestrationStrategy::ReviewAndRefine => {
                let t1 = Task::new(
                    "task-1-implement",
                    "Feature Implementation",
                    format!("Implement requested features or refactorings for: {prompt}"),
                    AgentRole::Implementer,
                );
                let t2 = Task::new(
                    "task-2-test",
                    "Automated Testing",
                    "Run test suites to ensure all tests pass.",
                    AgentRole::Tester,
                )
                .with_dependency("task-1-implement");
                let t3 = Task::new(
                    "task-3-review",
                    "Peer Review & Verification",
                    "Review changes against requirements and identify any edge cases.",
                    AgentRole::Reviewer,
                )
                .with_dependency("task-1-implement");
                tasks.push(t1);
                tasks.push(t2);
                tasks.push(t3);
            }
            OrchestrationStrategy::Sequential => {
                let t1 = Task::new(
                    "task-1-analyze",
                    "Analysis & Discovery",
                    format!("Analyze codebase context for: {prompt}"),
                    AgentRole::Analyst,
                );
                let t2 = Task::new(
                    "task-2-execute",
                    "Core Execution",
                    format!("Carry out core implementation of: {prompt}"),
                    AgentRole::Implementer,
                )
                .with_dependency("task-1-analyze");
                tasks.push(t1);
                tasks.push(t2);
            }
            OrchestrationStrategy::Parallel => {
                let t1 = Task::new(
                    "task-1-explore",
                    "Project Layout Scan",
                    format!("Inspect repository layout for: {prompt}"),
                    AgentRole::Explorer,
                );
                let t2 = Task::new(
                    "task-2-security",
                    "Security Scan",
                    format!("Audit security dependencies and sensitive files for: {prompt}"),
                    AgentRole::SecurityReviewer,
                );
                tasks.push(t1);
                tasks.push(t2);
            }
            OrchestrationStrategy::Direct => return None,
        }

        Some(TaskPlan::new(prompt, decision.strategy, tasks))
    }
}
