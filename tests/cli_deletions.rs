//! Destructive commands, confirmation, cascades, and the planning guard
//! (WP4-AC5–AC8).

mod support;

use vlab::app::context::save_active_lab;
use vlab::app::deletions::DeletionStore;
use vlab::app::directory::Directory;
use vlab::app::memories::MemoryStore;
use vlab::app::projects::ProjectStore;
use vlab::app::storage::{AppStateKey, AppStateStore};
use vlab::domain::agent::{AgentProfile, Description, Role};
use vlab::domain::ids::{AgentId, LabId, ProjectId};
use vlab::domain::lab::Lab;
use vlab::domain::memory::{Memory, MemoryScope};
use vlab::domain::name::Name;
use vlab::domain::project::Project;
use vlab::domain::settings::{ModelId, ProviderId};
use vlab::domain::skills;
use vlab::infra::db::Database;

use support::Sandbox;

fn exec(db: &Database, sql: String) {
    db.block_on(async {
        sqlx::query(sqlx::AssertSqlSafe(sql))
            .execute(db.pool())
            .await
            .unwrap();
    });
}

fn seed(sandbox: &Sandbox) -> (LabId, ProjectId, AgentId) {
    let db = sandbox.seed_database();
    let lab = LabId::parse("a3f91c07").unwrap();
    db.insert_lab(&Lab::new(lab.clone(), Name::parse("Payments").unwrap()))
        .unwrap();
    save_active_lab(&db, &lab).unwrap();
    let project = ProjectId::parse("0b1c2d3e").unwrap();
    db.insert_project(&Project::new(
        project.clone(),
        lab.clone(),
        Name::parse("Portal").unwrap(),
        None,
    ))
    .unwrap();
    let agent = AgentId::parse("1c2d3e4f").unwrap();
    db.insert_agent(
        &AgentProfile::new(
            agent.clone(),
            Name::parse("Ada").unwrap(),
            Role::Lead,
            Description::parse("Leads").unwrap(),
            skills::parse_list("").unwrap(),
            ProviderId::DeepSeek,
            ModelId::parse("deepseek-chat").unwrap(),
            None,
        )
        .unwrap(),
        std::slice::from_ref(&lab),
    )
    .unwrap();
    (lab, project, agent)
}

fn count(db: &Database, sql: &'static str) -> i64 {
    db.block_on(async { sqlx::query_scalar(sql).fetch_one(db.pool()).await.unwrap() })
}

#[test]
fn laboratory_deletion_is_atomic_cascading_and_clears_the_selection() {
    let sandbox = Sandbox::new();
    let (lab, _project, agent) = seed(&sandbox);
    let db = sandbox.seed_database();
    db.insert_memory(
        &Memory::new(
            vlab::domain::ids::MemoryId::generate(),
            MemoryScope::Lab(lab.clone()),
            "note",
        )
        .unwrap(),
    )
    .unwrap();

    sandbox
        .run(&["--delete-lab", lab.as_str()])
        .assert_failure(4, "requires confirmation");
    assert_eq!(count(&db, "SELECT count(*) FROM labs"), 1);

    sandbox
        .run(&["--delete-lab", lab.as_str(), "--yes"])
        .assert_success();
    assert_eq!(count(&db, "SELECT count(*) FROM labs"), 0);
    assert_eq!(count(&db, "SELECT count(*) FROM projects"), 0);
    assert_eq!(count(&db, "SELECT count(*) FROM lab_memberships"), 0);
    assert_eq!(count(&db, "SELECT count(*) FROM memories"), 0);
    assert_eq!(count(&db, "SELECT count(*) FROM agents"), 1);
    assert!(db.agent(&agent).unwrap().is_some());
    assert_eq!(db.get(AppStateKey::ActiveLabId).unwrap(), None);
}

#[test]
fn project_deletion_is_scoped_and_cascades_its_data() {
    let sandbox = Sandbox::new();
    let (_lab, project, _agent) = seed(&sandbox);
    let db = sandbox.seed_database();
    exec(
        &db,
        "INSERT INTO memories (id, scope_type, project_id, content, created_at) \
         VALUES ('2d3e4f50', 'project', '0b1c2d3e', 'note', '2026-01-01T00:00:00Z')"
            .to_owned(),
    );
    sandbox
        .run(&["--delete-project", project.as_str(), "--yes"])
        .assert_success();
    assert_eq!(count(&db, "SELECT count(*) FROM projects"), 0);
    assert_eq!(count(&db, "SELECT count(*) FROM memories"), 0);
    assert_eq!(count(&db, "SELECT count(*) FROM agents"), 1);
}

