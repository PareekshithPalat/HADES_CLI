pub mod error;
pub mod model;
pub mod repository;
pub mod service;

pub use error::StorageError;
pub use model::{
    generate_session_title, Message, MessageMetadata, MessageRole, SessionMetadata, SessionRecord,
    StorageHealth, StorageStatus,
};
pub use repository::{FileSessionRepository, SessionRepository};
pub use service::StorageService;

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};
    use tempfile::tempdir;

    #[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
    struct SampleData {
        id: u64,
        name: String,
        active: bool,
    }

    #[test]
    fn test_storage_save_load_delete_roundtrip() {
        let dir = tempdir().expect("create temp dir");
        let storage = StorageService::with_root(dir.path().join("data"));

        let data = SampleData {
            id: 42,
            name: "test-node".to_string(),
            active: true,
        };

        // Initially does not exist
        let loaded: Option<SampleData> = storage.load("sample").expect("load non-existent");
        assert_eq!(loaded, None);

        // Save
        storage.save("sample", &data).expect("save data");

        // Load
        let loaded: Option<SampleData> = storage.load("sample").expect("load existing");
        assert_eq!(loaded, Some(data));

        // Delete
        let deleted = storage.delete("sample").expect("delete data");
        assert!(deleted);

        // Load again
        let loaded_after: Option<SampleData> = storage.load("sample").expect("load after delete");
        assert_eq!(loaded_after, None);
    }

    #[test]
    fn test_invalid_keys() {
        let dir = tempdir().expect("create temp dir");
        let storage = StorageService::with_root(dir.path());

        assert!(matches!(
            storage.load::<SampleData>(""),
            Err(StorageError::InvalidKey(_))
        ));

        assert!(matches!(
            storage.load::<SampleData>("../secret"),
            Err(StorageError::InvalidKey(_))
        ));

        assert!(matches!(
            storage.load::<SampleData>("foo/bar"),
            Err(StorageError::InvalidKey(_))
        ));
    }

    #[test]
    fn test_storage_health() {
        let dir = tempdir().expect("create temp dir");
        let storage = StorageService::with_root(dir.path().join("health_data"));

        let health = storage.health().expect("storage health");
        assert_eq!(health.status, StorageStatus::Ready);
        assert!(health.writable);
    }

    #[tokio::test]
    async fn test_session_repository_lifecycle() {
        let dir = tempdir().expect("create temp dir");
        let repo = FileSessionRepository::with_dir(dir.path().join("sessions"));

        // 1. Create Session
        let session = repo
            .create_session(
                Some("Hades Development".to_string()),
                Some("groq".to_string()),
                Some("llama-3.3-70b-versatile".to_string()),
            )
            .await
            .expect("create session");

        assert_eq!(session.metadata.title, "Hades Development");
        assert_eq!(session.schema_version, 1);
        assert!(session.messages.is_empty());

        let active_id = repo.get_active_session_id().await.expect("get active");
        assert_eq!(active_id, Some(session.metadata.id.clone()));

        // 2. Add Messages
        let mut session = repo
            .get_session(&session.metadata.id)
            .await
            .expect("get session")
            .expect("session must exist");

        let msg1 = Message::user(&session.metadata.id, "Explain transformers architecture");
        session.add_message(msg1);

        let mut msg2 = Message::assistant(
            &session.metadata.id,
            "Transformers use multi-head self-attention.",
            Some("groq".to_string()),
            Some("llama-3.3-70b-versatile".to_string()),
        );
        msg2.metadata.input_tokens = Some(15);
        msg2.metadata.output_tokens = Some(25);
        msg2.metadata.total_tokens = Some(40);
        session.add_message(msg2);

        repo.save_session(&session).await.expect("save session");

        // 3. Reload and Verify
        let reloaded = repo
            .get_session(&session.metadata.id)
            .await
            .expect("get session")
            .expect("session must exist");

        assert_eq!(reloaded.messages.len(), 2);
        assert_eq!(reloaded.metadata.message_count, 2);
        assert_eq!(reloaded.metadata.total_tokens, 40);
        assert_eq!(reloaded.messages[0].role, MessageRole::User);
        assert_eq!(reloaded.messages[1].role, MessageRole::Assistant);

        // 4. Create Second Session
        let session2 = repo
            .create_session(None, None, None)
            .await
            .expect("create session 2");
        assert_eq!(session2.metadata.title, "New Session");

        // 5. Rename Session
        repo.rename_session(&session2.metadata.id, "Renamed Session Title")
            .await
            .expect("rename");
        let reloaded2 = repo
            .get_session(&session2.metadata.id)
            .await
            .expect("get")
            .expect("must exist");
        assert_eq!(reloaded2.metadata.title, "Renamed Session Title");

        // 6. List Sessions
        let list = repo.list_sessions().await.expect("list sessions");
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].id, session2.metadata.id); // Most recently updated
        assert_eq!(list[0].title, "Renamed Session Title");

        // 7. Delete Session
        let deleted = repo
            .delete_session(&session.metadata.id)
            .await
            .expect("delete");
        assert!(deleted);

        let list_after = repo.list_sessions().await.expect("list sessions after");
        assert_eq!(list_after.len(), 1);
        assert_eq!(list_after[0].id, session2.metadata.id);
    }

    #[test]
    fn test_title_generation() {
        assert_eq!(
            generate_session_title("explain quantum computing in simple terms"),
            "Explain quantum computing in simple"
        );
        assert_eq!(generate_session_title("   "), "New Session");
        assert_eq!(generate_session_title("what is Rust?"), "What is Rust?");
    }
}
