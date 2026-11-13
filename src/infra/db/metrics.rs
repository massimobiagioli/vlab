//! SQLite aggregate for the metrics dashboard (ADR-0051).

use std::collections::HashMap;

use crate::app::kanban::BoardCounts;
use crate::app::metrics::{LabMetrics, MetricsStore, ProjectMetrics};
use crate::app::storage::StorageError;
use crate::domain::ids::{LabId, ProjectId};
use crate::infra::db::{Database, map_sqlx_error};

const SELECT_LAB: &str = "SELECT name FROM labs WHERE id = ?1";

const SELECT_PROJECTS: &str = "\
SELECT p.id, p.name, s.state, count(s.id) \
FROM projects p LEFT JOIN stories s ON s.project_id = p.id \
WHERE p.lab_id = ?1 \
GROUP BY p.id, p.name, s.state \
ORDER BY p.name COLLATE NOCASE, p.id";

const SELECT_PENDING: &str = "\
SELECT count(*) FROM requests r JOIN projects p ON p.id = r.project_id \
WHERE p.lab_id = ?1 AND r.status IN ('TODO', 'PLANNING', 'NEEDS_CLARIFICATION', 'FAILED')";

type ProjectRow = (String, String, Option<String>, i64);

impl MetricsStore for Database {
    fn lab_metrics(&self, lab: &LabId) -> Result<LabMetrics, StorageError> {
        self.block_on(async {
            let mut connection = self
                .pool()
                .acquire()
                .await
                .map_err(|error| map_sqlx_error(self.path(), &error))?;
            let lab_name: String = sqlx::query_scalar(SELECT_LAB)
                .bind(lab.as_str())
                .fetch_one(&mut *connection)
                .await
                .map_err(|error| map_sqlx_error(self.path(), &error))?;
            let rows: Vec<ProjectRow> = sqlx::query_as(SELECT_PROJECTS)
                .bind(lab.as_str())
                .fetch_all(&mut *connection)
                .await
                .map_err(|error| map_sqlx_error(self.path(), &error))?;
            let pending_intake: i64 = sqlx::query_scalar(SELECT_PENDING)
                .bind(lab.as_str())
                .fetch_one(&mut *connection)
                .await
                .map_err(|error| map_sqlx_error(self.path(), &error))?;

            let mut order = Vec::new();
            let mut projects: HashMap<String, ProjectMetrics> = HashMap::new();
            for (id, name, state, count) in rows {
                if !projects.contains_key(&id) {
                    let parsed = ProjectId::parse(&id).ok_or(StorageError::Operation)?;
                    order.push(id.clone());
                    projects.insert(
                        id.clone(),
                        ProjectMetrics {
                            id: parsed,
                            name,
                            counts: BoardCounts::default(),
                        },
                    );
                }
                let project = projects.get_mut(&id).ok_or(StorageError::Operation)?;
                let count = u64::try_from(count).unwrap_or(0);
                match state.as_deref() {
                    Some("TODO") => project.counts.todo += count,
                    Some("IN_PROGRESS") => project.counts.in_progress += count,
                    Some("IN_REVIEW") => project.counts.in_review += count,
                    Some("BLOCKED") => project.counts.blocked += count,
                    Some("DONE") => project.counts.done += count,
                    // A `NULL` state means the project has no stories.
                    Some(_) => return Err(StorageError::Operation),
                    None => {}
                }
            }
            let projects = order
                .into_iter()
                .filter_map(|id| projects.remove(&id))
                .collect();
            Ok(LabMetrics {
                lab_id: lab.clone(),
                lab_name,
                projects,
                pending_intake: u64::try_from(pending_intake).unwrap_or(0),
            })
        })
    }
}
