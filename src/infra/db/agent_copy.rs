//! Atomic persistence for a duplicated agent (ADR-0058, ADR-0059).

use crate::app::agents::AgentCopyStore;
use crate::app::storage::StorageError;
use crate::domain::agent::AgentProfile;
use crate::domain::ids::LabId;
use crate::domain::memory::Memory;
use crate::infra::db::{Database, agents, memories, memory_files};
use crate::infra::files::MAX_MEMORY_BYTES;

impl AgentCopyStore for Database {
    fn insert_agent_copy(
        &self,
        profile: &AgentProfile,
        memberships: &[LabId],
        memories: &[Memory],
    ) -> Result<(), StorageError> {
        let profile = profile.clone();
        let memberships = memberships.to_vec();
        let memories = memories.to_vec();
        let files = self.files().clone();
        self.transaction(move |connection| {
            Box::pin(async move {
                let mut written = Vec::new();
                let result: Result<(), StorageError> = async {
                    agents::insert_profile(connection, &profile)
                        .await
                        .map_err(|_| StorageError::Operation)?;
                    agents::insert_memberships(connection, &profile.id, &memberships)
                        .await
                        .map_err(|_| StorageError::Operation)?;
                    for memory in &memories {
                        let path = memory_files::memory_path(connection, &memory.scope, &memory.id)
                            .await
                            .map_err(|_| StorageError::Operation)?;
                        files
                            .write(&path, &memory.content, MAX_MEMORY_BYTES)
                            .map_err(StorageError::from)?;
                        written.push(path.clone());
                        memories::insert(connection, memory, &path)
                            .await
                            .map_err(|_| StorageError::Operation)?;
                    }
                    Ok(())
                }
                .await;
                if result.is_err() {
                    for path in &written {
                        let _ = files.delete(path);
                    }
                }
                result
            })
        })
    }
}
