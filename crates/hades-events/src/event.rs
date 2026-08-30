use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// High-level application events representing state transitions and operational lifecycle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum HadesEvent {
    /// Application successfully started.
    ApplicationStarted {
        timestamp: DateTime<Utc>,
        version: String,
    },

    /// Application initiated shutdown.
    ApplicationShutdown {
        timestamp: DateTime<Utc>,
        reason: Option<String>,
    },

    /// A command string was submitted by the user.
    CommandEntered {
        timestamp: DateTime<Utc>,
        raw_input: String,
    },

    /// A command finished execution.
    CommandExecuted {
        timestamp: DateTime<Utc>,
        command: String,
        success: bool,
    },

    /// Application configuration was loaded.
    ConfigLoaded {
        timestamp: DateTime<Utc>,
        path: PathBuf,
    },

    /// Application configuration was saved.
    ConfigSaved {
        timestamp: DateTime<Utc>,
        path: PathBuf,
    },

    /// An AI provider was selected in the workflow.
    ProviderSelected {
        timestamp: DateTime<Utc>,
        provider_id: String,
    },

    /// An AI model was selected in the workflow.
    ModelSelected {
        timestamp: DateTime<Utc>,
        provider_id: String,
        model_id: String,
    },

    /// Credential configuration began for a provider.
    CredentialSetupStarted {
        timestamp: DateTime<Utc>,
        provider_id: String,
    },

    /// Credential verification commenced.
    CredentialVerificationStarted {
        timestamp: DateTime<Utc>,
        provider_id: String,
    },

    /// Credential and model verification succeeded.
    CredentialVerificationSucceeded {
        timestamp: DateTime<Utc>,
        provider_id: String,
        model_id: String,
    },

    /// Credential verification failed.
    CredentialVerificationFailed {
        timestamp: DateTime<Utc>,
        provider_id: String,
        error: String,
    },

    /// Active model and provider successfully loaded into runtime.
    ModelLoaded {
        timestamp: DateTime<Utc>,
        provider_id: String,
        model_id: String,
    },

    /// An AI prompt request was submitted to the model provider.
    ModelRequestStarted {
        timestamp: DateTime<Utc>,
        provider_id: String,
        model_id: String,
    },

    /// First token or response chunk received from the model.
    ModelResponseStarted {
        timestamp: DateTime<Utc>,
        provider_id: String,
        model_id: String,
    },

    /// Model response completed generation.
    ModelResponseCompleted {
        timestamp: DateTime<Utc>,
        provider_id: String,
        model_id: String,
        total_tokens: Option<u32>,
    },

    /// An error occurred in the provider subsystem.
    ProviderErrorOccurred {
        timestamp: DateTime<Utc>,
        provider_id: String,
        error: String,
    },

    /// A new conversation session was created.
    SessionCreated {
        timestamp: DateTime<Utc>,
        session_id: String,
        title: String,
    },

    /// The active session was switched.
    SessionSwitched {
        timestamp: DateTime<Utc>,
        from_session_id: Option<String>,
        to_session_id: String,
    },

    /// A conversation session was deleted.
    SessionDeleted {
        timestamp: DateTime<Utc>,
        session_id: String,
    },

    /// A conversation session was renamed.
    SessionRenamed {
        timestamp: DateTime<Utc>,
        session_id: String,
        old_title: String,
        new_title: String,
    },

    /// Active model was switched for the current session.
    ModelSwitched {
        timestamp: DateTime<Utc>,
        session_id: String,
        provider_id: String,
        model_id: String,
    },

    /// Older messages were truncated from active context window.
    ContextTruncated {
        timestamp: DateTime<Utc>,
        session_id: String,
        total_messages: usize,
        included_messages: usize,
        estimated_tokens: usize,
    },

    /// A structured message was persisted to session store.
    MessagePersisted {
        timestamp: DateTime<Utc>,
        session_id: String,
        message_id: String,
        role: String,
    },

    /// An application-level error occurred.
    ErrorOccurred {
        timestamp: DateTime<Utc>,
        error: String,
    },

    /// A project workspace was detected.
    WorkspaceDetected {
        timestamp: DateTime<Utc>,
        root: PathBuf,
        project_type: String,
    },

    /// A tool execution was requested.
    ToolRequested {
        timestamp: DateTime<Utc>,
        execution_id: String,
        session_id: String,
        tool_name: String,
        arguments: String,
    },

    /// Tool execution requires explicit user authorization.
    ToolApprovalRequested {
        timestamp: DateTime<Utc>,
        execution_id: String,
        session_id: String,
        tool_name: String,
        risk_level: String,
        summary: String,
        details: String,
    },

    /// Tool execution was approved by the user.
    ToolApproved {
        timestamp: DateTime<Utc>,
        execution_id: String,
        decision: String,
    },

    /// Tool execution was denied.
    ToolDenied {
        timestamp: DateTime<Utc>,
        execution_id: String,
        reason: String,
    },

    /// Tool execution commenced.
    ToolStarted {
        timestamp: DateTime<Utc>,
        execution_id: String,
        tool_name: String,
    },

    /// Tool execution completed.
    ToolCompleted {
        timestamp: DateTime<Utc>,
        execution_id: String,
        tool_name: String,
        status: String,
        is_truncated: bool,
    },

    /// Tool execution failed.
    ToolFailed {
        timestamp: DateTime<Utc>,
        execution_id: String,
        tool_name: String,
        error: String,
    },

    /// Tool execution was cancelled.
    ToolCancelled {
        timestamp: DateTime<Utc>,
        execution_id: String,
        tool_name: String,
    },

    /// Tool execution timed out.
    ToolTimedOut {
        timestamp: DateTime<Utc>,
        execution_id: String,
        tool_name: String,
        duration_ms: u64,
    },

    /// A file was created by a tool.
    FileCreated {
        timestamp: DateTime<Utc>,
        path: PathBuf,
        execution_id: String,
    },

    /// A file was modified by a tool.
    FileModified {
        timestamp: DateTime<Utc>,
        path: PathBuf,
        execution_id: String,
    },

    /// A file was deleted by a tool.
    FileDeleted {
        timestamp: DateTime<Utc>,
        path: PathBuf,
        execution_id: String,
    },

    /// A child process was started.
    ProcessStarted {
        timestamp: DateTime<Utc>,
        executable: String,
        arguments: Vec<String>,
        execution_id: String,
    },

    /// A child process exited.
    ProcessExited {
        timestamp: DateTime<Utc>,
        executable: String,
        exit_code: Option<i32>,
        execution_id: String,
    },

    /// An environment variable was modified.
    EnvironmentChanged {
        timestamp: DateTime<Utc>,
        key: String,
        execution_id: String,
    },

    /// Multi-agent orchestration was initiated.
    OrchestrationStarted {
        timestamp: DateTime<Utc>,
        session_id: String,
        orchestration_id: String,
        strategy: String,
        user_objective: String,
        agent_count: usize,
    },

    /// Multi-agent task plan was constructed.
    OrchestrationPlanned {
        timestamp: DateTime<Utc>,
        session_id: String,
        orchestration_id: String,
        tasks: Vec<String>,
    },

    /// A specialized subagent was spawned.
    AgentSpawned {
        timestamp: DateTime<Utc>,
        orchestration_id: String,
        agent_id: String,
        role: String,
        name: String,
    },

    /// A subagent emitted a live activity update.
    AgentProgressUpdated {
        timestamp: DateTime<Utc>,
        orchestration_id: String,
        agent_id: String,
        role: String,
        activity: String,
    },

    /// A subagent completed its execution.
    AgentCompleted {
        timestamp: DateTime<Utc>,
        orchestration_id: String,
        agent_id: String,
        role: String,
        status: String,
    },

    /// A subagent failed during execution.
    AgentFailed {
        timestamp: DateTime<Utc>,
        orchestration_id: String,
        agent_id: String,
        role: String,
        error: String,
    },

    /// An orchestrated subtask began execution.
    TaskStarted {
        timestamp: DateTime<Utc>,
        orchestration_id: String,
        task_id: String,
        title: String,
        assigned_agent: String,
    },

    /// An orchestrated subtask completed.
    TaskCompleted {
        timestamp: DateTime<Utc>,
        orchestration_id: String,
        task_id: String,
        title: String,
        status: String,
    },

    /// An orchestrated subtask failed.
    TaskFailed {
        timestamp: DateTime<Utc>,
        orchestration_id: String,
        task_id: String,
        title: String,
        error: String,
    },

    /// Multi-agent orchestration was cancelled.
    OrchestrationCancelled {
        timestamp: DateTime<Utc>,
        session_id: String,
        orchestration_id: String,
        reason: String,
    },

    /// Multi-agent orchestration completed.
    OrchestrationCompleted {
        timestamp: DateTime<Utc>,
        session_id: String,
        orchestration_id: String,
        status: String,
        summary: String,
        total_tokens: Option<u32>,
    },

    /// Browser sidecar process started.
    BrowserStarted {
        timestamp: DateTime<Utc>,
        session_id: String,
        browser: String,
        mode: String,
    },

    /// Browser sidecar process stopped.
    BrowserStopped {
        timestamp: DateTime<Utc>,
        session_id: String,
    },

    /// Browser navigated to a page.
    BrowserNavigated {
        timestamp: DateTime<Utc>,
        session_id: String,
        url: String,
        title: String,
    },

    /// Browser captured accessibility snapshot.
    BrowserSnapshotCaptured {
        timestamp: DateTime<Utc>,
        session_id: String,
        url: String,
        element_count: usize,
    },

    /// Browser performed an interactive action.
    BrowserActionExecuted {
        timestamp: DateTime<Utc>,
        session_id: String,
        action: String,
        target: String,
        success: bool,
    },

    /// Browser created an artifact.
    BrowserArtifactGenerated {
        timestamp: DateTime<Utc>,
        session_id: String,
        artifact_type: String,
        path: PathBuf,
    },

    /// The application requires user input or approval.
    InputRequired {
        timestamp: DateTime<Utc>,
        reason: String,
        details: String,
    },

    /// A notification sound or desktop alert was triggered.
    NotificationTriggered {
        timestamp: DateTime<Utc>,
        kind: String,
        sound_played: bool,
        desktop_sent: bool,
    },
}


