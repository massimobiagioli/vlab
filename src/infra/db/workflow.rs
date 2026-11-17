//! SQLite implementation of the real workflow store (ADR-0050, ADR-0067).

use sqlx::SqliteConnection;

use crate::app::storage::StorageError;
use crate::app::workflow::{
    DebtStory, PendingIntegration, StoryOutcome, WorkflowStore, WorkflowStory,
};
use crate::domain::ids::{AgentId, ProjectId, RequestId, StoryId};
use crate::infra::db::{Database, map_sqlx_error};

const SELECT_PENDING: &str = "\
SELECT s.id, s.request_id, s.project_id, s.title, s.description, \
       s.acceptance_criteria_json, s.priority, s.complexity, p.name AS project_name, \
       s.assigned_agent_id AS assignee, r.content AS request_text, s.due_date, \
       s.execution_attempts, s.remediation_attempts, s.tech_lead_context, \
       s.debt_of_story_id, o.work_outcome, o.review_outcome, \
       o.reviewer_agent_id, o.review_accepted, o.branch, o.worktree_path, o.commit_sha \
FROM stories s \
JOIN projects p ON p.id = s.project_id \
JOIN requests r ON r.id = s.request_id \
LEFT JOIN story_outcomes o ON o.story_id = s.id \
WHERE s.project_id = ?1 AND s.state IN ('TODO', 'IN_PROGRESS', 'IN_REVIEW') \
  AND s.assigned_agent_id IS NOT NULL \
  AND NOT (s.state = 'IN_PROGRESS' AND s.execution_attempts >= 3 \
           AND s.remediation_attempts < 3) \
  AND NOT (COALESCE(o.review_accepted, 1) = 0 AND s.remediation_attempts < 3) \
ORDER BY s.created_at, s.id";

const SELECT_BLOCKED: &str = "\
SELECT s.id, s.request_id, s.project_id, s.title, s.description, \
       s.acceptance_criteria_json, s.priority, s.complexity, p.name AS project_name, \
       s.assigned_agent_id AS assignee, r.content AS request_text, s.due_date, \
       s.execution_attempts, s.remediation_attempts, s.tech_lead_context, \
       s.debt_of_story_id, o.work_outcome, o.review_outcome, \
       o.reviewer_agent_id, o.review_accepted, o.branch, o.worktree_path, o.commit_sha \
FROM stories s \
JOIN projects p ON p.id = s.project_id \
JOIN requests r ON r.id = s.request_id \
LEFT JOIN story_outcomes o ON o.story_id = s.id \
WHERE s.project_id = ?1 AND s.assigned_agent_id IS NOT NULL \
  AND (s.state = 'BLOCKED' OR (s.state = 'IN_PROGRESS' AND \
       (s.execution_attempts >= 3 OR COALESCE(o.review_accepted, 1) = 0))) \
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

const CLOSE_STORY: &str = "\
UPDATE stories SET state = 'DONE', tech_lead_context = ?2, \
  updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') \
WHERE id = ?1";

const INSERT_DEBT: &str = "\
INSERT INTO stories (id, project_id, request_id, title, description, \
  acceptance_criteria_json, priority, complexity, required_skills_json, \
  assigned_agent_id, assignment_rationale, state, due_date, created_at, updated_at, \
  debt_of_story_id) \
VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, '[]', NULL, ?9, 'TODO', ?10, \
  strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), ?11)";

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
  completed_at = excluded.completed_at, \
  integrated_at = NULL, \
  push_ok = 0, \
  integration_error = NULL";

const SELECT_PENDING_INTEGRATIONS: &str = "\
SELECT o.story_id, o.branch, o.commit_sha \
FROM story_outcomes o \
JOIN stories s ON s.id = o.story_id \
WHERE s.project_id = ?1 AND s.state = 'DONE' \
  AND o.branch IS NOT NULL AND o.commit_sha IS NOT NULL \
  AND ((o.integrated_at IS NULL \
        AND COALESCE(o.integration_error, '') <> 'legacy_not_integrated') \
       OR o.push_ok = 0) \
ORDER BY o.completed_at, o.story_id";

const FINISH_INTEGRATION: &str = "\
UPDATE story_outcomes SET \
  integrated_at = CASE WHEN ?2 IS NULL THEN integrated_at \
    ELSE COALESCE(integrated_at, strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) END, \
  push_ok = ?3, \
  integration_error = ?4 \
WHERE story_id = ?1";

