//! Retention policy: application tables store no credentials (WP2-AC7).

mod support;

use vlab::infra::db::{Database, policy};
use vlab::obs::RunId;

#[test]
fn the_migrated_schema_has_no_credential_bearing_column() {
    let dir = support::private_tempdir();
    let db = Database::open(dir.path().join("vlab.db")).unwrap();
    db.migrate(RunId::generate()).unwrap();

    assert_eq!(
        policy::schema_violations(&db).unwrap(),
        Vec::<String>::new()
    );
}

#[test]
fn the_policy_rejects_a_forbidden_column() {
    let violations = policy::violations([
        "id".to_owned(),
        "value".to_owned(),
        "api_key".to_owned(),
        "client_secret".to_owned(),
    ]);
    assert_eq!(violations, ["api_key", "client_secret"]);
}

#[test]
fn schema_v3_contains_only_the_documented_application_tables() {
    let dir = support::private_tempdir();
    let db = Database::open(dir.path().join("vlab.db")).unwrap();
    db.migrate(RunId::generate()).unwrap();
    let mut tables: Vec<String> = db.block_on(async {
        sqlx::query_scalar(
            "SELECT name FROM sqlite_master \
                 WHERE type = 'table' AND name NOT LIKE 'sqlite\\_%' ESCAPE '\\' \
                   AND name NOT LIKE '\\_sqlx\\_%' ESCAPE '\\' ORDER BY name",
        )
        .fetch_all(db.pool())
        .await
        .unwrap()
    });
    tables.sort();
    assert_eq!(
        tables,
        [
            "agents",
            "app_state",
            "events",
            "interactions",
            "lab_memberships",
            "labs",
            "memories",
            "project_agents",
            "projects",
            "requests",
            "stories"
        ]
    );
}
