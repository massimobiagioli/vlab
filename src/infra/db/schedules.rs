//! SQLite persistence for recurring project schedules (ADR-0057).

use crate::app::schedules::{ProjectSchedules, ScheduleStore};
use crate::app::storage::StorageError;
use crate::domain::agent::Description;
use crate::domain::ids::{AgentId, LabId, ProjectId, RequestId, ScheduleId};
use crate::domain::name::Name;
use crate::domain::schedule::{Schedule, ScheduleInterval};
use crate::infra::db::{Database, map_sqlx_error};

const SELECT_PROJECT_NAME: &str = "SELECT name FROM projects WHERE id = ?1 AND lab_id = ?2";

const SELECT_BY_PROJECT: &str = "\
SELECT s.id, s.project_id, s.agent_id, s.title, s.request_text, s.interval_seconds, \
s.enabled, s.next_run_at, s.last_run_at, s.last_request_id, \
r.status AS last_request_status, s.last_error_code \
FROM schedules s LEFT JOIN requests r ON r.id = s.last_request_id \
WHERE s.project_id = ?1 ORDER BY s.created_at, s.id";

const SELECT_DUE: &str = "\
SELECT s.id, s.project_id, s.agent_id, s.title, s.request_text, s.interval_seconds, \
s.enabled, s.next_run_at, s.last_run_at, s.last_request_id, \
r.status AS last_request_status, s.last_error_code \
FROM schedules s LEFT JOIN requests r ON r.id = s.last_request_id \
WHERE s.project_id = ?1 AND s.enabled = 1 \
AND s.next_run_at <= strftime('%Y-%m-%dT%H:%M:%fZ', 'now') \
ORDER BY s.next_run_at, s.id";

const SELECT_ONE: &str = "\
SELECT s.id, s.project_id, s.agent_id, s.title, s.request_text, s.interval_seconds, \
s.enabled, s.next_run_at, s.last_run_at, s.last_request_id, \
r.status AS last_request_status, s.last_error_code \
FROM schedules s LEFT JOIN requests r ON r.id = s.last_request_id \
WHERE s.id = ?1";

const INSERT: &str = "\
INSERT INTO schedules (id, project_id, agent_id, title, request_text, interval_seconds, \
enabled, next_run_at, created_at, updated_at) \
VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, \
strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '+' || ?8 || ' seconds'), \
strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))";

const SET_ENABLED: &str = "\
UPDATE schedules SET enabled = ?2, \
updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?1";

const DELETE: &str = "DELETE FROM schedules WHERE id = ?1";

const RECORD_RUN: &str = "\
UPDATE schedules SET last_run_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), \
next_run_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '+' || interval_seconds || ' seconds'), \
last_request_id = ?2, last_error_code = ?3, \
updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?1";

#[derive(sqlx::FromRow)]
struct ScheduleDbRow {
    id: String,
    project_id: String,
    agent_id: String,
    title: String,
    request_text: String,
    interval_seconds: i64,
    enabled: i64,
    next_run_at: String,
    last_run_at: Option<String>,
    last_request_id: Option<String>,
    last_request_status: Option<String>,
    last_error_code: Option<String>,
}

impl Database {
    fn project_schedule_name(
        &self,
        project: &ProjectId,
        lab: &LabId,
    ) -> Result<Option<String>, StorageError> {
        self.block_on(async {
            let mut connection = self
                .pool()
                .acquire()
                .await
                .map_err(|error| map_sqlx_error(self.path(), &error))?;
            sqlx::query_scalar(SELECT_PROJECT_NAME)
                .bind(project.as_str())
                .bind(lab.as_str())
                .fetch_optional(&mut *connection)
                .await
                .map_err(|error| map_sqlx_error(self.path(), &error))
        })
    }
}

impl ScheduleStore for Database {
    fn project_schedules(
        &self,
        project: &ProjectId,
        lab: &LabId,
    ) -> Result<Option<ProjectSchedules>, StorageError> {
        let Some(project_name) = self.project_schedule_name(project, lab)? else {
            return Ok(None);
        };
        let schedules = self.block_on(async {
            let mut connection = self
                .pool()
                .acquire()
                .await
                .map_err(|error| map_sqlx_error(self.path(), &error))?;
            let rows: Vec<ScheduleDbRow> = sqlx::query_as(SELECT_BY_PROJECT)
                .bind(project.as_str())
                .fetch_all(&mut *connection)
                .await
                .map_err(|error| map_sqlx_error(self.path(), &error))?;
            rows.into_iter()
                .map(map_schedule)
                .collect::<Result<Vec<_>, _>>()
                .map_err(|error| map_sqlx_error(self.path(), &error))
        })?;
        Ok(Some(ProjectSchedules {
            project_name,
            schedules,
        }))
    }

