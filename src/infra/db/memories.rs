//! Scoped memory queries (WP-4, ADR-0022).

use sqlx::SqliteConnection;

use crate::app::memories::MemoryRecord;
use crate::domain::ids::MemoryId;
use crate::domain::memory::{Memory, MemoryScope};

const INSERT: &str = "\
INSERT INTO memories (id, scope_type, lab_id, project_id, agent_id, content, created_at) \
VALUES (?1, ?2, ?3, ?4, ?5, ?6, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))";

const UPDATE: &str = "UPDATE memories SET content = ?2 WHERE id = ?1";

const SELECT_MEMORY: &str = "\
SELECT id, scope_type, lab_id, project_id, agent_id, content, created_at \
FROM memories WHERE id = ?1";

const SELECT_LAB: &str = "\
SELECT id, scope_type, lab_id, project_id, agent_id, content, created_at \
FROM memories WHERE scope_type = 'lab' AND lab_id = ?1 ORDER BY created_at DESC, id DESC";

const SELECT_PROJECT: &str = "\
SELECT id, scope_type, lab_id, project_id, agent_id, content, created_at \
FROM memories WHERE scope_type = 'project' AND project_id = ?1 ORDER BY created_at DESC, id DESC";

const SELECT_AGENT: &str = "\
SELECT id, scope_type, lab_id, project_id, agent_id, content, created_at \
FROM memories WHERE scope_type = 'agent' AND agent_id = ?1 ORDER BY created_at DESC, id DESC";

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
    String,
);

pub async fn insert(connection: &mut SqliteConnection, memory: &Memory) -> Result<(), sqlx::Error> {
    let (kind, lab, project, agent) = scope_binds(&memory.scope);
    sqlx::query(INSERT)
        .bind(memory.id.as_str())
        .bind(kind)
        .bind(lab)
        .bind(project)
        .bind(agent)
        .bind(&memory.content)
        .execute(connection)
        .await
        .map(|_| ())
}

pub async fn update(connection: &mut SqliteConnection, memory: &Memory) -> Result<(), sqlx::Error> {
    sqlx::query(UPDATE)
        .bind(memory.id.as_str())
        .bind(&memory.content)
        .execute(connection)
        .await
        .map(|_| ())
}

pub async fn get(
    connection: &mut SqliteConnection,
    id: &MemoryId,
) -> Result<Option<MemoryRecord>, sqlx::Error> {
    sqlx::query_as(SELECT_MEMORY)
        .bind(id.as_str())
        .fetch_optional(connection)
        .await?
        .map(map_row)
        .transpose()
}

pub async fn list(
    connection: &mut SqliteConnection,
    scope: &MemoryScope,
) -> Result<Vec<MemoryRecord>, sqlx::Error> {
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

pub async fn delete(connection: &mut SqliteConnection, id: &MemoryId) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM memories WHERE id = ?1")
        .bind(id.as_str())
        .execute(connection)
        .await
        .map(|_| ())
}

fn scope_binds(scope: &MemoryScope) -> (&'static str, Option<&str>, Option<&str>, Option<&str>) {
    match scope {
        MemoryScope::Lab(id) => ("lab", Some(id.as_str()), None, None),
        MemoryScope::Project(id) => ("project", None, Some(id.as_str()), None),
        MemoryScope::Agent(id) => ("agent", None, None, Some(id.as_str())),
    }
}

fn map_row(
    (id, kind, lab, project, agent, content, created_at): Row,
) -> Result<MemoryRecord, sqlx::Error> {
    let id = MemoryId::parse(&id).ok_or(sqlx::Error::RowNotFound)?;
    let scope = match kind.as_str() {
        "lab" => MemoryScope::Lab(
            crate::domain::ids::LabId::parse(lab.as_deref().unwrap_or_default())
                .ok_or(sqlx::Error::RowNotFound)?,
        ),
        "project" => MemoryScope::Project(
            crate::domain::ids::ProjectId::parse(project.as_deref().unwrap_or_default())
                .ok_or(sqlx::Error::RowNotFound)?,
        ),
        "agent" => MemoryScope::Agent(
            crate::domain::ids::AgentId::parse(agent.as_deref().unwrap_or_default())
                .ok_or(sqlx::Error::RowNotFound)?,
        ),
        _ => return Err(sqlx::Error::RowNotFound),
    };
    let memory = Memory::new(id, scope, &content).map_err(|_| sqlx::Error::RowNotFound)?;
    Ok(MemoryRecord { memory, created_at })
}
