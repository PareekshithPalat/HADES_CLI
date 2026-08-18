use async_trait::async_trait;
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::error::StorageError;
use crate::model::{SessionMetadata, SessionRecord};

/// Abstract repository interface for persistent session management.
#[async_trait]
pub trait SessionRepository: Send + Sync {
    /// Creates and persists a new session.
    async fn create_session(
        &self,
        title: Option<String>,
        active_provider: Option<String>,
        active_model: Option<String>,
    ) -> Result<SessionRecord, StorageError>;

    /// Retrieves a session record by its unique identifier.
    async fn get_session(&self, session_id: &str) -> Result<Option<SessionRecord>, StorageError>;

    /// Persists or updates a session record atomically.
    async fn save_session(&self, record: &SessionRecord) -> Result<(), StorageError>;

    /// Lists metadata for all stored sessions, sorted from most recently updated to oldest.
    async fn list_sessions(&self) -> Result<Vec<SessionMetadata>, StorageError>;

    /// Deletes a session by identifier.
    async fn delete_session(&self, session_id: &str) -> Result<bool, StorageError>;

    /// Renames a session with a new human-readable title.
    async fn rename_session(&self, session_id: &str, new_title: &str) -> Result<(), StorageError>;

    /// Retrieves the ID of the most recently active session.
    async fn get_active_session_id(&self) -> Result<Option<String>, StorageError>;

    /// Sets the ID of the active session.
    async fn set_active_session_id(&self, session_id: &str) -> Result<(), StorageError>;
}

/// Filesystem-backed persistent session repository with atomic writes and schema versioning.
#[derive(Debug, Clone)]
pub struct FileSessionRepository {
    sessions_dir: PathBuf,
}

impl FileSessionRepository {
    const ACTIVE_SESSION_FILE: &'static str = "_active_session.json";

    /// Creates a new repository targeting the default `~/.hades/sessions/` directory.
    pub fn new() -> Result<Self, StorageError> {
        let home = dirs::home_dir().ok_or(StorageError::HomeDirectoryNotFound)?;
        let sessions_dir = home.join(".hades").join("sessions");
        Ok(Self::with_dir(sessions_dir))
    }

    /// Creates a new repository with a custom directory.
    pub fn with_dir<P: Into<PathBuf>>(sessions_dir: P) -> Self {
        Self {
            sessions_dir: sessions_dir.into(),
        }
    }

    /// Returns the sessions storage directory path.
    pub fn sessions_dir(&self) -> &Path {
        &self.sessions_dir
    }

    /// Ensures the sessions storage directory exists.
    pub fn initialize(&self) -> Result<(), StorageError> {
        if !self.sessions_dir.exists() {
            info!(path = %self.sessions_dir.display(), "Creating sessions storage directory");
            fs::create_dir_all(&self.sessions_dir).map_err(|e| {
                StorageError::InitializationFailed {
                    path: self.sessions_dir.clone(),
                    message: e.to_string(),
                }
            })?;
        }
        Ok(())
    }

    fn session_file_path(&self, session_id: &str) -> PathBuf {
        self.sessions_dir.join(format!("{}.json", session_id))
    }

    fn active_session_path(&self) -> PathBuf {
        self.sessions_dir.join(Self::ACTIVE_SESSION_FILE)
    }

    /// Writes content to a file atomically via temporary file and rename.
    fn atomic_write(&self, path: &Path, content: &str) -> Result<(), StorageError> {
        self.initialize()?;
        let tmp_path = path.with_extension(format!("tmp.{}", Uuid::new_v4()));

        fs::write(&tmp_path, content).map_err(|e| StorageError::Io {
            path: tmp_path.clone(),
            source: e,
        })?;

        fs::rename(&tmp_path, path).map_err(|e| {
            let _ = fs::remove_file(&tmp_path);
            StorageError::Io {
                path: path.to_path_buf(),
                source: e,
            }
        })?;

        Ok(())
    }
}

#[async_trait]
impl SessionRepository for FileSessionRepository {
    async fn create_session(
        &self,
        title: Option<String>,
        active_provider: Option<String>,
        active_model: Option<String>,
    ) -> Result<SessionRecord, StorageError> {
        let record = SessionRecord::new(title, active_provider, active_model);
        self.save_session(&record).await?;
        self.set_active_session_id(&record.metadata.id).await?;
        info!(session_id = %record.metadata.id, title = %record.metadata.title, "Created and activated new session");
        Ok(record)
    }

