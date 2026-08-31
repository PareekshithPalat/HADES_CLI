use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::error::CommandError;
use crate::state::AppState;
use hades_config::HadesConfig;
use hades_storage::{StorageHealth, StorageStatus};

/// Information entry for help listings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HelpEntry {
    pub name: String,
    pub description: String,
}

/// Structured status snapshot of the application.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatusInfo {
    pub application: String,
    pub version: String,
    pub session_id: String,
    pub session_title: String,
    pub messages: usize,
    pub context_usage: String,
    pub model: String,
    pub mode: String,
    pub storage_status: String,
    pub config_status: String,
}

impl fmt::Display for StatusInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "HADES STATUS")?;
        writeln!(f)?;
        writeln!(f, "Application:   {}", self.application)?;
        writeln!(f, "Version:       {}", self.version)?;
        writeln!(f, "Session ID:    {}", self.session_id)?;
        writeln!(f, "Session Title: {}", self.session_title)?;
        writeln!(f, "Messages:      {}", self.messages)?;
        writeln!(f, "Context:       {}", self.context_usage)?;
        writeln!(f, "Active Model:  {}", self.model)?;
        writeln!(f, "Mode:          {}", self.mode)?;
        writeln!(f, "Storage:       {}", self.storage_status)?;
        write!(f, "Configuration: {}", self.config_status)
    }
}

/// Output returned by command execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CommandOutput {
    /// Generic text output message.
    Text(String),

    /// Help listing showing all registered commands.
    Help(Vec<HelpEntry>),

    /// Application status report.
    Status(StatusInfo),

    /// Signal to open the interactive model/provider selection workflow.
    OpenModelSetup,

    /// Signal to switch active model for the current session.
    OpenModelSwitch,

    /// Signal to create a new session.
    NewSession,

    /// Signal to open the interactive session switcher overlay.
    OpenSessionPicker,

    /// Application exit signal.
    Exit,
}

impl fmt::Display for CommandOutput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Text(msg) => write!(f, "{}", msg),
            Self::Help(entries) => {
                writeln!(f, "HADES COMMANDS")?;
                writeln!(f)?;
                for entry in entries {
                    writeln!(f, "  {:<14} {}", entry.name, entry.description)?;
                }
                writeln!(f)?;
                writeln!(f, "KEYBOARD SHORTCUTS & CONTROLS")?;
                writeln!(f, "  {:<14} Submit prompt / Confirm selection", "Enter")?;
                writeln!(
                    f,
                    "  {:<14} Copy selected conversation / assistant response to clipboard",
                    "Ctrl+Y"
                )?;
                writeln!(
                    f,
                    "  {:<14} Interrupt active response / Shutdown Hades",
                    "Ctrl+C"
                )?;
                writeln!(f, "  {:<14} Open interactive command palette", "/")?;
                writeln!(
                    f,
                    "  {:<14} Scroll conversation / Navigate lists & palettes",
                    "Up / Down"
                )?;
                writeln!(f, "  {:<14} Scroll conversation by page", "PageUp / PageDn")?;
                writeln!(
                    f,
                    "  {:<14} Jump to top / bottom of conversation",
                    "Home / End"
                )?;
                writeln!(f, "  {:<14} Dismiss active modal / Close palette", "Esc")?;
                Ok(())
            }
            Self::Status(status) => write!(f, "{}", status),
            Self::OpenModelSetup => write!(f, "Opening AI model configuration..."),
            Self::OpenModelSwitch => write!(f, "Opening model switch for current session..."),
            Self::NewSession => write!(f, "Created new conversation session."),
            Self::OpenSessionPicker => write!(f, "Opening session switcher..."),
            Self::Exit => write!(f, "Exiting Hades..."),
        }
    }
}

/// Metadata describing a command for palettes and help menus.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandInfo {
    pub name: String,
    pub aliases: Vec<String>,
    pub description: String,
}