impl HadesEvent {
    /// Creates an `ApplicationStarted` event at the current time.
    pub fn app_started(version: impl Into<String>) -> Self {
        Self::ApplicationStarted {
            timestamp: Utc::now(),
            version: version.into(),
        }
    }

    /// Creates an `ApplicationShutdown` event at the current time.
    pub fn app_shutdown(reason: Option<String>) -> Self {
        Self::ApplicationShutdown {
            timestamp: Utc::now(),
            reason,
        }
    }

    /// Creates a `CommandEntered` event at the current time.
    pub fn command_entered(raw_input: impl Into<String>) -> Self {
        Self::CommandEntered {
            timestamp: Utc::now(),
            raw_input: raw_input.into(),
        }
    }

    /// Creates a `CommandExecuted` event at the current time.
    pub fn command_executed(command: impl Into<String>, success: bool) -> Self {
        Self::CommandExecuted {
            timestamp: Utc::now(),
            command: command.into(),
            success,
        }
    }

    /// Creates a `ConfigLoaded` event at the current time.
    pub fn config_loaded(path: impl Into<PathBuf>) -> Self {
        Self::ConfigLoaded {
            timestamp: Utc::now(),
            path: path.into(),
        }
    }

    /// Creates a `ConfigSaved` event at the current time.
    pub fn config_saved(path: impl Into<PathBuf>) -> Self {
        Self::ConfigSaved {
            timestamp: Utc::now(),
            path: path.into(),
        }
    }