    async fn get_session(&self, session_id: &str) -> Result<Option<SessionRecord>, StorageError> {
        let path = self.session_file_path(session_id);
        if !path.exists() {
            return Ok(None);
        }

        let content = match fs::read_to_string(&path) {
            Ok(c) => c,
            Err(e) => {
                warn!(session_id = %session_id, error = %e, "Failed to read session file");
                return Err(StorageError::Io { path, source: e });
            }
        };

        match serde_json::from_str::<SessionRecord>(&content) {
            Ok(record) => {
                debug!(session_id = %session_id, messages = record.messages.len(), "Loaded session successfully");
                Ok(Some(record))
            }
            Err(e) => {
                warn!(session_id = %session_id, error = %e, "Corrupted session record detected");
                Err(StorageError::Deserialization(format!(
                    "Corrupted session {session_id}: {e}"
                )))
            }
        }
    }

    async fn save_session(&self, record: &SessionRecord) -> Result<(), StorageError> {
        let path = self.session_file_path(&record.metadata.id);
        let json_str = serde_json::to_string_pretty(record)
            .map_err(|e| StorageError::Serialization(e.to_string()))?;

        self.atomic_write(&path, &json_str)?;
        debug!(session_id = %record.metadata.id, messages = record.messages.len(), "Persisted session atomically");
        Ok(())
    }

    async fn list_sessions(&self) -> Result<Vec<SessionMetadata>, StorageError> {
        self.initialize()?;
        let mut list = Vec::new();

        let entries = match fs::read_dir(&self.sessions_dir) {
            Ok(e) => e,
            Err(e) => {
                return Err(StorageError::Io {
                    path: self.sessions_dir.clone(),
                    source: e,
                })
            }
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                let filename = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or_default();
                if filename.starts_with('_') || filename.contains(".tmp.") {
                    continue;
                }
                if path.extension().and_then(|ext| ext.to_str()) == Some("json") {
                    if let Ok(content) = fs::read_to_string(&path) {
                        if let Ok(record) = serde_json::from_str::<SessionRecord>(&content) {
                            list.push(record.metadata);
                        } else {
                            warn!(file = %filename, "Skipping unparseable session file in listing");
                        }
                    }
                }
            }
        }

        // Sort descending by updated_at (most recently updated first)
        list.sort_by_key(|b| std::cmp::Reverse(b.updated_at));
        Ok(list)
    }

    async fn delete_session(&self, session_id: &str) -> Result<bool, StorageError> {
        let path = self.session_file_path(session_id);
        if !path.exists() {
            return Ok(false);
        }

        fs::remove_file(&path).map_err(|e| StorageError::Io {
            path: path.clone(),
            source: e,
        })?;

        // If the active session is the deleted one, clear the active pointer
        if let Ok(Some(active_id)) = self.get_active_session_id().await {
            if active_id == session_id {
                let _ = fs::remove_file(self.active_session_path());
            }
        }

        info!(session_id = %session_id, "Deleted session successfully");
        Ok(true)
    }

    async fn rename_session(&self, session_id: &str, new_title: &str) -> Result<(), StorageError> {
        let trimmed = new_title.trim();
        if trimmed.is_empty() {
            return Err(StorageError::InvalidKey(
                "Session title cannot be empty".to_string(),
            ));
        }

        let mut record = self
            .get_session(session_id)
            .await?
            .ok_or_else(|| StorageError::InvalidKey(format!("Session {session_id} not found")))?;

        record.metadata.title = trimmed.to_string();
        record.metadata.updated_at = chrono::Utc::now();
        self.save_session(&record).await?;
        info!(session_id = %session_id, new_title = %trimmed, "Renamed session successfully");
        Ok(())
    }

    async fn get_active_session_id(&self) -> Result<Option<String>, StorageError> {
        let path = self.active_session_path();
        if !path.exists() {
            return Ok(None);
        }

        match fs::read_to_string(&path) {
            Ok(content) => {
                let id = content.trim().to_string();
                if id.is_empty() {
                    Ok(None)
                } else {
                    Ok(Some(id))
                }
            }
            Err(_) => Ok(None),
        }
    }

    async fn set_active_session_id(&self, session_id: &str) -> Result<(), StorageError> {
        let path = self.active_session_path();
        self.atomic_write(&path, session_id)?;
        debug!(session_id = %session_id, "Updated active session pointer");
        Ok(())
    }
}