use hades_tools::{ToolRegistry, WorkspaceMetadata};

/// Context provided to commands during execution.
pub struct CommandContext<'a> {
    pub app_state: AppState,
    pub config: &'a HadesConfig,
    pub storage_health: &'a StorageHealth,
    pub session_id: Option<&'a str>,
    pub session_title: Option<&'a str>,
    pub message_count: usize,
    pub context_usage: Option<String>,
    pub active_model: Option<&'a str>,
    pub version: &'a str,
    pub shutdown_requested: bool,
    pub open_model_setup_requested: bool,
    pub open_model_switch_requested: bool,
    pub new_session_requested: bool,
    pub open_session_picker_requested: bool,
    pub available_commands: Vec<HelpEntry>,
    pub workspace_info: Option<&'a WorkspaceMetadata>,
    pub tool_registry: Option<&'a ToolRegistry>,
    pub session_permissions: Vec<String>,
    pub mcp_summaries: Vec<hades_mcp::McpServerSummary>,
    pub browser_status: Option<hades_browser::BrowserStatus>,
    pub browser_manager: Option<Arc<hades_browser::BrowserManager>>,
    pub raw_input: String,
}

impl<'a> CommandContext<'a> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        app_state: AppState,
        config: &'a HadesConfig,
        storage_health: &'a StorageHealth,
        session_id: Option<&'a str>,
        session_title: Option<&'a str>,
        message_count: usize,
        context_usage: Option<String>,
        active_model: Option<&'a str>,
        version: &'a str,
        available_commands: Vec<HelpEntry>,
    ) -> Self {
        Self {
            app_state,
            config,
            storage_health,
            session_id,
            session_title,
            message_count,
            context_usage,
            active_model,
            version,
            shutdown_requested: false,
            open_model_setup_requested: false,
            open_model_switch_requested: false,
            new_session_requested: false,
            open_session_picker_requested: false,
            available_commands,
            workspace_info: None,
            tool_registry: None,
            session_permissions: Vec::new(),
            mcp_summaries: Vec::new(),
            browser_status: None,
            browser_manager: None,
            raw_input: String::new(),
        }
    }

    /// Sets the raw command input string.
    pub fn with_raw_input(mut self, input: impl Into<String>) -> Self {
        self.raw_input = input.into();
        self
    }

    /// Attaches workspace, tools, and permission information to the command context.
    pub fn with_tools_and_workspace(
        mut self,
        workspace_info: Option<&'a WorkspaceMetadata>,
        tool_registry: Option<&'a ToolRegistry>,
        session_permissions: Vec<String>,
    ) -> Self {
        self.workspace_info = workspace_info;
        self.tool_registry = tool_registry;
        self.session_permissions = session_permissions;
        self
    }

    /// Attaches MCP server diagnostic summaries to the command context.
    pub fn with_mcp_summaries(mut self, summaries: Vec<hades_mcp::McpServerSummary>) -> Self {
        self.mcp_summaries = summaries;
        self
    }

    /// Attaches browser status and manager to the command context.
    pub fn with_browser(
        mut self,
        status: Option<hades_browser::BrowserStatus>,
        manager: Option<Arc<hades_browser::BrowserManager>>,
    ) -> Self {
        self.browser_status = status;
        self.browser_manager = manager;
        self
    }

    /// Requests application shutdown from within a command.
    pub fn request_shutdown(&mut self) {
        self.shutdown_requested = true;
    }

    /// Requests opening the interactive model setup workflow.
    pub fn request_model_setup(&mut self) {
        self.open_model_setup_requested = true;
    }

    /// Requests opening model switch workflow for current session.
    pub fn request_model_switch(&mut self) {
        self.open_model_switch_requested = true;
    }

    /// Requests creating a new session.
    pub fn request_new_session(&mut self) {
        self.new_session_requested = true;
    }

    /// Requests opening interactive session picker.
    pub fn request_session_picker(&mut self) {
        self.open_session_picker_requested = true;
    }
}

