//! SQLite implementation of the real workflow store (ADR-0050, ADR-0067).

use sqlx::SqliteConnection;

use crate::app::storage::StorageError;
use crate::app::workflow::{StoryOutcome, WorkflowStore, WorkflowStory};
use crate::domain::ids::{AgentId, ProjectId, StoryId};
use crate::infra::db::{Database, map_sqlx_error};

const SELECT_PENDING: &str = "\
SELECT s.id, s.title, s.description, s.acceptance_criteria_json, s.priority, \
       s.complexity, p.name AS project_name, s.assigned_agent_id AS assignee, \
       r.content AS request_text, s.execution_attempts, s.remediation_attempts, \
       s.tech_lead_context, o.work_outcome, o.review_outcome, \
       o.reviewer_agent_id, o.review_accepted, o.branch, o.worktree_path, o.commit_sha \
FROM stories s \
JOIN projects p ON p.id = s.project_id \
JOIN requests r ON r.id = s.request_id \
LEFT JOIN story_outcomes o ON o.story_id = s.id \
WHERE s.project_id = ?1 AND s.state IN ('TODO', 'IN_PROGRESS', 'IN_REVIEW') \
  AND s.assigned_agent_id IS NOT NULL \
ORDER BY s.created_at, s.id";

const SELECT_BLOCKED: &str = "\
SELECT s.id, s.title, s.description, s.acceptance_criteria_json, s.priority, \
       s.complexity, p.name AS project_name, s.assigned_agent_id AS assignee, \
       r.content AS request_text, s.execution_attempts, s.remediation_attempts, \
       s.tech_lead_context, o.work_outcome, o.review_outcome, \
       o.reviewer_agent_id, o.review_accepted, o.branch, o.worktree_path, o.commit_sha \
FROM stories s \
JOIN projects p ON p.id = s.project_id \
JOIN requests r ON r.id = s.request_id \
LEFT JOIN story_outcomes o ON o.story_id = s.id \
WHERE s.project_id = ?1 AND s.state = 'BLOCKED' AND s.assigned_agent_id IS NOT NULL \
ORDER BY s.created_at, s.id";

const SET_STATE: &str = "\
UPDATE stories SET state = ?2, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') \
WHERE id = ?1";

const INCREMENT_ATTEMPTS: &str = "\
UPDATE stories SET execution_attempts = execution_attempts + 1, \
  updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') \
WHERE id = ?1 RETURNING execution_attempts";

const REMEDIATE: &str = "\
UPDATE stories SET assigned_agent_id = ?2, state = 'IN_PROGRESS', \
  execution_attempts = 0, remediation_attempts = remediation_attempts + 1, \
  tech_lead_context = ?3, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') \
WHERE id = ?1";

const UPSERT_OUTCOME: &str = "\
INSERT INTO story_outcomes (story_id, work_outcome, review_outcome, reviewer_agent_id, \
  review_accepted, branch, worktree_path, commit_sha, completed_at) \
VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) \
ON CONFLICT (story_id) DO UPDATE SET \
  work_outcome = excluded.work_outcome, \
  review_outcome = excluded.review_outcome, \
  reviewer_agent_id = excluded.reviewer_agent_id, \
  review_accepted = excluded.review_accepted, \
  branch = excluded.branch, \
  worktree_path = excluded.worktree_path, \
  commit_sha = excluded.commit_sha, \
  completed_at = excluded.completed_at";

#[derive(sqlx::FromRow)]
struct Row {
    id: String,
    title: String,
    description: String,
    acceptance_criteria_json: String,
    priority: String,
    complexity: String,
    project_name: String,
    assignee: String,
    request_text: String,
    execution_attempts: i64,
    remediation_attempts: i64,
    tech_lead_context: Option<String>,
    work_outcome: Option<String>,
    review_outcome: Option<String>,
    reviewer_agent_id: Option<String>,
    review_accepted: Option<i64>,
    branch: Option<String>,
    worktree_path: Option<String>,
    commit_sha: Option<String>,
}

impl WorkflowStore for Database {
    fn in_progress_stories(&self, project: &ProjectId) -> Result<Vec<WorkflowStory>, StorageError> {
        self.stories(project, SELECT_PENDING)
    }

    fn blocked_stories(&self, project: &ProjectId) -> Result<Vec<WorkflowStory>, StorageError> {
        self.stories(project, SELECT_BLOCKED)
    }

