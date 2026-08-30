use hades_agent::{
    AgentBudget, AgentOrchestrator, AgentRole, DecisionEngine, OrchestrationStrategy,
    ResourceLockManager, SharedTaskContext, Task, TaskPlan,
};
use hades_config::ConfigService;
use hades_core::{CommandOutput, HadesApp};
use hades_events::EventBus;
use hades_provider::{
    CompletionRequest, CompletionResponse, Credential, FinishReason, MessageRole, Model,
    ModelCapabilities, Provider, ProviderError, ProviderMetadata, StreamResult, Usage,
};
use hades_storage::StorageService;
use hades_tools::ToolRegistry;
use std::sync::Arc;
use tempfile::tempdir;

struct TestProvider {
    metadata: ProviderMetadata,
}

impl TestProvider {
    fn new() -> Self {
        Self {
            metadata: ProviderMetadata {
                id: "test-provider".to_string(),
                name: "Test Provider".to_string(),
                description: "Integration test provider".to_string(),
                default_endpoint: None,
                supports_dynamic_model_discovery: false,
                requires_api_key: false,
                is_local: true,
            },
        }
    }
}

#[async_trait::async_trait]
impl Provider for TestProvider {
    fn id(&self) -> &str {
        "test-provider"
    }

    fn metadata(&self) -> &ProviderMetadata {
        &self.metadata
    }

    async fn authenticate(&self, _credential: &Credential) -> Result<(), ProviderError> {
        Ok(())
    }

    async fn list_models(&self, _credential: &Credential) -> Result<Vec<Model>, ProviderError> {
        Ok(vec![Model::new(
            "test-model",
            "test-provider",
            "Test Model",
        )])
    }

    async fn get_model(
        &self,
        model_id: &str,
        _credential: &Credential,
    ) -> Result<Model, ProviderError> {
        Ok(Model::new(model_id, "test-provider", "Test Model"))
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
            .find(|m| m.role == MessageRole::User)
            .map(|m| m.content.clone().unwrap_or_default())
            .unwrap_or_default();

        let content = if last_user_msg.contains("You are the primary HADES orchestrator") {
            "## Summary of Multi-Agent Execution\n- **Contributing Agents**: Explorer, Implementer, Reviewer\n- **Status**: Successfully refactored database access module and passed all audit checks."
                .to_string()
        } else {
            "Specialist subagent performed task: analysis completed with 0 errors.".to_string()
        };

        Ok(CompletionResponse {
            id: "test-resp-1".to_string(),
            model: req.model,
            content,
            tool_calls: Vec::new(),
            finish_reason: Some(FinishReason::Stop),
            usage: Some(Usage::new(Some(100), Some(50), Some(150))),
        })
    }

    async fn complete_stream(
        &self,
        _req: CompletionRequest,
        _credential: &Credential,
    ) -> Result<StreamResult, ProviderError> {
        Err(ProviderError::StreamError {
            provider: "test".to_string(),
            message: "Streaming not used in mock test".to_string(),
        })
    }
}

#[tokio::test]
async fn test_multi_agent_end_to_end_orchestration() {
    let tmp = tempdir().expect("tempdir");
    let mut orchestrator = AgentOrchestrator::new();
    let provider = Arc::new(TestProvider::new());
    let credential = Credential::with_api_key("test-provider", "key");
    let tool_registry = ToolRegistry::new();
    let mut permission_engine = hades_tools::PermissionEngine::new();

    let t1 = Task::new(
        "task-1-explore",
        "Codebase Layout Scan",
        "Scan workspace files and identify module dependencies",
        AgentRole::Explorer,
    );
    let t2 = Task::new(
        "task-2-implement",
        "Refactor DB Connection Pool",
        "Implement thread-safe connection pooling",
        AgentRole::Implementer,
    )
    .with_dependency("task-1-explore")
    .with_resource("src/db.rs");

    let t3 = Task::new(
        "task-3-review",
        "Peer Audit & Review",
        "Verify correctness, safety, and lack of regressions",
        AgentRole::Reviewer,
    )
    .with_dependency("task-2-implement");

    let mut plan = TaskPlan::new(
        "Refactor database module with connection pooling",
        OrchestrationStrategy::PlanAndExecute,
        vec![t1, t2, t3],
    );

    let mut shared_context = SharedTaskContext::new(
        "test-session-1",
        "Refactor database module with connection pooling",
        tmp.path(),
    );

    let synthesis = orchestrator
        .orchestrate(
            &mut plan,
            &mut shared_context,
            provider,
            "test-model",
            &credential,
            &tool_registry,
            &mut permission_engine,
        )
        .await
        .expect("orchestration succeeded");

    assert!(synthesis.contains("Contributing Agents"));
    assert!(plan.is_complete());
    assert!(!plan.has_failures());
    assert_eq!(shared_context.completed_task_summaries.len(), 3);
}