/// Abstraction for all Hades commands.
pub trait Command: Send + Sync {
    /// Canonical command name (including leading slash, e.g. "/help").
    fn name(&self) -> &'static str;

    /// Secondary aliases for the command (e.g. ["/h", "/?"]).
    fn aliases(&self) -> &'static [&'static str] {
        &[]
    }

    /// Short human-readable description of what the command does.
    fn description(&self) -> &'static str;

    /// Executes the command with the provided context.
    fn execute(&self, context: &mut CommandContext) -> Result<CommandOutput, CommandError>;
}

// Built-in Phase 0, Phase 1 & Phase 2 Commands

/// Command: `/help`
pub struct HelpCommand;

impl Command for HelpCommand {
    fn name(&self) -> &'static str {
        "/help"
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["/?", "/h"]
    }

    fn description(&self) -> &'static str {
        "Show available commands"
    }

    fn execute(&self, context: &mut CommandContext) -> Result<CommandOutput, CommandError> {
        Ok(CommandOutput::Help(context.available_commands.clone()))
    }
}

/// Command: `/status`
pub struct StatusCommand;

impl Command for StatusCommand {
    fn name(&self) -> &'static str {
        "/status"
    }

    fn description(&self) -> &'static str {
        "Show current application and session status"
    }

    fn execute(&self, context: &mut CommandContext) -> Result<CommandOutput, CommandError> {
        let storage_status_str = match &context.storage_health.status {
            StorageStatus::Ready => "Ready".to_string(),
            StorageStatus::Degraded(msg) => format!("Degraded ({})", msg),
            StorageStatus::Unhealthy(msg) => format!("Unhealthy ({})", msg),
        };

        let mode_display = match context.config.general.default_mode.as_str() {
            "simple" => "Simple",
            other => other,
        };

        let model_display = context.active_model.unwrap_or("Not configured").to_string();
        let session_id_str = context.session_id.unwrap_or("None").to_string();
        let session_title_str = context.session_title.unwrap_or("None").to_string();
        let context_usage_str = context
            .context_usage
            .clone()
            .unwrap_or_else(|| "0 / 32,768 (Estimated)".to_string());

        let status = StatusInfo {
            application: "Running".to_string(),
            version: context.version.to_string(),
            session_id: session_id_str,
            session_title: session_title_str,
            messages: context.message_count,
            context_usage: context_usage_str,
            model: model_display,
            mode: mode_display.to_string(),
            storage_status: storage_status_str,
            config_status: "Ready".to_string(),
        };

        Ok(CommandOutput::Status(status))
    }
}

/// Command: `/model` (or `/provider`)
pub struct ModelCommand;

impl Command for ModelCommand {
    fn name(&self) -> &'static str {
        "/model"
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["/provider", "/models"]
    }

    fn description(&self) -> &'static str {
        "Configure default AI model and provider"
    }

    fn execute(&self, context: &mut CommandContext) -> Result<CommandOutput, CommandError> {
        context.request_model_setup();
        Ok(CommandOutput::OpenModelSetup)
    }
}

/// Command: `/switch`
pub struct SwitchCommand;

impl Command for SwitchCommand {
    fn name(&self) -> &'static str {
        "/switch"
    }

    fn description(&self) -> &'static str {
        "Switch active model for current conversation session"
    }

    fn execute(&self, context: &mut CommandContext) -> Result<CommandOutput, CommandError> {
        context.request_model_switch();
        Ok(CommandOutput::OpenModelSwitch)
    }
}

/// Command: `/new`
pub struct NewSessionCommand;

impl Command for NewSessionCommand {
    fn name(&self) -> &'static str {
        "/new"
    }

    fn description(&self) -> &'static str {
        "Start a new conversation session"
    }

    fn execute(&self, context: &mut CommandContext) -> Result<CommandOutput, CommandError> {
        context.request_new_session();
        Ok(CommandOutput::NewSession)
    }
}

