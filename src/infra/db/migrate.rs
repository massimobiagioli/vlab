//! Embedded, forward-only schema migrations (ADR-0016).
//!
//! `sqlx::migrate!` embeds `migrations/*.sql` in the binary and records
//! applied versions in `_sqlx_migrations`. Each migration runs in a
//! transaction, so a failure leaves the previous schema untouched. The
//! runner also maintains `PRAGMA user_version`, which is what a newer binary
//! checks before touching the file.

use std::path::PathBuf;

use sqlx::migrate::{MigrateError, Migrator};

use super::{Database, SUPPORTED_SCHEMA_VERSION};
use crate::app::storage::StorageError;
use crate::infra::db::events;
use crate::obs::events::{AuditEvent, EventKind, Outcome};
use crate::obs::{ActionId, RunId};

/// The migrations compiled into this binary.
pub static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

/// Versions before and after a migration run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MigrationReport {
    pub from: i64,
    pub to: i64,
}

impl MigrationReport {
    /// Whether at least one migration was applied.
    pub fn applied(self) -> bool {
        self.to > self.from
    }
}

impl Database {
    /// Opens and migrates in one step: the entry point every database-backed
    /// command will use so that the schema is current before any query.
    pub fn open_and_migrate(
        path: impl Into<PathBuf>,
        run_id: RunId,
    ) -> Result<(Self, MigrationReport), StorageError> {
        let database = Self::open(path)?;
        let report = database.migrate(run_id)?;
        Ok((database, report))
    }

    /// Brings the database to [`SUPPORTED_SCHEMA_VERSION`].
    pub fn migrate(&self, run_id: RunId) -> Result<MigrationReport, StorageError> {
        self.migrate_with(&MIGRATOR, SUPPORTED_SCHEMA_VERSION, run_id)
    }

    /// Runs `migrator` up to `target`, then records the version. Exposed so
    /// tests can prove that a broken migration rolls back.
    pub fn migrate_with(
        &self,
        migrator: &Migrator,
        target: i64,
        run_id: RunId,
    ) -> Result<MigrationReport, StorageError> {
        let from = self.schema_version()?;
        if from > target {
            return Err(StorageError::NewerSchema {
                path: self.path().to_path_buf(),
                found: from,
                supported: target,
            });
        }

        self.block_on(migrator.run(self.pool()))
            .map_err(|error| map_migrate_error(&error))?;
        self.block_on(async {
            let sql = format!("PRAGMA user_version = {target}");
            // `target` is an i64, so the formatted statement cannot inject.
            sqlx::query(sqlx::AssertSqlSafe(sql))
                .execute(self.pool())
                .await
        })
        .map_err(|error| super::map_sqlx_error(self.path(), &error))?;

        let report = MigrationReport { from, to: target };
        if report.applied() {
            let event = AuditEvent::new(
                run_id,
                ActionId::generate(),
                EventKind::MigrationApplied,
                Outcome::Success,
            )
            .schema_versions(report.from, report.to);
            events::insert(self, &event)?;
        }
        Ok(report)
    }
}

fn map_migrate_error(error: &MigrateError) -> StorageError {
    // Migration errors can embed SQL and file paths; only the class is kept.
    match error {
        MigrateError::VersionMissing(_) | MigrateError::VersionMismatch(_) => {
            StorageError::SchemaMismatch
        }
        _ => StorageError::Migration,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_embedded_migrations_match_the_supported_version() {
        assert_eq!(
            i64::try_from(MIGRATOR.migrations.len()).unwrap_or(i64::MAX),
            SUPPORTED_SCHEMA_VERSION
        );
    }
}
