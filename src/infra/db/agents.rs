//! Agent and membership queries (WP-3).

use sqlx::SqliteConnection;

use crate::app::directory::{AgentRecord, LabOverview};
use crate::domain::agent::AgentProfile;
use crate::domain::ids::{AgentId, LabId};
use crate::domain::skills;

/// The nine columns selected for an agent profile.
pub(crate) type AgentRow = (
    String,
    String,
    String,
    String,
    String,
    String,
    String,
    Option<String>,
    Option<String>,
);

const INSERT_AGENT: &str = "\
INSERT INTO agents (\
    id, name, role, description, skills_json, provider, model, cv, archived_at, \
    created_at, updated_at\
) VALUES (\
    ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, NULL, \
    strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), strftime('%Y-%m-%dT%H:%M:%fZ', 'now')\
)";

const INSERT_MEMBERSHIP: &str = "\
INSERT INTO lab_memberships (lab_id, agent_id, joined_at) \
VALUES (?1, ?2, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))";

const AGENT_EXISTS: &str = "SELECT 1 FROM agents WHERE id = ?1";

const SELECT_AGENT: &str = "\
SELECT id, name, role, description, skills_json, provider, model, cv, archived_at \
FROM agents WHERE id = ?1";

const SELECT_AGENT_MEMBERSHIPS: &str = "\
SELECT l.id, l.name, (\
    SELECT count(*) FROM lab_memberships m2 \
    JOIN agents a2 ON a2.id = m2.agent_id \
    WHERE m2.lab_id = l.id AND a2.archived_at IS NULL\
) \
FROM labs l \
JOIN lab_memberships m ON m.lab_id = l.id \
WHERE m.agent_id = ?1 \
ORDER BY l.name COLLATE NOCASE, l.id";

pub async fn exists(connection: &mut SqliteConnection, id: &AgentId) -> Result<bool, sqlx::Error> {
    let found: Option<i64> = sqlx::query_scalar(AGENT_EXISTS)
        .bind(id.as_str())
        .fetch_optional(connection)
        .await?;
    Ok(found.is_some())
}

pub async fn get(
    connection: &mut SqliteConnection,
    id: &AgentId,
) -> Result<Option<AgentRecord>, sqlx::Error> {
    let row: Option<AgentRow> = sqlx::query_as(SELECT_AGENT)
        .bind(id.as_str())
        .fetch_optional(connection)
        .await?;
    row.map(map_agent).transpose()
}

pub async fn insert_profile(
    connection: &mut SqliteConnection,
    profile: &AgentProfile,
) -> Result<(), sqlx::Error> {
    sqlx::query(INSERT_AGENT)
        .bind(profile.id.as_str())
        .bind(profile.name.as_str())
        .bind(profile.role.as_str())
        .bind(profile.description.as_str())
        .bind(skills::encode(&profile.skills))
        .bind(profile.provider.as_str())
        .bind(profile.model.as_str())
        .bind(profile.cv.as_deref())
        .execute(connection)
        .await
        .map(|_| ())
}

pub async fn insert_memberships(
    connection: &mut SqliteConnection,
    agent: &AgentId,
    labs: &[LabId],
) -> Result<(), sqlx::Error> {
    for lab in labs {
        sqlx::query(INSERT_MEMBERSHIP)
            .bind(lab.as_str())
            .bind(agent.as_str())
            .execute(&mut *connection)
            .await?;
    }
    Ok(())
}

pub async fn memberships(
    connection: &mut SqliteConnection,
    id: &AgentId,
) -> Result<Vec<LabOverview>, sqlx::Error> {
    let rows: Vec<(String, String, i64)> = sqlx::query_as(SELECT_AGENT_MEMBERSHIPS)
        .bind(id.as_str())
        .fetch_all(connection)
        .await?;
    rows.into_iter()
        .map(|(id, name, count)| {
            let id = LabId::parse(&id).ok_or(sqlx::Error::RowNotFound)?;
            Ok(LabOverview {
                id,
                name,
                agent_count: u64::try_from(count).unwrap_or(0),
                project_count: 0,
            })
        })
        .collect()
}

pub(crate) fn map_agent(
    (id, name, role, description, skills_json, provider, model, cv, archived_at): AgentRow,
) -> Result<AgentRecord, sqlx::Error> {
    let id = AgentId::parse(&id).ok_or(sqlx::Error::RowNotFound)?;
    Ok(AgentRecord {
        id,
        name,
        role,
        description,
        skills_json,
        provider,
        model,
        cv,
        archived: archived_at.is_some(),
    })
}