/// Command: `/sessions` (or `/history`)
pub struct SessionsCommand;

impl Command for SessionsCommand {
    fn name(&self) -> &'static str {
        "/sessions"
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["/history"]
    }

    fn description(&self) -> &'static str {
        "List and switch conversation sessions"
    }

    fn execute(&self, context: &mut CommandContext) -> Result<CommandOutput, CommandError> {
        context.request_session_picker();
        Ok(CommandOutput::OpenSessionPicker)
    }
}

/// Command: `/tools`
pub struct ToolsCommand;

impl Command for ToolsCommand {
    fn name(&self) -> &'static str {
        "/tools"
    }

    fn description(&self) -> &'static str {
        "List available tools, capabilities, and risk levels"
    }

    fn execute(&self, context: &mut CommandContext) -> Result<CommandOutput, CommandError> {
        let mut output = String::from("HADES TOOLS & CAPABILITIES\n\n");

        if let Some(reg) = context.tool_registry {
            let tools = reg.list();

            let categories = [
                ("FILESYSTEM TOOLS (Workspace-Bound)", "filesystem."),
                ("WORKSPACE TOOLS (Workspace-Bound)", "workspace."),
                ("SHELL & EXECUTION TOOLS", "shell."),
                ("ENVIRONMENT TOOLS (System-Wide)", "environment."),
                (
                    "SYSTEM DIAGNOSTIC & PROCESS TOOLS (System-Wide)",
                    "system.process.",
                ),
                (
                    "SYSTEM DIAGNOSTIC & RUNTIME TOOLS (System-Wide)",
                    "system.runtime.",
                ),
                ("SYSTEM INFORMATION TOOLS (System-Wide)", "system.info"),
                ("NETWORK DIAGNOSTIC TOOLS (System-Wide)", "system.network."),
            ];

            for (cat_name, prefix) in categories {
                let cat_tools: Vec<_> = if cat_name.contains("INFORMATION") {
                    tools
                        .iter()
                        .filter(|t| {
                            t.name == "system.info"
                                || t.name == "system.platform"
                                || t.name == "system.architecture"
                                || t.name == "system.hostname"
                                || t.name == "system.uptime"
                        })
                        .collect()
                } else {
                    tools
                        .iter()
                        .filter(|t| t.name.starts_with(prefix))
                        .collect()
                };

                if !cat_tools.is_empty() {
                    output.push_str(&format!("── {cat_name} ──\n"));
                    for def in cat_tools {
                        let scope = if def.name.starts_with("filesystem.")
                            || def.name.starts_with("workspace.")
                        {
                            "Workspace-Bound"
                        } else {
                            "System-Wide"
                        };

                        let params = if let Some(props) = def
                            .parameters_schema
                            .get("properties")
                            .and_then(|p| p.as_object())
                        {
                            if props.is_empty() {
                                "none".to_string()
                            } else {
                                props.keys().cloned().collect::<Vec<_>>().join(", ")
                            }
                        } else {
                            "none".to_string()
                        };

                        output.push_str(&format!(
                            "  • {:<26} [{:<6} | mut: {:<3} | {}]\n    Params: {}\n    {}\n\n",
                            def.name,
                            def.risk_level.to_string(),
                            if def.is_mutating { "yes" } else { "no" },
                            scope,
                            params,
                            def.description
                        ));
                    }
                }
            }
        } else {
            output.push_str("No active tool registry available.\n");
        }

        Ok(CommandOutput::Text(output))
    }
}

/// Command: `/workspace`
pub struct WorkspaceCommand;

