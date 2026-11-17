//! Agent definitions from markdown files (ADR-0060).

mod support;

use support::Sandbox;
use vlab::app::agents;
use vlab::app::context::save_active_lab;
use vlab::app::directory::Directory;
use vlab::app::memories::MemoryStore;
use vlab::domain::agent::Role;
use vlab::domain::ids::LabId;
use vlab::domain::lab::Lab;
use vlab::domain::name::Name;
use vlab::domain::settings::Settings;
use vlab::infra::agent_definition;
use vlab::infra::db::Database;

const DEFINITION: &str = "\
---
name: Sven Drijkoningen
role: dev
description: Full-stack developer
skills: [rust, typescript]
---
# Profile

Fictional full-stack developer.
";

fn definition_file(sandbox: &Sandbox, name: &str, content: &str) -> std::path::PathBuf {
    let path = sandbox.root().join(name);
    std::fs::write(&path, content).unwrap();
    path
}

fn migrated(sandbox: &Sandbox) -> Database {
    sandbox.seed_database()
}

fn active_lab(db: &Database) -> LabId {
    let lab = LabId::parse("a3f91c07").unwrap();
    db.insert_lab(&Lab::new(lab.clone(), Name::parse("Payments").unwrap()))
        .unwrap();
    save_active_lab(db, &lab).unwrap();
    lab
}

#[test]
fn a_definition_creates_a_profile_with_the_body_as_cv() {
    let sandbox = Sandbox::new();
    let path = definition_file(&sandbox, "agent.md", DEFINITION);
    let definition = agent_definition::load_agent_definition(&path).unwrap();
    let db = migrated(&sandbox);
    let lab = active_lab(&db);
    let created = agents::create_from_definition(
        &db,
        &db,
        &definition,
        &Settings::parse("deepseek", "deepseek-chat").unwrap(),
    )
    .unwrap();
    assert_eq!(created.profile.name.as_str(), "Sven Drijkoningen");
    assert_eq!(created.profile.role, Role::Dev);
    assert_eq!(created.profile.skills.len(), 2);
    assert!(
        created
            .profile
            .cv
            .as_deref()
            .unwrap()
            .contains("Fictional full-stack developer.")
    );
    assert_eq!(created.memberships, vec![lab.clone()]);
    let stored = db.agent(&created.profile.id).unwrap().unwrap();
    assert_eq!(stored.profile().unwrap(), created.profile);
    assert_eq!(
        db.agent_memberships(&created.profile.id)
            .unwrap()
            .iter()
            .map(|lab| lab.id.clone())
            .collect::<Vec<_>>(),
        vec![lab]
    );
}

#[test]
fn a_second_lead_is_rejected_and_the_catalog_can_be_used_without_a_lab() {
    let sandbox = Sandbox::new();
    let db = migrated(&sandbox);
    let lab = active_lab(&db);
    let lead_definition = definition_file(
        &sandbox,
        "lead.md",
        "---\nname: Lead\nrole: lead\ndescription: Leads\nskills: []\n---\n",
    );
    let definition = agent_definition::load_agent_definition(&lead_definition).unwrap();
    agents::create_from_definition(
        &db,
        &db,
        &definition,
        &Settings::parse("deepseek", "deepseek-chat").unwrap(),
    )
    .unwrap();
    let error = agents::create_from_definition(
        &db,
        &db,
        &definition,
        &Settings::parse("deepseek", "deepseek-chat").unwrap(),
    )
    .unwrap_err();
    assert!(matches!(error, vlab::app::AppError::Validation { .. }));
    assert_eq!(db.lab_agents(&lab).unwrap().len(), 1);

    // Clear the active selection: a definition then creates a catalog profile.
    vlab::app::context::clear_active_lab(&db).unwrap();
    let dev_definition = definition_file(
        &sandbox,
        "dev.md",
        "---\nname: Catalog Dev\nrole: dev\ndescription: Works\nskills: [rust]\n---\n",
    );
    let definition = agent_definition::load_agent_definition(&dev_definition).unwrap();
    let created = agents::create_from_definition(
        &db,
        &db,
        &definition,
        &Settings::parse("deepseek", "deepseek-chat").unwrap(),
    )
    .unwrap();
    assert!(created.memberships.is_empty());
    assert!(
        db.agent_memberships(&created.profile.id)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn the_definition_must_be_markdown_and_well_formed() {
    let sandbox = Sandbox::new();
    let text = definition_file(&sandbox, "agent.txt", DEFINITION);
    assert!(matches!(
        agent_definition::load_agent_definition(&text),
        Err(vlab::app::AppError::Usage { .. })
    ));
    let invalid = definition_file(
        &sandbox,
        "bad.md",
        "---\nname: Ada\nrole: boss\ndescription: D\nskills: [rust]\n---\n",
    );
    assert!(matches!(
        agent_definition::load_agent_definition(&invalid),
        Err(vlab::app::AppError::Validation { .. })
    ));
    assert!(matches!(
        agent_definition::load_agent_definition(sandbox.root().join("missing.md").as_path()),
        Err(vlab::app::AppError::Precondition { .. })
    ));
}

#[test]
fn the_cli_creates_the_agent_and_the_structural_memory() {
    let sandbox = Sandbox::new();
    let path = definition_file(&sandbox, "agent.md", DEFINITION);
    sandbox.write_settings("provider: deepseek\nmodel: deepseek-chat\n");
    let db = migrated(&sandbox);
    let lab = active_lab(&db);
    let outcome = sandbox.run(&["--create-agent-from-file", path.to_str().unwrap()]);
    outcome.assert_success().assert_no_escape_bytes();
    assert!(
        outcome.stdout.contains("Agent created from definition."),
        "{}",
        outcome.stdout
    );
    let db = sandbox.seed_database();
    let agents = db.all_agents().unwrap();
    let agent = agents
        .iter()
        .find(|agent| agent.name == "Sven Drijkoningen")
        .expect("created agent");
    assert_eq!(
        db.agent_memberships(&agent.id)
            .unwrap()
            .iter()
            .map(|lab| lab.id.clone())
            .collect::<Vec<_>>(),
        vec![lab]
    );
    let memories = db
        .memories(&vlab::domain::memory::MemoryScope::Agent(agent.id.clone()))
        .unwrap();
    assert_eq!(memories.len(), 1);
}
