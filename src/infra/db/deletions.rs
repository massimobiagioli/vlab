//! Destructive operations and the active-planning guard (WP-4).

use std::path::Path;

use sqlx::{Executor, SqliteConnection};

use crate::app::AppError;
use crate::app::deletions::{
    AgentDeletionPlan, DeletionStore, LabDeletionPlan, ProjectDeletionPlan,
};
use crate::app::directory::LabOverview;
use crate::domain::ids::{AgentId, LabId, ProjectId};
use crate::infra::db::{Database, map_sqlx_error};

/// Planning lease from the execution limits (WP-4-T6).
pub const PLANNING_LEASE_SECONDS: i64 = 240;

const PLANNING_LAB: &str = "\
SELECT count(*) FROM requests r JOIN projects p ON p.id = r.project_id \
WHERE p.lab_id = ?1 AND r.status = 'PLANNING' AND r.planning_started_at IS NOT NULL \
AND r.planning_started_at >= strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '-240 seconds')";

const PLANNING_PROJECT: &str = "\
SELECT count(*) FROM requests \
WHERE project_id = ?1 AND status = 'PLANNING' AND planning_started_at IS NOT NULL \
AND planning_started_at >= strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '-240 seconds')";

const PLANNING_AGENT: &str = "\
SELECT count(*) FROM requests r \
JOIN projects p ON p.id = r.project_id \
JOIN lab_memberships m ON m.lab_id = p.lab_id \
WHERE m.agent_id = ?1 AND r.status = 'PLANNING' AND r.planning_started_at IS NOT NULL \
AND r.planning_started_at >= strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '-240 seconds')";

fn db_error(path: &Path, error: sqlx::Error) -> AppError {
    AppError::from(map_sqlx_error(path, &error))
}

fn changed(problem: &str, hint: &str) -> AppError {
    AppError::Precondition {
        problem: problem.to_owned(),
        hint: hint.to_owned(),
    }
}

impl DeletionStore for Database {
    fn lab_plan(&self, id: &LabId) -> Result<LabDeletionPlan, AppError> {
        self.block_on(async {
            let mut connection = self
                .pool()
                .acquire()
                .await
                .map_err(|error| db_error(self.path(), error))?;
            let plan = lab_plan_on(&mut connection, id)
                .await
                .map_err(|error| db_error(self.path(), error))?
                .ok_or_else(|| AppError::NotFound {
                    entity: crate::app::EntityKind::Laboratory,
                    id: id.to_string(),
                })?;
            if has_planning_lab(&mut connection, id)
                .await
                .map_err(|error| db_error(self.path(), error))?
            {
                return Err(changed(
                    "a planning operation is active in this laboratory.",
                    "Wait for planning to finish, then retry the deletion.",
                ));
            }
            Ok(plan)
        })
    }

    fn delete_lab(&self, plan: &LabDeletionPlan) -> Result<(), AppError> {
        let plan = plan.clone();
        self.block_on(async {
            let mut transaction = self
                .pool()
                .begin()
                .await
                .map_err(|error| db_error(self.path(), error))?;
            let current = lab_plan_on(&mut transaction, &plan.id)
                .await
                .map_err(|error| db_error(self.path(), error))?
                .ok_or_else(|| AppError::NotFound {
                    entity: crate::app::EntityKind::Laboratory,
                    id: plan.id.to_string(),
                })?;
            if current != plan {
                return Err(changed(
                    "the laboratory changed while confirmation was open.",
                    "Review the current values and run the command again.",
                ));
            }
            if has_planning_lab(&mut transaction, &plan.id)
                .await
                .map_err(|error| db_error(self.path(), error))?
            {
                return Err(changed(
                    "a planning operation is active in this laboratory.",
                    "Wait for planning to finish, then retry the deletion.",
                ));
            }
            transaction
                .execute(sqlx::query("DELETE FROM labs WHERE id = ?1").bind(plan.id.as_str()))
                .await
                .map_err(|error| db_error(self.path(), error))?;
            transaction
                .execute(
                    sqlx::query("DELETE FROM app_state WHERE key = 'active_lab_id' AND value = ?1")
                        .bind(plan.id.as_str()),
                )
                .await
                .map_err(|error| db_error(self.path(), error))?;
            transaction
                .commit()
                .await
                .map_err(|error| db_error(self.path(), error))
        })
    }

    fn project_plan(&self, id: &ProjectId, lab: &LabId) -> Result<ProjectDeletionPlan, AppError> {
        self.block_on(async {
            let mut connection = self
                .pool()
                .acquire()
                .await
                .map_err(|error| db_error(self.path(), error))?;
            let plan = project_plan_on(&mut connection, id, lab)
                .await
                .map_err(|error| db_error(self.path(), error))?
                .ok_or_else(|| AppError::NotFound {
                    entity: crate::app::EntityKind::Project,
                    id: id.to_string(),
                })?;
            if has_planning_project(&mut connection, id)
                .await
                .map_err(|error| db_error(self.path(), error))?
            {
                return Err(changed(
                    "a planning operation is active in this project.",
                    "Wait for planning to finish, then retry the deletion.",
                ));
            }
            Ok(plan)
        })
    }