impl Command for WorkspaceCommand {
    fn name(&self) -> &'static str {
        "/workspace"
    }

    fn description(&self) -> &'static str {
        "Display active workspace root, project type, and VCS status"
    }

    fn execute(&self, context: &mut CommandContext) -> Result<CommandOutput, CommandError> {
        let mut output = String::from("WORKSPACE OVERVIEW\n\n");
        if let Some(ws) = context.workspace_info {
            output.push_str(&format!("Name:             {}\n", ws.name()));
            output.push_str(&format!("Root Path:        {}\n", ws.root.display()));
            output.push_str(&format!("Working Dir:      {}\n", ws.current_dir.display()));
            output.push_str(&format!("Project Type:     {}\n", ws.project_type));
            if ws.has_git {
                let branch_str = ws.git_branch.as_deref().unwrap_or("detached");
                output.push_str(&format!(
                    "Git VCS:          Initialized (branch: {branch_str})\n"
                ));
            } else {
                output.push_str("Git VCS:          Not initialized\n");
            }
            output.push_str(&format!(
                "Languages:        {}\n",
                ws.detected_languages.join(", ")
            ));
            output.push_str("\nTop-level layout:\n");
            for entry in &ws.top_level_entries {
                output.push_str(&format!("  - {entry}\n"));
            }
        } else {
            output.push_str("No workspace metadata available.\n");
        }

        Ok(CommandOutput::Text(output))
    }
}

/// Command: `/permissions`
pub struct PermissionsCommand;

impl Command for PermissionsCommand {
    fn name(&self) -> &'static str {
        "/permissions"
    }

    fn description(&self) -> &'static str {
        "Display current session authorizations and security policy"
    }

    fn execute(&self, context: &mut CommandContext) -> Result<CommandOutput, CommandError> {
        let mut output = String::from("SECURITY & PERMISSION POLICY\n\n");
        output.push_str("Default Policy:\n");
        output.push_str("  - SAFE:     Permitted automatically within workspace\n");
        output.push_str("  - LOW:      Permitted automatically within workspace\n");
        output.push_str("  - MEDIUM:   Requires approval unless granted for session\n");
        output.push_str("  - HIGH:     Requires approval per invocation\n");
        output.push_str("  - CRITICAL: Requires explicit confirmation every time\n\n");

        output.push_str("Session Authorizations (granted via 'Allow Session'):\n");
        if context.session_permissions.is_empty() {
            output.push_str("  (None - standard approval prompts apply)\n");
        } else {
            for perm in &context.session_permissions {
                output.push_str(&format!("  - {perm}\n"));
            }
        }

        Ok(CommandOutput::Text(output))
    }
}

/// Command: `/mcp`
pub struct McpCommand;

impl Command for McpCommand {
    fn name(&self) -> &'static str {
        "/mcp"
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["/mcps"]
    }

    fn description(&self) -> &'static str {
        "Manage Model Context Protocol (MCP) servers and tools"
    }

    fn execute(&self, context: &mut CommandContext) -> Result<CommandOutput, CommandError> {
        let mut output = String::from("MODEL CONTEXT PROTOCOL (MCP) SERVERS\n\n");

        if context.mcp_summaries.is_empty() {
            output.push_str("No MCP servers configured.\n\n");
            output.push_str("To configure an MCP server, add it to your config.toml:\n");
            output.push_str("  [mcp.servers.github]\n");
            output.push_str("  transport = \"stdio\"\n");
            output.push_str("  command = \"npx\"\n");
            output.push_str("  args = [\"-y\", \"@modelcontextprotocol/server-github\"]\n");
            output.push_str("  token_env = \"GITHUB_TOKEN\"\n");
            return Ok(CommandOutput::Text(output));
        }

        output.push_str(&format!(
            "  {:<16} {:<12} {:<10} {:<8} {:<10} {}\n",
            "SERVER", "STATUS", "TRANSPORT", "TOOLS", "RESOURCES", "DIAGNOSTICS"
        ));
        output.push_str(&format!("  {}\n", "─".repeat(70)));

        for s in &context.mcp_summaries {
            let status_str = match &s.state {
                hades_mcp::McpServerState::Ready => "READY",
                hades_mcp::McpServerState::Connected => "CONNECTED",
                hades_mcp::McpServerState::Starting => "STARTING",
                hades_mcp::McpServerState::Configured => "CONFIGURED",
                hades_mcp::McpServerState::Disconnected => "DISCONNECTED",
                hades_mcp::McpServerState::Failed(_) => "FAILED",
                hades_mcp::McpServerState::Stopping => "STOPPING",
                hades_mcp::McpServerState::Stopped => "STOPPED",
            };

            let diag = s.error.as_deref().unwrap_or("ok");

            output.push_str(&format!(
                "  {:<16} {:<12} {:<10} {:<8} {:<10} {}\n",
                s.name, status_str, s.transport, s.tool_count, s.resource_count, diag
            ));
        }

        output.push_str("\nDiscovered MCP Tools:\n");
        if let Some(reg) = context.tool_registry {
            let mcp_tools: Vec<_> = reg
                .list()
                .into_iter()
                .filter(|t| t.name.contains('.'))
                .collect();
            if mcp_tools.is_empty() {
                output.push_str("  (No tools registered from active MCP servers)\n");
            } else {
                for t in mcp_tools {
                    output.push_str(&format!(
                        "  • {:<30} [{:<6}] {}\n",
                        t.name,
                        t.risk_level.to_string(),
                        t.description
                    ));
                }
            }
        }

        Ok(CommandOutput::Text(output))
    }
}

