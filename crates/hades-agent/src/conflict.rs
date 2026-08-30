use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::error::AgentError;

/// Thread-safe resource lock manager preventing conflicting concurrent writes to identical files or workspaces.
#[derive(Debug, Clone, Default)]
pub struct ResourceLockManager {
    /// Active write locks mapping resource normalized path -> owner agent ID.
    locks: Arc<RwLock<HashMap<String, String>>>,
}

impl ResourceLockManager {
    /// Creates a new `ResourceLockManager`.
    pub fn new() -> Self {
        Self {
            locks: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Attempts to acquire exclusive write locks for all specified resources.
    pub async fn acquire_locks(
        &self,
        agent_id: &str,
        resources: &[String],
    ) -> Result<(), AgentError> {
        let mut locks = self.locks.write().await;

        // 1. Check if any requested resource is currently locked by a different agent
        for resource in resources {
            let norm = resource.replace('\\', "/").to_lowercase();
            if let Some(holder) = locks.get(&norm) {
                if holder != agent_id {
                    return Err(AgentError::ResourceConflict {
                        resource: resource.clone(),
                        holder: holder.clone(),
                    });
                }
            }
        }

        // 2. Grant all locks
        for resource in resources {
            let norm = resource.replace('\\', "/").to_lowercase();
            locks.insert(norm, agent_id.to_string());
        }

        Ok(())
    }

    /// Releases all write locks held by the specified agent.
    pub async fn release_locks(&self, agent_id: &str) {
        let mut locks = self.locks.write().await;
        locks.retain(|_, holder| holder != agent_id);
    }

    /// Checks whether the specified set of resources can be locked without conflict.
    pub async fn can_acquire(&self, agent_id: &str, resources: &[String]) -> bool {
        let locks = self.locks.read().await;
        for resource in resources {
            let norm = resource.replace('\\', "/").to_lowercase();
            if let Some(holder) = locks.get(&norm) {
                if holder != agent_id {
                    return false;
                }
            }
        }
        true
    }
}
