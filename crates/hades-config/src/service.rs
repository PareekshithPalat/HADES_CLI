use std::fs;
use std::path::{Path, PathBuf};

use tracing::{debug, info};

use crate::error::ConfigError;
use crate::model::HadesConfig;

/// Service responsible for managing Hades application configuration.
#[derive(Debug, Clone)]
pub struct ConfigService {
    config_path: PathBuf,
}

impl ConfigService {
    /// Creates a new `ConfigService` targeting the standard user configuration path (`~/.hades/config.toml`).
    pub fn new() -> Result<Self, ConfigError> {
        let path = Self::default_config_path()?;
        Ok(Self::with_path(path))
    }

    /// Creates a new `ConfigService` targeting a specific file path.
    pub fn with_path<P: Into<PathBuf>>(path: P) -> Self {
        Self {
            config_path: path.into(),
        }
    }

    /// Returns the target configuration file path.
    pub fn config_path(&self) -> &Path {
        &self.config_path
    }

    /// Returns the default path `~/.hades/config.toml` in a cross-platform manner.
    pub fn default_config_path() -> Result<PathBuf, ConfigError> {
        let home = dirs::home_dir().ok_or(ConfigError::HomeDirectoryNotFound)?;
        Ok(home.join(".hades").join("config.toml"))
    }

    /// Returns the default directory `~/.hades/` in a cross-platform manner.
    pub fn default_base_dir() -> Result<PathBuf, ConfigError> {
        let home = dirs::home_dir().ok_or(ConfigError::HomeDirectoryNotFound)?;
        Ok(home.join(".hades"))
    }

    /// Loads the configuration from the configured path. If the file does not exist,
    /// creates it with default settings and returns the default configuration.
    pub fn load_or_create(&self) -> Result<HadesConfig, ConfigError> {
        if self.config_path.exists() {
            self.load()
        } else {
            info!(
                path = %self.config_path.display(),
                "Config file not found, creating default configuration"
            );
            let default_config = HadesConfig::default();
            self.save(&default_config)?;
            Ok(default_config)
        }
    }

    /// Loads and validates configuration from the file.
    pub fn load(&self) -> Result<HadesConfig, ConfigError> {
        debug!(path = %self.config_path.display(), "Loading configuration");
        let content = fs::read_to_string(&self.config_path).map_err(|e| ConfigError::Io {
            path: self.config_path.clone(),
            source: e,
        })?;

        let config: HadesConfig = toml::from_str(&content).map_err(|e| ConfigError::Parse {
            path: self.config_path.clone(),
            source: Box::new(e),
        })?;

        config.validate()?;
        Ok(config)
    }

    /// Saves and validates the given configuration to the file.
    pub fn save(&self, config: &HadesConfig) -> Result<(), ConfigError> {
        config.validate()?;

        if let Some(parent) = self.config_path.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent).map_err(|e| ConfigError::Io {
                    path: parent.to_path_buf(),
                    source: e,
                })?;
            }
        }

        let serialized = toml::to_string_pretty(config)?;
        fs::write(&self.config_path, serialized).map_err(|e| ConfigError::Io {
            path: self.config_path.clone(),
            source: e,
        })?;

        debug!(path = %self.config_path.display(), "Configuration saved successfully");
        Ok(())
    }
}
