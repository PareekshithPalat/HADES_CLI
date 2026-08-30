use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

use crate::error::AgentError;
use crate::result::AgentResult;
use crate::role::AgentRole;
use crate::strategy::OrchestrationStrategy;

pub type TaskId = String;

/// Operational lifecycle state of a subtask within an orchestration plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    /// Initial state, waiting to be scheduled.
    Pending,
    /// All prerequisite dependencies have satisfied completion; ready for execution.
    Ready,
    /// Subagent is actively executing this task.
    Running,
    /// Blocked waiting for prerequisite tasks to finish.
    WaitingForDependency,
    /// Blocked awaiting interactive user approval for tool execution.
    WaitingForPermission,
    /// Task execution finished successfully.
    Completed,
    /// Task execution encountered an unrecoverable failure.
    Failed,
    /// Task execution was cancelled by user or parent workflow.
    Cancelled,
    /// Task was skipped due to an earlier prerequisite failure.
    Skipped,
}

impl TaskStatus {
    /// Returns whether this state represents a terminal status.
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            Self::Completed | Self::Failed | Self::Cancelled | Self::Skipped
        )
    }

    /// Returns whether this task succeeded.
    pub fn is_success(&self) -> bool {
        matches!(self, Self::Completed)
    }
}

impl std::fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending => write!(f, "Pending"),
            Self::Ready => write!(f, "Ready"),
            Self::Running => write!(f, "Running"),
            Self::WaitingForDependency => write!(f, "Waiting on Dependencies"),
            Self::WaitingForPermission => write!(f, "Awaiting Permission"),
            Self::Completed => write!(f, "Completed"),
            Self::Failed => write!(f, "Failed"),
            Self::Cancelled => write!(f, "Cancelled"),
            Self::Skipped => write!(f, "Skipped"),
        }
    }
}

/// Execution priority level of a task.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default,
)]
#[serde(rename_all = "snake_case")]
pub enum TaskPriority {
    Low,
    #[default]
    Normal,
    High,
    Critical,
}

/// Structured task item representing a distinct unit of delegated work assigned to a subagent role.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Task {
    /// Unique task identifier.
    pub id: TaskId,
    /// Optional parent task identifier if this is a nested subtask.
    pub parent_task_id: Option<TaskId>,
    /// Concise, descriptive title (e.g. "Scan workspace dependencies").
    pub title: String,
    /// Detailed functional objective and execution constraints.
    pub objective: String,
    /// Logical role assigned to execute this task.
    pub assigned_role: AgentRole,
    /// Current execution lifecycle state.
    pub status: TaskStatus,
    /// Priority level of this task.
    pub priority: TaskPriority,
    /// List of task IDs that must complete before this task can start.
    pub dependencies: Vec<TaskId>,
    /// Resources/files declared to be read or modified by this task.
    pub affected_resources: Vec<String>,
    /// Whether this task performs mutating actions (e.g. file writing).
    pub is_mutating: bool,
    /// Output result if execution succeeded.
    pub result: Option<AgentResult>,
    /// Error description if execution failed.
    pub error: Option<String>,
    /// Number of retries already attempted.
    pub retries: usize,
    /// Maximum allowed retries for this specific task.
    pub max_retries: usize,
    /// Execution timeout duration in seconds.
    pub timeout_secs: u64,
}

impl Task {
    /// Constructs a new `Task` with standard defaults.
    pub fn new(
        id: impl Into<TaskId>,
        title: impl Into<String>,
        objective: impl Into<String>,
        assigned_role: AgentRole,
    ) -> Self {
        let is_mutating = assigned_role.is_mutating_allowed();
        let timeout_secs = assigned_role.default_timeout_secs();
        Self {
            id: id.into(),
            parent_task_id: None,
            title: title.into(),
            objective: objective.into(),
            assigned_role,
            status: TaskStatus::Pending,
            priority: TaskPriority::Normal,
            dependencies: Vec::new(),
            affected_resources: Vec::new(),
            is_mutating,
            result: None,
            error: None,
            retries: 0,
            max_retries: 2,
            timeout_secs,
        }
    }

    /// Adds a prerequisite dependency task ID.
    pub fn with_dependency(mut self, dependency_id: impl Into<TaskId>) -> Self {
        self.dependencies.push(dependency_id.into());
        self
    }

    /// Declares an affected file/resource path.
    pub fn with_resource(mut self, resource_path: impl Into<String>) -> Self {
        self.affected_resources.push(resource_path.into());
        self
    }

    /// Sets task priority.
    pub fn with_priority(mut self, priority: TaskPriority) -> Self {
        self.priority = priority;
        self
    }
}

/// Dependency-aware task execution plan orchestrating multi-agent collaboration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskPlan {
    /// Unique plan identifier.
    pub plan_id: String,
    /// High-level user objective this plan fulfills.
    pub objective: String,
    /// Execution strategy employed by this plan.
    pub strategy: OrchestrationStrategy,
    /// Ordered list of tasks forming the plan graph.
    pub tasks: Vec<Task>,
    /// Creation timestamp in UTC.
    pub created_at: DateTime<Utc>,
}

