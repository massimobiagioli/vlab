//! `app_state` persistence and the active-laboratory typed helpers
//! (WP2-AC5, WP2-AC6).

mod support;

use vlab::app::context::{ActiveLab, active_lab, clear_active_lab, save_active_lab};
use vlab::app::storage::{AppStateKey, AppStateStore, StorageError};
use vlab::domain::ids::LabId;
use vlab::infra::db::{Database, app_state};
use vlab::obs::RunId;

fn migrated(path: &std::path::Path) -> Database {
    let db = Database::open(path).unwrap();
    db.migrate(RunId::generate()).unwrap();
    db
}

#[test]
fn the_selection_persists_across_connections_and_clears() {
    let dir = support::private_tempdir();
    let path = dir.path().join("vlab.db");
    let id = LabId::parse("a3f91c07").unwrap();

    {
        let db = migrated(&path);
        assert_eq!(active_lab(&db).unwrap(), ActiveLab::NotSelected);
        save_active_lab(&db, &id).unwrap();
        assert_eq!(active_lab(&db).unwrap(), ActiveLab::Selected(id.clone()));
    }

    {
        let db = migrated(&path);
        assert_eq!(active_lab(&db).unwrap(), ActiveLab::Selected(id));
        clear_active_lab(&db).unwrap();
        assert_eq!(active_lab(&db).unwrap(), ActiveLab::NotSelected);
    }

    let db = migrated(&path);
    assert_eq!(db.get(AppStateKey::ActiveLabId).unwrap(), None);
}

#[test]
fn a_corrupt_stored_value_is_treated_as_no_active_laboratory() {
    let dir = support::private_tempdir();
    let db = migrated(&dir.path().join("vlab.db"));
    db.set(AppStateKey::ActiveLabId, "not-an-identifier")
        .unwrap();

    let state = active_lab(&db).unwrap();
    assert_eq!(state, ActiveLab::Invalid);
    assert!(state.warning().is_some());
    assert!(state.require().is_err());
}

#[test]
fn a_transaction_scoped_write_commits_only_on_success() {
    let dir = support::private_tempdir();
    let db = migrated(&dir.path().join("vlab.db"));

    db.transaction(|connection| {
        Box::pin(async move {
            app_state::set(connection, AppStateKey::ActiveLabId, "a3f91c07").await?;
            Ok(())
        })
    })
    .unwrap();
    assert_eq!(
        db.get(AppStateKey::ActiveLabId).unwrap().as_deref(),
        Some("a3f91c07")
    );

    let result: Result<(), StorageError> = db.transaction(|connection| {
        Box::pin(async move {
            app_state::set(connection, AppStateKey::ActiveLabId, "0b1c2d3e").await?;
            Err(StorageError::Operation)
        })
    });
    assert_eq!(result, Err(StorageError::Operation));
    assert_eq!(
        db.get(AppStateKey::ActiveLabId).unwrap().as_deref(),
        Some("a3f91c07"),
        "the failed transaction must roll back"
    );
}

#[test]
fn the_timestamp_is_recorded_in_utc() {
    let dir = support::private_tempdir();
    let db = migrated(&dir.path().join("vlab.db"));
    db.set(AppStateKey::ActiveLabId, "a3f91c07").unwrap();
    let updated_at: String = db.block_on(async {
        sqlx::query_scalar("SELECT updated_at FROM app_state WHERE key = ?1")
            .bind(AppStateKey::ActiveLabId.as_str())
            .fetch_one(db.pool())
            .await
            .unwrap()
    });
    assert!(updated_at.ends_with('Z'), "{updated_at}");
    assert!(updated_at.contains('T'), "{updated_at}");
}
