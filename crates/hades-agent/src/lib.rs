pub mod budget;
pub mod conflict;
pub mod context;
pub mod decision;
pub mod definition;
pub mod error;
pub mod orchestrator;
pub mod progress;
pub mod result;
pub mod role;
pub mod strategy;
pub mod task;

pub use budget::AgentBudget;
pub use conflict::ResourceLockManager;
pub use context::{ContextSlicer, SharedTaskContext};
pub use decision::{DecisionEngine, OrchestrationDecision};
pub use definition::{AgentDefinition, AgentExecution, AgentStatus};
pub use error::AgentError;
pub use orchestrator::AgentOrchestrator;
pub use progress::{AgentLiveState, AgentProgressUpdate, OrchestrationProgress};
pub use result::AgentResult;
pub use role::AgentRole;
pub use strategy::OrchestrationStrategy;
pub use task::{Task, TaskId, TaskPlan, TaskPriority, TaskStatus};

#[cfg(test)]
mod tests {
    use super::*;
    use hades_provider::{
        CompletionRequest, CompletionResponse, Credential, FinishReason, Model, ModelCapabilities,
        Provider, ProviderError, ProviderMetadata, StreamResult, Usage,
    };
    use hades_tools::ToolRegistry;
    use std::sync::Arc;

    struct MockProvider {
        metadata: ProviderMetadata,
        response_text: String,
    }

    impl MockProvider {
        fn new(text: impl Into<String>) -> Self {
            Self {
                metadata: ProviderMetadata {
                    id: "mock-provider".to_string(),
                    name: "Mock Provider".to_string(),
                    description: "Mock testing provider".to_string(),
                    default_endpoint: None,
                    supports_dynamic_model_discovery: false,
                    requires_api_key: false,
                    is_local: true,
                },
                response_text: text.into(),
            }
        }
    }

    #[async_trait::async_trait]
    impl Provider for MockProvider {
        fn id(&self) -> &str {
            "mock-provider"
        }

        fn metadata(&self) -> &ProviderMetadata {
            &self.metadata
        }

        async fn authenticate(&self, _credential: &Credential) -> Result<(), ProviderError> {
            Ok(())
        }

        async fn list_models(&self, _credential: &Credential) -> Result<Vec<Model>, ProviderError> {
            Ok(vec![Model::new(
                "mock-model",
                "mock-provider",
                "Mock Model",
            )])
        }

        async fn get_model(
            &self,
            model_id: &str,
            _credential: &Credential,
        ) -> Result<Model, ProviderError> {
            Ok(Model::new(model_id, "mock-provider", "Mock Model"))
        }

        fn capabilities(&self, _model_id: &str) -> ModelCapabilities {
            ModelCapabilities::standard_text()
        }

        async fn complete(
            &self,
            req: CompletionRequest,
            _credential: &Credential,
        ) -> Result<CompletionResponse, ProviderError> {
            let last_user_msg = req
                .messages
                .iter()
                .rev()
                .find(|m| m.role == hades_provider::MessageRole::User)
                .map(|m| m.content.clone().unwrap_or_default())
                .unwrap_or_default();

            let content = if last_user_msg.contains("You are the primary HADES orchestrator") {
                format!(
                    "Synthesized Response: Successfully executed tasks.\n- Contributing Agents: Explorer, Implementer, Reviewer\n- Output: {}",
                    self.response_text
                )
            } else {
                format!("Subagent executed finding: {}", self.response_text)
            };

            Ok(CompletionResponse {
                id: "resp-1".to_string(),
                model: req.model,
                content,
                tool_calls: Vec::new(),
                finish_reason: Some(FinishReason::Stop),
                usage: Some(Usage::new(Some(50), Some(25), Some(75))),
            })
        }

        async fn complete_stream(
            &self,
            _req: CompletionRequest,
            _credential: &Credential,
        ) -> Result<StreamResult, ProviderError> {
            Err(ProviderError::StreamError {
                provider: "mock".to_string(),
                message: "Not implemented in mock".to_string(),
            })
        }
    }

    #[test]
    fn test_agent_roles_and_mutations() {
        assert!(AgentRole::Implementer.is_mutating_allowed());
        assert!(AgentRole::GeneralSpecialist.is_mutating_allowed());
        assert!(!AgentRole::Planner.is_mutating_allowed());
        assert!(!AgentRole::Reviewer.is_mutating_allowed());
        assert!(!AgentRole::SecurityReviewer.is_mutating_allowed());
        assert!(!AgentRole::Explorer.is_mutating_allowed());

        assert_eq!(AgentRole::Planner.name(), "Planner");
        assert_eq!(AgentRole::SecurityReviewer.name(), "Security Reviewer");
        assert!(!AgentRole::Planner.allowed_tool_patterns().is_empty());
    }

