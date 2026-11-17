//! Document references and relative paths (ADR-0059).

use crate::app::documents::{Document, DocumentStore};
use crate::app::storage::StorageError;
use crate::domain::ids::DocumentId;
use crate::domain::memory::MemoryScope;
use crate::infra::db::{Database, map_sqlx_error, memory_files};

const INSERT: &str = "\
INSERT INTO documents (id, scope_type, agent_id, project_id, lab_id, path, title, \
created_at, updated_at) \
VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, \
strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))";

impl DocumentStore for Database {
    fn insert_document(&self, document: &Document) -> Result<(), StorageError> {
        let (kind, agent, project, lab) = scope_binds(&document.scope);
        self.block_on(async {
            let mut connection = self
                .pool()
                .acquire()
                .await
                .map_err(|error| map_sqlx_error(self.path(), &error))?;
            sqlx::query(INSERT)
                .bind(document.id.as_str())
                .bind(kind)
                .bind(agent)
                .bind(project)
                .bind(lab)
                .bind(&document.path)
                .bind(&document.title)
                .execute(&mut *connection)
                .await
                .map(|_| ())
                .map_err(|error| map_sqlx_error(self.path(), &error))
        })
    }

    fn document_exists(&self, id: &DocumentId) -> Result<bool, StorageError> {
        self.block_on(async {
            let mut connection = self
                .pool()
                .acquire()
                .await
                .map_err(|error| map_sqlx_error(self.path(), &error))?;
            let found: Option<i64> = sqlx::query_scalar("SELECT 1 FROM documents WHERE id = ?1")
                .bind(id.as_str())
                .fetch_optional(&mut *connection)
                .await
                .map_err(|error| map_sqlx_error(self.path(), &error))?;
            Ok(found.is_some())
        })
    }

    fn document_path(&self, id: &DocumentId, scope: &MemoryScope) -> Result<String, StorageError> {
        self.block_on(async {
            let mut connection = self
                .pool()
                .acquire()
                .await
                .map_err(|error| map_sqlx_error(self.path(), &error))?;
            memory_files::document_path(&mut connection, scope, id)
                .await
                .map_err(|error| map_sqlx_error(self.path(), &error))
        })
    }
}

fn scope_binds(scope: &MemoryScope) -> (&'static str, Option<&str>, Option<&str>, Option<&str>) {
    match scope {
        MemoryScope::Lab(id) => ("lab", None, None, Some(id.as_str())),
        MemoryScope::Project(id) => ("project", None, Some(id.as_str()), None),
        MemoryScope::Agent(id) => ("agent", Some(id.as_str()), None, None),
    }
}
