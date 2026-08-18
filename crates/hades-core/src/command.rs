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
    pub model: String,
    pub mode: String,
    pub storage_status: String,
    pub config_status: String,
}

impl fmt::Display for StatusInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "HADES STATUS")?;
        writeln!(f)?;
        writeln!(f, "Application: {}", self.application)?;
        writeln!(f, "Version: {}", self.version)?;
        writeln!(f, "Model: {}", self.model)?;
        writeln!(f, "Mode: {}", self.mode)?;
        writeln!(f, "Storage: {}", self.storage_status)?;
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
            Self::OpenModelSetup => write!(f, "Opening AI model setup..."),
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

/// Context provided to commands during execution.
pub struct CommandContext<'a> {
    pub app_state: AppState,
    pub config: &'a HadesConfig,
    pub storage_health: &'a StorageHealth,
    pub active_model: Option<&'a str>,
    pub version: &'a str,
    pub shutdown_requested: bool,
    pub open_model_setup_requested: bool,
    pub available_commands: Vec<HelpEntry>,
}

impl<'a> CommandContext<'a> {
    pub fn new(
        app_state: AppState,
        config: &'a HadesConfig,
        storage_health: &'a StorageHealth,
        active_model: Option<&'a str>,
        version: &'a str,
        available_commands: Vec<HelpEntry>,
    ) -> Self {
        Self {
            app_state,
            config,
            storage_health,
            active_model,
            version,
            shutdown_requested: false,
            open_model_setup_requested: false,
            available_commands,
        }
    }

    /// Requests application shutdown from within a command.
    pub fn request_shutdown(&mut self) {
        self.shutdown_requested = true;
    }

    /// Requests opening the interactive model setup workflow.
    pub fn request_model_setup(&mut self) {
        self.open_model_setup_requested = true;
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

// Built-in Phase 0 & Phase 1 Commands

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
        "Show current application status"
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

        let status = StatusInfo {
            application: "Running".to_string(),
            version: context.version.to_string(),
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
        "Configure active AI model and provider"
    }

    fn execute(&self, context: &mut CommandContext) -> Result<CommandOutput, CommandError> {
        context.request_model_setup();
        Ok(CommandOutput::OpenModelSetup)
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

    /// Creates a registry pre-populated with standard default commands (`/help`, `/status`, `/model`, `/exit`).
    pub fn with_defaults() -> Self {
        let mut registry = Self::new();
        registry.register(HelpCommand);
        registry.register(StatusCommand);
        registry.register(ModelCommand);
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
