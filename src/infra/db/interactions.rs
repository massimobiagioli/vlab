//! Content-free interaction metadata (WP-6-T1).

use crate::app::invocation::{Interaction, InvocationStore};
use crate::app::storage::StorageError;
use crate::infra::db::{Database, map_sqlx_error};

const INSERT: &str = "\
INSERT INTO interactions (id, run_id, agent_id, kind, provider, model, outcome, \
error_code, duration_ms, input_tokens, output_tokens, created_at) \
VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, \
strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))";

impl InvocationStore for Database {
    fn record_interaction(&self, interaction: &Interaction) -> Result<(), StorageError> {
        self.block_on(async {
            let mut connection = self
                .pool()
                .acquire()
                .await
                .map_err(|error| map_sqlx_error(self.path(), &error))?;
            sqlx::query(INSERT)
                .bind(uuid::Uuid::new_v4().simple().to_string())
                .bind(interaction.run_id.to_string())
                .bind(interaction.agent_id.as_str())
                .bind(interaction.kind.as_str())
                .bind(&interaction.provider)
                .bind(&interaction.model)
                .bind(interaction.outcome.as_str())
                .bind(interaction.error_code)
                .bind(i64::try_from(interaction.duration_ms).unwrap_or(i64::MAX))
                .bind(
                    interaction
                        .input_tokens
                        .and_then(|value| i64::try_from(value).ok()),
                )
                .bind(
                    interaction
                        .output_tokens
                        .and_then(|value| i64::try_from(value).ok()),
                )
                .execute(&mut *connection)
                .await
                .map(|_| ())
                .map_err(|error| map_sqlx_error(self.path(), &error))
        })
    }

    fn record_event(&self, event: &crate::obs::events::AuditEvent) -> Result<(), StorageError> {
        Database::record(self, event)
    }
}
