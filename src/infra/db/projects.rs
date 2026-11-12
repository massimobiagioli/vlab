//! Project queries (WP-4).

use sqlx::SqliteConnection;

use crate::app::projects::ProjectOverview;
use crate::domain::ids::{LabId, ProjectId};
use crate::domain::project::Project;

const INSERT: &str = "\
INSERT INTO projects (id, lab_id, name, github_source, created_at, updated_at) \
VALUES (?1, ?2, ?3, ?4, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))";

pub type OverviewRow = (
    String,
    String,
    Option<String>,
    i64,
    i64,
    i64,
    i64,
    i64,
    i64,
    i64,
);

pub async fn insert(
    connection: &mut SqliteConnection,
    project: &Project,
) -> Result<(), sqlx::Error> {
    sqlx::query(INSERT)
        .bind(project.id.as_str())
        .bind(project.lab_id.as_str())
        .bind(project.name.as_str())
        .bind(project.github_source.as_ref().map(|source| source.as_str()))
        .execute(connection)
        .await
        .map(|_| ())
}

pub async fn find(
    connection: &mut SqliteConnection,
    id: &ProjectId,
) -> Result<Option<(ProjectId, LabId, String, Option<String>)>, sqlx::Error> {
    let row: Option<(String, String, String, Option<String>)> =
        sqlx::query_as("SELECT id, lab_id, name, github_source FROM projects WHERE id = ?1")
            .bind(id.as_str())
            .fetch_optional(connection)
            .await?;
    row.map(|(id, lab, name, github_source)| {
        Ok((
            ProjectId::parse(&id).ok_or(sqlx::Error::RowNotFound)?,
            LabId::parse(&lab).ok_or(sqlx::Error::RowNotFound)?,
            name,
            github_source,
        ))
    })
    .transpose()
}

const SELECT_BY_LAB: &str = "SELECT p.id, p.name, p.github_source,  (SELECT count(*) FROM requests r WHERE r.project_id = p.id),  (SELECT count(*) FROM stories s WHERE s.project_id = p.id),  (SELECT count(*) FROM stories s WHERE s.project_id = p.id AND s.state = 'TODO'),  (SELECT count(*) FROM stories s WHERE s.project_id = p.id AND s.state = 'IN_PROGRESS'),  (SELECT count(*) FROM stories s WHERE s.project_id = p.id AND s.state = 'IN_REVIEW'),  (SELECT count(*) FROM stories s WHERE s.project_id = p.id AND s.state = 'BLOCKED'),  (SELECT count(*) FROM stories s WHERE s.project_id = p.id AND s.state = 'DONE') FROM projects p WHERE p.lab_id = ?1 ORDER BY p.created_at, p.id";

const SELECT_IN_LAB: &str = "SELECT p.id, p.name, p.github_source,  (SELECT count(*) FROM requests r WHERE r.project_id = p.id),  (SELECT count(*) FROM stories s WHERE s.project_id = p.id),  (SELECT count(*) FROM stories s WHERE s.project_id = p.id AND s.state = 'TODO'),  (SELECT count(*) FROM stories s WHERE s.project_id = p.id AND s.state = 'IN_PROGRESS'),  (SELECT count(*) FROM stories s WHERE s.project_id = p.id AND s.state = 'IN_REVIEW'),  (SELECT count(*) FROM stories s WHERE s.project_id = p.id AND s.state = 'BLOCKED'),  (SELECT count(*) FROM stories s WHERE s.project_id = p.id AND s.state = 'DONE') FROM projects p WHERE p.id = ?1 AND p.lab_id = ?2";

pub async fn list(
    connection: &mut SqliteConnection,
    lab: &LabId,
) -> Result<Vec<ProjectOverview>, sqlx::Error> {
    let rows: Vec<OverviewRow> = sqlx::query_as(SELECT_BY_LAB)
        .bind(lab.as_str())
        .fetch_all(connection)
        .await?;
    rows.into_iter().map(map_overview).collect()
}

pub async fn in_lab(
    connection: &mut SqliteConnection,
    id: &ProjectId,
    lab: &LabId,
) -> Result<Option<ProjectOverview>, sqlx::Error> {
    let row: Option<OverviewRow> = sqlx::query_as(SELECT_IN_LAB)
        .bind(id.as_str())
        .bind(lab.as_str())
        .fetch_optional(connection)
        .await?;
    row.map(map_overview).transpose()
}

fn map_overview(
    (id, name, github_source, requests, stories, todo, in_progress, in_review, blocked, done): OverviewRow,
) -> Result<ProjectOverview, sqlx::Error> {
    Ok(ProjectOverview {
        id: ProjectId::parse(&id).ok_or(sqlx::Error::RowNotFound)?,
        name,
        github_source,
        request_count: u64::try_from(requests).unwrap_or(0),
        story_count: u64::try_from(stories).unwrap_or(0),
        todo: u64::try_from(todo).unwrap_or(0),
        in_progress: u64::try_from(in_progress).unwrap_or(0),
        in_review: u64::try_from(in_review).unwrap_or(0),
        blocked: u64::try_from(blocked).unwrap_or(0),
        done: u64::try_from(done).unwrap_or(0),
    })
}
