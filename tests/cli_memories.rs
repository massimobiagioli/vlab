//! Automatic memory creation (ADR-0035, ADR-0036).

mod support;

use vlab::app::context::{save_active_lab, save_active_project};
use vlab::app::directory::Directory;
use vlab::app::projects::ProjectStore;
use vlab::domain::agent::{AgentProfile, Description, Role};
use vlab::domain::ids::{AgentId, LabId, ProjectId};
use vlab::domain::lab::Lab;
use vlab::domain::name::Name;
use vlab::domain::project::Project;
use vlab::domain::settings::{ModelId, ProviderId};
use vlab::domain::skills;

use support::Sandbox;

fn seed_lab(sandbox: &Sandbox) -> LabId {
    let db = sandbox.seed_database();
    let lab = LabId::parse("a3f91c07").unwrap();
    db.insert_lab(&Lab::new(lab.clone(), Name::parse("Payments").unwrap()))
        .unwrap();
    save_active_lab(&db, &lab).unwrap();
    lab
}

fn seed_agent(sandbox: &Sandbox, lab: &LabId, id: &str, archived: bool) {
    let db = sandbox.seed_database();
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
    db.insert_agent(&profile, std::slice::from_ref(lab))
        .unwrap();
    if archived {
        db.block_on(async {
            sqlx::query("UPDATE agents SET archived_at = '2026-01-01T00:00:00Z' WHERE id = ?1")
                .bind(profile.id.as_str())
                .execute(db.pool())
                .await
                .unwrap();
        });
    }
}

#[test]
fn creating_a_laboratory_writes_a_lab_memory() {
    let sandbox = Sandbox::new();
    let outcome = sandbox.run(&["--new-lab", "Payments"]);
    outcome.assert_success();
    let db = sandbox.seed_database();
    let (scope, content): (String, String) = db.block_on(async {
        sqlx::query_as("SELECT scope_type, content FROM memories")
            .fetch_one(db.pool())
            .await
            .unwrap()
    });
    assert_eq!(scope, "lab");
    assert_eq!(content, "Laboratory 'Payments' was created.");
}

#[test]
fn creating_a_project_writes_a_project_memory_and_selects_it() {
    let sandbox = Sandbox::new();
    seed_lab(&sandbox);
    let created = sandbox.run(&["--new-project", "Portal"]);
    created.assert_success();
    let id = created
        .stdout
        .lines()
        .find_map(|line| line.strip_prefix("ID:   "))
        .unwrap()
        .trim()
        .to_owned();
    let db = sandbox.seed_database();
    let (scope, content): (String, String) = db.block_on(async {
        sqlx::query_as("SELECT scope_type, content FROM memories")
            .fetch_one(db.pool())
            .await
            .unwrap()
    });
    assert_eq!(scope, "project");
    assert_eq!(content, "Project 'Portal' was created.");

    // --new-project selected the project, so the board works without --open-project.
    sandbox.run(&["--show-project-kanban"]).assert_success();
    let count: i64 = db.block_on(async {
        sqlx::query_scalar("SELECT count(*) FROM projects WHERE id = ?1")
            .bind(&id)
            .fetch_one(db.pool())
            .await
            .unwrap()
    });
    assert_eq!(count, 1);
}

#[test]
fn assign_agent_is_idempotent_and_requires_a_member() {
    let sandbox = Sandbox::new();
    let lab = seed_lab(&sandbox);
    let project = ProjectId::parse("0b1c2d3e").unwrap();
    let db = sandbox.seed_database();
    db.insert_project(&Project::new(
        project.clone(),
        lab.clone(),
        Name::parse("Portal").unwrap(),
        None,
    ))
    .unwrap();
    save_active_project(&db, &project).unwrap();
    seed_agent(&sandbox, &lab, "1c2d3e4f", false);

    sandbox
        .run(&["--assign-agent", "1c2d3e4f"])
        .assert_success();
    sandbox
        .run(&["--assign-agent", "1c2d3e4f"])
        .assert_success();
    let count: i64 = db.block_on(async {
        sqlx::query_scalar("SELECT count(*) FROM project_agents")
            .fetch_one(db.pool())
            .await
            .unwrap()
    });
    assert_eq!(count, 1);

    sandbox
        .run(&["--assign-agent", "ffffffff"])
        .assert_failure(3, "no agent with identifier");
}

#[test]
fn an_archived_agent_cannot_be_assigned() {
    let sandbox = Sandbox::new();
    let lab = seed_lab(&sandbox);
    let project = ProjectId::parse("0b1c2d3e").unwrap();
    let db = sandbox.seed_database();
    db.insert_project(&Project::new(
        project.clone(),
        lab.clone(),
        Name::parse("Portal").unwrap(),
        None,
    ))
    .unwrap();
    save_active_project(&db, &project).unwrap();
    seed_agent(&sandbox, &lab, "1c2d3e4f", true);

    sandbox
        .run(&["--assign-agent", "1c2d3e4f"])
        .assert_failure(4, "archived");
}
