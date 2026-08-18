pub mod error;
pub mod model;
pub mod service;

pub use error::StorageError;
pub use model::{StorageHealth, StorageStatus};
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
}
