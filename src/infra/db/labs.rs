//! Laboratory queries (WP-3).

use sqlx::SqliteConnection;

use super::agents::{AgentRow, map_agent};
use crate::app::directory::{AgentRecord, LabOverview};
use crate::domain::ids::LabId;
use crate::domain::lab::Lab;

const INSERT_LAB: &str = "\
INSERT INTO labs (id, name, description, created_at, updated_at) \
VALUES (?1, ?2, ?3, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))";

const LAB_EXISTS: &str = "SELECT 1 FROM labs WHERE id = ?1";

const SELECT_LAB: &str = "\
SELECT l.id, l.name, l.description, (\
    SELECT count(*) FROM lab_memberships m \
    JOIN agents a ON a.id = m.agent_id \
    WHERE m.lab_id = l.id AND a.archived_at IS NULL\
), (SELECT count(*) FROM projects p WHERE p.lab_id = l.id) \
FROM labs l WHERE l.id = ?1";

const SELECT_LABS: &str = "\
SELECT l.id, l.name, l.description, (\
    SELECT count(*) FROM lab_memberships m \
    JOIN agents a ON a.id = m.agent_id \
    WHERE m.lab_id = l.id AND a.archived_at IS NULL\
), (SELECT count(*) FROM projects p WHERE p.lab_id = l.id) \
FROM labs l ORDER BY l.created_at, l.id";

const SELECT_LAB_AGENTS: &str = "\
SELECT a.id, a.name, a.role, a.description, a.skills_json, a.provider, \
       a.model, a.cv, a.archived_at \
FROM agents a \
JOIN lab_memberships m ON m.agent_id = a.id \
WHERE m.lab_id = ?1 \
ORDER BY a.name COLLATE NOCASE, a.id";

pub async fn insert(connection: &mut SqliteConnection, lab: &Lab) -> Result<(), sqlx::Error> {
    sqlx::query(INSERT_LAB)
        .bind(lab.id.as_str())
        .bind(lab.name.as_str())
        .bind(
            lab.description
                .as_ref()
                .map(|description| description.as_str())
                .unwrap_or(""),
        )
        .execute(connection)
        .await
        .map(|_| ())
}

pub async fn exists(connection: &mut SqliteConnection, id: &LabId) -> Result<bool, sqlx::Error> {
    let found: Option<i64> = sqlx::query_scalar(LAB_EXISTS)
        .bind(id.as_str())
        .fetch_optional(connection)
        .await?;
    Ok(found.is_some())
}

pub async fn get(
    connection: &mut SqliteConnection,
    id: &LabId,
) -> Result<Option<LabOverview>, sqlx::Error> {
    let row: Option<(String, String, String, i64, i64)> = sqlx::query_as(SELECT_LAB)
        .bind(id.as_str())
        .fetch_optional(connection)
        .await?;
    row.map(map_overview).transpose()
}

pub async fn list(connection: &mut SqliteConnection) -> Result<Vec<LabOverview>, sqlx::Error> {
    let rows: Vec<(String, String, String, i64, i64)> =
        sqlx::query_as(SELECT_LABS).fetch_all(connection).await?;
    rows.into_iter().map(map_overview).collect()
}

pub async fn agents(
    connection: &mut SqliteConnection,
    lab: &LabId,
) -> Result<Vec<AgentRecord>, sqlx::Error> {
    let rows: Vec<AgentRow> = sqlx::query_as(SELECT_LAB_AGENTS)
        .bind(lab.as_str())
        .fetch_all(connection)
        .await?;
    rows.into_iter().map(map_agent).collect()
}

fn map_overview(
    (id, name, description, agent_count, project_count): (String, String, String, i64, i64),
) -> Result<LabOverview, sqlx::Error> {
    let id = LabId::parse(&id).ok_or(sqlx::Error::RowNotFound)?;
    Ok(LabOverview {
        id,
        name,
        description,
        agent_count: u64::try_from(agent_count).unwrap_or(0),
        project_count: u64::try_from(project_count).unwrap_or(0),
    })
}

pub async fn update(connection: &mut SqliteConnection, lab: &Lab) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE labs SET name = ?2, description = ?3, \
         updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?1",
    )
    .bind(lab.id.as_str())
    .bind(lab.name.as_str())
    .bind(
        lab.description
            .as_ref()
            .map(|description| description.as_str())
            .unwrap_or(""),
    )
    .execute(connection)
    .await
    .map(|_| ())
}
