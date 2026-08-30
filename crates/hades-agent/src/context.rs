use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use crate::definition::AgentDefinition;
use crate::result::AgentResult;
use crate::task::{Task, TaskId};
use hades_provider::ChatMessage;

/// Shared runtime context maintaining artifacts, summaries, and permissions across collaborating agents.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedTaskContext {
    /// Active conversation session ID.
    pub session_id: String,
    /// Overall user objective.
    pub user_objective: String,
    /// Root directory of the active workspace.
    pub workspace_root: PathBuf,
    /// Completed task summaries indexed by task ID.
    pub completed_task_summaries: HashMap<TaskId, String>,
    /// Global list of shared artifacts created during orchestration.
    pub shared_artifacts: Vec<String>,
    /// Set of files modified across all subagents.
    pub changed_files: HashSet<String>,
    /// Session-granted tool permissions.
    pub active_permissions: HashSet<String>,
}

impl SharedTaskContext {
    /// Constructs a fresh `SharedTaskContext`.
    pub fn new(
        session_id: impl Into<String>,
        user_objective: impl Into<String>,
        workspace_root: impl Into<PathBuf>,
    ) -> Self {
        Self {
            session_id: session_id.into(),
            user_objective: user_objective.into(),
            workspace_root: workspace_root.into(),
            completed_task_summaries: HashMap::new(),
            shared_artifacts: Vec::new(),
            changed_files: HashSet::new(),
            active_permissions: HashSet::new(),
        }
    }

    /// Records findings and artifacts from a completed task result.
    pub fn record_task_result(&mut self, task_id: &str, result: &AgentResult) {
        self.completed_task_summaries
            .insert(task_id.to_string(), result.to_condensed_markdown());

        for artifact in &result.artifacts {
            if !self.shared_artifacts.contains(artifact) {
                self.shared_artifacts.push(artifact.clone());
            }
        }

        for file in &result.changed_files {
            self.changed_files.insert(file.clone());
        }
    }

    /// Retrieves condensed summaries for the specified prerequisite dependency task IDs.
    pub fn get_dependency_summaries(&self, dependencies: &[TaskId]) -> Vec<String> {
        dependencies
            .iter()
            .filter_map(|dep_id| self.completed_task_summaries.get(dep_id).cloned())
            .collect()
    }
}

/// Slices global context into focused, isolated prompt payloads for individual subagents.
pub struct ContextSlicer;

impl ContextSlicer {
    /// Constructs the isolated chat message history for a specialized subagent run.
    pub fn build_subagent_messages(
        agent: &AgentDefinition,
        task: &Task,
        shared: &SharedTaskContext,
        project_type: Option<&str>,
    ) -> Vec<ChatMessage> {
        let mut system_prompt = String::new();

        // 1. Role System Instructions
        system_prompt.push_str(&format!(
            "You are the **{}** subagent in the HADES multi-agent system.\n",
            agent.name
        ));
        system_prompt.push_str(&format!("{}\n\n", agent.system_instruction));

        // 2. Behavioral Constraints & Safety
        system_prompt.push_str("### EXECUTION GUIDELINES\n");
        system_prompt.push_str("- You are an autonomous subagent reporting structured results directly back to the HADES Orchestrator.\n");
        system_prompt.push_str("- DO NOT speak as a chat assistant to the end user. Return focused findings, code changes, and analysis.\n");
        if agent.role.is_mutating_allowed() {
            system_prompt.push_str("- You are authorized to modify workspace files safely when assigned an implementation objective.\n");
        } else {
            system_prompt.push_str(
                "- You are in a READ-ONLY / AUDIT role. Do not attempt to modify files.\n",
            );
        }
        system_prompt.push_str(&format!(
            "- Workspace Root: {}\n",
            shared.workspace_root.display()
        ));
        if let Some(pt) = project_type {
            system_prompt.push_str(&format!("- Detected Project Type: {pt}\n"));
        }
        system_prompt.push('\n');

        // 3. User Objective & Prerequisite Findings
        let mut user_prompt = String::new();
        user_prompt.push_str(&format!("### OVERALL GOAL\n{}\n\n", shared.user_objective));
        user_prompt.push_str(&format!(
            "### ASSIGNED TASK: {}\n{}\n\n",
            task.title, task.objective
        ));

        let dep_summaries = shared.get_dependency_summaries(&task.dependencies);
        if !dep_summaries.is_empty() {
            user_prompt.push_str("### PREREQUISITE TASK FINDINGS\n");
            for summary in dep_summaries {
                user_prompt.push_str(&format!("{summary}\n"));
            }
            user_prompt.push('\n');
        }

        user_prompt.push_str("### REQUIRED OUTPUT FORMAT\n");
        user_prompt.push_str("Perform the requested actions using available tools if needed. Conclude your execution with:\n");
        user_prompt.push_str("1. Summary of actions taken / findings\n");
        user_prompt.push_str("2. Details or key discoveries\n");
        user_prompt.push_str("3. Changed files or artifacts (if any)\n");

        vec![
            ChatMessage::system(system_prompt),
            ChatMessage::user(user_prompt),
        ]
    }
}
