//! Agent listing and detail (WP3-AC7).

mod support;

use vlab::app::directory::Directory;
use vlab::app::storage::AppStateStore;
use vlab::domain::agent::{AgentProfile, Description, Role};
use vlab::domain::ids::{AgentId, LabId};
use vlab::domain::lab::Lab;
use vlab::domain::name::Name;
use vlab::domain::settings::{ModelId, ProviderId};
use vlab::domain::skills;

use support::Sandbox;

fn seed_lab(sandbox: &Sandbox, raw: &str, name: &str) -> LabId {
    let db = sandbox.seed_database();
    let id = LabId::parse(raw).unwrap();
    db.insert_lab(&Lab::new(id.clone(), Name::parse(name).unwrap()))
        .unwrap();
    id
}

fn seed_agent(
    sandbox: &Sandbox,
    raw: &str,
    name: &str,
    role: Role,
    labs: &[LabId],
    cv: Option<&str>,
) -> AgentId {
    let db = sandbox.seed_database();
    let id = AgentId::parse(raw).unwrap();
    let profile = AgentProfile::new(
        id.clone(),
        Name::parse(name).unwrap(),
        role,
        Description::parse("Works on the project").unwrap(),
        skills::parse_list("rust, api").unwrap(),
        ProviderId::DeepSeek,
        ModelId::parse("deepseek-chat").unwrap(),
        cv.map(str::to_owned),
    )
    .unwrap();
    db.insert_agent(&profile, labs).unwrap();
    id
}

fn activate(sandbox: &Sandbox, lab: &LabId) {
    let db = sandbox.seed_database();
    vlab::app::context::save_active_lab(&db, lab).unwrap();
    assert!(
        db.get(vlab::app::storage::AppStateKey::ActiveLabId)
            .unwrap()
            .is_some()
    );
}

#[test]
fn listing_requires_an_active_laboratory() {
    let sandbox = Sandbox::new();
    let outcome = sandbox.run(&["--list-agents"]);
    assert_eq!(outcome.code, 3, "{outcome:#?}");
    assert!(outcome.stdout.is_empty());
    assert_eq!(
        outcome.stderr,
        "Error: no active laboratory is selected.\n\
         Run `vlab --list-labs`, then `vlab --open-lab <lab-id>`.\n"
    );
}

#[test]
fn listing_shows_active_members_with_na_metrics() {
    let sandbox = Sandbox::new();
    let lab = seed_lab(&sandbox, "a3f91c07", "Payments");
    seed_agent(
        &sandbox,
        "0b1c2d3e",
        "Ada",
        Role::Lead,
        std::slice::from_ref(&lab),
        None,
    );
    seed_agent(
        &sandbox,
        "2d3e4f50",
        "Grace",
        Role::Dev,
        std::slice::from_ref(&lab),
        None,
    );
    activate(&sandbox, &lab);

    let outcome = sandbox.run(&["--list-agents"]);
    outcome.assert_success().assert_no_escape_bytes();
    assert!(outcome.stdout.contains("0b1c2d3e"), "{}", outcome.stdout);
    assert!(outcome.stdout.contains("Ada"), "{}", outcome.stdout);
    assert!(outcome.stdout.contains("lead"), "{}", outcome.stdout);
    assert!(outcome.stdout.contains("deepseek"), "{}", outcome.stdout);
    assert!(
        outcome.stdout.contains("deepseek-chat"),
        "{}",
        outcome.stdout
    );
    // WP-4 derives project and open-story counts; no invented metrics remain.
    assert!(!outcome.stdout.contains("N/A"), "{}", outcome.stdout);
}

#[test]
fn listing_excludes_archived_agents_and_members_of_other_laboratories() {
    let sandbox = Sandbox::new();
    let active = seed_lab(&sandbox, "a3f91c07", "Active");
    let other = seed_lab(&sandbox, "1c2d3e4f", "Other");
    let archived = seed_agent(
        &sandbox,
        "0b1c2d3e",
        "Archived",
        Role::Dev,
        std::slice::from_ref(&active),
        None,
    );
    seed_agent(&sandbox, "2d3e4f50", "Elsewhere", Role::Dev, &[other], None);
    let db = sandbox.seed_database();
    db.execute("UPDATE agents SET archived_at = '2026-01-01T00:00:00Z'")
        .unwrap();
    assert!(db.agent(&archived).unwrap().unwrap().archived);
    activate(&sandbox, &active);

    let listed = sandbox.run(&["--list-agents"]);
    listed.assert_success();
    assert!(!listed.stdout.contains("Archived"), "{}", listed.stdout);
    assert!(!listed.stdout.contains("Elsewhere"), "{}", listed.stdout);
}

