use serde::{Deserialize, Serialize};

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
