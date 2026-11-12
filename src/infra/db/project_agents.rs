//! Project-agent assignments (ADR-0034).

use sqlx::SqliteConnection;

use crate::domain::ids::{AgentId, ProjectId};

pub async fn assign(
    connection: &mut SqliteConnection,
    project: &ProjectId,
    agent: &AgentId,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT OR IGNORE INTO project_agents (project_id, agent_id, created_at) \
         VALUES (?1, ?2, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))",
    )
    .bind(project.as_str())
    .bind(agent.as_str())
    .execute(connection)
    .await
    .map(|_| ())
}

pub async fn ids(
    connection: &mut SqliteConnection,
    project: &ProjectId,
) -> Result<Vec<AgentId>, sqlx::Error> {
    let rows: Vec<String> = sqlx::query_scalar(
        "SELECT agent_id FROM project_agents WHERE project_id = ?1 ORDER BY created_at, agent_id",
    )
    .bind(project.as_str())
    .fetch_all(connection)
    .await?;
    rows.into_iter()
        .map(|id| AgentId::parse(&id).ok_or(sqlx::Error::RowNotFound))
        .collect()
}
