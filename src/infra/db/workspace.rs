//! SQLite implementations for projects, memories, and the Kanban port.

use crate::app::kanban::{Board, BoardStore, StoryDetail};
use crate::app::memories::{MemoryRecord, MemoryStore};
use crate::app::projects::{ProjectOverview, ProjectStore};
use crate::app::storage::StorageError;
use crate::domain::ids::{AgentId, LabId, MemoryId, ProjectId, StoryId};
use crate::domain::memory::{MEMORY_MAX_PER_SCOPE, Memory, MemoryScope};
use crate::domain::name::Name;
use crate::domain::project::Project;
use crate::infra::db::{Database, kanban, map_sqlx_error, memories, projects};

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
        self.transaction(move |connection| {
            Box::pin(async move {
                let count = memories::count(connection, &memory.scope)
                    .await
                    .map_err(|_| StorageError::Operation)?;
                if count >= MEMORY_MAX_PER_SCOPE as u64 {
                    return Err(StorageError::Operation);
                }
                memories::insert(connection, &memory)
                    .await
                    .map_err(|_| StorageError::Operation)
            })
        })
    }

    fn memories(&self, scope: &MemoryScope) -> Result<Vec<MemoryRecord>, StorageError> {
        self.block_on(async {
            let mut connection = self
                .pool()
                .acquire()
                .await
                .map_err(|error| map_sqlx_error(self.path(), &error))?;
            memories::list(&mut connection, scope)
                .await
                .map_err(|error| map_sqlx_error(self.path(), &error))
        })
    }

    fn memory(&self, id: &MemoryId) -> Result<Option<MemoryRecord>, StorageError> {
        self.block_on(async {
            let mut connection = self
                .pool()
                .acquire()
                .await
                .map_err(|error| map_sqlx_error(self.path(), &error))?;
            memories::get(&mut connection, id)
                .await
                .map_err(|error| map_sqlx_error(self.path(), &error))
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

    fn delete_memory(&self, id: &MemoryId) -> Result<(), StorageError> {
        self.block_on(async {
            let mut connection = self
                .pool()
                .acquire()
                .await
                .map_err(|error| map_sqlx_error(self.path(), &error))?;
            memories::delete(&mut connection, id)
                .await
                .map_err(|error| map_sqlx_error(self.path(), &error))
        })
    }
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
