//! Embedded migrations and schema v1 (WP2-AC2).

mod support;

use std::fs;

use vlab::app::storage::StorageError;
use vlab::infra::db::{Database, SUPPORTED_SCHEMA_VERSION};
use vlab::obs::RunId;

fn open() -> (tempfile::TempDir, Database) {
    let dir = support::private_tempdir();
    let path = dir.path().join("vlab.db");
    let db = Database::open(&path).unwrap();
    (dir, db)
}

fn table_exists(db: &Database, name: &str) -> bool {
    db.block_on(async {
        let count: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
        )
        .bind(name)
        .fetch_one(db.pool())
        .await
        .unwrap();
        count == 1
    })
}

fn event_rows(db: &Database) -> Vec<(String, i64, i64, String)> {
    db.block_on(async {
        sqlx::query_as(
            "SELECT kind, schema_from, schema_to, created_at FROM events \
             WHERE kind = 'migration_applied' ORDER BY created_at",
        )
        .fetch_all(db.pool())
        .await
        .unwrap()
    })
}

#[test]
fn a_fresh_database_reaches_the_supported_version_and_records_it() {
    let (_dir, db) = open();
    assert_eq!(db.schema_version().unwrap(), 0);

    let report = db.migrate(RunId::generate()).unwrap();
    assert_eq!((report.from, report.to), (0, SUPPORTED_SCHEMA_VERSION));
    assert!(report.applied());
    assert_eq!(db.schema_version().unwrap(), SUPPORTED_SCHEMA_VERSION);
    assert!(table_exists(&db, "app_state"));
    assert!(table_exists(&db, "events"));

    let events = event_rows(&db);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].1, 0);
    assert_eq!(events[0].2, SUPPORTED_SCHEMA_VERSION);
    assert!(
        events[0].3.ends_with('Z') && events[0].3.contains('T'),
        "created_at must be RFC 3339 UTC: {}",
        events[0].3
    );
}

#[test]
fn first_use_opens_and_migrates() {
    let dir = support::private_tempdir();
    let path = dir.path().join("vlab.db");
    let (db, report) = Database::open_and_migrate(&path, RunId::generate()).unwrap();
    assert!(report.applied());
    assert_eq!(db.schema_version().unwrap(), SUPPORTED_SCHEMA_VERSION);
    drop(db);

    let (_db, report) = Database::open_and_migrate(&path, RunId::generate()).unwrap();
    assert!(!report.applied());
}

#[test]
fn migrating_again_applies_nothing_and_records_nothing() {
    let (_dir, db) = open();
    db.migrate(RunId::generate()).unwrap();
    let report = db.migrate(RunId::generate()).unwrap();
    assert_eq!(
        (report.from, report.to),
        (SUPPORTED_SCHEMA_VERSION, SUPPORTED_SCHEMA_VERSION)
    );
    assert!(!report.applied());
    assert_eq!(db.schema_version().unwrap(), SUPPORTED_SCHEMA_VERSION);
    assert_eq!(event_rows(&db).len(), 1);
}

fn migrator_before_assigned_progress(
    db: &Database,
) -> (tempfile::TempDir, sqlx::migrate::Migrator) {
    let dir = tempfile::tempdir().unwrap();
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations");
    for entry in fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let name = entry.file_name();
        if name.to_string_lossy().starts_with("0008") {
            continue;
        }
        fs::copy(entry.path(), dir.path().join(name)).unwrap();
    }
    let migrator = db
        .block_on(sqlx::migrate::Migrator::new(dir.path()))
        .unwrap();
    (dir, migrator)
}

