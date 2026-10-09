use async_trait::async_trait;
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::error::StorageError;
use crate::model::{SessionMetadata, SessionRecord};

/// Selects which sessions [`SessionRepository::prune_sessions`] removes.
///
/// A session is removed when it matches any enabled criterion.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PruneCriteria {
    /// Remove sessions that contain no messages.
    pub empty: bool,
    /// Remove sessions with no activity for more than this many days.
    pub older_than_days: Option<u32>,
}

impl PruneCriteria {
    /// Criteria matching only sessions with zero messages.
    pub fn empty_sessions() -> Self {
        Self {
            empty: true,
            older_than_days: None,
        }
    }

    /// Returns whether `session` should be pruned at time `now`.
    pub fn matches(&self, session: &SessionMetadata, now: chrono::DateTime<chrono::Utc>) -> bool {
        let is_empty = self.empty && session.message_count == 0;
        let is_stale = self.older_than_days.is_some_and(|days| {
            now.signed_duration_since(session.updated_at) > chrono::Duration::days(days.into())
        });
        is_empty || is_stale
    }
}

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

    /// Deletes every session matching `criteria` and returns the metadata of removed sessions.
    ///
    /// The session referenced by the active-session pointer and `keep_session_id` (the
    /// in-memory session of the caller) are never deleted.
    async fn prune_sessions(
        &self,
        criteria: PruneCriteria,
        keep_session_id: Option<&str>,
    ) -> Result<Vec<SessionMetadata>, StorageError> {
        let active_id = self.get_active_session_id().await?;
        let now = chrono::Utc::now();
        let mut removed = Vec::new();

        for session in self.list_sessions().await? {
            let id = session.id.as_str();
            if Some(id) == keep_session_id || Some(id) == active_id.as_deref() {
                continue;
            }
            if criteria.matches(&session, now) && self.delete_session(id).await? {
                removed.push(session);
            }
        }

        info!(removed = removed.len(), ?criteria, "Pruned sessions");
        Ok(removed)
    }

    /// Deletes all sessions with zero messages, except the active one. Returns how many were removed.
    async fn prune_empty_sessions(
        &self,
        keep_session_id: Option<&str>,
    ) -> Result<usize, StorageError> {
        let removed = self
            .prune_sessions(PruneCriteria::empty_sessions(), keep_session_id)
            .await?;
        Ok(removed.len())
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Message;
    use tempfile::tempdir;

    async fn session_with_messages(repo: &FileSessionRepository, count: usize) -> SessionRecord {
        let mut record = repo.create_session(None, None, None).await.unwrap();
        for i in 0..count {
            record.add_message(Message::user(&record.metadata.id, format!("message {i}")));
        }
        repo.save_session(&record).await.unwrap();
        record
    }

    #[test]
    fn test_normalize_tag() {
        use crate::model::normalize_tag;
        assert_eq!(normalize_tag("bugfix").as_deref(), Some("bugfix"));
        assert_eq!(normalize_tag(" [Refactor] ").as_deref(), Some("refactor"));
        assert_eq!(normalize_tag("#Docs").as_deref(), Some("docs"));
        assert_eq!(normalize_tag("v1.2_rc-1").as_deref(), Some("v1.2_rc-1"));
        for invalid in ["", "  ", "[]", "two words", "semi;colon", &"x".repeat(25)] {
            assert_eq!(normalize_tag(invalid), None, "{invalid:?} rejected");
        }
    }

    #[test]
    fn test_add_and_remove_tags() {
        let mut meta = SessionMetadata::new("id", "title", None, None);
        assert!(meta.add_tag("bugfix"));
        assert!(!meta.add_tag("bugfix"), "duplicates ignored");
        for i in 0..20 {
            meta.add_tag(&format!("t{i}"));
        }
        assert_eq!(meta.tags.len(), crate::model::MAX_SESSION_TAGS);
        assert!(meta.remove_tag("bugfix"));
        assert!(!meta.remove_tag("bugfix"));
    }

    #[tokio::test]
    async fn test_tags_persist_and_old_sessions_without_tags_still_load() {
        let dir = tempdir().unwrap();
        let repo = FileSessionRepository::with_dir(dir.path());

        let mut record = repo.create_session(None, None, None).await.unwrap();
        record.metadata.add_tag("review");
        repo.save_session(&record).await.unwrap();
        let loaded = repo
            .get_session(&record.metadata.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(loaded.metadata.tags, vec!["review".to_string()]);

        // A session file written before tags existed has no `tags` key.
        let mut legacy = serde_json::to_value(&record).unwrap();
        legacy["metadata"].as_object_mut().unwrap().remove("tags");
        legacy["metadata"]["id"] = serde_json::json!("legacy-session");
        std::fs::write(
            dir.path().join("legacy-session.json"),
            serde_json::to_string(&legacy).unwrap(),
        )
        .unwrap();
        let old = repo.get_session("legacy-session").await.unwrap().unwrap();
        assert!(old.metadata.tags.is_empty());

        // Untagged sessions do not write an empty `tags` key.
        let untagged = repo.create_session(None, None, None).await.unwrap();
        let raw =
            std::fs::read_to_string(dir.path().join(format!("{}.json", untagged.metadata.id)))
                .unwrap();
        assert!(!raw.contains("\"tags\""));
    }

    #[test]
    fn test_prune_criteria_matching() {
        let now = chrono::Utc::now();
        let mut meta = SessionMetadata::new("id", "title", None, None);
        meta.updated_at = now - chrono::Duration::days(10);

        assert!(PruneCriteria::empty_sessions().matches(&meta, now));
        assert!(!PruneCriteria::default().matches(&meta, now));

        meta.message_count = 3;
        assert!(!PruneCriteria::empty_sessions().matches(&meta, now));
        let stale = PruneCriteria {
            empty: false,
            older_than_days: Some(7),
        };
        assert!(stale.matches(&meta, now));
        let fresh = PruneCriteria {
            empty: false,
            older_than_days: Some(30),
        };
        assert!(!fresh.matches(&meta, now));
    }

    #[tokio::test]
    async fn test_prune_empty_sessions_keeps_active_and_non_empty() {
        let dir = tempdir().unwrap();
        let repo = FileSessionRepository::with_dir(dir.path());

        let empty_a = session_with_messages(&repo, 0).await;
        let empty_b = session_with_messages(&repo, 0).await;
        let used = session_with_messages(&repo, 2).await;
        let active_empty = session_with_messages(&repo, 0).await; // most recent -> active pointer

        let removed = repo.prune_empty_sessions(None).await.unwrap();

        assert_eq!(removed, 2);
        let remaining: Vec<String> = repo
            .list_sessions()
            .await
            .unwrap()
            .into_iter()
            .map(|m| m.id)
            .collect();
        assert!(!remaining.contains(&empty_a.metadata.id));
        assert!(!remaining.contains(&empty_b.metadata.id));
        assert!(remaining.contains(&used.metadata.id));
        assert!(remaining.contains(&active_empty.metadata.id));
        assert_eq!(
            repo.get_active_session_id().await.unwrap().as_deref(),
            Some(active_empty.metadata.id.as_str())
        );
    }

    #[tokio::test]
    async fn test_prune_sessions_respects_keep_and_age() {
        let dir = tempdir().unwrap();
        let repo = FileSessionRepository::with_dir(dir.path());

        let kept_empty = session_with_messages(&repo, 0).await;
        let mut old = session_with_messages(&repo, 1).await;
        old.metadata.updated_at = chrono::Utc::now() - chrono::Duration::days(90);
        repo.save_session(&old).await.unwrap();
        let recent = session_with_messages(&repo, 1).await;

        let criteria = PruneCriteria {
            empty: true,
            older_than_days: Some(30),
        };
        let removed = repo
            .prune_sessions(criteria, Some(&kept_empty.metadata.id))
            .await
            .unwrap();

        let removed_ids: Vec<&str> = removed.iter().map(|m| m.id.as_str()).collect();
        assert_eq!(removed_ids, vec![old.metadata.id.as_str()]);
        assert!(repo
            .get_session(&kept_empty.metadata.id)
            .await
            .unwrap()
            .is_some());
        assert!(repo
            .get_session(&recent.metadata.id)
            .await
            .unwrap()
            .is_some());
    }
}
