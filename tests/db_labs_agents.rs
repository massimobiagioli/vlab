//! Schema v2, constraints, and directory persistence (WP3-AC3).

mod support;

use std::fs;

use vlab::app::directory::Directory;
use vlab::app::projects::ProjectStore;
use vlab::app::storage::StorageError;
use vlab::domain::agent::{AgentProfile, Description, Role};
use vlab::domain::ids::{AgentId, LabId, ProjectId};
use vlab::domain::lab::{Lab, LabDescription};
use vlab::domain::name::Name;
use vlab::domain::project::Project;
use vlab::domain::settings::{ModelId, ProviderId, Settings};
use vlab::domain::skills;
use vlab::infra::db::{Database, SUPPORTED_SCHEMA_VERSION};

fn migrated() -> (tempfile::TempDir, Database) {
    let dir = support::private_tempdir();
    let db = Database::open(dir.path().join("vlab.db")).unwrap();
    db.migrate(vlab::obs::RunId::generate()).unwrap();
    (dir, db)
}

fn count(db: &Database, sql: &'static str) -> i64 {
    db.block_on(async { sqlx::query_scalar(sql).fetch_one(db.pool()).await.unwrap() })
}

fn lab(name: &str) -> Lab {
    Lab::new(
        LabId::parse("a3f91c07").unwrap(),
        Name::parse(name).unwrap(),
    )
}

fn profile(provider: &str, model: &str) -> AgentProfile {
    AgentProfile::new(
        AgentId::parse("0b1c2d3e").unwrap(),
        Name::parse("Ada").unwrap(),
        Role::Dev,
        Description::parse("Writes code").unwrap(),
        skills::parse_list("rust, api").unwrap(),
        ProviderId::parse(provider).unwrap(),
        ModelId::parse(model).unwrap(),
        Some("Synthetic CV text".to_owned()),
    )
    .unwrap()
}

#[test]
fn schema_v2_rejects_invalid_roles_and_names() {
    let (_dir, db) = migrated();
    let invalid_role = db.execute(
        "INSERT INTO agents (id, name, role, description, skills_json, provider, model, created_at, updated_at) \
         VALUES ('0b1c2d3e', 'Ada', 'boss', 'x', '[]', 'deepseek', 'm', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
    );
    assert!(invalid_role.is_err(), "the role CHECK must reject `boss`");

    let invalid_name = db.execute(
        "INSERT INTO labs (id, name, created_at, updated_at) \
         VALUES ('a3f91c07', '   ', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
    );
    assert!(invalid_name.is_err(), "blank names must be rejected");
}

#[test]
fn memberships_cascade_when_a_laboratory_is_deleted() {
    let (_dir, db) = migrated();
    db.execute(
        "INSERT INTO labs (id, name, created_at, updated_at) VALUES ('a3f91c07', 'Lab', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
    )
    .unwrap();
    db.execute(
        "INSERT INTO agents (id, name, role, description, skills_json, provider, model, created_at, updated_at) \
         VALUES ('0b1c2d3e', 'Ada', 'dev', 'x', '[]', 'deepseek', 'm', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
    )
    .unwrap();
    db.execute(
        "INSERT INTO lab_memberships (lab_id, agent_id, joined_at) \
         VALUES ('a3f91c07', '0b1c2d3e', '2026-01-01T00:00:00Z')",
    )
    .unwrap();
    db.execute("DELETE FROM labs WHERE id = 'a3f91c07'")
        .unwrap();
    assert_eq!(count(&db, "SELECT count(*) FROM lab_memberships"), 0);
    assert_eq!(count(&db, "SELECT count(*) FROM agents"), 1);
}

#[test]
fn migration_from_v1_preserves_application_state() {
    let dir = support::private_tempdir();
    let db = Database::open(dir.path().join("vlab.db")).unwrap();

    // Apply only the v1 migration, as an older binary would have done.
    let v1_only = support::private_tempdir();
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations/0001_init.sql");
    fs::copy(source, v1_only.path().join("0001_init.sql")).unwrap();
    let migrator = db
        .block_on(sqlx::migrate::Migrator::new(v1_only.path()))
        .unwrap();
    let report = db
        .migrate_with(&migrator, 1, vlab::obs::RunId::generate())
        .unwrap();
    assert_eq!(report.to, 1);

    use vlab::app::storage::{AppStateKey, AppStateStore};
    db.set(AppStateKey::ActiveLabId, "a3f91c07").unwrap();

    let report = db.migrate(vlab::obs::RunId::generate()).unwrap();
    assert_eq!((report.from, report.to), (1, SUPPORTED_SCHEMA_VERSION));
    assert_eq!(
        db.get(AppStateKey::ActiveLabId).unwrap().as_deref(),
        Some("a3f91c07")
    );
    assert_eq!(count(&db, "SELECT count(*) FROM labs"), 0);
}