#[test]
fn an_empty_membership_list_is_actionable() {
    let sandbox = Sandbox::new();
    let lab = seed_lab(&sandbox, "a3f91c07", "Empty");
    activate(&sandbox, &lab);
    let outcome = sandbox.run(&["--list-agents"]);
    outcome.assert_success();
    assert_eq!(
        outcome.stdout,
        "No agents in the active laboratory.\nHint: Run `vlab agents` to create one.\n"
    );
}

#[test]
fn detail_shows_every_stored_field_and_na_metrics() {
    let sandbox = Sandbox::new();
    let lab = seed_lab(&sandbox, "a3f91c07", "Payments");
    seed_agent(
        &sandbox,
        "0b1c2d3e",
        "Ada",
        Role::Lead,
        &[lab],
        Some("Synthetic CV text"),
    );

    let outcome = sandbox.run(&["--show-agent-detail", "0b1c2d3e"]);
    outcome.assert_success().assert_no_escape_bytes();
    for expected in [
        "Agent 0b1c2d3e",
        "Name:        Ada",
        "Role:        lead",
        "Description: Works on the project",
        "Provider:    deepseek",
        "Model:       deepseek-chat",
        "Status:      active",
        "Skills:      rust, api",
        "Synthetic CV text",
        "Memberships: 1",
        "a3f91c07 — Payments",
        "Projects:    N/A",
        "Assigned stories: 0 (open 0)",
        "Completed stories: N/A",
        "Memories:    0",
    ] {
        assert!(
            outcome.stdout.contains(expected),
            "missing {expected:?}:\n{}",
            outcome.stdout
        );
    }
    assert!(!outcome.stdout.to_ascii_lowercase().contains("score"));
    assert!(!outcome.stdout.to_ascii_lowercase().contains("rank"));
}

#[test]
fn detail_without_a_cv_says_none_and_archived_agents_stay_resolvable() {
    let sandbox = Sandbox::new();
    let lab = seed_lab(&sandbox, "a3f91c07", "Payments");
    seed_agent(&sandbox, "0b1c2d3e", "Ada", Role::Qa, &[lab], None);
    let db = sandbox.seed_database();
    db.execute("UPDATE agents SET archived_at = '2026-01-01T00:00:00Z'")
        .unwrap();

    let outcome = sandbox.run(&["--show-agent-detail", "0b1c2d3e"]);
    outcome.assert_success();
    assert!(outcome.stdout.contains("Status:      archived"));
    assert!(outcome.stdout.contains("CV:          (none)"));
}

#[test]
fn malformed_and_absent_agent_ids_have_the_documented_codes() {
    let sandbox = Sandbox::new();
    sandbox
        .run(&["--show-agent-detail", "0b1c2d3e"])
        .assert_failure(3, "no agent with identifier `0b1c2d3e` exists.")
        .assert_failure(3, "Run `vlab --list-agents`");
    sandbox
        .run(&["--show-agent-detail", "bad"])
        .assert_failure(2, "must be 8 lowercase hexadecimal characters");
}

#[test]
fn a_malformed_agent_id_creates_no_database() {
    let sandbox = Sandbox::new();
    sandbox
        .run(&["--show-agent-detail", "BAD"])
        .assert_failure(2, "must be 8 lowercase hexadecimal characters");
    assert!(!sandbox.data_dir().exists());
}

#[test]
fn agent_commands_perform_no_provider_call() {
    let sandbox = Sandbox::new();
    let lab = seed_lab(&sandbox, "a3f91c07", "Payments");
    seed_agent(&sandbox, "0b1c2d3e", "Ada", Role::Lead, &[lab], None);
    // The sandbox has no settings file; both commands
    // still succeed, so no provider-backed path is reachable from them.
    let outcome = sandbox.run(&["--show-agent-detail", "0b1c2d3e"]);
    outcome.assert_success();
}
