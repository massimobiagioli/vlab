//! Schema policy: application tables must not store credentials (WP2-AC7).
//!
//! The policy is enforced by `tests/db_schema_policy.rs`, which runs against
//! the migrated schema, and by the unit test on [`violations`] so that adding
//! a forbidden column fails before it can ship. `app_state.key` is allowed
//! explicitly because it holds a state name such as `active_lab_id`, not a
//! credential.

use super::{Database, map_sqlx_error};
use crate::app::storage::StorageError;

/// Column-name fragments that must not appear in any application table.
const FORBIDDEN_FRAGMENTS: [&str; 6] = [
    "apikey",
    "secret",
    "password",
    "credential",
    "accesstoken",
    "authtoken",
];

/// True when `column` names something that may hold a credential.
pub fn is_forbidden(column: &str) -> bool {
    let normalized: String = column
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_lowercase())
        .collect();
    FORBIDDEN_FRAGMENTS
        .iter()
        .any(|fragment| normalized.contains(fragment))
}

/// The violating column names in `columns`, in input order.
pub fn violations(columns: impl IntoIterator<Item = String>) -> Vec<String> {
    columns.into_iter().filter(|c| is_forbidden(c)).collect()
}

/// Reads the live schema and returns forbidden `table.column` pairs.
pub fn schema_violations(db: &Database) -> Result<Vec<String>, StorageError> {
    db.block_on(async {
        let mut connection = db
            .pool()
            .acquire()
            .await
            .map_err(|error| map_sqlx_error(db.path(), &error))?;
        let tables: Vec<String> = sqlx::query_scalar(
            "SELECT name FROM sqlite_master \
             WHERE type = 'table' \
               AND name NOT LIKE 'sqlite\\_%' ESCAPE '\\' \
               AND name NOT LIKE '\\_sqlx\\_%' ESCAPE '\\'",
        )
        .fetch_all(&mut *connection)
        .await
        .map_err(|_| StorageError::Operation)?;

        let mut found = Vec::new();
        for table in tables {
            // The table-valued pragma is parameterized, so the table name is
            // bound data rather than interpolated SQL.
            let columns: Vec<String> = sqlx::query_scalar("SELECT name FROM pragma_table_info(?1)")
                .bind(&table)
                .fetch_all(&mut *connection)
                .await
                .map_err(|_| StorageError::Operation)?;
            for name in columns {
                if is_forbidden(&name) {
                    found.push(format!("{table}.{name}"));
                }
            }
        }
        Ok(found)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn credential_bearing_names_are_rejected() {
        for column in [
            "api_key",
            "apiKey",
            "secret",
            "client_secret",
            "password",
            "credential",
            "access_token",
            "auth-token",
        ] {
            assert!(is_forbidden(column), "{column}");
        }
    }

    #[test]
    fn ordinary_columns_and_state_keys_are_allowed() {
        for column in ["key", "value", "updated_at", "tokens_in", "id", "kind"] {
            assert!(!is_forbidden(column), "{column}");
        }
        assert!(violations(["id".to_owned(), "api_key".to_owned()]) == ["api_key"]);
    }
}
