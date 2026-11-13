//! Schema v3, project persistence, cascades, and migration preservation.

mod support;

use std::fs;

use vlab::app::directory::Directory;
use vlab::app::projects::ProjectStore;
use vlab::app::storage::{AppStateKey, AppStateStore};
use vlab::domain::agent::{AgentProfile, Description, Role};
use vlab::domain::ids::AgentId;
use vlab::domain::ids::{LabId, ProjectId};
use vlab::domain::lab::Lab;
use vlab::domain::name::Name;
use vlab::domain::project::Project;
use vlab::domain::settings::{ModelId, ProviderId};
use vlab::domain::skills;
use vlab::infra::db::{Database, SUPPORTED_SCHEMA_VERSION};

fn migrated() -> (tempfile::TempDir, Database) {
    let dir = support::private_tempdir();
    let db = Database::open(dir.path().join("vlab.db")).unwrap();
    db.migrate(vlab::obs::RunId::generate()).unwrap();
    (dir, db)
}

fn lab(id: &str, name: &str) -> Lab {
    Lab::new(LabId::parse(id).unwrap(), Name::parse(name).unwrap())
}

fn project(id: &str, lab: &LabId, name: &str) -> Project {
    Project::new(
        ProjectId::parse(id).unwrap(),
        lab.clone(),
        Name::parse(name).unwrap(),
        None,
    )
}

fn count(db: &Database, sql: &'static str) -> i64 {
    db.block_on(async { sqlx::query_scalar(sql).fetch_one(db.pool()).await.unwrap() })
}

#[test]
fn projects_round_trip_and_report_counts() {
    let (_dir, db) = migrated();
    let lab = lab("a3f91c07", "Payments");
    db.insert_lab(&lab).unwrap();
    let project = project("0b1c2d3e", &lab.id, "Portal");
    db.insert_project(&project).unwrap();

    let listed = db.projects(&lab.id).unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, project.id);
    assert_eq!(listed[0].name, "Portal");
    assert_eq!(listed[0].request_count, 0);
    assert_eq!(
        db.project_in_lab(&project.id, &lab.id)
            .unwrap()
            .unwrap()
            .name,
        "Portal"
    );
}

#[test]
fn deleting_a_laboratory_cascades_projects_requests_stories_and_memories() {
    let (_dir, db) = migrated();
    db.insert_lab(&lab("a3f91c07", "Payments")).unwrap();
    db.insert_project(&project(
        "0b1c2d3e",
        &LabId::parse("a3f91c07").unwrap(),
        "Portal",
    ))
    .unwrap();
    db.execute(
        "INSERT INTO requests (id, project_id, content, status, created_at, updated_at) \
         VALUES ('1c2d3e4f', '0b1c2d3e', 'Add reset', 'TODO', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
    )
    .unwrap();
    db.execute(
        "INSERT INTO stories (id, project_id, request_id, title, description, acceptance_criteria_json, priority, complexity, required_skills_json, state, due_date, created_at, updated_at) \
         VALUES ('2d3e4f50', '0b1c2d3e', '1c2d3e4f', 'Reset', 'D', '[]', 'high', 'small', '[]', 'TODO', '2026-01-03', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
    )
    .unwrap();
    db.execute(
        "INSERT INTO memories (id, scope_type, project_id, content, created_at) \
         VALUES ('3e4f5061', 'project', '0b1c2d3e', 'Note', '2026-01-01T00:00:00Z')",
    )
    .unwrap();
    db.set(AppStateKey::ActiveLabId, "a3f91c07").unwrap();

    db.execute("DELETE FROM labs WHERE id = 'a3f91c07'")
        .unwrap();
    for table in ["projects", "requests", "stories", "memories"] {
        let sql = match table {
            "projects" => "SELECT count(*) FROM projects",
            "requests" => "SELECT count(*) FROM requests",
            "stories" => "SELECT count(*) FROM stories",
            _ => "SELECT count(*) FROM memories",
        };
        assert_eq!(count(&db, sql), 0, "{table}");
    }
}

#[test]
fn memory_scope_check_requires_exactly_one_owner() {
    let (_dir, db) = migrated();
    for sql in [
        "INSERT INTO memories (id, scope_type, content, created_at) VALUES ('0b1c2d3e', 'lab', 'x', '2026-01-01T00:00:00Z')",
        "INSERT INTO memories (id, scope_type, lab_id, project_id, content, created_at) VALUES ('1c2d3e4f', 'lab', 'a3f91c07', '0b1c2d3e', 'x', '2026-01-01T00:00:00Z')",
        "INSERT INTO memories (id, scope_type, lab_id, content, created_at) VALUES ('2d3e4f50', 'project', 'a3f91c07', 'x', '2026-01-01T00:00:00Z')",
    ] {
        assert!(db.execute(sql).is_err(), "{sql}");
    }
}

