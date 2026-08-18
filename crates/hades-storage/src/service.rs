use serde::de::DeserializeOwned;
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

use crate::error::StorageError;
use crate::model::{StorageHealth, StorageStatus};

/// Structured persistent storage service for Hades.
#[derive(Debug, Clone)]
pub struct StorageService {
    root_dir: PathBuf,
}

impl StorageService {
    /// Creates a new `StorageService` using the standard directory `~/.hades/data/`.
    pub fn new() -> Result<Self, StorageError> {
        let root = Self::default_storage_dir()?;
        Ok(Self::with_root(root))
    }

    /// Creates a new `StorageService` using a custom directory.
    pub fn with_root<P: Into<PathBuf>>(root_dir: P) -> Self {
        Self {
            root_dir: root_dir.into(),
        }
    }

    /// Returns the root storage directory.
    pub fn root_dir(&self) -> &Path {
        &self.root_dir
    }

    /// Returns default path `~/.hades/data/` in a cross-platform manner.
    pub fn default_storage_dir() -> Result<PathBuf, StorageError> {
        let home = dirs::home_dir().ok_or(StorageError::HomeDirectoryNotFound)?;
        Ok(home.join(".hades").join("data"))
    }

    /// Validates key string to prevent directory traversal and invalid filename characters.
    fn validate_key(&self, key: &str) -> Result<(), StorageError> {
        if key.is_empty() {
            return Err(StorageError::InvalidKey("Key cannot be empty".to_string()));
        }
        if key.contains("..") || key.contains('/') || key.contains('\\') || key.contains('\0') {
            return Err(StorageError::InvalidKey(key.to_string()));
        }
        let is_valid = key
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_' || c == '.');
        if !is_valid {
            return Err(StorageError::InvalidKey(key.to_string()));
        }
        Ok(())
    }

    fn key_to_path(&self, key: &str) -> Result<PathBuf, StorageError> {
        self.validate_key(key)?;
        Ok(self.root_dir.join(format!("{}.json", key)))
    }

    /// Initializes the storage subsystem by ensuring root directory exists and is writable.
    pub fn initialize(&self) -> Result<(), StorageError> {
        if !self.root_dir.exists() {
            info!(path = %self.root_dir.display(), "Creating storage root directory");
            fs::create_dir_all(&self.root_dir).map_err(|e| StorageError::InitializationFailed {
                path: self.root_dir.clone(),
                message: e.to_string(),
            })?;
        }
        Ok(())
    }

    /// Saves a serializable value under the given key.
    pub fn save<T: Serialize>(&self, key: &str, value: &T) -> Result<(), StorageError> {
        self.initialize()?;
        let path = self.key_to_path(key)?;
        let json_str = serde_json::to_string_pretty(value)
            .map_err(|e| StorageError::Serialization(e.to_string()))?;

        // Write directly to file
        fs::write(&path, json_str).map_err(|e| StorageError::Io {
            path: path.clone(),
            source: e,
        })?;

        debug!(key = %key, path = %path.display(), "Stored key successfully");
        Ok(())
    }

    /// Loads a deserializable value associated with the given key.
    pub fn load<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>, StorageError> {
        let path = self.key_to_path(key)?;
        if !path.exists() {
            return Ok(None);
        }

        let content = fs::read_to_string(&path).map_err(|e| StorageError::Io {
            path: path.clone(),
            source: e,
        })?;

        let value: T = serde_json::from_str(&content)
            .map_err(|e| StorageError::Deserialization(e.to_string()))?;

        debug!(key = %key, "Loaded key successfully");
        Ok(Some(value))
    }

    /// Deletes the item associated with the given key.
    pub fn delete(&self, key: &str) -> Result<bool, StorageError> {
        let path = self.key_to_path(key)?;
        if !path.exists() {
            return Ok(false);
        }

        fs::remove_file(&path).map_err(|e| StorageError::Io {
            path: path.clone(),
            source: e,
        })?;

        debug!(key = %key, "Deleted key successfully");
        Ok(true)
    }

    /// Checks the operational health of the storage subsystem.
    pub fn health(&self) -> Result<StorageHealth, StorageError> {
        if !self.root_dir.exists() {
            if let Err(e) = self.initialize() {
                return Ok(StorageHealth {
                    status: StorageStatus::Unhealthy(e.to_string()),
                    root_dir: self.root_dir.clone(),
                    writable: false,
                });
            }
        }

        // Test writability with a small probe file
        let probe_file = self.root_dir.join(".health_probe.tmp");
        let write_test = fs::write(&probe_file, b"ok");
        let writable = match write_test {
            Ok(_) => {
                let _ = fs::remove_file(&probe_file);
                true
            }
            Err(e) => {
                warn!(error = %e, "Storage health check write test failed");
                false
            }
        };

        let status = if writable {
            StorageStatus::Ready
        } else {
            StorageStatus::Degraded("Storage directory is not writable".to_string())
        };

        Ok(StorageHealth {
            status,
            root_dir: self.root_dir.clone(),
            writable,
        })
    }
}
