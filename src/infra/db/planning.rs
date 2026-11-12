//! Request and story persistence for planning (WP-5-T8).

use crate::app::planning::{PlannedStoryRow, PlanningStore};
use crate::app::storage::StorageError;
use crate::domain::ids::{ProjectId, RequestId};
use crate::domain::skills;
use crate::infra::db::{Database, map_sqlx_error};

const INSERT_REQUEST: &str = "\
INSERT INTO requests (id, project_id, content, status, created_at, updated_at) \
VALUES (?1, ?2, ?3, 'TODO', strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))";

const MARK_PLANNING: &str = "\
UPDATE requests SET status = 'PLANNING', \
planning_started_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), \
updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?1";

const MARK_PLANNED: &str = "\
UPDATE requests SET status = 'PLANNED', updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') \
WHERE id = ?1";

const MARK_CLARIFICATION: &str = "\
UPDATE requests SET status = 'NEEDS_CLARIFICATION', questions_json = ?2, \
updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?1";

const MARK_FAILED: &str = "\
UPDATE requests SET status = 'FAILED', error_code = ?2, error_message = ?3, \
updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?1";

const INSERT_STORY: &str = "\
INSERT INTO stories (id, project_id, request_id, title, description, acceptance_criteria_json, \
priority, complexity, required_skills_json, assigned_agent_id, assignment_rationale, state, \
due_date, created_at, updated_at) \
VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, 'TODO', ?12, \
strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))";

impl Database {
    pub fn insert_request(
        &self,
        id: &RequestId,
        project: &ProjectId,
        content: &str,
    ) -> Result<(), StorageError> {
        self.block_on(async {
            let mut connection = self
                .pool()
                .acquire()
                .await
                .map_err(|error| map_sqlx_error(self.path(), &error))?;
            sqlx::query(INSERT_REQUEST)
                .bind(id.as_str())
                .bind(project.as_str())
                .bind(content)
                .execute(&mut *connection)
                .await
                .map(|_| ())
                .map_err(|error| map_sqlx_error(self.path(), &error))
        })
    }

    pub fn request_created_at(&self, id: &RequestId) -> Result<Option<String>, StorageError> {
        self.block_on(async {
            let mut connection = self
                .pool()
                .acquire()
                .await
                .map_err(|error| map_sqlx_error(self.path(), &error))?;
            sqlx::query_scalar("SELECT created_at FROM requests WHERE id = ?1")
                .bind(id.as_str())
                .fetch_optional(&mut *connection)
                .await
                .map_err(|error| map_sqlx_error(self.path(), &error))
        })
    }

    pub fn mark_planning(&self, id: &RequestId) -> Result<(), StorageError> {
        self.update(MARK_PLANNING, id, None, None)
    }

    pub fn finish_planned(
        &self,
        id: &RequestId,
        project: &ProjectId,
        stories: &[PlannedStoryRow],
    ) -> Result<(), StorageError> {
        let stories = stories.to_vec();
        let id = id.clone();
        let project = project.clone();
        self.transaction(move |connection| {
            Box::pin(async move {
                for story in &stories {
                    let criteria = serde_json::to_string(&story.acceptance_criteria)
                        .map_err(|_| StorageError::Operation)?;
                    let tags = skills::parse_tags(story.required_skills.iter().map(String::as_str))
                        .map_err(|_| StorageError::Operation)?;
                    sqlx::query(INSERT_STORY)
                        .bind(story.id.as_str())
                        .bind(project.as_str())
                        .bind(id.as_str())
                        .bind(&story.title)
                        .bind(&story.description)
                        .bind(criteria)
                        .bind(&story.priority)
                        .bind(&story.complexity)
                        .bind(skills::encode(&tags))
                        .bind(story.assigned_agent_id.as_deref())
                        .bind(&story.assignment_rationale)
                        .bind(&story.due_date)
                        .execute(&mut *connection)
                        .await
                        .map_err(|_| StorageError::Operation)?;
                }
                sqlx::query(MARK_PLANNED)
                    .bind(id.as_str())
                    .execute(&mut *connection)
                    .await
                    .map_err(|_| StorageError::Operation)?;
                Ok(())
            })
        })
    }