    fn delete_project(&self, plan: &ProjectDeletionPlan) -> Result<(), AppError> {
        let plan = plan.clone();
        self.block_on(async {
            let mut transaction = self
                .pool()
                .begin()
                .await
                .map_err(|error| db_error(self.path(), error))?;
            let current = project_plan_on(&mut transaction, &plan.id, &plan.lab_id)
                .await
                .map_err(|error| db_error(self.path(), error))?
                .ok_or_else(|| AppError::NotFound {
                    entity: crate::app::EntityKind::Project,
                    id: plan.id.to_string(),
                })?;
            if current != plan {
                return Err(changed(
                    "the project changed while confirmation was open.",
                    "Review the current values and run the command again.",
                ));
            }
            if has_planning_project(&mut transaction, &plan.id)
                .await
                .map_err(|error| db_error(self.path(), error))?
            {
                return Err(changed(
                    "a planning operation is active in this project.",
                    "Wait for planning to finish, then retry the deletion.",
                ));
            }
            transaction
                .execute(sqlx::query("DELETE FROM projects WHERE id = ?1").bind(plan.id.as_str()))
                .await
                .map_err(|error| db_error(self.path(), error))?;
            transaction
                .commit()
                .await
                .map_err(|error| db_error(self.path(), error))
        })
    }

    fn agent_plan(&self, id: &AgentId) -> Result<AgentDeletionPlan, AppError> {
        self.block_on(async {
            let mut connection = self
                .pool()
                .acquire()
                .await
                .map_err(|error| db_error(self.path(), error))?;
            let plan = agent_plan_on(&mut connection, id)
                .await
                .map_err(|error| db_error(self.path(), error))?
                .ok_or_else(|| AppError::NotFound {
                    entity: crate::app::EntityKind::Agent,
                    id: id.to_string(),
                })?;
            if plan.archived {
                return Err(changed(
                    "the agent is already archived.",
                    "Nothing to delete; inspect it with `--show-agent-detail`.",
                ));
            }
            if plan.assigned_open_stories > 0 {
                return Err(changed(
                    "the agent has unfinished assigned stories.",
                    "Finish or unassign those stories before deleting the agent.",
                ));
            }
            if has_planning_agent(&mut connection, id)
                .await
                .map_err(|error| db_error(self.path(), error))?
            {
                return Err(changed(
                    "the agent belongs to a laboratory with active planning.",
                    "Wait for planning to finish, then retry the deletion.",
                ));
            }
            Ok(plan)
        })
    }

    fn delete_agent(&self, plan: &AgentDeletionPlan) -> Result<(), AppError> {
        let plan = plan.clone();
        self.block_on(async {
            let mut transaction = self
                .pool()
                .begin()
                .await
                .map_err(|error| db_error(self.path(), error))?;
            let current = agent_plan_on(&mut transaction, &plan.id)
                .await
                .map_err(|error| db_error(self.path(), error))?
                .ok_or_else(|| AppError::NotFound {
                    entity: crate::app::EntityKind::Agent,
                    id: plan.id.to_string(),
                })?;
            if current != plan {
                return Err(changed(
                    "the agent changed while confirmation was open.",
                    "Review the current values and run the command again.",
                ));
            }
            if current.archived {
                return Err(changed(
                    "the agent is already archived.",
                    "Nothing to delete; inspect it with `--show-agent-detail`.",
                ));
            }
            if current.assigned_open_stories > 0 {
                return Err(changed(
                    "the agent has unfinished assigned stories.",
                    "Finish or unassign those stories before deleting the agent.",
                ));
            }
            if has_planning_agent(&mut transaction, &plan.id)
                .await
                .map_err(|error| db_error(self.path(), error))?
            {
                return Err(changed(
                    "the agent belongs to a laboratory with active planning.",
                    "Wait for planning to finish, then retry the deletion.",
                ));
            }
            transaction
                .execute(
                    sqlx::query(
                        "UPDATE agents SET archived_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), \
                         updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?1",
                    )
                    .bind(plan.id.as_str()),
                )
                .await
                .map_err(|error| db_error(self.path(), error))?;
            transaction
                .execute(
                    sqlx::query("DELETE FROM lab_memberships WHERE agent_id = ?1")
                        .bind(plan.id.as_str()),
                )
                .await
                .map_err(|error| db_error(self.path(), error))?;
            transaction
                .commit()
                .await
                .map_err(|error| db_error(self.path(), error))
        })
    }
}

async fn lab_plan_on(
    connection: &mut SqliteConnection,
    id: &LabId,
) -> Result<Option<LabDeletionPlan>, sqlx::Error> {
    let row: Option<(String, i64, i64, i64)> = sqlx::query_as(
        "SELECT l.name, (SELECT count(*) FROM projects p WHERE p.lab_id = l.id), \
         (SELECT count(*) FROM lab_memberships m WHERE m.lab_id = l.id), \
         (SELECT count(*) FROM memories mem WHERE mem.scope_type = 'lab' AND mem.lab_id = l.id) \
         FROM labs l WHERE l.id = ?1",
    )
    .bind(id.as_str())
    .fetch_optional(connection)
    .await?;
    Ok(
        row.map(|(name, projects, memberships, memories)| LabDeletionPlan {
            id: id.clone(),
            name,
            projects: u64::try_from(projects).unwrap_or(0),
            memberships: u64::try_from(memberships).unwrap_or(0),
            memories: u64::try_from(memories).unwrap_or(0),
        }),
    )
}

