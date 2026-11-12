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