/// Command: `/exit`
pub struct ExitCommand;

impl Command for ExitCommand {
    fn name(&self) -> &'static str {
        "/exit"
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["/quit", "/q"]
    }

    fn description(&self) -> &'static str {
        "Exit Hades"
    }

    fn execute(&self, context: &mut CommandContext) -> Result<CommandOutput, CommandError> {
        context.request_shutdown();
        Ok(CommandOutput::Exit)
    }
}

/// Command: `/agents`
pub struct AgentsCommand;

impl Command for AgentsCommand {
    fn name(&self) -> &'static str {
        "/agents"
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["/agent", "/subagents", "/team"]
    }

    fn description(&self) -> &'static str {
        "Inspect specialized collaborative subagents and orchestration status"
    }

    fn execute(&self, context: &mut CommandContext) -> Result<CommandOutput, CommandError> {
        let trimmed = context.raw_input.trim();
        let tokens: Vec<&str> = trimmed.split_whitespace().collect();

        if tokens.len() > 1 && tokens[1].eq_ignore_ascii_case("plan") {
            let objective = tokens[2..].join(" ");
            if objective.is_empty() {
                return Ok(CommandOutput::Text(
                    "Usage: /agents plan <objective>\nExample: /agents plan Audit dependencies and implement security fix"
                        .to_string(),
                ));
            }
            let decision = hades_agent::DecisionEngine::evaluate(&objective, true);
            let plan = hades_agent::DecisionEngine::build_plan(&objective, &decision);
            let mut out = format!(
                "MULTI-AGENT EXECUTION PLAN\n\nObjective: {}\nStrategy:  {}\nReason:    {}\n\nProposed Subtasks:\n",
                objective,
                decision.strategy,
                decision.reason
            );
            if let Some(p) = plan {
                for (i, t) in p.tasks.iter().enumerate() {
                    let deps = if t.dependencies.is_empty() {
                        "none".to_string()
                    } else {
                        t.dependencies.join(", ")
                    };
                    out.push_str(&format!(
                        "  {}. [{}] {}\n     Role: {}\n     Dependencies: {}\n\n",
                        i + 1,
                        t.id,
                        t.title,
                        t.assigned_role.name(),
                        deps
                    ));
                }
            } else {
                out.push_str(
                    "  (Direct single-agent execution recommended - no subagents needed)\n",
                );
            }
            return Ok(CommandOutput::Text(out));
        }

        let mut output = String::from("SPECIALIST SUBAGENTS & ORCHESTRATION ROLES\n\n");
        output.push_str(&format!(
            "  {:<20} {:<8} {:<10} {}\n",
            "ROLE", "MUTATING", "TIMEOUT", "RESPONSIBILITY & SPECIALIZATION"
        ));
        output.push_str(&format!("  {}\n", "─".repeat(80)));

        let roles = vec![
            hades_agent::AgentRole::Planner,
            hades_agent::AgentRole::Explorer,
            hades_agent::AgentRole::Researcher,
            hades_agent::AgentRole::Analyst,
            hades_agent::AgentRole::Implementer,
            hades_agent::AgentRole::Reviewer,
            hades_agent::AgentRole::Tester,
            hades_agent::AgentRole::Debugger,
            hades_agent::AgentRole::SecurityReviewer,
            hades_agent::AgentRole::FileInvestigator,
            hades_agent::AgentRole::SystemInvestigator,
            hades_agent::AgentRole::GeneralSpecialist,
        ];

        for r in roles {
            output.push_str(&format!(
                "  {:<20} {:<8} {:<10} {}\n",
                r.name(),
                if r.is_mutating_allowed() { "Yes" } else { "No" },
                format!("{}s", r.default_timeout_secs()),
                r.description()
            ));
        }

        output.push_str("\nSupported Orchestration Strategies:\n");
        output.push_str(
            "  - Direct:           Single primary agent execution (zero subagent overhead)\n",
        );
        output.push_str("  - Sequential:       Linear dependent subtask execution\n");
        output.push_str(
            "  - Parallel:         Concurrent execution of independent tasks (max 4 concurrent)\n",
        );
        output.push_str(
            "  - Plan & Execute:   Upfront planning -> Subtask execution -> Primary synthesis\n",
        );
        output.push_str("  - Review & Refine:  Implementation -> Independent peer audit -> Primary synthesis\n\n");
        output.push_str("Commands:\n");
        output.push_str("  /agents                   List available roles & strategies\n");
        output.push_str("  /agents plan <objective>  Formulate and inspect a multi-agent plan\n");

        Ok(CommandOutput::Text(output))
    }
}