#[tokio::test]
async fn test_resource_lock_concurrency_isolation() {
    let lock_manager = ResourceLockManager::new();
    let res_a = vec!["crates/hades-agent/src/lib.rs".to_string()];
    let res_b = vec!["crates/hades-agent/src/role.rs".to_string()];

    // Subagent 1 acquires lock on lib.rs
    assert!(lock_manager.acquire_locks("agent-1", &res_a).await.is_ok());

    // Subagent 2 can concurrently acquire lock on disjoint role.rs
    assert!(lock_manager.acquire_locks("agent-2", &res_b).await.is_ok());

    // Subagent 3 tries to lock lib.rs -> Conflict Error
    assert!(lock_manager.acquire_locks("agent-3", &res_a).await.is_err());

    // Release agent-1 locks
    lock_manager.release_locks("agent-1").await;

    // Subagent 3 now succeeds
    assert!(lock_manager.acquire_locks("agent-3", &res_a).await.is_ok());
}

#[test]
fn test_decision_engine_and_plan_generation() {
    // 1. Direct prompt
    let d1 = DecisionEngine::evaluate("How do I format a date in Rust?", true);
    assert!(!d1.should_delegate);
    assert_eq!(d1.strategy, OrchestrationStrategy::Direct);

    // 2. Complex security audit prompt
    let d2 = DecisionEngine::evaluate(
        "Perform a comprehensive security review and audit all dependencies",
        true,
    );
    assert!(d2.should_delegate);
    assert_eq!(d2.strategy, OrchestrationStrategy::PlanAndExecute);

    let plan = DecisionEngine::build_plan(
        "Perform a comprehensive security review and audit all dependencies",
        &d2,
    )
    .expect("plan built");
    assert!(plan.tasks.len() >= 3);
    assert!(plan.validate_dependencies().is_ok());
}

#[test]
fn test_agent_budget_and_depth_limits() {
    let mut budget = AgentBudget::new()
        .with_max_tokens(500)
        .with_max_total_agents(3);

    // Spawn 1st agent at depth 1
    assert!(budget.validate_agent_spawn(1).is_ok());
    budget.record_agent_spawned();

    // Spawn 2nd agent at depth 1
    assert!(budget.validate_agent_spawn(1).is_ok());
    budget.record_agent_spawned();

    // Spawn 3rd agent at depth 1
    assert!(budget.validate_agent_spawn(1).is_ok());
    budget.record_agent_spawned();

    // 4th agent exceeds total count limit
    assert!(budget.validate_agent_spawn(1).is_err());

    // Depth 3 exceeds hard limit ceiling of 2
    assert!(budget.validate_agent_spawn(3).is_err());

    // Token limit
    assert!(budget.record_usage("a1", 300).is_ok());
    assert_eq!(budget.remaining_tokens(), 200);
    assert!(budget.record_usage("a2", 250).is_err());
}

#[tokio::test]
async fn test_hades_app_agents_command_inspection() {
    let tmp = tempdir().expect("tempdir");
    let cfg_service = ConfigService::with_path(tmp.path().join("config.toml"));
    let storage_service = StorageService::with_root(tmp.path().join("storage"));
    let bus = EventBus::new();

    let mut app = HadesApp::new(cfg_service, storage_service, bus);
    app.init().expect("app init");

    // 1. Execute /agents
    let out = app.execute_command("/agents").expect("execute /agents");
    if let CommandOutput::Text(txt) = out {
        assert!(txt.contains("SPECIALIST SUBAGENTS & ORCHESTRATION ROLES"));
        assert!(txt.contains("Planner"));
        assert!(txt.contains("Explorer"));
        assert!(txt.contains("Implementer"));
        assert!(txt.contains("Reviewer"));
        assert!(txt.contains("Security Reviewer"));
    } else {
        panic!("Expected CommandOutput::Text");
    }

    // 2. Execute /agents plan
    let plan_out = app
        .execute_command("/agents plan Audit security vulnerabilities and fix auth module")
        .expect("execute /agents plan");
    if let CommandOutput::Text(txt) = plan_out {
        assert!(txt.contains("MULTI-AGENT EXECUTION PLAN"));
        assert!(txt.contains("Objective:"));
        assert!(txt.contains("Strategy:"));
    } else {
        panic!("Expected CommandOutput::Text");
    }
}
