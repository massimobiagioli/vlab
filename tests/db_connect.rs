//! SQLite connection, pragmas, and private storage (WP2-AC1).

mod support;

use std::fs;
use std::time::Duration;

use vlab::app::AppError;
use vlab::app::storage::StorageError;
use vlab::cli::exit::{exit_code, render};
use vlab::infra::db::{DEFAULT_BUSY_TIMEOUT, Database, SUPPORTED_SCHEMA_VERSION};

fn database_path() -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = support::private_tempdir();
    let path = dir.path().join("vlab.db");
    (dir, path)
}

fn pragma(db: &Database, statement: &'static str) -> i64 {
    db.block_on(async {
        sqlx::query_scalar(statement)
            .fetch_one(db.pool())
            .await
            .unwrap()
    })
}

#[cfg(unix)]
fn mode(path: &std::path::Path) -> u32 {
    use std::os::unix::fs::PermissionsExt;
    fs::metadata(path).unwrap().permissions().mode() & 0o777
}

#[test]
fn opening_creates_a_private_database_with_foreign_keys_and_a_busy_timeout() {
    let (dir, path) = database_path();
    let db = Database::open(&path).unwrap();

    assert_eq!(pragma(&db, "PRAGMA foreign_keys"), 1);
    assert_eq!(
        pragma(&db, "PRAGMA busy_timeout"),
        i64::try_from(DEFAULT_BUSY_TIMEOUT.as_millis()).unwrap()
    );
    let journal_mode: String = db.block_on(async {
        sqlx::query_scalar("PRAGMA journal_mode")
            .fetch_one(db.pool())
            .await
            .unwrap()
    });
    assert_eq!(journal_mode, "delete", "WAL must stay off");
    assert_eq!(db.schema_version().unwrap(), 0);

    #[cfg(unix)]
    {
        assert_eq!(mode(&path), 0o600);
        assert_eq!(mode(dir.path()), 0o700);
    }
}

#[test]
fn every_acquired_connection_reports_foreign_keys_enabled() {
    let (_dir, path) = database_path();
    let db = Database::open(&path).unwrap();
    for _ in 0..3 {
        db.block_on(async {
            let mut connection = db.pool().acquire().await.unwrap();
            let enabled: i64 = sqlx::query_scalar("PRAGMA foreign_keys")
                .fetch_one(&mut *connection)
                .await
                .unwrap();
            assert_eq!(enabled, 1);
        });
    }
}

#[test]
fn foreign_key_violations_are_rejected() {
    let (_dir, path) = database_path();
    let db = Database::open(&path).unwrap();
    db.execute(
        "CREATE TABLE parent (id TEXT PRIMARY KEY) STRICT;\
         CREATE TABLE child (id TEXT PRIMARY KEY, parent_id TEXT NOT NULL REFERENCES parent(id)) STRICT",
    )
    .unwrap();
    let error = db
        .execute("INSERT INTO child (id, parent_id) VALUES ('a', 'missing')")
        .unwrap_err();
    assert_eq!(error, StorageError::Operation);
}

#[test]
fn a_held_write_transaction_produces_the_documented_lock_failure() {
    let (_dir, path) = database_path();
    Database::open(&path).unwrap();
    let writer = Database::open_with(&path, Duration::from_millis(50)).unwrap();
    let contender = Database::open_with(&path, Duration::from_millis(50)).unwrap();

    let (ready_sender, ready_receiver) = std::sync::mpsc::channel();
    let handle = std::thread::spawn(move || {
        writer.block_on(async {
            let mut connection = writer.pool().acquire().await.unwrap();
            sqlx::raw_sql("BEGIN IMMEDIATE")
                .execute(&mut *connection)
                .await
                .unwrap();
            ready_sender.send(()).unwrap();
            std::thread::sleep(Duration::from_millis(300));
            sqlx::raw_sql("COMMIT")
                .execute(&mut *connection)
                .await
                .unwrap();
        });
    });

    ready_receiver.recv().unwrap();
    let error = contender
        .execute("CREATE TABLE blocked (id TEXT)")
        .expect_err("the write must wait and then fail");
    assert!(matches!(error, StorageError::Locked { .. }), "{error:?}");

    let app = AppError::from(error);
    assert_eq!(exit_code(&app), 1);
    let rendered = render(&app);
    assert!(
        rendered.contains("is in use by another process"),
        "{rendered}"
    );
    assert!(
        rendered.contains("Hint: Wait for the other vlab process"),
        "{rendered}"
    );

    handle.join().unwrap();
    // The database remains usable once the lock is released.
    contender
        .execute("CREATE TABLE after_lock (id TEXT)")
        .unwrap();
}

#[test]
fn a_file_that_is_not_a_database_is_reported_as_corrupt() {
    let (_dir, path) = database_path();
    fs::write(&path, b"this is not a SQLite database").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    }
    let error = match Database::open(&path) {
        Ok(_) => panic!("a corrupt file must not open"),
        Err(error) => error,
    };
    assert!(matches!(error, StorageError::Corrupt { .. }), "{error:?}");
}

#[test]
fn a_database_created_by_a_newer_build_is_refused() {
    let (_dir, path) = database_path();
    let db = Database::open(&path).unwrap();
    let newer = SUPPORTED_SCHEMA_VERSION + 1;
    db.block_on(async {
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "PRAGMA user_version = {newer}"
        )))
        .execute(db.pool())
        .await
        .unwrap();
    });
    drop(db);

    let error = match Database::open(&path) {
        Ok(_) => panic!("a newer schema must be refused"),
        Err(error) => error,
    };
    match &error {
        StorageError::NewerSchema {
            found, supported, ..
        } => {
            assert_eq!((*found, *supported), (newer, SUPPORTED_SCHEMA_VERSION));
        }
        other => panic!("unexpected error: {other:?}"),
    }
    let app = AppError::from(error);
    assert_eq!(exit_code(&app), 1);
    assert!(render(&app).contains(&format!("schema version {newer}")));
}

#[test]
fn help_does_not_create_a_database() {
    let sandbox = support::Sandbox::new();
    sandbox.run(&["--help"]).assert_success();
    assert!(!sandbox.data_dir().exists());
}