    /// Creates a `ProviderSelected` event.
    pub fn provider_selected(provider_id: impl Into<String>) -> Self {
        Self::ProviderSelected {
            timestamp: Utc::now(),
            provider_id: provider_id.into(),
        }
    }

    /// Creates a `ModelSelected` event.
    pub fn model_selected(provider_id: impl Into<String>, model_id: impl Into<String>) -> Self {
        Self::ModelSelected {
            timestamp: Utc::now(),
            provider_id: provider_id.into(),
            model_id: model_id.into(),
        }
    }

    /// Creates a `ModelLoaded` event.
    pub fn model_loaded(provider_id: impl Into<String>, model_id: impl Into<String>) -> Self {
        Self::ModelLoaded {
            timestamp: Utc::now(),
            provider_id: provider_id.into(),
            model_id: model_id.into(),
        }
    }

    /// Creates a `SessionCreated` event.
    pub fn session_created(session_id: impl Into<String>, title: impl Into<String>) -> Self {
        Self::SessionCreated {
            timestamp: Utc::now(),
            session_id: session_id.into(),
            title: title.into(),
        }
    }

    /// Creates a `SessionSwitched` event.
    pub fn session_switched(
        from_session_id: Option<String>,
        to_session_id: impl Into<String>,
    ) -> Self {
        Self::SessionSwitched {
            timestamp: Utc::now(),
            from_session_id,
            to_session_id: to_session_id.into(),
        }
    }

