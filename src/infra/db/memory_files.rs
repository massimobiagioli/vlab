//! Relative paths and stable entity folders for the markdown file store
//! (ADR-0059).

use sqlx::SqliteConnection;

use crate::domain::ids::{DocumentId, MemoryId};
use crate::domain::memory::MemoryScope;

/// The relative path of one memory markdown file.
pub async fn memory_path(
    connection: &mut SqliteConnection,
    scope: &MemoryScope,
    id: &MemoryId,
) -> Result<String, sqlx::Error> {
    let (kind, folder_kind) = scope_folder(scope);
    let owner = scope.owner_id();
    let name = owner_name(connection, scope).await?;
    let folder = allocate_folder(connection, kind, owner, &name).await?;
    Ok(format!("memories/{folder_kind}/{folder}/{id}.md"))
}

/// The relative path of one document markdown file.
pub async fn document_path(
    connection: &mut SqliteConnection,
    scope: &MemoryScope,
    id: &DocumentId,
) -> Result<String, sqlx::Error> {
    let (kind, folder_kind) = scope_folder(scope);
    let owner = scope.owner_id();
    let name = owner_name(connection, scope).await?;
    let folder = allocate_folder(connection, kind, owner, &name).await?;
    Ok(format!("documents/{folder_kind}/{folder}/{id}.md"))
}

/// Stable folder for an entity, if one was already allocated.
pub async fn folder_for(
    connection: &mut SqliteConnection,
    kind: &str,
    entity_id: &str,
) -> Result<Option<String>, sqlx::Error> {
    sqlx::query_scalar("SELECT folder FROM entity_folders WHERE kind = ?1 AND entity_id = ?2")
        .bind(kind)
        .bind(entity_id)
        .fetch_optional(connection)
        .await
}

fn scope_folder(scope: &MemoryScope) -> (&'static str, &'static str) {
    match scope {
        MemoryScope::Lab(_) => ("lab", "lab"),
        MemoryScope::Project(_) => ("project", "projects"),
        MemoryScope::Agent(_) => ("agent", "agents"),
    }
}

async fn owner_name(
    connection: &mut SqliteConnection,
    scope: &MemoryScope,
) -> Result<String, sqlx::Error> {
    let (sql, id) = match scope {
        MemoryScope::Lab(id) => ("SELECT name FROM labs WHERE id = ?1", id.as_str()),
        MemoryScope::Project(id) => ("SELECT name FROM projects WHERE id = ?1", id.as_str()),
        MemoryScope::Agent(id) => ("SELECT name FROM agents WHERE id = ?1", id.as_str()),
    };
    sqlx::query_scalar(sql)
        .bind(id)
        .fetch_optional(&mut *connection)
        .await?
        .ok_or(sqlx::Error::RowNotFound)
}

async fn allocate_folder(
    connection: &mut SqliteConnection,
    kind: &str,
    entity_id: &str,
    name: &str,
) -> Result<String, sqlx::Error> {
    if let Some(folder) = folder_for(&mut *connection, kind, entity_id).await? {
        return Ok(folder);
    }
    let base = slug(name);
    let candidates = [base.clone(), format!("{base}-{entity_id}")];
    for candidate in candidates {
        let result = sqlx::query(
            "INSERT OR IGNORE INTO entity_folders (kind, entity_id, folder) VALUES (?1, ?2, ?3)",
        )
        .bind(kind)
        .bind(entity_id)
        .bind(&candidate)
        .execute(&mut *connection)
        .await?;
        if result.rows_affected() == 1 {
            return Ok(candidate);
        }
    }
    Err(sqlx::Error::RowNotFound)
}

fn slug(name: &str) -> String {
    let mut out = String::new();
    let mut previous_dash = false;
    for ch in name.chars() {
        if ch.is_alphanumeric() {
            for lower in ch.to_lowercase() {
                out.push(lower);
            }
            previous_dash = false;
        } else if !previous_dash {
            out.push('-');
            previous_dash = true;
        }
    }
    let out: String = out.trim_matches('-').chars().take(80).collect();
    if out.is_empty() {
        "entity".to_owned()
    } else {
        out
    }
}