#[test]
fn the_directory_round_trips_profiles_and_memberships() {
    let (_dir, db) = migrated();
    let lab = lab("Payments");
    db.insert_lab(&lab).unwrap();
    let profile = profile("deepseek", "deepseek-chat");
    db.insert_agent(&profile, std::slice::from_ref(&lab.id))
        .unwrap();

    let overview = db.lab(&lab.id).unwrap().unwrap();
    assert_eq!(overview.name, "Payments");
    assert_eq!(overview.agent_count, 1);

    let stored = db.agent(&profile.id).unwrap().unwrap();
    assert_eq!(stored.name, "Ada");
    assert_eq!(stored.role, "dev");
    assert_eq!(stored.provider, "deepseek");
    assert_eq!(stored.model, "deepseek-chat");
    assert_eq!(stored.cv.as_deref(), Some("Synthetic CV text"));
    assert!(!stored.archived);
    assert_eq!(stored.profile().unwrap(), profile);

    let members = db.lab_agents(&lab.id).unwrap();
    assert_eq!(members, vec![stored]);
    let memberships = db.agent_memberships(&profile.id).unwrap();
    assert_eq!(memberships, vec![overview]);
}

#[test]
fn copied_provider_and_model_do_not_follow_later_settings_changes() {
    let (_dir, db) = migrated();
    let lab = lab("Payments");
    db.insert_lab(&lab).unwrap();
    let original = profile("deepseek", "deepseek-chat");
    db.insert_agent(&original, std::slice::from_ref(&lab.id))
        .unwrap();

    // The user edits settings afterwards.
    let changed = Settings::parse("deepseek", "deepseek-reasoner").unwrap();
    assert_eq!(changed.model.as_str(), "deepseek-reasoner");

    let stored = db.agent(&original.id).unwrap().unwrap();
    assert_eq!(stored.model, "deepseek-chat");
}

#[test]
fn an_agent_can_belong_to_several_laboratories() {
    let (_dir, db) = migrated();
    let first = Lab::new(
        LabId::parse("a3f91c07").unwrap(),
        Name::parse("First").unwrap(),
    );
    let second = Lab::new(
        LabId::parse("1c2d3e4f").unwrap(),
        Name::parse("Second").unwrap(),
    );
    db.insert_lab(&first).unwrap();
    db.insert_lab(&second).unwrap();
    let profile = profile("deepseek", "deepseek-chat");
    db.insert_agent(&profile, &[first.id.clone(), second.id.clone()])
        .unwrap();
    assert_eq!(db.agent_memberships(&profile.id).unwrap().len(), 2);
    assert_eq!(db.lab(&first.id).unwrap().unwrap().agent_count, 1);
    assert_eq!(db.lab(&second.id).unwrap().unwrap().agent_count, 1);
}

#[test]
fn a_foreign_key_violation_rolls_back_the_whole_agent_insert() {
    let (_dir, db) = migrated();
    let profile = profile("deepseek", "deepseek-chat");
    let missing = LabId::parse("ffffffff").unwrap();
    let error = db
        .insert_agent(&profile, &[missing])
        .expect_err("an unknown laboratory must fail");
    assert!(matches!(error, StorageError::Operation), "{error:?}");
    assert!(db.agent(&profile.id).unwrap().is_none(), "no partial row");
}

#[test]
fn an_agents_projects_include_the_project_roster() {
    let (_dir, db) = migrated();
    let lab = lab("Payments");
    db.insert_lab(&lab).unwrap();
    let profile = profile("deepseek", "deepseek-chat");
    db.insert_agent(&profile, std::slice::from_ref(&lab.id))
        .unwrap();
    let project = ProjectId::parse("1c2d3e4f").unwrap();
    db.insert_project(&Project::new(
        project.clone(),
        lab.id,
        Name::parse("Portal").unwrap(),
        None,
    ))
    .unwrap();
    db.assign_project_agent(&project, &profile.id).unwrap();
    assert_eq!(db.agent_project_count(&profile.id).unwrap(), 1);
    assert_eq!(
        db.agent_project_names(&profile.id).unwrap(),
        vec![(project, "Portal".to_owned())]
    );
}

#[test]
fn a_laboratory_description_can_be_created_and_updated() {
    let (_dir, db) = migrated();
    let mut lab = lab("Payments");
    db.insert_lab(&lab).unwrap();
    assert_eq!(db.labs().unwrap()[0].description, "");
    lab.description = LabDescription::parse("Platform team").unwrap();
    db.update_lab(&lab).unwrap();
    assert_eq!(db.labs().unwrap()[0].description, "Platform team");
}
