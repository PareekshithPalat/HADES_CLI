use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{RwLock, Semaphore};
use tracing::{info, warn};

use crate::budget::AgentBudget;
use crate::conflict::ResourceLockManager;
use crate::context::{ContextSlicer, SharedTaskContext};
use crate::definition::{AgentDefinition, AgentStatus};
use crate::error::AgentError;
use crate::progress::OrchestrationProgress;
use crate::result::AgentResult;
use crate::task::{Task, TaskPlan};
use hades_events::{EventBus, HadesEvent};
use hades_provider::{ChatMessage, CompletionRequest, Credential, Provider};
use hades_tools::{PermissionEngine, ToolContext, ToolRegistry, ToolStatus};

/// Master coordinator managing multi-agent delegation, concurrency limits, tool execution, and result synthesis.
pub struct AgentOrchestrator {
    budget: AgentBudget,
    lock_manager: ResourceLockManager,
    event_bus: Option<EventBus>,
    active_progress: Arc<RwLock<Option<OrchestrationProgress>>>,
    is_cancelled: Arc<AtomicBool>,
}

impl Default for AgentOrchestrator {
    fn default() -> Self {
        Self::new()
    }
}

impl AgentOrchestrator {
    /// Creates a fresh `AgentOrchestrator` runtime.
    pub fn new() -> Self {
        Self {
            budget: AgentBudget::default(),
            lock_manager: ResourceLockManager::new(),
            event_bus: None,
            active_progress: Arc::new(RwLock::new(None)),
            is_cancelled: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Attaches an event bus for lifecycle telemetry.
    pub fn with_event_bus(mut self, event_bus: EventBus) -> Self {
        self.event_bus = Some(event_bus);
        self
    }

    /// Configures execution budget limits.
    pub fn with_budget(mut self, budget: AgentBudget) -> Self {
        self.budget = budget;
        self
    }

    /// Signals cancellation to all active subagents and child tasks.
    pub fn cancel(&self) {
        self.is_cancelled.store(true, Ordering::SeqCst);
    }

    /// Returns whether cancellation has been requested.
    pub fn is_cancelled(&self) -> bool {
        self.is_cancelled.load(Ordering::Relaxed)
    }

    /// Returns a clone of the current orchestration progress if active.
    pub async fn active_progress(&self) -> Option<OrchestrationProgress> {
        self.active_progress.read().await.clone()
    }

    /// Executes a complete task plan, scheduling subagents and synthesizing final results.
    #[allow(clippy::too_many_arguments)]
    pub async fn orchestrate(
        &mut self,
        plan: &mut TaskPlan,
        shared_context: &mut SharedTaskContext,
        provider: Arc<dyn Provider>,
        model_id: &str,
        credential: &Credential,
        tool_registry: &ToolRegistry,
        permission_engine: &mut PermissionEngine,
    ) -> Result<String, AgentError> {
        // 1. Initialize and publish orchestration lifecycle
        self.is_cancelled.store(false, Ordering::SeqCst);
        let orchestration_id = plan.plan_id.clone();
        let strategy = plan.strategy;

        let mut progress = OrchestrationProgress::new(&orchestration_id, strategy);
        for task in &plan.tasks {
            progress.register_agent(
                task.assigned_role.clone(),
                task.assigned_role.name(),
                &task.title,
            );
        }
        *self.active_progress.write().await = Some(progress);

        if let Some(ref bus) = self.event_bus {
            bus.publish(HadesEvent::orchestration_started(
                &shared_context.session_id,
                &orchestration_id,
                strategy.name(),
                &plan.objective,
                plan.tasks.len(),
            ));
        }

        info!(
            orchestration_id = %orchestration_id,
            strategy = %strategy,
            task_count = plan.tasks.len(),
            "Starting multi-agent orchestration execution"
        );

        let concurrency_semaphore = Arc::new(Semaphore::new(self.budget.max_concurrent_agents));

        // 2. Execution Loop: Schedule ready tasks until plan completion or failure
        while !plan.is_complete() {
            if self.is_cancelled() {
                if let Some(ref bus) = self.event_bus {
                    bus.publish(HadesEvent::orchestration_cancelled(
                        &shared_context.session_id,
                        &orchestration_id,
                        "User cancellation",
                    ));
                }
                return Err(AgentError::Cancelled(
                    "Multi-agent orchestration was cancelled".into(),
                ));
            }

            plan.update_ready_tasks();
            let ready_tasks: Vec<Task> = plan.get_ready_tasks().into_iter().cloned().collect();

            if ready_tasks.is_empty() {
                if plan.is_complete() {
                    break;
                }
                // No tasks ready but plan not complete -> potential deadlock or dependency failure
                if plan.has_failures() {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(50)).await;
                continue;
            }

            // Execute each ready task
            for task_clone in ready_tasks {
                let task_id = task_clone.id.clone();
                let agent_def = AgentDefinition::for_role(task_clone.assigned_role.clone(), 1);

                // Check and validate spawn budget
                if let Err(e) = self.budget.validate_agent_spawn(1) {
                    plan.mark_task_failed(&task_id, &e.to_string());
                    continue;
                }
                self.budget.record_agent_spawned();

                // Acquire exclusive write locks for mutating tasks
                if task_clone.is_mutating && !task_clone.affected_resources.is_empty() {
                    if let Err(e) = self
                        .lock_manager
                        .acquire_locks(&agent_def.id, &task_clone.affected_resources)
                        .await
                    {
                        warn!(task_id = %task_id, error = %e, "Resource lock conflict; delaying task execution");
                        continue;
                    }
                }

                // Update progress status
                {
                    let mut p = self.active_progress.write().await;
                    if let Some(ref mut prog) = *p {
                        prog.update_agent(
                            &task_clone.assigned_role,
                            AgentStatus::Running,
                            Some(format!("Executing: {}", task_clone.title)),
                        );
                    }
                }

                if let Some(ref bus) = self.event_bus {
                    bus.publish(HadesEvent::task_started(
                        &orchestration_id,
                        &task_id,
                        &task_clone.title,
                        task_clone.assigned_role.name(),
                    ));
                }

                let _permit = concurrency_semaphore.acquire().await.map_err(|_| {
                    AgentError::Execution("Failed to acquire concurrency semaphore".into())
                })?;

                // Execute Subagent Run
                let result = Self::run_subagent_task(
                    &agent_def,
                    &task_clone,
                    shared_context,
                    provider.clone(),
                    model_id,
                    credential,
                    tool_registry,
                    permission_engine,
                    self.is_cancelled.clone(),
                    self.event_bus.as_ref(),
                )
                .await;

                // Release locks held by agent
                self.lock_manager.release_locks(&agent_def.id).await;

                match result {
                    Ok(agent_res) => {
                        // Track token consumption in budget
                        if let Some(ref usage) = agent_res.token_usage {
                            if let Some(total) = usage.total_tokens {
                                let _ = self.budget.record_usage(&agent_def.id, total as usize);
                            }
                        }

                        shared_context.record_task_result(&task_id, &agent_res);
                        plan.mark_task_completed(&task_id, agent_res.clone());

                        {
                            let mut p = self.active_progress.write().await;
                            if let Some(ref mut prog) = *p {
                                prog.update_agent(
                                    &task_clone.assigned_role,
                                    AgentStatus::Completed,
                                    None,
                                );
                            }
                        }

                        if let Some(ref bus) = self.event_bus {
                            bus.publish(HadesEvent::task_completed(
                                &orchestration_id,
                                &task_id,
                                &task_clone.title,
                                &agent_res.status,
                            ));
                        }
                    }
                    Err(e) => {
                        let err_msg = e.to_string();
                        plan.mark_task_failed(&task_id, &err_msg);

                        {
                            let mut p = self.active_progress.write().await;
                            if let Some(ref mut prog) = *p {
                                prog.update_agent(
                                    &task_clone.assigned_role,
                                    AgentStatus::Failed,
                                    Some(err_msg.clone()),
                                );
                            }
                        }

                        if let Some(ref bus) = self.event_bus {
                            bus.publish(HadesEvent::task_failed(
                                &orchestration_id,
                                &task_id,
                                &task_clone.title,
                                &err_msg,
                            ));
                        }
                    }
                }
            }
        }

        // 3. Synthesis: Prompt Primary Agent with Summarized Subagent Results
        let final_synthesis = self
            .synthesize_results(plan, shared_context, provider, model_id, credential)
            .await?;

        // 4. Mark completion
        {
            let mut p = self.active_progress.write().await;
            if let Some(ref mut prog) = *p {
                prog.finish("All delegated subtasks completed successfully.");
            }
        }

        if let Some(ref bus) = self.event_bus {
            bus.publish(HadesEvent::orchestration_completed(
                &shared_context.session_id,
                &orchestration_id,
                "SUCCESS",
                "Orchestration completed successfully",
                Some(self.budget.used_tokens as u32),
            ));
        }

        Ok(final_synthesis)
    }

    /// Executes a single subagent run pass with tool execution capabilities.
    #[allow(clippy::too_many_arguments)]
    async fn run_subagent_task(
        agent: &AgentDefinition,
        task: &Task,
        shared_context: &SharedTaskContext,
        provider: Arc<dyn Provider>,
        model_id: &str,
        credential: &Credential,
        tool_registry: &ToolRegistry,
        permission_engine: &mut PermissionEngine,
        is_cancelled: Arc<AtomicBool>,
        _event_bus: Option<&EventBus>,
    ) -> Result<AgentResult, AgentError> {
        let messages = ContextSlicer::build_subagent_messages(agent, task, shared_context, None);
        let mut available_tools = Vec::new();

        for tool_def in tool_registry.list() {
            if agent.is_tool_allowed(&tool_def.name) {
                available_tools.push(hades_provider::ToolDefinitionPayload::function(
                    tool_def.name,
                    tool_def.description,
                    tool_def.parameters_schema,
                ));
            }
        }

        let request = CompletionRequest {
            model: model_id.to_string(),
            messages,
            tools: if available_tools.is_empty() {
                None
            } else {
                Some(available_tools)
            },
            tool_choice: None,
            temperature: Some(0.2),
            max_tokens: None,
            stream: false,
        };

        let timeout_duration = Duration::from_secs(task.timeout_secs.max(10));
        let response =
            tokio::time::timeout(timeout_duration, provider.complete(request, credential))
                .await
                .map_err(|_| AgentError::Timeout {
                    task_id: task.id.clone(),
                    duration_ms: timeout_duration.as_millis() as u64,
                })??;

        let mut result = AgentResult::success(
            uuid::Uuid::new_v4().to_string(),
            &task.id,
            agent.role.clone(),
            &response.content,
        )
        .with_usage(response.usage);

        // Handle tool calls if returned by subagent
        for call in response.tool_calls {
            if is_cancelled.load(Ordering::Relaxed) {
                return Err(AgentError::Cancelled(
                    "Subagent cancelled during tool calls".into(),
                ));
            }

            let args_val: serde_json::Value = serde_json::from_str(&call.function.arguments)
                .unwrap_or(serde_json::Value::Object(Default::default()));
            let tool_call = hades_tools::ToolCall::new(&call.id, &call.function.name, args_val);

            if let Some(tool) = tool_registry.get(&tool_call.tool_name) {
                let tool_def = tool.definition();
                let tool_context = ToolContext::new(
                    &shared_context.session_id,
                    &shared_context.workspace_root,
                    &shared_context.workspace_root,
                );

                let eval = permission_engine.evaluate(&tool_call, &tool_def, &tool_context);
                match eval {
                    hades_tools::EvaluationResult::Permitted { .. } => {
                        let tool_res = tool
                            .execute(&call.id, tool_call.arguments.clone(), &tool_context)
                            .await;
                        if tool_res.status == ToolStatus::Success {
                            result.tool_calls_count += 1;
                            result.detailed_findings.push(format!(
                                "Tool {} executed: {}",
                                tool_call.tool_name, tool_res.output
                            ));
                            if let Some(path_str) =
                                tool_call.arguments.get("path").and_then(|v| v.as_str())
                            {
                                if tool_def.is_mutating {
                                    result.changed_files.push(path_str.to_string());
                                }
                            }
                        }
                    }
                    hades_tools::EvaluationResult::RequiresApproval { summary, .. } => {
                        warn!(
                            agent = %agent.name,
                            tool = %tool_call.tool_name,
                            summary = %summary,
                            "Subagent tool call requires user authorization"
                        );
                        result.warnings.push(format!(
                            "Tool call {} deferred: {}",
                            tool_call.tool_name, summary
                        ));
                    }
                    hades_tools::EvaluationResult::Denied { reason } => {
                        return Err(AgentError::PermissionDenied {
                            agent_role: agent.role.name().to_string(),
                            reason,
                        });
                    }
                }
            }
        }

        Ok(result)
    }

    /// Synthesizes all collected subagent results through the primary model into a cohesive, user-facing response.
    async fn synthesize_results(
        &self,
        plan: &TaskPlan,
        shared_context: &SharedTaskContext,
        provider: Arc<dyn Provider>,
        model_id: &str,
        credential: &Credential,
    ) -> Result<String, AgentError> {
        let mut prompt = String::new();
        prompt.push_str("You are the primary HADES orchestrator.\n\n");
        prompt.push_str("### USER OBJECTIVE\n");
        prompt.push_str(&format!("{}\n\n", shared_context.user_objective));

        prompt.push_str("### COMPLETED SPECIALIST SUBAGENT RESULTS\n");
        for task in &plan.tasks {
            if let Some(ref res) = task.result {
                prompt.push_str(&format!(
                    "#### Task: {} (Agent: {})\n",
                    task.title,
                    task.assigned_role.name()
                ));
                prompt.push_str(&format!("**Status**: {}\n", res.status));
                prompt.push_str(&format!("**Summary**: {}\n", res.summary));
                if !res.detailed_findings.is_empty() {
                    for f in &res.detailed_findings {
                        prompt.push_str(&format!("- {f}\n"));
                    }
                }
                if !res.changed_files.is_empty() {
                    prompt.push_str(&format!(
                        "- Changed files: {}\n",
                        res.changed_files.join(", ")
                    ));
                }
                prompt.push('\n');
            }
        }

        prompt.push_str("### INSTRUCTIONS FOR FINAL SYNTHESIS\n");
        prompt.push_str("Synthesize a cohesive, high-quality, professional Markdown response fulfilling the user's objective.\n");
        prompt.push_str("Begin with a concise bullet list summarizing the specialist agents that contributed to this task, followed by your detailed findings, code changes, and answers.\n");

        let messages = vec![
            ChatMessage::system("You are Hades, an expert autonomous AI software engineer. Synthesize the final answer cleanly without exposing internal scratchpads."),
            ChatMessage::user(prompt),
        ];

        let req = CompletionRequest {
            model: model_id.to_string(),
            messages,
            tools: None,
            tool_choice: None,
            temperature: Some(0.3),
            max_tokens: None,
            stream: false,
        };
        let resp = provider.complete(req, credential).await?;
        Ok(resp.content)
    }
}
