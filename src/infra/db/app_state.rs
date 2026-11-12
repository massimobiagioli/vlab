//! The `app_state` repository (ADR-0002, ADR-0016).
//!
//! Application state is key/value text. Keys come from the typed
//! [`AppStateKey`] enum, and every statement is parameterized. A stored value
//! is never interpreted here: [`crate::app::context`] decides what a value
//! means and treats corrupt values as no selection.

use sqlx::SqliteConnection;

use super::{Database, map_sqlx_error};
use crate::app::storage::{AppStateKey, AppStateStore, StorageError};

const GET: &str = "SELECT value FROM app_state WHERE key = ?1";

const SET: &str = "\
INSERT INTO app_state (key, value, updated_at) \
VALUES (?1, ?2, strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) \
ON CONFLICT (key) DO UPDATE SET \
    value = excluded.value, \
    updated_at = excluded.updated_at";

const CLEAR: &str = "DELETE FROM app_state WHERE key = ?1";

impl AppStateStore for Database {
    fn get(&self, key: AppStateKey) -> Result<Option<String>, StorageError> {
        self.block_on(async {
            let mut connection = self
                .pool()
                .acquire()
                .await
                .map_err(|error| map_sqlx_error(self.path(), &error))?;
            get(&mut connection, key).await
        })
    }

    fn set(&self, key: AppStateKey, value: &str) -> Result<(), StorageError> {
        self.block_on(async {
            let mut connection = self
                .pool()
                .acquire()
                .await
                .map_err(|error| map_sqlx_error(self.path(), &error))?;
            set(&mut connection, key, value).await
        })
    }

    fn clear(&self, key: AppStateKey) -> Result<(), StorageError> {
        self.block_on(async {
            let mut connection = self
                .pool()
                .acquire()
                .await
                .map_err(|error| map_sqlx_error(self.path(), &error))?;
            clear(&mut connection, key).await
        })
    }
}

/// Reads one key on an existing connection.
pub async fn get(
    connection: &mut SqliteConnection,
    key: AppStateKey,
) -> Result<Option<String>, StorageError> {
    sqlx::query_scalar(GET)
        .bind(key.as_str())
        .fetch_optional(connection)
        .await
        .map_err(|_| StorageError::Operation)
}

/// Writes one key on an existing connection. Used inside transactions.
pub async fn set(
    connection: &mut SqliteConnection,
    key: AppStateKey,
    value: &str,
) -> Result<(), StorageError> {
    sqlx::query(SET)
        .bind(key.as_str())
        .bind(value)
        .execute(connection)
        .await
        .map(|_| ())
        .map_err(|_| StorageError::Operation)
}

/// Removes one key on an existing connection. Used inside transactions.
pub async fn clear(
    connection: &mut SqliteConnection,
    key: AppStateKey,
) -> Result<(), StorageError> {
    sqlx::query(CLEAR)
        .bind(key.as_str())
        .execute(connection)
        .await
        .map(|_| ())
        .map_err(|_| StorageError::Operation)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn statements_are_parameterized_and_use_utc_timestamps() {
        for statement in [GET, SET, CLEAR] {
            assert!(
                statement.contains("?1"),
                "statement must be parameterized: {statement}"
            );
        }
        assert!(SET.contains("strftime('%Y-%m-%dT%H:%M:%fZ', 'now')"));
        assert!(
            !SET.contains("datetime("),
            "local-time functions are forbidden"
        );
    }
}