#[test]
fn assigned_progress_migration_moves_only_assigned_todo_stories() {
    let (_dir, db) = open();
    let (_migrations, migrator) = migrator_before_assigned_progress(&db);
    db.migrate_with(&migrator, 7, RunId::generate()).unwrap();
    for sql in [
        "INSERT INTO labs (id, name, created_at, updated_at) VALUES ('a3f91c07', 'Lab', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        "INSERT INTO agents (id, name, role, description, skills_json, provider, model, created_at, updated_at) VALUES ('0b1c2d3e', 'Ada', 'dev', 'x', '[]', 'deepseek', 'm', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        "INSERT INTO projects (id, lab_id, name, created_at, updated_at) VALUES ('1c2d3e4f', 'a3f91c07', 'Portal', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        "INSERT INTO requests (id, project_id, content, status, created_at, updated_at) VALUES ('2d3e4f50', '1c2d3e4f', 'R', 'PLANNED', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        "INSERT INTO stories (id, project_id, request_id, title, description, acceptance_criteria_json, priority, complexity, required_skills_json, assigned_agent_id, assignment_rationale, state, due_date, created_at, updated_at) VALUES ('3e4f5061', '1c2d3e4f', '2d3e4f50', 'Assigned', 'D', '[]', 'high', 'small', '[]', '0b1c2d3e', 'R', 'TODO', '2026-01-03', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        "INSERT INTO stories (id, project_id, request_id, title, description, acceptance_criteria_json, priority, complexity, required_skills_json, assigned_agent_id, assignment_rationale, state, due_date, created_at, updated_at) VALUES ('4f506172', '1c2d3e4f', '2d3e4f50', 'Waiting', 'D', '[]', 'low', 'small', '[]', NULL, 'R', 'TODO', '2026-01-04', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
    ] {
        db.execute(sql).unwrap();
    }
    db.migrate(RunId::generate()).unwrap();
    let rows: Vec<(String, String)> = db.block_on(async {
        sqlx::query_as("SELECT title, state FROM stories ORDER BY title")
            .fetch_all(db.pool())
            .await
            .unwrap()
    });
    assert_eq!(
        rows,
        vec![
            ("Assigned".to_owned(), "IN_PROGRESS".to_owned()),
            ("Waiting".to_owned(), "TODO".to_owned()),
        ]
    );
}

#[test]
fn workflow_schema_accepts_workflow_interactions_and_outcomes() {
    let (_dir, db) = open();
    db.migrate(RunId::generate()).unwrap();
    for sql in [
        "INSERT INTO labs (id, name, created_at, updated_at) VALUES ('a3f91c07', 'Lab', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        "INSERT INTO agents (id, name, role, description, skills_json, provider, model, created_at, updated_at) VALUES ('0b1c2d3e', 'Ada', 'dev', 'x', '[]', 'deepseek', 'm', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        "INSERT INTO projects (id, lab_id, name, created_at, updated_at) VALUES ('1c2d3e4f', 'a3f91c07', 'Portal', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        "INSERT INTO requests (id, project_id, content, status, created_at, updated_at) VALUES ('2d3e4f50', '1c2d3e4f', 'R', 'PLANNED', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        "INSERT INTO stories (id, project_id, request_id, title, description, acceptance_criteria_json, priority, complexity, required_skills_json, assigned_agent_id, assignment_rationale, state, due_date, created_at, updated_at) VALUES ('3e4f5061', '1c2d3e4f', '2d3e4f50', 'T', 'D', '[]', 'high', 'small', '[]', '0b1c2d3e', 'R', 'DONE', '2026-01-03', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        "INSERT INTO story_outcomes (story_id, work_outcome, review_outcome, reviewer_agent_id, review_accepted, branch, worktree_path, commit_sha) VALUES ('3e4f5061', 'Work', 'Review', '0b1c2d3e', 1, 'vlab/3e4f5061', '.worktrees/3e4f5061', 'abcdef1')",
        "UPDATE stories SET execution_attempts = 2 WHERE id = '3e4f5061'",
        "INSERT INTO interactions (id, run_id, agent_id, kind, provider, model, outcome, duration_ms, created_at) VALUES ('4f506172', 'run', '0b1c2d3e', 'workflow', 'deepseek', 'm', 'success', 1, '2026-01-01T00:00:00Z')",
    ] {
        db.execute(sql).unwrap();
    }
    let (work, review, branch, worktree, commit, accepted): (
        String,
        String,
        Option<String>,
        Option<String>,
        Option<String>,
        i64,
    ) = db.block_on(async {
        sqlx::query_as(
            "SELECT work_outcome, review_outcome, branch, worktree_path, commit_sha, review_accepted \
             FROM story_outcomes WHERE story_id = '3e4f5061'",
        )
        .fetch_one(db.pool())
        .await
        .unwrap()
    });
    assert_eq!((work.as_str(), review.as_str()), ("Work", "Review"));
    assert_eq!(branch.as_deref(), Some("vlab/3e4f5061"));
    assert_eq!(worktree.as_deref(), Some(".worktrees/3e4f5061"));
    assert_eq!(commit.as_deref(), Some("abcdef1"));
    assert_eq!(accepted, 1);
    let attempts: i64 = db.block_on(async {
        sqlx::query_scalar("SELECT execution_attempts FROM stories WHERE id = '3e4f5061'")
            .fetch_one(db.pool())
            .await
            .unwrap()
    });
    assert_eq!(attempts, 2);
}

#[test]
fn a_broken_migration_rolls_back_without_leaving_partial_schema() {
    let (_dir, db) = open();
    let migrations = support::private_tempdir();
    fs::write(
        migrations.path().join("0001_broken.sql"),
        "CREATE TABLE partial (id TEXT PRIMARY KEY) STRICT;\nCREATE TABLE oops (;\n",
    )
    .unwrap();
    let migrator = db
        .block_on(sqlx::migrate::Migrator::new(migrations.path()))
        .unwrap();

    let error = db
        .migrate_with(&migrator, 1, RunId::generate())
        .unwrap_err();
    assert_eq!(error, StorageError::Migration);
    assert_eq!(db.schema_version().unwrap(), 0);
    assert!(!table_exists(&db, "partial"));
    assert!(!table_exists(&db, "events"));
}

#[test]
fn a_migration_history_from_another_build_is_refused() {
    let (_dir, db) = open();
    db.migrate(RunId::generate()).unwrap();

    // The same version with different content is a modified migration.
    let migrations = support::private_tempdir();
    fs::write(
        migrations.path().join("0001_init.sql"),
        "CREATE TABLE other (id TEXT);\n",
    )
    .unwrap();
    let migrator = db
        .block_on(sqlx::migrate::Migrator::new(migrations.path()))
        .unwrap();

    let error = db
        .migrate_with(&migrator, SUPPORTED_SCHEMA_VERSION, RunId::generate())
        .unwrap_err();
    assert_eq!(error, StorageError::SchemaMismatch);
    assert!(
        table_exists(&db, "app_state"),
        "the original schema is intact"
    );
}

#[test]
fn a_target_older_than_the_database_is_refused() {
    let (_dir, db) = open();
    db.migrate(RunId::generate()).unwrap();
    let error = db
        .migrate_with(&vlab::infra::db::migrate::MIGRATOR, 0, RunId::generate())
        .unwrap_err();
    assert!(
        matches!(error, StorageError::NewerSchema { found, .. } if found == SUPPORTED_SCHEMA_VERSION),
        "{error:?}"
    );
}
