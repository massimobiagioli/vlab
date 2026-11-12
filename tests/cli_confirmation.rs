//! Non-interactive confirmation behavior for every destructive command
//! (WP4-AC5).

mod support;

use vlab::app::context::save_active_lab;
use vlab::app::directory::Directory;
use vlab::app::memories::MemoryStore;
use vlab::app::projects::ProjectStore;
use vlab::domain::agent::{AgentProfile, Description, Role};
use vlab::domain::ids::{AgentId, LabId, MemoryId, ProjectId};
use vlab::domain::lab::Lab;
use vlab::domain::memory::{Memory, MemoryScope};
use vlab::domain::name::Name;
use vlab::domain::project::Project;
use vlab::domain::settings::{ModelId, ProviderId};
use vlab::domain::skills;

use support::Sandbox;

fn seed_lab(sandbox: &Sandbox) -> vlab::infra::db::Database {
    let db = sandbox.seed_database();
    let lab = LabId::parse("a3f91c07").unwrap();
    db.insert_lab(&Lab::new(lab.clone(), Name::parse("Payments").unwrap()))
        .unwrap();
    save_active_lab(&db, &lab).unwrap();
    db
}

#[test]
fn a_non_interactive_invocation_without_yes_fails_closed() {
    let sandbox = Sandbox::new();
    let db = seed_lab(&sandbox);
    let lab = LabId::parse("a3f91c07").unwrap();
    db.insert_project(&Project::new(
        ProjectId::parse("0b1c2d3e").unwrap(),
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
    db.insert_memory(
        &Memory::new(
            MemoryId::parse("2d3e4f50").unwrap(),
            MemoryScope::Agent(agent),
            "history",
        )
        .unwrap(),
    )
    .unwrap();

    sandbox
        .run(&["--delete-lab", "a3f91c07"])
        .assert_failure(4, "requires confirmation");
    sandbox
        .run(&["--delete-project", "0b1c2d3e"])
        .assert_failure(4, "requires confirmation");
    sandbox
        .run(&["--delete-agent", "1c2d3e4f"])
        .assert_failure(4, "requires confirmation");
    sandbox
        .run(&["--delete-memory", "2d3e4f50"])
        .assert_failure(4, "requires confirmation");
}

#[test]
fn yes_executes_and_changes_only_the_target() {
    let sandbox = Sandbox::new();
    let db = seed_lab(&sandbox);
    let lab = LabId::parse("a3f91c07").unwrap();
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
    let memory = MemoryId::parse("2d3e4f50").unwrap();
    db.insert_memory(
        &Memory::new(memory.clone(), MemoryScope::Agent(agent.clone()), "history").unwrap(),
    )
    .unwrap();

    sandbox
        .run(&["--delete-memory", memory.as_str(), "--yes"])
        .assert_success();
    assert!(db.memory(&memory).unwrap().is_none());
    sandbox
        .run(&["--delete-project", project.as_str(), "--yes"])
        .assert_success();
    assert!(db.project(&project).unwrap().is_none());
    let archived = sandbox.run(&["--delete-agent", agent.as_str(), "--yes"]);
    assert_eq!(archived.code, 0, "{archived:#?}");
    assert!(db.agent(&agent).unwrap().unwrap().archived);
    sandbox
        .run(&["--delete-lab", lab.as_str(), "--yes"])
        .assert_success();
    assert!(db.labs().unwrap().is_empty());
}