async fn project_plan_on(
    connection: &mut SqliteConnection,
    id: &ProjectId,
    lab: &LabId,
) -> Result<Option<ProjectDeletionPlan>, sqlx::Error> {
    let row: Option<(String, i64, i64, i64)> = sqlx::query_as(
        "SELECT p.name, (SELECT count(*) FROM requests r WHERE r.project_id = p.id), \
         (SELECT count(*) FROM stories s WHERE s.project_id = p.id), \
         (SELECT count(*) FROM memories mem WHERE mem.scope_type = 'project' AND mem.project_id = p.id) \
         FROM projects p WHERE p.id = ?1 AND p.lab_id = ?2",
    )
    .bind(id.as_str())
    .bind(lab.as_str())
    .fetch_optional(connection)
    .await?;
    Ok(
        row.map(|(name, requests, stories, memories)| ProjectDeletionPlan {
            id: id.clone(),
            lab_id: lab.clone(),
            name,
            requests: u64::try_from(requests).unwrap_or(0),
            stories: u64::try_from(stories).unwrap_or(0),
            memories: u64::try_from(memories).unwrap_or(0),
        }),
    )
}

async fn agent_plan_on(
    connection: &mut SqliteConnection,
    id: &AgentId,
) -> Result<Option<AgentDeletionPlan>, sqlx::Error> {
    let row: Option<(String, Option<String>, i64, i64)> = sqlx::query_as(
        "SELECT a.name, a.archived_at, \
         (SELECT count(*) FROM stories s WHERE s.assigned_agent_id = a.id AND s.state != 'DONE'), \
         (SELECT count(*) FROM memories mem WHERE mem.scope_type = 'agent' AND mem.agent_id = a.id) \
         FROM agents a WHERE a.id = ?1",
    )
    .bind(id.as_str())
    .fetch_optional(&mut *connection)
    .await?;
    let Some((name, archived_at, open, memories)) = row else {
        return Ok(None);
    };
    if archived_at.is_some() {
        return Ok(Some(AgentDeletionPlan {
            id: id.clone(),
            name,
            labs: Vec::new(),
            assigned_open_stories: 0,
            lead_labs: Vec::new(),
            memories: u64::try_from(memories).unwrap_or(0),
            archived: true,
        }));
    }
    let labs: Vec<(String, String)> = sqlx::query_as(
        "SELECT l.id, l.name FROM labs l JOIN lab_memberships m ON m.lab_id = l.id \
         WHERE m.agent_id = ?1 ORDER BY l.name COLLATE NOCASE, l.id",
    )
    .bind(id.as_str())
    .fetch_all(&mut *connection)
    .await?;
    let lead_labs: Vec<(String,)> = sqlx::query_as(
        "SELECT m.lab_id FROM lab_memberships m JOIN agents a ON a.id = m.agent_id \
         WHERE a.id = ?1 AND a.role = 'lead'",
    )
    .bind(id.as_str())
    .fetch_all(&mut *connection)
    .await?;
    Ok(Some(AgentDeletionPlan {
        id: id.clone(),
        name,
        labs: labs
            .into_iter()
            .filter_map(|(id, name)| {
                Some(LabOverview {
                    id: LabId::parse(&id)?,
                    name,
                    agent_count: 0,
                    project_count: 0,
                })
            })
            .collect(),
        assigned_open_stories: u64::try_from(open).unwrap_or(0),
        lead_labs: lead_labs
            .into_iter()
            .filter_map(|(id,)| LabId::parse(&id))
            .collect(),
        memories: u64::try_from(memories).unwrap_or(0),
        archived: false,
    }))
}

async fn has_planning_lab(
    connection: &mut SqliteConnection,
    id: &LabId,
) -> Result<bool, sqlx::Error> {
    let count: i64 = sqlx::query_scalar(PLANNING_LAB)
        .bind(id.as_str())
        .fetch_one(connection)
        .await?;
    Ok(count > 0)
}

async fn has_planning_project(
    connection: &mut SqliteConnection,
    id: &ProjectId,
) -> Result<bool, sqlx::Error> {
    let count: i64 = sqlx::query_scalar(PLANNING_PROJECT)
        .bind(id.as_str())
        .fetch_one(connection)
        .await?;
    Ok(count > 0)
}

async fn has_planning_agent(
    connection: &mut SqliteConnection,
    id: &AgentId,
) -> Result<bool, sqlx::Error> {
    let count: i64 = sqlx::query_scalar(PLANNING_AGENT)
        .bind(id.as_str())
        .fetch_one(connection)
        .await?;
    Ok(count > 0)
}
