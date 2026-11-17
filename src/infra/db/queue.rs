//! SQLite request-queue queries (ADR-0061).

use crate::app::queue::{QueueStore, QueuedRequest};
use crate::app::storage::StorageError;
use crate::domain::ids::{LabId, ProjectId, RequestId};
use crate::infra::db::{Database, map_sqlx_error};

const SELECT_QUEUED: &str = "\
SELECT r.id, r.project_id, p.lab_id, r.content, r.status, r.created_at \
FROM requests r JOIN projects p ON p.id = r.project_id \
WHERE p.lab_id = ?1 AND r.status IN ('TODO', 'PLANNING') \
ORDER BY r.created_at, r.id";

const SELECT_NEXT: &str = "\
SELECT r.id, r.project_id, p.lab_id, r.content, r.status, r.created_at \
FROM requests r JOIN projects p ON p.id = r.project_id \
WHERE p.lab_id = ?1 AND r.status = 'TODO' \
ORDER BY r.created_at, r.id LIMIT 1";

type QueueRow = (String, String, String, String, String, String);

fn from_row(value: QueueRow) -> Result<QueuedRequest, StorageError> {
    Ok(QueuedRequest {
        id: RequestId::parse(&value.0).ok_or(StorageError::Operation)?,
        project_id: ProjectId::parse(&value.1).ok_or(StorageError::Operation)?,
        lab_id: LabId::parse(&value.2).ok_or(StorageError::Operation)?,
        content: value.3,
        status: value.4,
        created_at: value.5,
    })
}

impl QueueStore for Database {
    fn next_todo_request(&self, lab: &LabId) -> Result<Option<QueuedRequest>, StorageError> {
        self.block_on(async {
            let mut connection = self
                .pool()
                .acquire()
                .await
                .map_err(|error| map_sqlx_error(self.path(), &error))?;
            let row: Option<QueueRow> = sqlx::query_as(SELECT_NEXT)
                .bind(lab.as_str())
                .fetch_optional(&mut *connection)
                .await
                .map_err(|error| map_sqlx_error(self.path(), &error))?;
            row.map(from_row).transpose()
        })
    }

    fn queued_requests(&self, lab: &LabId) -> Result<Vec<QueuedRequest>, StorageError> {
        self.block_on(async {
            let mut connection = self
                .pool()
                .acquire()
                .await
                .map_err(|error| map_sqlx_error(self.path(), &error))?;
            let rows: Vec<QueueRow> = sqlx::query_as(SELECT_QUEUED)
                .bind(lab.as_str())
                .fetch_all(&mut *connection)
                .await
                .map_err(|error| map_sqlx_error(self.path(), &error))?;
            rows.into_iter().map(from_row).collect()
        })
    }
}