/// Command to inspect and manage headless browser sidecar and web retrieval.
pub struct BrowserCommand;

impl Command for BrowserCommand {
    fn name(&self) -> &'static str {
        "/browser"
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["/web"]
    }

    fn description(&self) -> &'static str {
        "Inspect browser automation state, active tabs, and web capabilities (/browser, /web)"
    }

    fn execute(&self, context: &mut CommandContext) -> Result<CommandOutput, CommandError> {
        let mut out = String::from("HADES WEB INTELLIGENCE & BROWSER SIDECAR\n\n");
        if let Some(ref status) = context.browser_status {
            out.push_str(&format!("  Browser Engine:     {}\n", status.browser_name));
            out.push_str(&format!("  Version:            {}\n", status.version));
            out.push_str(&format!(
                "  Status:             {}\n",
                if status.is_running {
                    "Running (Active Sidecar)"
                } else {
                    "Idle / Standby"
                }
            ));
            out.push_str(&format!("  Mode:               {}\n", status.mode));
            out.push_str(&format!("  Active Tabs:        {}\n", status.active_tabs));
            if let Some(port) = status.cdp_port {
                out.push_str(&format!("  CDP Port:           {}\n", port));
            }
            if let Some(ref path) = status.binary_path {
                out.push_str(&format!("  Binary Location:    {}\n", path.display()));
            }
        } else {
            out.push_str("  Status:             Idle (Starts automatically on first web/browser tool call)\n");
        }

        out.push_str("\nAvailable Web Retrieval & Automation Capabilities:\n");
        out.push_str("  1. Search Layer:    Fast DuckDuckGo search (web.search)\n");
        out.push_str(
            "  2. Fetch Layer:     Direct HTTP page reading & Markdown conversion (web.fetch)\n",
        );
        out.push_str(
            "  3. Browser Sidecar: Headless Chromium engine (browser.open, browser.snapshot)\n",
        );
        out.push_str(
            "  4. Actions:         Accessibility-first interaction (browser.click, browser.fill)\n",
        );
        out.push_str(
            "  5. Artifacts:       Screenshots & PDF documents (browser.screenshot, browser.pdf)\n",
        );
        out.push_str("  6. Diagnostics:     Console & Network telemetry (browser.console, browser.network)\n\n");
        out.push_str("Usage:\n");
        out.push_str("  /browser                  Show browser and web status\n");
        out.push_str("  /browser status           Show detailed status\n");

        Ok(CommandOutput::Text(out))
    }
}

