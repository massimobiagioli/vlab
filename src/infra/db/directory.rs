//! SQLite implementation of the laboratory and agent directory port.

use crate::app::directory::{AgentRecord, AgentTask, Directory, LabOverview};
use crate::app::kanban::StoryState;
use crate::app::storage::StorageError;
use crate::domain::agent::AgentProfile;
use crate::domain::ids::{AgentId, LabId, ProjectId, StoryId};
use crate::domain::lab::Lab;
use crate::infra::db::{Database, agents, labs, map_sqlx_error};

impl Directory for Database {
    fn insert_lab(&self, lab: &Lab) -> Result<(), StorageError> {
        self.block_on(async {
            let mut connection = self
                .pool()
                .acquire()
                .await
                .map_err(|error| map_sqlx_error(self.path(), &error))?;
            labs::insert(&mut connection, lab)
                .await
                .map_err(|error| map_sqlx_error(self.path(), &error))
        })
    }

    fn lab_exists(&self, id: &LabId) -> Result<bool, StorageError> {
        self.block_on(async {
            let mut connection = self
                .pool()
                .acquire()
                .await
                .map_err(|error| map_sqlx_error(self.path(), &error))?;
            labs::exists(&mut connection, id)
                .await
                .map_err(|error| map_sqlx_error(self.path(), &error))
        })
    }

    fn update_lab(&self, lab: &Lab) -> Result<(), StorageError> {
        self.block_on(async {
            let mut connection = self
                .pool()
                .acquire()
                .await
                .map_err(|error| map_sqlx_error(self.path(), &error))?;
            crate::infra::db::labs::update(&mut connection, lab)
                .await
                .map_err(|error| map_sqlx_error(self.path(), &error))
        })
    }

    fn lab(&self, id: &LabId) -> Result<Option<LabOverview>, StorageError> {
        self.block_on(async {
            let mut connection = self
                .pool()
                .acquire()
                .await
                .map_err(|error| map_sqlx_error(self.path(), &error))?;
            labs::get(&mut connection, id)
                .await
                .map_err(|error| map_sqlx_error(self.path(), &error))
        })
    }

    fn labs(&self) -> Result<Vec<LabOverview>, StorageError> {
        self.block_on(async {
            let mut connection = self
                .pool()
                .acquire()
                .await
                .map_err(|error| map_sqlx_error(self.path(), &error))?;
            labs::list(&mut connection)
                .await
                .map_err(|error| map_sqlx_error(self.path(), &error))
        })
    }

    fn lab_agents(&self, lab: &LabId) -> Result<Vec<AgentRecord>, StorageError> {
        self.block_on(async {
            let mut connection = self
                .pool()
                .acquire()
                .await
                .map_err(|error| map_sqlx_error(self.path(), &error))?;
            labs::agents(&mut connection, lab)
                .await
                .map_err(|error| map_sqlx_error(self.path(), &error))
        })
    }

    fn insert_agent(
        &self,
        profile: &AgentProfile,
        memberships: &[LabId],
    ) -> Result<(), StorageError> {
        // The transaction closure must be usable for any connection borrow,
        // so captured data is owned rather than borrowed from the caller.
        let profile = profile.clone();
        let memberships = memberships.to_vec();
        self.transaction(move |connection| {
            Box::pin(async move {
                agents::insert_profile(connection, &profile)
                    .await
                    .map_err(|_| StorageError::Operation)?;
                agents::insert_memberships(connection, &profile.id, &memberships)
                    .await
                    .map_err(|_| StorageError::Operation)?;
                Ok(())
            })
        })
    }

    fn insert_agents(&self, agents: &[(AgentProfile, Vec<LabId>)]) -> Result<(), StorageError> {
        let agents = agents.to_vec();
        self.transaction(move |connection| {
            Box::pin(async move {
                for (profile, memberships) in &agents {
                    agents::insert_profile(connection, profile)
                        .await
                        .map_err(|_| StorageError::Operation)?;
                    agents::insert_memberships(connection, &profile.id, memberships)
                        .await
                        .map_err(|_| StorageError::Operation)?;
                }
                Ok(())
            })
        })
    }

