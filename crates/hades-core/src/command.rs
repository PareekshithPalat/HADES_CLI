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
                    writeln!(f, "{:<12} {}", entry.name, entry.description)?;
                }
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
        }
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
        output.push_str(&format!(
            "{:<20} {:<10} {:<8} {}\n",
            "NAME", "RISK", "MUTATING", "DESCRIPTION"
        ));
        output.push_str(&format!("{:-<20} {:-<10} {:-<8} {:-<40}\n", "", "", "", ""));

        if let Some(reg) = context.tool_registry {
            for def in reg.list() {
                output.push_str(&format!(
                    "{:<20} {:<10} {:<8} {}\n",
                    def.name,
                    def.risk_level.to_string(),
                    if def.is_mutating { "yes" } else { "no" },
                    def.description
                ));
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

    /// Creates a registry pre-populated with standard default commands (`/help`, `/status`, `/model`, `/switch`, `/new`, `/sessions`, `/tools`, `/workspace`, `/permissions`, `/exit`).
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
