use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::error::ConfigError;

pub const CURRENT_CONFIG_VERSION: &str = "0.1.0";

/// Top-level Hades application configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HadesConfig {
    /// Schema/Configuration version.
    #[serde(default = "default_version")]
    pub version: String,

    /// General application settings.
    #[serde(default)]
    pub general: GeneralConfig,

    /// UI-related settings.
    #[serde(default)]
    pub ui: UiConfig,

    /// Configured active model and provider settings, if configured.
    #[serde(default)]
    pub model: Option<ActiveModelConfig>,

    /// Model Context Protocol (MCP) settings and server definitions.
    #[serde(default)]
    pub mcp: McpConfig,
}

fn default_version() -> String {
    CURRENT_CONFIG_VERSION.to_string()
}

impl Default for HadesConfig {
    fn default() -> Self {
        Self {
            version: default_version(),
            general: GeneralConfig::default(),
            ui: UiConfig::default(),
            model: None,
            mcp: McpConfig::default(),
        }
    }
}

impl HadesConfig {
    /// Validates the configuration parameters.
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.version.trim().is_empty() {
            return Err(ConfigError::Validation(
                "Configuration version must not be empty".to_string(),
            ));
        }

        self.general.validate()?;
        self.ui.validate()?;

        if let Some(ref m) = self.model {
            m.validate()?;
        }

        self.mcp.validate()?;

        Ok(())
    }
}

/// General Hades application configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GeneralConfig {
    /// Application name identifier.
    #[serde(default = "default_app_name")]
    pub app_name: String,

    /// Default interaction mode (e.g., "simple").
    #[serde(default = "default_mode")]
    pub default_mode: String,
}

fn default_app_name() -> String {
    "hades".to_string()
}

fn default_mode() -> String {
    "simple".to_string()
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            app_name: default_app_name(),
            default_mode: default_mode(),
        }
    }
}

impl GeneralConfig {
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.app_name.trim().is_empty() {
            return Err(ConfigError::Validation(
                "Application name cannot be empty".to_string(),
            ));
        }
        if self.default_mode.trim().is_empty() {
            return Err(ConfigError::Validation(
                "Default mode cannot be empty".to_string(),
            ));
        }
        Ok(())
    }
}

/// UI presentation configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiConfig {
    /// Theme name (e.g., "dark", "light", "default").
    #[serde(default = "default_theme")]
    pub theme: String,

    /// Whether to show the bottom status bar.
    #[serde(default = "default_show_status_bar")]
    pub show_status_bar: bool,
}

fn default_theme() -> String {
    "dark".to_string()
}

fn default_show_status_bar() -> bool {
    true
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            show_status_bar: default_show_status_bar(),
        }
    }
}

impl UiConfig {
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.theme.trim().is_empty() {
            return Err(ConfigError::Validation(
                "UI theme cannot be empty".to_string(),
            ));
        }
        Ok(())
    }
}

/// Active model and provider configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActiveModelConfig {
    /// Configured provider identifier (e.g. "openai", "groq", "ollama", "custom").
    pub provider_id: String,

    /// Selected model identifier (e.g. "gpt-4o", "llama-3.3-70b-versatile").
    pub model_id: String,

    /// Optional custom endpoint URL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,
}

impl ActiveModelConfig {
    /// Creates a new active model configuration.
    pub fn new(provider_id: impl Into<String>, model_id: impl Into<String>) -> Self {
        Self {
            provider_id: provider_id.into(),
            model_id: model_id.into(),
            endpoint: None,
        }
    }

    /// Sets custom endpoint URL.
    pub fn with_endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.endpoint = Some(endpoint.into());
        self
    }

    /// Validates model configuration.
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.provider_id.trim().is_empty() {
            return Err(ConfigError::Validation(
                "Provider ID cannot be empty".to_string(),
            ));
        }
        if self.model_id.trim().is_empty() {
            return Err(ConfigError::Validation(
                "Model ID cannot be empty".to_string(),
            ));
        }
        Ok(())
    }
}

/// Top-level Model Context Protocol (MCP) configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpConfig {
    /// Whether the MCP subsystem is globally enabled.
    #[serde(default = "default_mcp_enabled")]
    pub enabled: bool,

    /// Map of configured MCP servers.
    #[serde(default)]
    pub servers: BTreeMap<String, McpServerConfig>,
}

fn default_mcp_enabled() -> bool {
    true
}

impl Default for McpConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            servers: BTreeMap::new(),
        }
    }
}

impl McpConfig {
    pub fn validate(&self) -> Result<(), ConfigError> {
        for (name, server) in &self.servers {
            if name.trim().is_empty() {
                return Err(ConfigError::Validation(
                    "MCP server name cannot be empty".to_string(),
                ));
            }
            server.validate(name)?;
        }
        Ok(())
    }
}

/// Transport mechanism for an MCP server connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum McpTransportType {
    #[default]
    Stdio,
    Http,
}

fn default_server_enabled() -> bool {
    true
}

fn default_server_auto_start() -> bool {
    true
}

fn default_mcp_timeout() -> u64 {
    30
}

/// Configuration for an individual MCP server.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpServerConfig {
    /// Transport mechanism (stdio or http).
    #[serde(default)]
    pub transport: McpTransportType,

    /// Executable command for STDIO transport (e.g. "npx", "uvx", "docker", "python").
    #[serde(default)]
    pub command: Option<String>,

    /// Command-line arguments for STDIO transport.
    #[serde(default)]
    pub args: Vec<String>,

    /// Optional working directory for the spawned process.
    #[serde(default)]
    pub working_dir: Option<String>,

    /// Environment variables to inject into the process.
    #[serde(default)]
    pub env: BTreeMap<String, String>,

    /// Environment variable name referencing an API token/secret (prevents hardcoding tokens in config).
    #[serde(default)]
    pub token_env: Option<String>,

    /// HTTP endpoint URL (for HTTP transport).
    #[serde(default)]
    pub url: Option<String>,

    /// Whether this MCP server is enabled.
    #[serde(default = "default_server_enabled")]
    pub enabled: bool,

    /// Whether to automatically connect on Hades startup.
    #[serde(default = "default_server_auto_start")]
    pub auto_start: bool,

    /// Execution and request timeout in seconds.
    #[serde(default = "default_mcp_timeout")]
    pub timeout_secs: u64,
}

impl Default for McpServerConfig {
    fn default() -> Self {
        Self {
            transport: McpTransportType::Stdio,
            command: None,
            args: Vec::new(),
            working_dir: None,
            env: BTreeMap::new(),
            token_env: None,
            url: None,
            enabled: true,
            auto_start: true,
            timeout_secs: 30,
        }
    }
}

impl McpServerConfig {
    pub fn validate(&self, server_name: &str) -> Result<(), ConfigError> {
        match self.transport {
            McpTransportType::Stdio => {
                if let Some(ref cmd) = self.command {
                    if cmd.trim().is_empty() {
                        return Err(ConfigError::Validation(format!(
                            "MCP server '{server_name}' command cannot be empty"
                        )));
                    }
                }
            }
            McpTransportType::Http => {
                if let Some(ref u) = self.url {
                    if u.trim().is_empty() {
                        return Err(ConfigError::Validation(format!(
                            "MCP server '{server_name}' URL cannot be empty"
                        )));
                    }
                }
            }
        }
        Ok(())
    }
}