impl TaskPlan {
    /// Constructs a new `TaskPlan`.
    pub fn new(
        objective: impl Into<String>,
        strategy: OrchestrationStrategy,
        tasks: Vec<Task>,
    ) -> Self {
        let mut plan = Self {
            plan_id: uuid::Uuid::new_v4().to_string(),
            objective: objective.into(),
            strategy,
            tasks,
            created_at: Utc::now(),
        };
        plan.update_ready_tasks();
        plan
    }

    /// Validates the plan dependency graph, checking for cycles and missing references.
    pub fn validate_dependencies(&self) -> Result<(), AgentError> {
        let task_ids: HashSet<&str> = self.tasks.iter().map(|t| t.id.as_str()).collect();

        // 1. Check for missing dependency IDs
        for task in &self.tasks {
            for dep in &task.dependencies {
                if !task_ids.contains(dep.as_str()) {
                    return Err(AgentError::Execution(format!(
                        "Task '{}' depends on non-existent task '{}'",
                        task.id, dep
                    )));
                }
            }
        }

        // 2. Cycle detection using Depth-First Search
        let mut visited = HashMap::new(); // 0 = unvisited, 1 = visiting, 2 = visited
        for task in &self.tasks {
            visited.insert(task.id.as_str(), 0);
        }

        fn dfs<'a>(
            task_id: &'a str,
            tasks: &'a [Task],
            visited: &mut HashMap<&'a str, i32>,
        ) -> Result<(), AgentError> {
            visited.insert(task_id, 1);
            if let Some(t) = tasks.iter().find(|t| t.id == task_id) {
                for dep in &t.dependencies {
                    let state = visited.get(dep.as_str()).copied().unwrap_or(0);
                    if state == 1 {
                        return Err(AgentError::CircularDependency(format!(
                            "Cycle involving tasks '{}' and '{}'",
                            task_id, dep
                        )));
                    }
                    if state == 0 {
                        dfs(dep.as_str(), tasks, visited)?;
                    }
                }
            }
            visited.insert(task_id, 2);
            Ok(())
        }

        for task in &self.tasks {
            if visited.get(task.id.as_str()) == Some(&0) {
                dfs(task.id.as_str(), &self.tasks, &mut visited)?;
            }
        }

        Ok(())
    }

    /// Returns references to all tasks currently eligible to start execution.
    pub fn get_ready_tasks(&self) -> Vec<&Task> {
        self.tasks
            .iter()
            .filter(|t| t.status == TaskStatus::Ready)
            .collect()
    }

    /// Updates task statuses based on dependency completions.
    pub fn update_ready_tasks(&mut self) {
        let completed_ids: HashSet<String> = self
            .tasks
            .iter()
            .filter(|t| t.status == TaskStatus::Completed)
            .map(|t| t.id.clone())
            .collect();

        let failed_ids: HashSet<String> = self
            .tasks
            .iter()
            .filter(|t| t.status == TaskStatus::Failed || t.status == TaskStatus::Skipped)
            .map(|t| t.id.clone())
            .collect();

        for task in &mut self.tasks {
            if task.status == TaskStatus::Pending || task.status == TaskStatus::WaitingForDependency
            {
                // Check if any prerequisite failed -> mark skipped
                let has_failed_dep = task.dependencies.iter().any(|d| failed_ids.contains(d));
                if has_failed_dep {
                    task.status = TaskStatus::Skipped;
                    continue;
                }

                // Check if all prerequisites completed
                let all_deps_met = task.dependencies.iter().all(|d| completed_ids.contains(d));
                if all_deps_met {
                    task.status = TaskStatus::Ready;
                } else {
                    task.status = TaskStatus::WaitingForDependency;
                }
            }
        }
    }

    /// Retrieves an immutable reference to a task by ID.
    pub fn get_task(&self, task_id: &str) -> Option<&Task> {
        self.tasks.iter().find(|t| t.id == task_id)
    }

    /// Retrieves a mutable reference to a task by ID.
    pub fn get_task_mut(&mut self, task_id: &str) -> Option<&mut Task> {
        self.tasks.iter_mut().find(|t| t.id == task_id)
    }

    /// Marks a task as completed with its result and recomputes ready tasks.
    pub fn mark_task_completed(&mut self, task_id: &str, result: AgentResult) {
        if let Some(t) = self.get_task_mut(task_id) {
            t.status = TaskStatus::Completed;
            t.result = Some(result);
        }
        self.update_ready_tasks();
    }

    /// Marks a task as failed and propagates skipping to dependent tasks.
    pub fn mark_task_failed(&mut self, task_id: &str, error: &str) {
        if let Some(t) = self.get_task_mut(task_id) {
            t.status = TaskStatus::Failed;
            t.error = Some(error.to_string());
        }
        self.update_ready_tasks();
    }

    /// Returns whether all tasks in the plan have reached a terminal state.
    pub fn is_complete(&self) -> bool {
        self.tasks.iter().all(|t| t.status.is_terminal())
    }

    /// Returns whether any task in the plan has failed.
    pub fn has_failures(&self) -> bool {
        self.tasks
            .iter()
            .any(|t| t.status == TaskStatus::Failed || t.status == TaskStatus::Skipped)
    }
}