#[test]
fn migration_from_v2_preserves_earlier_data() {
    let dir = support::private_tempdir();
    let db = Database::open(dir.path().join("vlab.db")).unwrap();
    let v2_only = support::private_tempdir();
    for name in ["0001_init.sql", "0002_labs_agents.sql"] {
        fs::copy(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("migrations")
                .join(name),
            v2_only.path().join(name),
        )
        .unwrap();
    }
    let migrator = db
        .block_on(sqlx::migrate::Migrator::new(v2_only.path()))
        .unwrap();
    db.migrate_with(&migrator, 2, vlab::obs::RunId::generate())
        .unwrap();
    db.execute(
        "INSERT INTO labs (id, name, created_at, updated_at) \
         VALUES ('a3f91c07', 'Preserved', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
    )
    .unwrap();

    let report = db.migrate(vlab::obs::RunId::generate()).unwrap();
    assert_eq!(report.to, SUPPORTED_SCHEMA_VERSION);
    assert_eq!(db.labs().unwrap().len(), 1);
    assert_eq!(db.labs().unwrap()[0].name, "Preserved");
}

#[test]
fn agent_counts_track_assigned_stories_and_projects() {
    let (_dir, db) = migrated();
    let lab = lab("a3f91c07", "Payments");
    db.insert_lab(&lab).unwrap();
    let project = project("0b1c2d3e", &lab.id, "Portal");
    db.insert_project(&project).unwrap();
    let agent = AgentId::parse("1c2d3e4f").unwrap();
    db.insert_agent(
        &AgentProfile::new(
            agent.clone(),
            Name::parse("Ada").unwrap(),
            Role::Dev,
            Description::parse("Writes code").unwrap(),
            skills::parse_list("rust").unwrap(),
            ProviderId::DeepSeek,
            ModelId::parse("deepseek-chat").unwrap(),
            None,
        )
        .unwrap(),
        std::slice::from_ref(&lab.id),
    )
    .unwrap();
    db.execute(
        "INSERT INTO requests (id, project_id, content, status, created_at, updated_at)          VALUES ('2d3e4f50', '0b1c2d3e', 'R', 'TODO', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
    )
    .unwrap();
    db.execute(
        "INSERT INTO stories (id, project_id, request_id, title, description, acceptance_criteria_json, priority, complexity, required_skills_json, assigned_agent_id, state, due_date, created_at, updated_at)          VALUES ('3e4f5061', '0b1c2d3e', '2d3e4f50', 'T', 'D', '[]', 'high', 'small', '[]', '1c2d3e4f', 'TODO', '2026-01-03', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
    )
    .unwrap();
    assert_eq!(db.agent_project_count(&agent).unwrap(), 1);
    assert_eq!(db.agent_open_story_count(&agent).unwrap(), 1);
    assert_eq!(db.agent_assigned_story_count(&agent).unwrap(), 1);
    assert_eq!(db.agent_completed_story_count(&agent).unwrap(), 0);
    assert_eq!(
        db.agent_project_names(&agent).unwrap(),
        vec![(project.id.clone(), "Portal".to_owned())]
    );
}

#[test]
fn migration_from_v3_adds_the_github_source_column() {
    let dir = support::private_tempdir();
    let db = Database::open(dir.path().join("vlab.db")).unwrap();
    let v3_only = support::private_tempdir();
    for name in [
        "0001_init.sql",
        "0002_labs_agents.sql",
        "0003_projects_requests_stories.sql",
    ] {
        fs::copy(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("migrations")
                .join(name),
            v3_only.path().join(name),
        )
        .unwrap();
    }
    let migrator = db
        .block_on(sqlx::migrate::Migrator::new(v3_only.path()))
        .unwrap();
    db.migrate_with(&migrator, 3, vlab::obs::RunId::generate())
        .unwrap();
    let lab = lab("a3f91c07", "Payments");
    db.execute(
        "INSERT INTO labs (id, name, created_at, updated_at) \
         VALUES ('a3f91c07', 'Payments', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
    )
    .unwrap();
    db.execute(
        "INSERT INTO projects (id, lab_id, name, created_at, updated_at) \
         VALUES ('0b1c2d3e', 'a3f91c07', 'Portal', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
    )
    .unwrap();

    let report = db.migrate(vlab::obs::RunId::generate()).unwrap();
    assert_eq!(report.to, SUPPORTED_SCHEMA_VERSION);
    let stored = db.projects(&lab.id).unwrap();
    assert_eq!(stored.len(), 1);
    assert_eq!(stored[0].github_source, None);
}

#[test]
fn an_unknown_project_is_missing() {
    let (_dir, db) = migrated();
    let missing = ProjectId::parse("ffffffff").unwrap();
    assert!(db.project(&missing).unwrap().is_none());
    assert!(
        db.project_in_lab(&missing, &LabId::parse("a3f91c07").unwrap())
            .unwrap()
            .is_none()
    );
}

#[test]
fn project_names_are_constrained() {
    let (_dir, db) = migrated();
    let error = db.execute(
        "INSERT INTO projects (id, lab_id, name, created_at, updated_at) \
         VALUES ('0b1c2d3e', 'a3f91c07', '   ', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
    );
    assert!(error.is_err());
}
