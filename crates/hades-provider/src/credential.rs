use async_trait::async_trait;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;
use tracing::debug;

/// Errors encountered while reading or storing provider credentials.
#[derive(Debug, Error)]
pub enum CredentialError {
    #[error("I/O error with credential storage at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("Credential not found for provider: {0}")]
    NotFound(String),

    #[error("Credential backend unavailable: {0}")]
    StorageUnavailable(String),
}

/// Secure container for sensitive secret strings (e.g. API keys, auth tokens).
///
/// Implements redacting `Debug` and `Display` implementations to prevent accidental
/// leakage in logging, panics, formatting, and diagnostic transcripts.
#[derive(Clone, PartialEq, Eq)]
pub struct CredentialSecret(String);

impl CredentialSecret {
    /// Creates a new secret container.
    pub fn new(secret: impl Into<String>) -> Self {
        Self(secret.into())
    }

    /// Exposes the underlying plaintext secret value for authorized network requests.
    pub fn expose_secret(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for CredentialSecret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[REDACTED]")
    }
}

impl fmt::Display for CredentialSecret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[REDACTED]")
    }
}

impl From<String> for CredentialSecret {
    fn from(s: String) -> Self {
        Self::new(s)
    }
}

impl From<&str> for CredentialSecret {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

impl Serialize for CredentialSecret {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for CredentialSecret {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Ok(Self::new(s))
    }
}

/// Normalized provider credentials for authentication and endpoint routing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Credential {
    /// Target provider identifier.
    pub provider_id: String,

    /// Optional API key / bearer token.
    pub api_key: Option<CredentialSecret>,

    /// Custom base URL endpoint override (e.g. `http://localhost:11434/v1`).
    pub endpoint: Option<String>,

    /// Optional additional headers (e.g., organization IDs, custom auth headers).
    pub custom_headers: HashMap<String, CredentialSecret>,
}

impl Credential {
    /// Creates a new `Credential` for a provider with an API key.
    pub fn with_api_key(provider_id: impl Into<String>, api_key: impl Into<String>) -> Self {
        Self {
            provider_id: provider_id.into(),
            api_key: Some(CredentialSecret::new(api_key)),
            endpoint: None,
            custom_headers: HashMap::new(),
        }
    }

    /// Creates a new `Credential` for a provider with an endpoint URL and optional key.
    pub fn with_endpoint(
        provider_id: impl Into<String>,
        endpoint: impl Into<String>,
        api_key: Option<String>,
    ) -> Self {
        Self {
            provider_id: provider_id.into(),
            api_key: api_key.map(CredentialSecret::new),
            endpoint: Some(endpoint.into()),
            custom_headers: HashMap::new(),
        }
    }
}

/// Abstract storage backend for provider credentials.
#[async_trait]
pub trait CredentialBackend: Send + Sync {
    /// Retrieves the stored credential for a given provider.
    async fn get_credential(
        &self,
        provider_id: &str,
    ) -> Result<Option<Credential>, CredentialError>;

    /// Persists or updates the credential for a provider.
    async fn store_credential(&self, credential: &Credential) -> Result<(), CredentialError>;

    /// Deletes the credential for a provider. Returns true if removed, false if not found.
    async fn delete_credential(&self, provider_id: &str) -> Result<bool, CredentialError>;
}

/// File-based credential storage engine.
///
/// Persists credentials in JSON format at a designated secure location (default `~/.hades/credentials.json`).
/// NOTE: In Phase 1 this provides a clean platform abstraction; native OS keyrings (Windows Credential
/// Manager, macOS Keychain, Linux Secret Service) are scheduled for subsequent security hardening phases.
#[derive(Debug, Clone)]
pub struct FileCredentialBackend {
    path: PathBuf,
}

impl FileCredentialBackend {
    /// Creates a `FileCredentialBackend` at the default platform location `~/.hades/credentials.json`.
    pub fn default_location() -> Result<Self, CredentialError> {
        let home_dir = dirs::home_dir().ok_or_else(|| {
            CredentialError::StorageUnavailable(
                "Unable to determine user home directory".to_string(),
            )
        })?;
        let path = home_dir.join(".hades").join("credentials.json");
        Ok(Self { path })
    }

    /// Creates a `FileCredentialBackend` with an explicit file path.
    pub fn with_path(path: impl AsRef<Path>) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
        }
    }

    fn read_store(&self) -> Result<HashMap<String, Credential>, CredentialError> {
        if !self.path.exists() {
            return Ok(HashMap::new());
        }
        let content = fs::read_to_string(&self.path).map_err(|e| CredentialError::Io {
            path: self.path.clone(),
            source: e,
        })?;
        let store: HashMap<String, Credential> = serde_json::from_str(&content)
            .map_err(|e| CredentialError::Serialization(e.to_string()))?;
        Ok(store)
    }

    fn write_store(&self, store: &HashMap<String, Credential>) -> Result<(), CredentialError> {
        if let Some(parent) = self.path.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent).map_err(|e| CredentialError::Io {
                    path: parent.to_path_buf(),
                    source: e,
                })?;
            }
        }
        let json = serde_json::to_string_pretty(store)
            .map_err(|e| CredentialError::Serialization(e.to_string()))?;
        fs::write(&self.path, json).map_err(|e| CredentialError::Io {
            path: self.path.clone(),
            source: e,
        })?;
        Ok(())
    }
}

#[async_trait]
impl CredentialBackend for FileCredentialBackend {
    async fn get_credential(
        &self,
        provider_id: &str,
    ) -> Result<Option<Credential>, CredentialError> {
        debug!(provider = %provider_id, "Fetching credential from backend");
        let store = self.read_store()?;
        Ok(store.get(provider_id).cloned())
    }

    async fn store_credential(&self, credential: &Credential) -> Result<(), CredentialError> {
        debug!(provider = %credential.provider_id, "Storing credential in backend");
        let mut store = self.read_store()?;
        store.insert(credential.provider_id.clone(), credential.clone());
        self.write_store(&store)?;
        Ok(())
    }

    async fn delete_credential(&self, provider_id: &str) -> Result<bool, CredentialError> {
        debug!(provider = %provider_id, "Removing credential from backend");
        let mut store = self.read_store()?;
        let removed = store.remove(provider_id).is_some();
        if removed {
            self.write_store(&store)?;
        }
        Ok(removed)
    }
}
