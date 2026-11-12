//! Scoped memory persistence, caps, and cascade behavior (WP4-AC10).

mod support;

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
        memories::create(&db, &db, scope.clone(), "Keep work small.").unwrap();
        assert_eq!(memories::list_checked(&db, &db, scope).unwrap().len(), 1);
    }
}

#[test]
fn the_twenty_first_memory_in_a_scope_is_rejected() {
    let (_dir, db) = migrated();
    seed_lab(&db, "a3f91c07", "Payments");
    let scope = MemoryScope::Lab(LabId::parse("a3f91c07").unwrap());
    for index in 0..MEMORY_MAX_PER_SCOPE {
        memories::create(&db, &db, scope.clone(), &format!("note {index}")).unwrap();
    }
    let error = memories::create(&db, &db, scope, "one too many").unwrap_err();
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
    let error = memories::create(&db, &db, MemoryScope::Project(project), "x").unwrap_err();
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
    memories::create(&db, &db, scope.clone(), "Historical note").unwrap();
    db.execute("UPDATE agents SET archived_at = '2026-01-01T00:00:00Z'")
        .unwrap();
    assert_eq!(
        memories::list_checked(&db, &db, scope.clone())
            .unwrap()
            .len(),
        1
    );
    let error = memories::create(&db, &db, scope, "new").unwrap_err();
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
    memories::create(&db, &db, lab_scope.clone(), "lab note").unwrap();
    memories::create(&db, &db, project_scope.clone(), "project note").unwrap();
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