/// Extensible command registry storing and dispatching commands.
#[derive(Default)]
pub struct CommandRegistry {
    commands: Vec<Arc<dyn Command>>,
    lookup: HashMap<String, usize>,
}

impl CommandRegistry {
    /// Creates an empty `CommandRegistry`.
    pub fn new() -> Self {
        Self {
            commands: Vec::new(),
            lookup: HashMap::new(),
        }
    }

    /// Creates a registry pre-populated with standard default commands (`/help`, `/status`, `/model`, `/switch`, `/new`, `/sessions`, `/tools`, `/workspace`, `/permissions`, `/mcp`, `/agents`, `/browser`, `/exit`).
    pub fn with_defaults() -> Self {
        let mut registry = Self::new();
        registry.register(HelpCommand);
        registry.register(StatusCommand);
        registry.register(ModelCommand);
        registry.register(SwitchCommand);
        registry.register(NewSessionCommand);
        registry.register(SessionsCommand);
        registry.register(ToolsCommand);
        registry.register(WorkspaceCommand);
        registry.register(PermissionsCommand);
        registry.register(McpCommand);
        registry.register(AgentsCommand);
        registry.register(BrowserCommand);
        registry.register(ExitCommand);
        registry
    }

    /// Registers a new command into the registry.
    pub fn register<C: Command + 'static>(&mut self, command: C) {
        let idx = self.commands.len();
        let cmd_arc = Arc::new(command);

        self.lookup.insert(cmd_arc.name().to_lowercase(), idx);
        for alias in cmd_arc.aliases() {
            self.lookup.insert(alias.to_lowercase(), idx);
        }

        self.commands.push(cmd_arc);
    }

    /// Finds a command by name or alias.
    pub fn find(&self, name_or_alias: &str) -> Option<Arc<dyn Command>> {
        let key = name_or_alias.trim().to_lowercase();
        self.lookup
            .get(&key)
            .map(|&idx| Arc::clone(&self.commands[idx]))
    }

    /// Lists all unique registered commands in order of registration.
    pub fn list(&self) -> Vec<CommandInfo> {
        self.commands
            .iter()
            .map(|cmd| CommandInfo {
                name: cmd.name().to_string(),
                aliases: cmd.aliases().iter().map(|s| s.to_string()).collect(),
                description: cmd.description().to_string(),
            })
            .collect()
    }

    /// Formats help entries for all registered commands.
    pub fn help_entries(&self) -> Vec<HelpEntry> {
        self.commands
            .iter()
            .map(|cmd| HelpEntry {
                name: cmd.name().to_string(),
                description: cmd.description().to_string(),
            })
            .collect()
    }

    /// Parses input, finds the matching command, and executes it.
    pub fn execute(
        &self,
        input: &str,
        context: &mut CommandContext,
    ) -> Result<CommandOutput, CommandError> {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return Err(CommandError::EmptyInput);
        }

        // Extract command name (first word)
        let command_token = trimmed.split_whitespace().next().unwrap_or("");

        match self.find(command_token) {
            Some(cmd) => cmd.execute(context),
            None => Err(CommandError::UnknownCommand(command_token.to_string())),
        }
    }
}