    pub fn finish_clarification(
        &self,
        id: &RequestId,
        questions: &[String],
    ) -> Result<(), StorageError> {
        let questions = serde_json::to_string(questions).map_err(|_| StorageError::Operation)?;
        self.update(MARK_CLARIFICATION, id, Some(questions), None)
    }

    pub fn finish_failed(
        &self,
        id: &RequestId,
        code: &str,
        message: &str,
    ) -> Result<(), StorageError> {
        self.update(
            MARK_FAILED,
            id,
            Some(code.to_owned()),
            Some(message.to_owned()),
        )
    }

    /// Records a stale PLANNING row as failed. Returns how many rows were
    /// recovered. Never retries.
    pub fn recover_stale_planning(&self, lease_seconds: i64) -> Result<u64, StorageError> {
        self.block_on(async {
            let mut connection = self
                .pool()
                .acquire()
                .await
                .map_err(|error| map_sqlx_error(self.path(), &error))?;
            let sql = format!(
                "UPDATE requests SET status = 'FAILED', error_code = 'stale_planning', \
                 error_message = 'the previous planning attempt expired.', \
                 updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') \
                 WHERE status = 'PLANNING' AND planning_started_at < \
                 strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '-{lease_seconds} seconds')"
            );
            let result = sqlx::query(sqlx::AssertSqlSafe(sql))
                .execute(&mut *connection)
                .await
                .map_err(|error| map_sqlx_error(self.path(), &error))?;
            Ok(result.rows_affected())
        })
    }

    fn update(
        &self,
        sql: &'static str,
        id: &RequestId,
        first: Option<String>,
        second: Option<String>,
    ) -> Result<(), StorageError> {
        self.block_on(async {
            let mut connection = self
                .pool()
                .acquire()
                .await
                .map_err(|error| map_sqlx_error(self.path(), &error))?;
            let mut query = sqlx::query(sql).bind(id.as_str());
            if let Some(value) = first {
                query = query.bind(value);
            }
            if let Some(value) = second {
                query = query.bind(value);
            }
            query
                .execute(&mut *connection)
                .await
                .map(|_| ())
                .map_err(|error| map_sqlx_error(self.path(), &error))
        })
    }
}

impl PlanningStore for Database {
    fn insert_request(
        &self,
        id: &RequestId,
        project: &ProjectId,
        content: &str,
    ) -> Result<(), StorageError> {
        Database::insert_request(self, id, project, content)
    }

    fn request_created_at(&self, id: &RequestId) -> Result<Option<String>, StorageError> {
        Database::request_created_at(self, id)
    }

    fn mark_planning(&self, id: &RequestId) -> Result<(), StorageError> {
        Database::mark_planning(self, id)
    }

    fn finish_planned(
        &self,
        id: &RequestId,
        project: &ProjectId,
        stories: &[PlannedStoryRow],
    ) -> Result<(), StorageError> {
        Database::finish_planned(self, id, project, stories)
    }

    fn finish_clarification(
        &self,
        id: &RequestId,
        questions: &[String],
    ) -> Result<(), StorageError> {
        Database::finish_clarification(self, id, questions)
    }

    fn finish_failed(&self, id: &RequestId, code: &str, message: &str) -> Result<(), StorageError> {
        Database::finish_failed(self, id, code, message)
    }

    fn recover_stale_planning(&self, lease_seconds: i64) -> Result<u64, StorageError> {
        Database::recover_stale_planning(self, lease_seconds)
    }

    fn record_event(&self, event: &crate::obs::events::AuditEvent) -> Result<(), StorageError> {
        Database::record(self, event)
    }
}