    fn all_agents(&self) -> Result<Vec<AgentRecord>, StorageError> {
        self.block_on(async {
            let mut connection = self
                .pool()
                .acquire()
                .await
                .map_err(|error| map_sqlx_error(self.path(), &error))?;
            agents::list_all(&mut connection)
                .await
                .map_err(|error| map_sqlx_error(self.path(), &error))
        })
    }

    fn update_lab_memberships(
        &self,
        lab: &LabId,
        add: &[AgentId],
        remove: &[AgentId],
    ) -> Result<(), StorageError> {
        let lab = lab.clone();
        let add = add.to_vec();
        let remove = remove.to_vec();
        self.transaction(move |connection| {
            Box::pin(async move {
                agents::remove_memberships(connection, &lab, &remove)
                    .await
                    .map_err(|_| StorageError::Operation)?;
                agents::add_memberships_if_missing(connection, &lab, &add)
                    .await
                    .map_err(|_| StorageError::Operation)?;
                Ok(())
            })
        })
    }

    fn agent_exists(&self, id: &AgentId) -> Result<bool, StorageError> {
        self.block_on(async {
            let mut connection = self
                .pool()
                .acquire()
                .await
                .map_err(|error| map_sqlx_error(self.path(), &error))?;
            agents::exists(&mut connection, id)
                .await
                .map_err(|error| map_sqlx_error(self.path(), &error))
        })
    }

    fn agent(&self, id: &AgentId) -> Result<Option<AgentRecord>, StorageError> {
        self.block_on(async {
            let mut connection = self
                .pool()
                .acquire()
                .await
                .map_err(|error| map_sqlx_error(self.path(), &error))?;
            agents::get(&mut connection, id)
                .await
                .map_err(|error| map_sqlx_error(self.path(), &error))
        })
    }

    fn agent_memberships(&self, id: &AgentId) -> Result<Vec<LabOverview>, StorageError> {
        self.block_on(async {
            let mut connection = self
                .pool()
                .acquire()
                .await
                .map_err(|error| map_sqlx_error(self.path(), &error))?;
            agents::memberships(&mut connection, id)
                .await
                .map_err(|error| map_sqlx_error(self.path(), &error))
        })
    }

    fn agent_project_count(&self, id: &AgentId) -> Result<u64, StorageError> {
        self.count_agent(
            "SELECT count(*) FROM (\
                 SELECT project_id FROM project_agents WHERE agent_id = ?1 \
                 UNION SELECT project_id FROM stories WHERE assigned_agent_id = ?1)",
            id,
        )
    }

    fn agent_open_story_count(&self, id: &AgentId) -> Result<u64, StorageError> {
        self.count_agent(
            "SELECT count(*) FROM stories WHERE assigned_agent_id = ?1 AND state != 'DONE'",
            id,
        )
    }

    fn agent_assigned_story_count(&self, id: &AgentId) -> Result<u64, StorageError> {
        self.count_agent(
            "SELECT count(*) FROM stories WHERE assigned_agent_id = ?1",
            id,
        )
    }

    fn agent_completed_story_count(&self, id: &AgentId) -> Result<u64, StorageError> {
        self.count_agent(
            "SELECT count(*) FROM stories WHERE assigned_agent_id = ?1 AND state = 'DONE'",
            id,
        )
    }

    fn agent_memory_count(&self, id: &AgentId) -> Result<u64, StorageError> {
        self.count_agent(
            "SELECT count(*) FROM memories WHERE scope_type = 'agent' AND agent_id = ?1",
            id,
        )
    }