    /// Creates a `ErrorOccurred` event at the current time.
    pub fn error_occurred(error: impl Into<String>) -> Self {
        Self::ErrorOccurred {
            timestamp: Utc::now(),
            error: error.into(),
        }
    }

    /// Creates an `OrchestrationStarted` event.
    pub fn orchestration_started(
        session_id: impl Into<String>,
        orchestration_id: impl Into<String>,
        strategy: impl Into<String>,
        user_objective: impl Into<String>,
        agent_count: usize,
    ) -> Self {
        Self::OrchestrationStarted {
            timestamp: Utc::now(),
            session_id: session_id.into(),
            orchestration_id: orchestration_id.into(),
            strategy: strategy.into(),
            user_objective: user_objective.into(),
            agent_count,
        }
    }

    /// Creates a `TaskStarted` event.
    pub fn task_started(
        orchestration_id: impl Into<String>,
        task_id: impl Into<String>,
        title: impl Into<String>,
        assigned_agent: impl Into<String>,
    ) -> Self {
        Self::TaskStarted {
            timestamp: Utc::now(),
            orchestration_id: orchestration_id.into(),
            task_id: task_id.into(),
            title: title.into(),
            assigned_agent: assigned_agent.into(),
        }
    }

    /// Creates a `TaskCompleted` event.
    pub fn task_completed(
        orchestration_id: impl Into<String>,
        task_id: impl Into<String>,
        title: impl Into<String>,
        status: impl Into<String>,
    ) -> Self {
        Self::TaskCompleted {
            timestamp: Utc::now(),
            orchestration_id: orchestration_id.into(),
            task_id: task_id.into(),
            title: title.into(),
            status: status.into(),
        }
    }

    /// Creates a `TaskFailed` event.
    pub fn task_failed(
        orchestration_id: impl Into<String>,
        task_id: impl Into<String>,
        title: impl Into<String>,
        error: impl Into<String>,
    ) -> Self {
        Self::TaskFailed {
            timestamp: Utc::now(),
            orchestration_id: orchestration_id.into(),
            task_id: task_id.into(),
            title: title.into(),
            error: error.into(),
        }
    }

    /// Creates an `OrchestrationCancelled` event.
    pub fn orchestration_cancelled(
        session_id: impl Into<String>,
        orchestration_id: impl Into<String>,
        reason: impl Into<String>,
    ) -> Self {
        Self::OrchestrationCancelled {
            timestamp: Utc::now(),
            session_id: session_id.into(),
            orchestration_id: orchestration_id.into(),
            reason: reason.into(),
        }
    }

    /// Creates an `OrchestrationCompleted` event.
    pub fn orchestration_completed(
        session_id: impl Into<String>,
        orchestration_id: impl Into<String>,
        status: impl Into<String>,
        summary: impl Into<String>,
        total_tokens: Option<u32>,
    ) -> Self {
        Self::OrchestrationCompleted {
            timestamp: Utc::now(),
            session_id: session_id.into(),
            orchestration_id: orchestration_id.into(),
            status: status.into(),
            summary: summary.into(),
            total_tokens,
        }
    }

