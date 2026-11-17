//! Read model for the project interaction history (ADR-0056).
//!
//! The queries return project data for the application builder. The
//! content-free `interactions` table is not involved.

use crate::app::interactions::{
    InteractionData, InteractionStore, Participant, RequestInteractionRow, StoryInteractionRow,
};
use crate::app::storage::StorageError;
use crate::domain::ids::{AgentId, LabId, ProjectId, RequestId, StoryId};
use crate::infra::db::{Database, map_sqlx_error};

const SELECT_PROJECT: &str = "SELECT name FROM projects WHERE id = ?1 AND lab_id = ?2";

const SELECT_REQUESTS: &str = "\
SELECT r.id, r.content, r.status, r.error_code, r.questions_json, r.created_at, \
       r.planned_by_agent_id, a.name AS planner_name, a.role AS planner_role \
FROM requests r \
LEFT JOIN agents a ON a.id = r.planned_by_agent_id \
WHERE r.project_id = ?1 \
ORDER BY r.created_at DESC, r.id";

const SELECT_STORIES: &str = "\
SELECT s.id, s.request_id, s.title, s.state, s.created_at, s.updated_at, \
       s.assigned_agent_id, assignee.name AS assignee_name, assignee.role AS assignee_role, \
       o.work_outcome, o.review_outcome, o.reviewer_agent_id, \
       reviewer.name AS reviewer_name, reviewer.role AS reviewer_role, \
       o.review_accepted, o.completed_at \
FROM stories s \
LEFT JOIN agents assignee ON assignee.id = s.assigned_agent_id \
LEFT JOIN story_outcomes o ON o.story_id = s.id \
LEFT JOIN agents reviewer ON reviewer.id = o.reviewer_agent_id \
WHERE s.project_id = ?1 \
ORDER BY s.created_at DESC, s.id";

#[derive(sqlx::FromRow)]
struct RequestDbRow {
    id: String,
    content: String,
    status: String,
    error_code: Option<String>,
    questions_json: Option<String>,
    created_at: String,
    planned_by_agent_id: Option<String>,
    planner_name: Option<String>,
    planner_role: Option<String>,
}

#[derive(sqlx::FromRow)]
struct StoryDbRow {
    id: String,
    request_id: String,
    title: String,
    state: String,
    created_at: String,
    updated_at: String,
    assigned_agent_id: Option<String>,
    assignee_name: Option<String>,
    assignee_role: Option<String>,
    work_outcome: Option<String>,
    review_outcome: Option<String>,
    reviewer_agent_id: Option<String>,
    reviewer_name: Option<String>,
    reviewer_role: Option<String>,
    review_accepted: Option<i64>,
    completed_at: Option<String>,
}

impl InteractionStore for Database {
    fn project_interaction_data(
        &self,
        project: &ProjectId,
        lab: &LabId,
    ) -> Result<Option<InteractionData>, StorageError> {
        self.block_on(async {
            let mut connection = self
                .pool()
                .acquire()
                .await
                .map_err(|error| map_sqlx_error(self.path(), &error))?;
            let project_name: Option<String> = sqlx::query_scalar(SELECT_PROJECT)
                .bind(project.as_str())
                .bind(lab.as_str())
                .fetch_optional(&mut *connection)
                .await
                .map_err(|error| map_sqlx_error(self.path(), &error))?;
            let Some(project_name) = project_name else {
                return Ok(None);
            };
            let requests: Vec<RequestDbRow> = sqlx::query_as(SELECT_REQUESTS)
                .bind(project.as_str())
                .fetch_all(&mut *connection)
                .await
                .map_err(|error| map_sqlx_error(self.path(), &error))?;
            let stories: Vec<StoryDbRow> = sqlx::query_as(SELECT_STORIES)
                .bind(project.as_str())
                .fetch_all(&mut *connection)
                .await
                .map_err(|error| map_sqlx_error(self.path(), &error))?;
            let requests = requests
                .into_iter()
                .map(map_request)
                .collect::<Result<Vec<_>, _>>()
                .map_err(|error| map_sqlx_error(self.path(), &error))?;
            let stories = stories
                .into_iter()
                .map(map_story)
                .collect::<Result<Vec<_>, _>>()
                .map_err(|error| map_sqlx_error(self.path(), &error))?;
            Ok(Some(InteractionData {
                project_id: project.clone(),
                project_name,
                requests,
                stories,
            }))
        })
    }
}

fn map_request(row: RequestDbRow) -> Result<RequestInteractionRow, sqlx::Error> {
    Ok(RequestInteractionRow {
        id: RequestId::parse(&row.id).ok_or(sqlx::Error::RowNotFound)?,
        content: row.content,
        status: row.status,
        error_code: row.error_code,
        questions: row
            .questions_json
            .as_deref()
            .and_then(|json| serde_json::from_str(json).ok())
            .unwrap_or_default(),
        created_at: row.created_at,
        planner: participant(row.planned_by_agent_id, row.planner_name, row.planner_role)?,
    })
}

fn map_story(row: StoryDbRow) -> Result<StoryInteractionRow, sqlx::Error> {
    Ok(StoryInteractionRow {
        id: StoryId::parse(&row.id).ok_or(sqlx::Error::RowNotFound)?,
        request_id: RequestId::parse(&row.request_id).ok_or(sqlx::Error::RowNotFound)?,
        title: row.title,
        state: row.state,
        created_at: row.created_at,
        updated_at: row.updated_at,
        assignee: participant(row.assigned_agent_id, row.assignee_name, row.assignee_role)?,
        work_outcome: row.work_outcome,
        review_outcome: row.review_outcome,
        reviewer: participant(row.reviewer_agent_id, row.reviewer_name, row.reviewer_role)?,
        accepted: row.review_accepted.map(|value| value != 0),
        completed_at: row.completed_at,
    })
}

fn participant(
    id: Option<String>,
    name: Option<String>,
    role: Option<String>,
) -> Result<Option<Participant>, sqlx::Error> {
    match (id, name, role) {
        (Some(id), Some(name), Some(role)) => Ok(Some(Participant {
            id: AgentId::parse(&id).ok_or(sqlx::Error::RowNotFound)?,
            name,
            role,
        })),
        _ => Ok(None),
    }
}