    fn agent_project_names(
        &self,
        id: &AgentId,
    ) -> Result<Vec<(crate::domain::ids::ProjectId, String)>, StorageError> {
        self.block_on(async {
            let mut connection = self
                .pool()
                .acquire()
                .await
                .map_err(|error| map_sqlx_error(self.path(), &error))?;
            let rows: Vec<(String, String)> = sqlx::query_as(
                "SELECT p.id, p.name FROM projects p \
                 WHERE p.id IN (\
                     SELECT project_id FROM project_agents WHERE agent_id = ?1 \
                     UNION SELECT project_id FROM stories WHERE assigned_agent_id = ?1) \
                 ORDER BY p.name COLLATE NOCASE, p.id",
            )
            .bind(id.as_str())
            .fetch_all(&mut *connection)
            .await
            .map_err(|error| map_sqlx_error(self.path(), &error))?;
            rows.into_iter()
                .map(|(id, name)| {
                    Ok((
                        crate::domain::ids::ProjectId::parse(&id).ok_or(StorageError::Operation)?,
                        name,
                    ))
                })
                .collect()
        })
    }

    fn agent_open_tasks(&self, id: &AgentId) -> Result<Vec<AgentTask>, StorageError> {
        self.open_tasks("s.assigned_agent_id = ?1", Some(id.as_str()), None)
    }

    fn project_open_tasks(&self, id: &ProjectId) -> Result<Vec<AgentTask>, StorageError> {
        self.open_tasks("s.project_id = ?1", None, Some(id.as_str()))
    }
}

impl Database {
    fn open_tasks(
        &self,
        filter: &'static str,
        agent: Option<&str>,
        project: Option<&str>,
    ) -> Result<Vec<AgentTask>, StorageError> {
        self.block_on(async {
            let mut connection = self
                .pool()
                .acquire()
                .await
                .map_err(|error| map_sqlx_error(self.path(), &error))?;
            let sql = format!(
                "SELECT s.id, s.project_id, p.name, s.title, s.state, s.priority, \
                        s.due_date, s.assigned_agent_id, a.name \
                 FROM stories s JOIN projects p ON p.id = s.project_id \
                 LEFT JOIN agents a ON a.id = s.assigned_agent_id \
                 WHERE s.state != 'DONE' AND {filter} \
                 ORDER BY CASE s.state \
                     WHEN 'IN_PROGRESS' THEN 0 WHEN 'IN_REVIEW' THEN 1 \
                     WHEN 'BLOCKED' THEN 2 ELSE 3 END, s.due_date, s.id"
            );
            let mut query = sqlx::query_as::<
                _,
                (
                    String,
                    String,
                    String,
                    String,
                    String,
                    String,
                    String,
                    Option<String>,
                    Option<String>,
                ),
            >(sqlx::AssertSqlSafe(sql));
            if let Some(agent) = agent {
                query = query.bind(agent);
            } else if let Some(project) = project {
                query = query.bind(project);
            }
            let rows = query
                .fetch_all(&mut *connection)
                .await
                .map_err(|error| map_sqlx_error(self.path(), &error))?;
            rows.into_iter()
                .map(
                    |(
                        id,
                        project_id,
                        project_name,
                        title,
                        state,
                        priority,
                        due_date,
                        assignee,
                        assignee_name,
                    )| {
                        Ok(AgentTask {
                            id: StoryId::parse(&id).ok_or(StorageError::Operation)?,
                            project_id: ProjectId::parse(&project_id)
                                .ok_or(StorageError::Operation)?,
                            project_name,
                            title,
                            state: StoryState::parse(&state).ok_or(StorageError::Operation)?,
                            priority,
                            due_date,
                            assignee: match assignee.as_deref() {
                                Some(value) => {
                                    Some(AgentId::parse(value).ok_or(StorageError::Operation)?)
                                }
                                None => None,
                            },
                            assignee_name,
                        })
                    },
                )
                .collect()
        })
    }
}

impl Database {
    fn count_agent(&self, sql: &'static str, id: &AgentId) -> Result<u64, StorageError> {
        self.block_on(async {
            let mut connection = self
                .pool()
                .acquire()
                .await
                .map_err(|error| map_sqlx_error(self.path(), &error))?;
            let count: i64 = sqlx::query_scalar(sql)
                .bind(id.as_str())
                .fetch_one(&mut *connection)
                .await
                .map_err(|error| map_sqlx_error(self.path(), &error))?;
            Ok(u64::try_from(count).unwrap_or(0))
        })
    }
}