    fn due_schedules(&self, project: &ProjectId) -> Result<Vec<Schedule>, StorageError> {
        self.block_on(async {
            let mut connection = self
                .pool()
                .acquire()
                .await
                .map_err(|error| map_sqlx_error(self.path(), &error))?;
            let rows: Vec<ScheduleDbRow> = sqlx::query_as(SELECT_DUE)
                .bind(project.as_str())
                .fetch_all(&mut *connection)
                .await
                .map_err(|error| map_sqlx_error(self.path(), &error))?;
            rows.into_iter()
                .map(map_schedule)
                .collect::<Result<Vec<_>, _>>()
                .map_err(|error| map_sqlx_error(self.path(), &error))
        })
    }

    fn schedule(&self, id: &ScheduleId) -> Result<Option<Schedule>, StorageError> {
        self.block_on(async {
            let mut connection = self
                .pool()
                .acquire()
                .await
                .map_err(|error| map_sqlx_error(self.path(), &error))?;
            let row: Option<ScheduleDbRow> = sqlx::query_as(SELECT_ONE)
                .bind(id.as_str())
                .fetch_optional(&mut *connection)
                .await
                .map_err(|error| map_sqlx_error(self.path(), &error))?;
            row.map(map_schedule)
                .transpose()
                .map_err(|error| map_sqlx_error(self.path(), &error))
        })
    }

    fn insert_schedule(&self, schedule: &Schedule) -> Result<(), StorageError> {
        let interval = schedule.interval.seconds();
        self.block_on(async {
            let mut connection = self
                .pool()
                .acquire()
                .await
                .map_err(|error| map_sqlx_error(self.path(), &error))?;
            sqlx::query(INSERT)
                .bind(schedule.id.as_str())
                .bind(schedule.project_id.as_str())
                .bind(schedule.agent_id.as_str())
                .bind(schedule.title.as_str())
                .bind(schedule.request_text.as_str())
                .bind(interval)
                .bind(if schedule.enabled { 1 } else { 0 })
                .bind(interval)
                .execute(&mut *connection)
                .await
                .map(|_| ())
                .map_err(|error| map_sqlx_error(self.path(), &error))
        })
    }

    fn set_schedule_enabled(&self, id: &ScheduleId, enabled: bool) -> Result<(), StorageError> {
        self.block_on(async {
            let mut connection = self
                .pool()
                .acquire()
                .await
                .map_err(|error| map_sqlx_error(self.path(), &error))?;
            sqlx::query(SET_ENABLED)
                .bind(id.as_str())
                .bind(if enabled { 1 } else { 0 })
                .execute(&mut *connection)
                .await
                .map(|_| ())
                .map_err(|error| map_sqlx_error(self.path(), &error))
        })
    }

    fn delete_schedule(&self, id: &ScheduleId) -> Result<(), StorageError> {
        self.block_on(async {
            let mut connection = self
                .pool()
                .acquire()
                .await
                .map_err(|error| map_sqlx_error(self.path(), &error))?;
            sqlx::query(DELETE)
                .bind(id.as_str())
                .execute(&mut *connection)
                .await
                .map(|_| ())
                .map_err(|error| map_sqlx_error(self.path(), &error))
        })
    }

    fn record_schedule_run(
        &self,
        id: &ScheduleId,
        request: Option<&RequestId>,
        error_code: Option<&str>,
    ) -> Result<(), StorageError> {
        self.block_on(async {
            let mut connection = self
                .pool()
                .acquire()
                .await
                .map_err(|error| map_sqlx_error(self.path(), &error))?;
            sqlx::query(RECORD_RUN)
                .bind(id.as_str())
                .bind(request.map(RequestId::as_str))
                .bind(error_code)
                .execute(&mut *connection)
                .await
                .map(|_| ())
                .map_err(|error| map_sqlx_error(self.path(), &error))
        })
    }
}

fn map_schedule(row: ScheduleDbRow) -> Result<Schedule, sqlx::Error> {
    Ok(Schedule {
        id: ScheduleId::parse(&row.id).ok_or(sqlx::Error::RowNotFound)?,
        project_id: ProjectId::parse(&row.project_id).ok_or(sqlx::Error::RowNotFound)?,
        agent_id: AgentId::parse(&row.agent_id).ok_or(sqlx::Error::RowNotFound)?,
        title: Name::parse(&row.title).map_err(|_| sqlx::Error::RowNotFound)?,
        request_text: Description::parse(&row.request_text)
            .map_err(|_| sqlx::Error::RowNotFound)?,
        interval: ScheduleInterval::from_seconds(row.interval_seconds)
            .ok_or(sqlx::Error::RowNotFound)?,
        enabled: row.enabled != 0,
        next_run_at: row.next_run_at,
        last_run_at: row.last_run_at,
        last_request_id: row
            .last_request_id
            .map(|id| RequestId::parse(&id).ok_or(sqlx::Error::RowNotFound))
            .transpose()?,
        last_request_status: row.last_request_status,
        last_error_code: row.last_error_code,
    })
}
