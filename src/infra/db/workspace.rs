//! SQLite implementations for projects, memories, and the Kanban port.

use crate::app::kanban::{Board, BoardStore, StoryDetail};
use crate::app::memories::{MemoryRecord, MemoryStore};
use crate::app::projects::{ProjectOverview, ProjectStore};
use crate::app::storage::StorageError;
use crate::domain::ids::{AgentId, LabId, MemoryId, ProjectId, StoryId};
use crate::domain::memory::{MEMORY_MAX_PER_SCOPE, Memory, MemoryScope};
use crate::domain::name::Name;
use crate::domain::project::Project;
use crate::infra::db::memories::StoredMemory;
use crate::infra::db::{Database, kanban, map_sqlx_error, memories, memory_files, projects};
use crate::infra::files::{FileStore, MAX_MEMORY_BYTES};

impl ProjectStore for Database {
    fn insert_project(&self, project: &Project) -> Result<(), StorageError> {
        self.block_on(async {
            let mut connection = self
                .pool()
                .acquire()
                .await
                .map_err(|error| map_sqlx_error(self.path(), &error))?;
            projects::insert(&mut connection, project)
                .await
                .map_err(|error| map_sqlx_error(self.path(), &error))
        })
    }

    fn project(
        &self,
        id: &ProjectId,
    ) -> Result<Option<(ProjectId, LabId, String, Option<String>)>, StorageError> {
        self.block_on(async {
            let mut connection = self
                .pool()
                .acquire()
                .await
                .map_err(|error| map_sqlx_error(self.path(), &error))?;
            projects::find(&mut connection, id)
                .await
                .map_err(|error| map_sqlx_error(self.path(), &error))
        })
    }

    fn projects(&self, lab: &LabId) -> Result<Vec<ProjectOverview>, StorageError> {
        self.block_on(async {
            let mut connection = self
                .pool()
                .acquire()
                .await
                .map_err(|error| map_sqlx_error(self.path(), &error))?;
            projects::list(&mut connection, lab)
                .await
                .map_err(|error| map_sqlx_error(self.path(), &error))
        })
    }

    fn update_project_name(&self, id: &ProjectId, name: &Name) -> Result<(), StorageError> {
        self.block_on(async {
            let mut connection = self
                .pool()
                .acquire()
                .await
                .map_err(|error| map_sqlx_error(self.path(), &error))?;
            projects::update_name(&mut connection, id, name)
                .await
                .map_err(|error| map_sqlx_error(self.path(), &error))
        })
    }

    fn assign_project_agent(
        &self,
        project: &ProjectId,
        agent: &AgentId,
    ) -> Result<(), StorageError> {
        self.block_on(async {
            let mut connection = self
                .pool()
                .acquire()
                .await
                .map_err(|error| map_sqlx_error(self.path(), &error))?;
            crate::infra::db::project_agents::assign(&mut connection, project, agent)
                .await
                .map_err(|error| map_sqlx_error(self.path(), &error))
        })
    }

    fn project_agent_ids(&self, project: &ProjectId) -> Result<Vec<AgentId>, StorageError> {
        self.block_on(async {
            let mut connection = self
                .pool()
                .acquire()
                .await
                .map_err(|error| map_sqlx_error(self.path(), &error))?;
            crate::infra::db::project_agents::ids(&mut connection, project)
                .await
                .map_err(|error| map_sqlx_error(self.path(), &error))
        })
    }

    fn project_in_lab(
        &self,
        id: &ProjectId,
        lab: &LabId,
    ) -> Result<Option<ProjectOverview>, StorageError> {
        self.block_on(async {
            let mut connection = self
                .pool()
                .acquire()
                .await
                .map_err(|error| map_sqlx_error(self.path(), &error))?;
            projects::in_lab(&mut connection, id, lab)
                .await
                .map_err(|error| map_sqlx_error(self.path(), &error))
        })
    }
}

impl MemoryStore for Database {
    fn insert_memory(&self, memory: &Memory) -> Result<(), StorageError> {
        let memory = memory.clone();
        let files = self.files().clone();
        self.block_on(async {
            let mut connection = self
                .pool()
                .acquire()
                .await
                .map_err(|error| map_sqlx_error(self.path(), &error))?;
            let count = memories::count(&mut connection, &memory.scope)
                .await
                .map_err(|error| map_sqlx_error(self.path(), &error))?;
            if count >= MEMORY_MAX_PER_SCOPE as u64 {
                return Err(StorageError::Operation);
            }
            let path = memory_files::memory_path(&mut connection, &memory.scope, &memory.id)
                .await
                .map_err(|error| map_sqlx_error(self.path(), &error))?;
            files
                .write(&path, &memory.content, MAX_MEMORY_BYTES)
                .map_err(StorageError::from)?;
            if let Err(error) = memories::insert(&mut connection, &memory, &path).await {
                let _ = files.delete(&path);
                return Err(map_sqlx_error(self.path(), &error));
            }
            Ok(())
        })
    }

    fn memories(&self, scope: &MemoryScope) -> Result<Vec<MemoryRecord>, StorageError> {
        let files = self.files().clone();
        self.block_on(async {
            let mut connection = self
                .pool()
                .acquire()
                .await
                .map_err(|error| map_sqlx_error(self.path(), &error))?;
            let stored = memories::list_refs(&mut connection, scope)
                .await
                .map_err(|error| map_sqlx_error(self.path(), &error))?;
            let mut records = Vec::with_capacity(stored.len());
            for memory in stored {
                if memory.path.is_none() && memory.content.is_empty() {
                    return Err(StorageError::Operation);
                }
                if let Some(record) = record_from(&files, memory) {
                    records.push(record);
                }
            }
            Ok(records)
        })
    }

