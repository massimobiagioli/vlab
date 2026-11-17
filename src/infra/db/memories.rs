//! Scoped memory references and markdown paths (WP-4, ADR-0022, ADR-0059).
//!
//! The database stores the scope and a relative file path. Memory content is
//! read from and written to the markdown file store by the adapter wrapper.

use sqlx::SqliteConnection;

use crate::domain::ids::{AgentId, LabId, MemoryId, ProjectId};
use crate::domain::memory::{Memory, MemoryScope};

const INSERT: &str = "\
INSERT INTO memories (id, scope_type, lab_id, project_id, agent_id, content, path, created_at) \
VALUES (?1, ?2, ?3, ?4, ?5, '', ?6, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))";

const SELECT_MEMORY: &str = "\
SELECT id, scope_type, lab_id, project_id, agent_id, content, path, created_at \
FROM memories WHERE id = ?1";

const SELECT_LAB: &str = "\
SELECT id, scope_type, lab_id, project_id, agent_id, content, path, created_at \
FROM memories WHERE scope_type = 'lab' AND lab_id = ?1 ORDER BY created_at DESC, id DESC";

const SELECT_PROJECT: &str = "\
SELECT id, scope_type, lab_id, project_id, agent_id, content, path, created_at \
FROM memories WHERE scope_type = 'project' AND project_id = ?1 ORDER BY created_at DESC, id DESC";

const SELECT_AGENT: &str = "\
SELECT id, scope_type, lab_id, project_id, agent_id, content, path, created_at \
FROM memories WHERE scope_type = 'agent' AND agent_id = ?1 ORDER BY created_at DESC, id DESC";

const SELECT_LEGACY: &str = "\
SELECT id, scope_type, lab_id, project_id, agent_id, content, path, created_at \
FROM memories WHERE path IS NULL AND content <> '' ORDER BY created_at DESC, id DESC";

const COUNT_LAB: &str = "SELECT count(*) FROM memories WHERE scope_type = 'lab' AND lab_id = ?1";
const COUNT_PROJECT: &str =
    "SELECT count(*) FROM memories WHERE scope_type = 'project' AND project_id = ?1";
const COUNT_AGENT: &str =
    "SELECT count(*) FROM memories WHERE scope_type = 'agent' AND agent_id = ?1";

type Row = (
    String,
    String,
    Option<String>,
    Option<String>,
    Option<String>,
    String,
    Option<String>,
    String,
);

/// One stored memory reference before the markdown content is loaded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredMemory {
    pub id: MemoryId,
    pub scope: MemoryScope,
    pub path: Option<String>,
    pub content: String,
    pub created_at: String,
}

pub async fn insert(
    connection: &mut SqliteConnection,
    memory: &Memory,
    path: &str,
) -> Result<(), sqlx::Error> {
    let (kind, lab, project, agent) = scope_binds(&memory.scope);
    sqlx::query(INSERT)
        .bind(memory.id.as_str())
        .bind(kind)
        .bind(lab)
        .bind(project)
        .bind(agent)
        .bind(path)
        .execute(connection)
        .await
        .map(|_| ())
}

pub async fn get_ref(
    connection: &mut SqliteConnection,
    id: &MemoryId,
) -> Result<Option<StoredMemory>, sqlx::Error> {
    let row: Option<Row> = sqlx::query_as(SELECT_MEMORY)
        .bind(id.as_str())
        .fetch_optional(connection)
        .await?;
    row.map(map_row).transpose()
}

pub async fn list_refs(
    connection: &mut SqliteConnection,
    scope: &MemoryScope,
) -> Result<Vec<StoredMemory>, sqlx::Error> {
    let rows: Vec<Row> = match scope {
        MemoryScope::Lab(id) => {
            sqlx::query_as(SELECT_LAB)
                .bind(id.as_str())
                .fetch_all(connection)
                .await?
        }
        MemoryScope::Project(id) => {
            sqlx::query_as(SELECT_PROJECT)
                .bind(id.as_str())
                .fetch_all(connection)
                .await?
        }
        MemoryScope::Agent(id) => {
            sqlx::query_as(SELECT_AGENT)
                .bind(id.as_str())
                .fetch_all(connection)
                .await?
        }
    };
    rows.into_iter().map(map_row).collect()
}

pub async fn legacy_refs(
    connection: &mut SqliteConnection,
) -> Result<Vec<StoredMemory>, sqlx::Error> {
    let rows: Vec<Row> = sqlx::query_as(SELECT_LEGACY).fetch_all(connection).await?;
    rows.into_iter().map(map_row).collect()
}

pub async fn path(
    connection: &mut SqliteConnection,
    id: &MemoryId,
) -> Result<Option<String>, sqlx::Error> {
    let row: Option<(Option<String>,)> = sqlx::query_as("SELECT path FROM memories WHERE id = ?1")
        .bind(id.as_str())
        .fetch_optional(connection)
        .await?;
    Ok(row.and_then(|(path,)| path))
}

pub async fn set_path(
    connection: &mut SqliteConnection,
    id: &MemoryId,
    value: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE memories SET path = ?2, content = '' WHERE id = ?1")
        .bind(id.as_str())
        .bind(value)
        .execute(connection)
        .await
        .map(|_| ())
}

pub async fn delete(connection: &mut SqliteConnection, id: &MemoryId) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM memories WHERE id = ?1")
        .bind(id.as_str())
        .execute(connection)
        .await
        .map(|_| ())
}

pub async fn count(
    connection: &mut SqliteConnection,
    scope: &MemoryScope,
) -> Result<u64, sqlx::Error> {
    let (sql, owner) = match scope {
        MemoryScope::Lab(id) => (COUNT_LAB, id.as_str()),
        MemoryScope::Project(id) => (COUNT_PROJECT, id.as_str()),
        MemoryScope::Agent(id) => (COUNT_AGENT, id.as_str()),
    };
    let count: i64 = sqlx::query_scalar(sql)
        .bind(owner)
        .fetch_one(connection)
        .await?;
    Ok(u64::try_from(count).unwrap_or(0))
}

fn map_row(
    (id, scope_type, lab, project, agent, content, path, created_at): Row,
) -> Result<StoredMemory, sqlx::Error> {
    let id = MemoryId::parse(&id).ok_or(sqlx::Error::RowNotFound)?;
    let scope = match scope_type.as_str() {
        "lab" => MemoryScope::Lab(
            LabId::parse(lab.as_deref().ok_or(sqlx::Error::RowNotFound)?)
                .ok_or(sqlx::Error::RowNotFound)?,
        ),
        "project" => MemoryScope::Project(
            ProjectId::parse(project.as_deref().ok_or(sqlx::Error::RowNotFound)?)
                .ok_or(sqlx::Error::RowNotFound)?,
        ),
        "agent" => MemoryScope::Agent(
            AgentId::parse(agent.as_deref().ok_or(sqlx::Error::RowNotFound)?)
                .ok_or(sqlx::Error::RowNotFound)?,
        ),
        _ => return Err(sqlx::Error::RowNotFound),
    };
    Ok(StoredMemory {
        id,
        scope,
        path,
        content,
        created_at,
    })
}

fn scope_binds(scope: &MemoryScope) -> (&'static str, Option<&str>, Option<&str>, Option<&str>) {
    match scope {
        MemoryScope::Lab(id) => ("lab", Some(id.as_str()), None, None),
        MemoryScope::Project(id) => ("project", None, Some(id.as_str()), None),
        MemoryScope::Agent(id) => ("agent", None, None, Some(id.as_str())),
    }
}
