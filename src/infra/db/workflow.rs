//! SQLite implementation of the real workflow store (ADR-0050).

use sqlx::SqliteConnection;

use crate::app::storage::StorageError;
use crate::app::workflow::{StoryOutcome, WorkflowStore, WorkflowStory};
use crate::domain::ids::{AgentId, ProjectId, StoryId};
use crate::infra::db::{Database, map_sqlx_error};

const SELECT_PENDING: &str = "\
SELECT s.id, s.title, s.description, s.acceptance_criteria_json, s.priority, \
       s.complexity, p.name, s.assigned_agent_id, r.content, s.execution_attempts \
FROM stories s \
JOIN projects p ON p.id = s.project_id \
JOIN requests r ON r.id = s.request_id \
LEFT JOIN story_outcomes o ON o.story_id = s.id \
WHERE s.project_id = ?1 AND s.state IN ('TODO', 'IN_PROGRESS', 'IN_REVIEW') \
  AND s.assigned_agent_id IS NOT NULL AND o.story_id IS NULL \
ORDER BY s.created_at, s.id";

const SET_STATE: &str = "\
UPDATE stories SET state = ?2, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') \
WHERE id = ?1";

const INCREMENT_ATTEMPTS: &str = "\
UPDATE stories SET execution_attempts = execution_attempts + 1, \
  updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') \
WHERE id = ?1 RETURNING execution_attempts";

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

type Row = (
    String,
    String,
    String,
    String,
    String,
    String,
    String,
    String,
    String,
    i64,
);

impl WorkflowStore for Database {
    fn in_progress_stories(&self, project: &ProjectId) -> Result<Vec<WorkflowStory>, StorageError> {
        self.block_on(async {
            let mut connection = self
                .pool()
                .acquire()
                .await
                .map_err(|error| map_sqlx_error(self.path(), &error))?;
            let rows: Vec<Row> = sqlx::query_as(SELECT_PENDING)
                .bind(project.as_str())
                .fetch_all(&mut *connection)
                .await
                .map_err(|error| map_sqlx_error(self.path(), &error))?;
            rows.into_iter()
                .map(|row| map_row(row).ok_or(StorageError::Operation))
                .collect()
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
    let (
        id,
        title,
        description,
        criteria,
        priority,
        complexity,
        project_name,
        assignee,
        request_text,
        execution_attempts,
    ) = row;
    Some(WorkflowStory {
        id: StoryId::parse(&id)?,
        title,
        description,
        acceptance_criteria: serde_json::from_str(&criteria).ok()?,
        priority,
        complexity,
        project_name,
        assignee: AgentId::parse(&assignee)?,
        request_text,
        execution_attempts: u32::try_from(execution_attempts).ok()?,
    })
}