    #[test]
    fn test_task_plan_dependencies_and_lifecycle() {
        let t1 = Task::new("t1", "Explore", "Explore code", AgentRole::Explorer);
        let t2 = Task::new("t2", "Implement", "Implement code", AgentRole::Implementer)
            .with_dependency("t1");
        let t3 =
            Task::new("t3", "Review", "Review code", AgentRole::Reviewer).with_dependency("t2");

        let mut plan = TaskPlan::new("Goal", OrchestrationStrategy::Sequential, vec![t1, t2, t3]);
        assert!(plan.validate_dependencies().is_ok());

        // Initially only t1 is ready
        let ready = plan.get_ready_tasks();
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0].id, "t1");

        // Complete t1
        plan.mark_task_completed(
            "t1",
            AgentResult::success("exec-1", "t1", AgentRole::Explorer, "Found files"),
        );
        let ready = plan.get_ready_tasks();
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0].id, "t2");

        // Complete t2
        plan.mark_task_completed(
            "t2",
            AgentResult::success("exec-2", "t2", AgentRole::Implementer, "Modified files"),
        );
        let ready = plan.get_ready_tasks();
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0].id, "t3");

        // Complete t3
        plan.mark_task_completed(
            "t3",
            AgentResult::success("exec-3", "t3", AgentRole::Reviewer, "Verified clean"),
        );
        assert!(plan.is_complete());
        assert!(!plan.has_failures());
    }

    #[test]
    fn test_task_plan_circular_dependency_detection() {
        let t1 = Task::new("t1", "T1", "T1", AgentRole::Explorer).with_dependency("t2");
        let t2 = Task::new("t2", "T2", "T2", AgentRole::Implementer).with_dependency("t1");

        let plan = TaskPlan::new("Cycle", OrchestrationStrategy::Sequential, vec![t1, t2]);
        assert!(plan.validate_dependencies().is_err());
    }

    #[test]
    fn test_task_plan_failure_propagation() {
        let t1 = Task::new("t1", "Explore", "Explore code", AgentRole::Explorer);
        let t2 = Task::new("t2", "Implement", "Implement code", AgentRole::Implementer)
            .with_dependency("t1");

        let mut plan = TaskPlan::new("Goal", OrchestrationStrategy::Sequential, vec![t1, t2]);
        plan.mark_task_failed("t1", "Path not found");
        assert!(plan.is_complete());
        assert!(plan.has_failures());
        assert_eq!(plan.get_task("t2").unwrap().status, TaskStatus::Skipped);
    }

    #[tokio::test]
    async fn test_resource_lock_manager_conflict_prevention() {
        let lock_manager = ResourceLockManager::new();
        let files = vec!["src/main.rs".to_string(), "src/lib.rs".to_string()];

        // Agent A acquires locks
        assert!(lock_manager.acquire_locks("agent-a", &files).await.is_ok());

        // Agent B attempts to acquire conflicting lock -> Error
        assert!(lock_manager.acquire_locks("agent-b", &files).await.is_err());
        assert!(!lock_manager.can_acquire("agent-b", &files).await);

        // Agent A releases locks
        lock_manager.release_locks("agent-a").await;

        // Agent B now succeeds
        assert!(lock_manager.acquire_locks("agent-b", &files).await.is_ok());
    }

    #[test]
    fn test_decision_engine_evaluation() {
        // Direct
        let d1 = DecisionEngine::evaluate("What is the speed of light?", true);
        assert!(!d1.should_delegate);

        // Explicit user override: without subagents
        let d2 = DecisionEngine::evaluate("Analyze this complex project without subagents", true);
        assert!(!d2.should_delegate);

        // Explicit user override: use subagents
        let d3 =
            DecisionEngine::evaluate("Use subagents to audit and optimize database queries", true);
        assert!(d3.should_delegate);

        // Complex security audit
        let d4 = DecisionEngine::evaluate(
            "Perform security audit on dependencies and fix vulnerabilities",
            true,
        );
        assert!(d4.should_delegate);
        assert_eq!(d4.strategy, OrchestrationStrategy::PlanAndExecute);
    }

    #[test]
    fn test_agent_budget_constraints() {
        let mut budget = AgentBudget::new()
            .with_max_tokens(1000)
            .with_max_total_agents(2);

        assert!(budget.validate_agent_spawn(1).is_ok());
        budget.record_agent_spawned();

        assert!(budget.validate_agent_spawn(1).is_ok());
        budget.record_agent_spawned();

        // 3rd agent exceeds total limit
        assert!(budget.validate_agent_spawn(1).is_err());

        // Max delegation depth check
        assert!(budget.validate_agent_spawn(3).is_err());

        // Token budget tracking
        assert!(budget.record_usage("agent-1", 600).is_ok());
        assert_eq!(budget.remaining_tokens(), 400);
        assert!(budget.record_usage("agent-2", 500).is_err());
    }

    #[tokio::test]
    async fn test_full_orchestration_execution() {
        let mut orchestrator = AgentOrchestrator::new();
        let provider = Arc::new(MockProvider::new(
            "All code reviewed and formatted cleanly.",
        ));
        let tool_registry = ToolRegistry::new();
        let mut permission_engine = hades_tools::PermissionEngine::new();

        let t1 = Task::new("t1", "Explore", "Explore code", AgentRole::Explorer);
        let t2 = Task::new("t2", "Implement", "Implement code", AgentRole::Implementer)
            .with_dependency("t1");
        let mut plan = TaskPlan::new(
            "Refactor authentication module",
            OrchestrationStrategy::PlanAndExecute,
            vec![t1, t2],
        );
        let mut shared_context =
            SharedTaskContext::new("sess-1", "Refactor authentication module", ".");

        let credential = Credential::with_api_key("mock-provider", "test-key");
        let result = orchestrator
            .orchestrate(
                &mut plan,
                &mut shared_context,
                provider,
                "gpt-4o",
                &credential,
                &tool_registry,
                &mut permission_engine,
            )
            .await
            .expect("orchestration succeeds");

        assert!(result.contains("Synthesized Response"));
        assert!(plan.is_complete());
        assert!(!plan.has_failures());
    }
}