    fn memory(&self, id: &MemoryId) -> Result<Option<MemoryRecord>, StorageError> {
        let files = self.files().clone();
        self.block_on(async {
            let mut connection = self
                .pool()
                .acquire()
                .await
                .map_err(|error| map_sqlx_error(self.path(), &error))?;
            let stored = memories::get_ref(&mut connection, id)
                .await
                .map_err(|error| map_sqlx_error(self.path(), &error))?;
            let Some(memory) = stored else {
                return Ok(None);
            };
            if memory.path.is_none() && memory.content.is_empty() {
                return Err(StorageError::Operation);
            }
            Ok(record_from(&files, memory))
        })
    }

    fn memory_count(&self, scope: &MemoryScope) -> Result<u64, StorageError> {
        self.block_on(async {
            let mut connection = self
                .pool()
                .acquire()
                .await
                .map_err(|error| map_sqlx_error(self.path(), &error))?;
            memories::count(&mut connection, scope)
                .await
                .map_err(|error| map_sqlx_error(self.path(), &error))
        })
    }

    fn update_memory(&self, memory: &Memory) -> Result<(), StorageError> {
        let memory = memory.clone();
        let files = self.files().clone();
        self.block_on(async {
            let mut connection = self
                .pool()
                .acquire()
                .await
                .map_err(|error| map_sqlx_error(self.path(), &error))?;
            let existing = memories::path(&mut connection, &memory.id)
                .await
                .map_err(|error| map_sqlx_error(self.path(), &error))?;
            let (path, needs_path) = match existing {
                Some(path) => (path, false),
                None => (
                    memory_files::memory_path(&mut connection, &memory.scope, &memory.id)
                        .await
                        .map_err(|error| map_sqlx_error(self.path(), &error))?,
                    true,
                ),
            };
            files
                .write(&path, &memory.content, MAX_MEMORY_BYTES)
                .map_err(StorageError::from)?;
            if needs_path {
                memories::set_path(&mut connection, &memory.id, &path)
                    .await
                    .map_err(|error| map_sqlx_error(self.path(), &error))?;
            }
            Ok(())
        })
    }

    fn delete_memory(&self, id: &MemoryId) -> Result<(), StorageError> {
        let files = self.files().clone();
        self.block_on(async {
            let mut connection = self
                .pool()
                .acquire()
                .await
                .map_err(|error| map_sqlx_error(self.path(), &error))?;
            let path = memories::path(&mut connection, id)
                .await
                .map_err(|error| map_sqlx_error(self.path(), &error))?;
            memories::delete(&mut connection, id)
                .await
                .map_err(|error| map_sqlx_error(self.path(), &error))?;
            if let Some(path) = path {
                let _ = files.delete(&path);
            }
            Ok(())
        })
    }
}

impl Database {
    /// Materializes legacy memory rows as markdown files. Best effort: a row
    /// whose file cannot be written stays available from its legacy content.
    pub(crate) fn materialize_memories(&self) -> Result<(), StorageError> {
        let files = self.files().clone();
        self.block_on(async {
            let mut connection = self
                .pool()
                .acquire()
                .await
                .map_err(|error| map_sqlx_error(self.path(), &error))?;
            let legacy = memories::legacy_refs(&mut connection)
                .await
                .map_err(|error| map_sqlx_error(self.path(), &error))?;
            for stored in legacy {
                let path =
                    match memory_files::memory_path(&mut connection, &stored.scope, &stored.id)
                        .await
                    {
                        Ok(path) => path,
                        Err(_) => continue,
                    };
                if files
                    .write(&path, &stored.content, MAX_MEMORY_BYTES)
                    .is_ok()
                {
                    let _ = memories::set_path(&mut connection, &stored.id, &path).await;
                }
            }
            Ok(())
        })
    }
}

fn record_from(files: &FileStore, stored: StoredMemory) -> Option<MemoryRecord> {
    let content = match stored.path.as_deref() {
        Some(path) => files
            .read(path, MAX_MEMORY_BYTES)
            .ok()
            .or_else(|| (!stored.content.is_empty()).then_some(stored.content)),
        None => (!stored.content.is_empty()).then_some(stored.content),
    }?;
    let memory = Memory::new(stored.id, stored.scope, &content).ok()?;
    Some(MemoryRecord {
        memory,
        created_at: stored.created_at,
    })
}

impl BoardStore for Database {
    fn board(&self, project: &ProjectId, lab: &LabId) -> Result<Option<Board>, StorageError> {
        self.block_on(async {
            let mut connection = self
                .pool()
                .acquire()
                .await
                .map_err(|error| map_sqlx_error(self.path(), &error))?;
            kanban::board(&mut connection, project, lab)
                .await
                .map_err(|error| map_sqlx_error(self.path(), &error))
        })
    }

    fn story(
        &self,
        project: &ProjectId,
        lab: &LabId,
        story: &StoryId,
    ) -> Result<Option<StoryDetail>, StorageError> {
        self.block_on(async {
            let mut connection = self
                .pool()
                .acquire()
                .await
                .map_err(|error| map_sqlx_error(self.path(), &error))?;
            kanban::story(&mut connection, project, lab, story)
                .await
                .map_err(|error| map_sqlx_error(self.path(), &error))
        })
    }
}