    /// Returns the timestamp of the event.
    pub fn timestamp(&self) -> DateTime<Utc> {
        match self {
            Self::ApplicationStarted { timestamp, .. } => *timestamp,
            Self::ApplicationShutdown { timestamp, .. } => *timestamp,
            Self::CommandEntered { timestamp, .. } => *timestamp,
            Self::CommandExecuted { timestamp, .. } => *timestamp,
            Self::ConfigLoaded { timestamp, .. } => *timestamp,
            Self::ConfigSaved { timestamp, .. } => *timestamp,
            Self::ProviderSelected { timestamp, .. } => *timestamp,
            Self::ModelSelected { timestamp, .. } => *timestamp,
            Self::CredentialSetupStarted { timestamp, .. } => *timestamp,
            Self::CredentialVerificationStarted { timestamp, .. } => *timestamp,
            Self::CredentialVerificationSucceeded { timestamp, .. } => *timestamp,
            Self::CredentialVerificationFailed { timestamp, .. } => *timestamp,
            Self::ModelLoaded { timestamp, .. } => *timestamp,
            Self::ModelRequestStarted { timestamp, .. } => *timestamp,
            Self::ModelResponseStarted { timestamp, .. } => *timestamp,
            Self::ModelResponseCompleted { timestamp, .. } => *timestamp,
            Self::ProviderErrorOccurred { timestamp, .. } => *timestamp,
            Self::SessionCreated { timestamp, .. } => *timestamp,
            Self::SessionSwitched { timestamp, .. } => *timestamp,
            Self::SessionDeleted { timestamp, .. } => *timestamp,
            Self::SessionRenamed { timestamp, .. } => *timestamp,
            Self::ModelSwitched { timestamp, .. } => *timestamp,
            Self::ContextTruncated { timestamp, .. } => *timestamp,
            Self::MessagePersisted { timestamp, .. } => *timestamp,
            Self::ErrorOccurred { timestamp, .. } => *timestamp,
            Self::WorkspaceDetected { timestamp, .. } => *timestamp,
            Self::ToolRequested { timestamp, .. } => *timestamp,
            Self::ToolApprovalRequested { timestamp, .. } => *timestamp,
            Self::ToolApproved { timestamp, .. } => *timestamp,
            Self::ToolDenied { timestamp, .. } => *timestamp,
            Self::ToolStarted { timestamp, .. } => *timestamp,
            Self::ToolCompleted { timestamp, .. } => *timestamp,
            Self::ToolFailed { timestamp, .. } => *timestamp,
            Self::ToolCancelled { timestamp, .. } => *timestamp,
            Self::ToolTimedOut { timestamp, .. } => *timestamp,
            Self::FileCreated { timestamp, .. } => *timestamp,
            Self::FileModified { timestamp, .. } => *timestamp,
            Self::FileDeleted { timestamp, .. } => *timestamp,
            Self::ProcessStarted { timestamp, .. } => *timestamp,
            Self::ProcessExited { timestamp, .. } => *timestamp,
            Self::EnvironmentChanged { timestamp, .. } => *timestamp,
            Self::OrchestrationStarted { timestamp, .. } => *timestamp,
            Self::OrchestrationPlanned { timestamp, .. } => *timestamp,
            Self::AgentSpawned { timestamp, .. } => *timestamp,
            Self::AgentProgressUpdated { timestamp, .. } => *timestamp,
            Self::AgentCompleted { timestamp, .. } => *timestamp,
            Self::AgentFailed { timestamp, .. } => *timestamp,
            Self::TaskStarted { timestamp, .. } => *timestamp,
            Self::TaskCompleted { timestamp, .. } => *timestamp,
            Self::TaskFailed { timestamp, .. } => *timestamp,
            Self::OrchestrationCancelled { timestamp, .. } => *timestamp,
            Self::OrchestrationCompleted { timestamp, .. } => *timestamp,
            Self::BrowserStarted { timestamp, .. } => *timestamp,
            Self::BrowserStopped { timestamp, .. } => *timestamp,
            Self::BrowserNavigated { timestamp, .. } => *timestamp,
            Self::BrowserSnapshotCaptured { timestamp, .. } => *timestamp,
            Self::BrowserActionExecuted { timestamp, .. } => *timestamp,
            Self::BrowserArtifactGenerated { timestamp, .. } => *timestamp,
            Self::InputRequired { timestamp, .. } => *timestamp,
            Self::NotificationTriggered { timestamp, .. } => *timestamp,
        }
    }
}