#[derive(sqlx::FromRow)]
struct Row {
    id: String,
    request_id: String,
    project_id: String,
    title: String,
    description: String,
    acceptance_criteria_json: String,
    priority: String,
    complexity: String,
    project_name: String,
    assignee: String,
    request_text: String,
    due_date: String,
    execution_attempts: i64,
    remediation_attempts: i64,
    tech_lead_context: Option<String>,
    debt_of_story_id: Option<String>,
    work_outcome: Option<String>,
    review_outcome: Option<String>,
    reviewer_agent_id: Option<String>,
    review_accepted: Option<i64>,
    branch: Option<String>,
    worktree_path: Option<String>,
    commit_sha: Option<String>,
}

#[derive(sqlx::FromRow)]
struct PendingRow {
    story_id: String,
    branch: String,
    commit_sha: String,
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

    fn close_with_debt(
        &self,
        story: &WorkflowStory,
        outcome: &StoryOutcome,
        decision: &str,
        debt: Option<&DebtStory>,
    ) -> Result<(), StorageError> {
        let story_id = story.id.clone();
        let project_id = story.project_id.clone();
        let request_id = story.request_id.clone();
        let priority = story.priority.clone();
        let complexity = story.complexity.clone();
        let outcome = outcome.clone();
        let decision = decision.to_owned();
        let debt = debt.cloned();
        self.transaction(move |connection| {
            Box::pin(async move {
                sqlx::query(UPSERT_OUTCOME)
                    .bind(story_id.as_str())
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
                sqlx::query(CLOSE_STORY)
                    .bind(story_id.as_str())
                    .bind(&decision)
                    .execute(&mut *connection)
                    .await
                    .map_err(|_| StorageError::Operation)?;
                if let Some(debt) = debt {
                    let criteria = serde_json::to_string(&debt.acceptance_criteria)
                        .map_err(|_| StorageError::Operation)?;
                    let rationale = format!("Technical debt from {}.", story_id);
                    sqlx::query(INSERT_DEBT)
                        .bind(debt.id.as_str())
                        .bind(project_id.as_str())
                        .bind(request_id.as_str())
                        .bind(&debt.title)
                        .bind(&debt.description)
                        .bind(&criteria)
                        .bind(&priority)
                        .bind(&complexity)
                        .bind(&rationale)
                        .bind(&debt.due_date)
                        .bind(story_id.as_str())
                        .execute(&mut *connection)
                        .await
                        .map_err(|_| StorageError::Operation)?;
                }
                Ok(())
            })
        })
    }

    fn pending_integrations(
        &self,
        project: &ProjectId,
    ) -> Result<Vec<PendingIntegration>, StorageError> {
        self.block_on(async {
            let mut connection = self
                .pool()
                .acquire()
                .await
                .map_err(|error| map_sqlx_error(self.path(), &error))?;
            let rows: Vec<PendingRow> = sqlx::query_as(SELECT_PENDING_INTEGRATIONS)
                .bind(project.as_str())
                .fetch_all(&mut *connection)
                .await
                .map_err(|error| map_sqlx_error(self.path(), &error))?;
            rows.into_iter()
                .map(|row| {
                    Some(PendingIntegration {
                        story_id: StoryId::parse(&row.story_id)?,
                        branch: row.branch,
                        commit: row.commit_sha,
                    })
                })
                .collect::<Option<Vec<_>>>()
                .ok_or(StorageError::Operation)
        })
    }

    fn finish_integration(
        &self,
        story: &StoryId,
        merge_commit: Option<&str>,
        push_ok: bool,
        error: Option<&str>,
    ) -> Result<(), StorageError> {
        let story = story.clone();
        let merge_commit = merge_commit.map(str::to_owned);
        let error = error.map(str::to_owned);
        self.block_on(async {
            let mut connection = self
                .pool()
                .acquire()
                .await
                .map_err(|error| map_sqlx_error(self.path(), &error))?;
            sqlx::query(FINISH_INTEGRATION)
                .bind(story.as_str())
                .bind(merge_commit.as_deref())
                .bind(push_ok)
                .bind(error.as_deref())
                .execute(&mut *connection)
                .await
                .map_err(|error| map_sqlx_error(self.path(), &error))?;
            Ok(())
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
        request_id: RequestId::parse(&row.request_id)?,
        project_id: ProjectId::parse(&row.project_id)?,
        title: row.title,
        description: row.description,
        acceptance_criteria: serde_json::from_str(&row.acceptance_criteria_json).ok()?,
        priority: row.priority,
        complexity: row.complexity,
        project_name: row.project_name,
        assignee: AgentId::parse(&row.assignee)?,
        request_text: row.request_text,
        due_date: row.due_date,
        execution_attempts: u32::try_from(row.execution_attempts).ok()?,
        remediation_attempts: u32::try_from(row.remediation_attempts).ok()?,
        tech_lead_context: row.tech_lead_context,
        debt_of_story_id: row.debt_of_story_id.as_deref().and_then(StoryId::parse),
        last_outcome,
    })
}
