use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Operational health status of the storage backend.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum StorageStatus {
    Ready,
    Degraded(String),
    Unhealthy(String),
}

/// Comprehensive health report for the storage subsystem.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StorageHealth {
    pub status: StorageStatus,
    pub root_dir: PathBuf,
    pub writable: bool,
}
