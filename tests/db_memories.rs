//! Scoped memory persistence, caps, and cascade behavior (WP4-AC10).

mod support;

use std::fs;

use vlab::app::context::save_active_lab;
use vlab::app::directory::Directory;
use vlab::app::memories::{self, MemoryStore};
use vlab::app::projects::ProjectStore;
use vlab::domain::agent::{AgentProfile, Description, Role};
use vlab::domain::ids::{AgentId, LabId, ProjectId};
use vlab::domain::lab::Lab;
use vlab::domain::memory::{MEMORY_MAX_PER_SCOPE, MemoryScope};
use vlab::domain::name::Name;
use vlab::domain::settings::{ModelId, ProviderId};
use vlab::domain::skills;
use vlab::infra::db::Database;

fn migrated() -> (tempfile::TempDir, Database) {
    let dir = support::private_tempdir();
    let db = Database::open(dir.path().join("vlab.db")).unwrap();
    db.migrate(vlab::obs::RunId::generate()).unwrap();
    (dir, db)
}

fn seed_lab(db: &Database, id: &str, name: &str) {
    db.insert_lab(&Lab::new(
        LabId::parse(id).unwrap(),
        Name::parse(name).unwrap(),
    ))
    .unwrap();
}

fn seed_project(db: &Database, id: &str, lab: &LabId, name: &str) -> ProjectId {
    let id = ProjectId::parse(id).unwrap();
    db.insert_project(&vlab::domain::project::Project::new(
        id.clone(),
        lab.clone(),
        Name::parse(name).unwrap(),
        None,
    ))
    .unwrap();
    id
}

fn seed_agent(db: &Database, id: &str, labs: &[LabId]) {
    let profile = AgentProfile::new(
        AgentId::parse(id).unwrap(),
        Name::parse("Ada").unwrap(),
        Role::Lead,
        Description::parse("Leads").unwrap(),
        skills::parse_list("").unwrap(),
        ProviderId::DeepSeek,
        ModelId::parse("deepseek-chat").unwrap(),
        None,
    )
    .unwrap();
    db.insert_agent(&profile, labs).unwrap();
}

#[test]
fn lab_project_and_agent_memories_round_trip() {
    let (_dir, db) = migrated();
    let lab = LabId::parse("a3f91c07").unwrap();
    seed_lab(&db, "a3f91c07", "Payments");
    save_active_lab(&db, &lab).unwrap();
    let project = seed_project(&db, "0b1c2d3e", &lab, "Portal");
    seed_agent(&db, "1c2d3e4f", std::slice::from_ref(&lab));

    for scope in [
        MemoryScope::Lab(lab.clone()),
        MemoryScope::Project(project),
        MemoryScope::Agent(AgentId::parse("1c2d3e4f").unwrap()),
    ] {
        memories::create(&db, &db, scope.clone(), "Keep work small.", None).unwrap();
        assert_eq!(memories::list_checked(&db, &db, scope).unwrap().len(), 1);
    }
}

#[test]
fn memory_content_is_stored_in_markdown_and_the_database_has_only_the_path() {
    let (dir, db) = migrated();
    let lab = LabId::parse("a3f91c07").unwrap();
    seed_lab(&db, "a3f91c07", "Payments");
    save_active_lab(&db, &lab).unwrap();
    let scope = MemoryScope::Lab(lab);
    let memory = memories::create(
        &db,
        &db,
        scope.clone(),
        "# Keep work small.\n\nTests first.",
        None,
    )
    .unwrap();
    let path: Option<String> = db
        .block_on(
            sqlx::query_scalar("SELECT path FROM memories WHERE id = ?1")
                .bind(memory.id.as_str())
                .fetch_one(db.pool()),
        )
        .unwrap();
    let path = path.expect("path");
    assert!(path.starts_with("memories/lab/"), "{path}");
    assert!(path.ends_with(".md"), "{path}");
    assert_eq!(
        fs::read_to_string(dir.path().join(&path)).unwrap(),
        "# Keep work small.\n\nTests first."
    );
    let content: String = db
        .block_on(
            sqlx::query_scalar("SELECT content FROM memories WHERE id = ?1")
                .bind(memory.id.as_str())
                .fetch_one(db.pool()),
        )
        .unwrap();
    assert!(content.is_empty(), "{content:?}");

    let updated = memories::update(&db, &db, memory.id.as_str(), "Updated note.", None).unwrap();
    assert_eq!(
        fs::read_to_string(dir.path().join(&path)).unwrap(),
        "Updated note."
    );
    db.delete_memory(&updated.id).unwrap();
    assert!(!dir.path().join(&path).exists());
}