#[test]
fn a_second_laboratory_cannot_delete_the_first_projects() {
    let sandbox = Sandbox::new();
    let (_lab, project, _agent) = seed(&sandbox);
    let db = sandbox.seed_database();
    let other = LabId::parse("ffffffff").unwrap();
    db.insert_lab(&Lab::new(other.clone(), Name::parse("Other").unwrap()))
        .unwrap();
    save_active_lab(&db, &other).unwrap();
    sandbox
        .run(&["--delete-project", project.as_str(), "--yes"])
        .assert_failure(3, "no project with identifier");
}

#[test]
fn agent_deletion_blocks_unfinished_stories_and_archives_otherwise() {
    let sandbox = Sandbox::new();
    let (_lab, _project, agent) = seed(&sandbox);
    let db = sandbox.seed_database();
    exec(
        &db,
        "INSERT INTO requests (id, project_id, content, status, created_at, updated_at) \
         VALUES ('2d3e4f50', '0b1c2d3e', 'R', 'TODO', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')".to_owned(),
    );
    exec(
        &db,
        "INSERT INTO stories (id, project_id, request_id, title, description, acceptance_criteria_json, priority, complexity, required_skills_json, assigned_agent_id, state, due_date, created_at, updated_at) \
         VALUES ('3e4f5061', '0b1c2d3e', '2d3e4f50', 'T', 'D', '[]', 'high', 'small', '[]', '1c2d3e4f', 'TODO', '2026-01-03', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')".to_owned(),
    );
    sandbox
        .run(&["--delete-agent", agent.as_str(), "--yes"])
        .assert_failure(4, "unfinished assigned stories");

    exec(&db, "UPDATE stories SET state = 'DONE'".to_owned());
    db.insert_memory(
        &Memory::new(
            vlab::domain::ids::MemoryId::generate(),
            MemoryScope::Agent(agent.clone()),
            "history",
        )
        .unwrap(),
    )
    .unwrap();
    let outcome = sandbox.run(&["--delete-agent", agent.as_str(), "--yes"]);
    assert_eq!(outcome.code, 0, "{outcome:#?}");
    assert!(outcome.stdout.contains("archived"));
    assert!(
        outcome
            .stderr
            .contains("Warning: this agent is a Tech Lead")
    );
    assert!(db.agent(&agent).unwrap().unwrap().archived);
    assert_eq!(count(&db, "SELECT count(*) FROM lab_memberships"), 0);
    assert_eq!(count(&db, "SELECT count(*) FROM memories"), 1);
    assert_eq!(count(&db, "SELECT count(*) FROM agents"), 1);
}

#[test]
fn active_planning_blocks_deletion_and_stale_planning_does_not() {
    let sandbox = Sandbox::new();
    let (lab, project, agent) = seed(&sandbox);
    let db = sandbox.seed_database();
    exec(
        &db,
        "INSERT INTO requests (id, project_id, content, status, planning_started_at, created_at, updated_at) \
         VALUES ('2d3e4f50', '0b1c2d3e', 'R', 'PLANNING', strftime('%Y-%m-%dT%H:%M:%fZ','now'), '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')".to_owned(),
    );
    sandbox
        .run(&["--delete-project", project.as_str(), "--yes"])
        .assert_failure(4, "planning operation is active");
    sandbox
        .run(&["--delete-lab", lab.as_str(), "--yes"])
        .assert_failure(4, "planning operation is active");
    sandbox
        .run(&["--delete-agent", agent.as_str(), "--yes"])
        .assert_failure(4, "active planning");

    exec(
        &db,
        "UPDATE requests SET planning_started_at = '2000-01-01T00:00:00Z'".to_owned(),
    );
    sandbox
        .run(&["--delete-project", project.as_str(), "--yes"])
        .assert_success();
}

#[test]
fn a_changed_snapshot_aborts_deletion() {
    let sandbox = Sandbox::new();
    let (lab, _project, _agent) = seed(&sandbox);
    let db = sandbox.seed_database();
    let plan = db.lab_plan(&lab).unwrap();
    db.insert_project(&Project::new(
        ProjectId::parse("ffffffff").unwrap(),
        lab.clone(),
        Name::parse("New").unwrap(),
        None,
    ))
    .unwrap();
    let error = db.delete_lab(&plan).unwrap_err();
    assert!(
        matches!(error, vlab::app::AppError::Precondition { .. }),
        "{error:?}"
    );
    assert_eq!(count(&db, "SELECT count(*) FROM labs"), 1);
}