    fn remediate_story(
        &self,
        story: &StoryId,
        assignee: &AgentId,
        context: &str,
    ) -> Result<(), StorageError> {
        let story = story.clone();
        let assignee = assignee.clone();
        let context = context.to_owned();
        self.transaction(move |connection| {
            Box::pin(async move {
                sqlx::query(REMEDIATE)
                    .bind(story.as_str())
                    .bind(assignee.as_str())
                    .bind(&context)
                    .execute(&mut *connection)
                    .await
                    .map_err(|_| StorageError::Operation)?;
                sqlx::query("DELETE FROM story_outcomes WHERE story_id = ?1")
                    .bind(story.as_str())
                    .execute(&mut *connection)
                    .await
                    .map_err(|_| StorageError::Operation)?;
                Ok(())
            })
        })
    }

    fn update_story_state(&self, story: &StoryId, state: &str) -> Result<(), StorageError> {
        self.block_on(async {
            let mut connection = self
                .pool()
                .acquire()
                .await
                .map_err(|error| map_sqlx_error(self.path(), &error))?;
            set_state(&mut connection, story, state)
                .await
                .map_err(|error| map_sqlx_error(self.path(), &error))
        })
    }

    fn increment_attempts(&self, story: &StoryId) -> Result<u32, StorageError> {
        self.block_on(async {
            let mut connection = self
                .pool()
                .acquire()
                .await
                .map_err(|error| map_sqlx_error(self.path(), &error))?;
            let attempts: i64 = sqlx::query_scalar(INCREMENT_ATTEMPTS)
                .bind(story.as_str())
                .fetch_one(&mut *connection)
                .await
                .map_err(|error| map_sqlx_error(self.path(), &error))?;
            u32::try_from(attempts).map_err(|_| StorageError::Operation)
        })
    }

    fn finish_story_outcome(
        &self,
        story: &StoryId,
        outcome: &StoryOutcome,
        final_state: &str,
    ) -> Result<(), StorageError> {
        let story = story.clone();
        let outcome = outcome.clone();
        let final_state = final_state.to_owned();
        self.transaction(move |connection| {
            Box::pin(async move {
                sqlx::query(UPSERT_OUTCOME)
                    .bind(story.as_str())
                    .bind(&outcome.work_outcome)
                    .bind(&outcome.review_outcome)
                    .bind(outcome.reviewer_agent_id.as_str())
                    .bind(outcome.accepted)
                    .bind(&outcome.branch)
                    .bind(&outcome.worktree_path)
                    .bind(&outcome.commit_sha)
                    .execute(&mut *connection)
                    .await
                    .map_err(|_| StorageError::Operation)?;
                set_state(connection, &story, &final_state)
                    .await
                    .map_err(|_| StorageError::Operation)
            })
        })
    }
}

impl Database {
    fn stories(
        &self,
        project: &ProjectId,
        sql: &'static str,
    ) -> Result<Vec<WorkflowStory>, StorageError> {
        self.block_on(async {
            let mut connection = self
                .pool()
                .acquire()
                .await
                .map_err(|error| map_sqlx_error(self.path(), &error))?;
            let rows: Vec<Row> = sqlx::query_as(sql)
                .bind(project.as_str())
                .fetch_all(&mut *connection)
                .await
                .map_err(|error| map_sqlx_error(self.path(), &error))?;
            rows.into_iter()
                .map(|row| map_row(row).ok_or(StorageError::Operation))
                .collect()
        })
    }
}

async fn set_state(
    connection: &mut SqliteConnection,
    story: &StoryId,
    state: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(SET_STATE)
        .bind(story.as_str())
        .bind(state)
        .execute(connection)
        .await
        .map(|_| ())
}

fn map_row(row: Row) -> Option<WorkflowStory> {
    let last_outcome = match (row.work_outcome, row.review_outcome) {
        (Some(work_outcome), Some(review_outcome)) => {
            let reviewer_agent_id = AgentId::parse(row.reviewer_agent_id.as_deref()?)?;
            Some(StoryOutcome {
                work_outcome,
                review_outcome,
                reviewer_agent_id,
                accepted: row.review_accepted.unwrap_or(0) != 0,
                branch: row.branch.unwrap_or_default(),
                worktree_path: row.worktree_path.unwrap_or_default(),
                commit_sha: row.commit_sha.unwrap_or_default(),
            })
        }
        _ => None,
    };
    Some(WorkflowStory {
        id: StoryId::parse(&row.id)?,
        title: row.title,
        description: row.description,
        acceptance_criteria: serde_json::from_str(&row.acceptance_criteria_json).ok()?,
        priority: row.priority,
        complexity: row.complexity,
        project_name: row.project_name,
        assignee: AgentId::parse(&row.assignee)?,
        request_text: row.request_text,
        execution_attempts: u32::try_from(row.execution_attempts).ok()?,
        remediation_attempts: u32::try_from(row.remediation_attempts).ok()?,
        tech_lead_context: row.tech_lead_context,
        last_outcome,
    })
}