#[test]
fn a_v14_memory_is_materialized_as_a_markdown_file_on_migration() {
    let dir = support::private_tempdir();
    let migrations = support::private_tempdir();
    for entry in
        fs::read_dir(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations")).unwrap()
    {
        let entry = entry.unwrap();
        let name = entry.file_name();
        if name.to_string_lossy().as_ref() < "0015" && name.to_string_lossy().ends_with(".sql") {
            fs::copy(entry.path(), migrations.path().join(&name)).unwrap();
        }
    }
    let db = Database::open(dir.path().join("vlab.db")).unwrap();
    let migrator = db
        .block_on(sqlx::migrate::Migrator::new(migrations.path()))
        .unwrap();
    db.migrate_with(&migrator, 14, vlab::obs::RunId::generate())
        .unwrap();
    seed_lab(&db, "a3f91c07", "Payments");
    db.execute(
        "INSERT INTO memories (id, scope_type, lab_id, content, created_at) \
         VALUES ('0b1c2d3e', 'lab', 'a3f91c07', 'Legacy note.', '2026-01-01T00:00:00Z')",
    )
    .unwrap();
    db.migrate(vlab::obs::RunId::generate()).unwrap();
    let row: (Option<String>, String) = db
        .block_on(
            sqlx::query_as("SELECT path, content FROM memories WHERE id = '0b1c2d3e'")
                .fetch_one(db.pool()),
        )
        .unwrap();
    let path = row.0.expect("path");
    assert!(path.starts_with("memories/lab/payments/"), "{path}");
    assert!(row.1.is_empty());
    assert_eq!(
        fs::read_to_string(dir.path().join(path)).unwrap(),
        "Legacy note."
    );
}

#[test]
fn the_twenty_first_memory_in_a_scope_is_rejected() {
    let (_dir, db) = migrated();
    seed_lab(&db, "a3f91c07", "Payments");
    let scope = MemoryScope::Lab(LabId::parse("a3f91c07").unwrap());
    for index in 0..MEMORY_MAX_PER_SCOPE {
        memories::create(&db, &db, scope.clone(), &format!("note {index}"), None).unwrap();
    }
    let error = memories::create(&db, &db, scope, "one too many", None).unwrap_err();
    assert!(
        matches!(error, vlab::app::AppError::Validation { .. }),
        "{error:?}"
    );
}

#[test]
fn project_scope_is_bound_to_the_active_laboratory() {
    let (_dir, db) = migrated();
    seed_lab(&db, "a3f91c07", "A");
    seed_lab(&db, "1c2d3e4f", "B");
    save_active_lab(&db, &LabId::parse("a3f91c07").unwrap()).unwrap();
    let project = seed_project(&db, "0b1c2d3e", &LabId::parse("1c2d3e4f").unwrap(), "Other");
    let error = memories::create(&db, &db, MemoryScope::Project(project), "x", None).unwrap_err();
    assert!(
        matches!(error, vlab::app::AppError::NotFound { .. }),
        "{error:?}"
    );
}

#[test]
fn archived_agent_memories_remain_listable_but_new_ones_are_rejected() {
    let (_dir, db) = migrated();
    let lab = LabId::parse("a3f91c07").unwrap();
    seed_lab(&db, "a3f91c07", "Payments");
    save_active_lab(&db, &lab).unwrap();
    seed_agent(&db, "1c2d3e4f", std::slice::from_ref(&lab));
    let scope = MemoryScope::Agent(AgentId::parse("1c2d3e4f").unwrap());
    memories::create(&db, &db, scope.clone(), "Historical note", None).unwrap();
    db.execute("UPDATE agents SET archived_at = '2026-01-01T00:00:00Z'")
        .unwrap();
    assert_eq!(
        memories::list_checked(&db, &db, scope.clone())
            .unwrap()
            .len(),
        1
    );
    let error = memories::create(&db, &db, scope, "new", None).unwrap_err();
    assert!(
        matches!(error, vlab::app::AppError::Validation { .. }),
        "{error:?}"
    );
}

#[test]
fn deleting_a_laboratory_or_project_removes_its_memories() {
    let (_dir, db) = migrated();
    let lab = LabId::parse("a3f91c07").unwrap();
    seed_lab(&db, "a3f91c07", "Payments");
    save_active_lab(&db, &lab).unwrap();
    let project = seed_project(&db, "0b1c2d3e", &lab, "Portal");
    let lab_scope = MemoryScope::Lab(lab.clone());
    let project_scope = MemoryScope::Project(project.clone());
    memories::create(&db, &db, lab_scope.clone(), "lab note", None).unwrap();
    memories::create(&db, &db, project_scope.clone(), "project note", None).unwrap();
    db.execute("DELETE FROM projects WHERE id = '0b1c2d3e'")
        .unwrap();
    assert!(db.memories(&project_scope).unwrap().is_empty());
    assert_eq!(db.memories(&lab_scope).unwrap().len(), 1);
    db.execute("DELETE FROM labs WHERE id = 'a3f91c07'")
        .unwrap();
    assert!(db.memories(&lab_scope).unwrap().is_empty());
}

#[test]
fn a_malformed_stored_memory_is_reported_not_panicked() {
    let (_dir, db) = migrated();
    seed_lab(&db, "a3f91c07", "Payments");
    db.execute(
        "INSERT INTO memories (id, scope_type, lab_id, content, created_at) \
         VALUES ('0b1c2d3e', 'lab', 'a3f91c07', '', '2026-01-01T00:00:00Z')",
    )
    .unwrap();
    let error = db
        .memories(&MemoryScope::Lab(LabId::parse("a3f91c07").unwrap()))
        .unwrap_err();
    assert!(
        matches!(error, vlab::app::storage::StorageError::Operation),
        "{error:?}"
    );
}
