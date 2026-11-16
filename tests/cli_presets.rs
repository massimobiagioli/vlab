//! Preset catalog loading (ADR-0052).

mod support;

use std::path::{Path, PathBuf};

use support::Sandbox;
use vlab::app::directory::Directory;

fn catalog() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/agents.yaml")
}

fn settings(sandbox: &Sandbox) {
    sandbox.write_settings("provider: deepseek\nmodel: deepseek-chat\n");
}

#[test]
fn the_tracked_catalog_creates_thirty_profiles_without_memberships() {
    let sandbox = Sandbox::new();
    settings(&sandbox);
    let outcome = sandbox.run(&["--load-preset", catalog().to_str().unwrap()]);
    outcome.assert_success().assert_no_escape_bytes();
    assert!(outcome.stdout.contains("Created: 30"), "{}", outcome.stdout);
    assert!(
        outcome.stdout.contains("Klaus Vandenberg"),
        "{}",
        outcome.stdout
    );

    let db = sandbox.seed_database();
    let agents = db.all_agents().unwrap();
    assert_eq!(agents.len(), 30);
    assert!(agents.iter().all(|agent| !agent.archived));
    for agent in &agents {
        assert!(
            db.agent_memberships(&agent.id).unwrap().is_empty(),
            "{} was assigned to a laboratory",
            agent.name
        );
    }
}

#[test]
fn a_second_load_skips_every_existing_profile() {
    let sandbox = Sandbox::new();
    settings(&sandbox);
    let first = sandbox.run(&["--load-preset", catalog().to_str().unwrap()]);
    first.assert_success();
    let second = sandbox.run(&["--load-preset", catalog().to_str().unwrap()]);
    second.assert_success();
    assert!(second.stdout.contains("Created: 0"), "{}", second.stdout);
    assert!(second.stdout.contains("skipped: 30"), "{}", second.stdout);
    assert_eq!(sandbox.seed_database().all_agents().unwrap().len(), 30);
}

#[test]
fn a_missing_catalog_is_a_precondition_and_creates_no_database() {
    let sandbox = Sandbox::new();
    settings(&sandbox);
    sandbox
        .run(&["--load-preset", "/nonexistent/vlab/agents.yaml"])
        .assert_failure(4, "does not exist");
    assert!(!sandbox.data_dir().exists());
}

#[test]
fn a_malformed_catalog_is_a_validation_error_and_creates_no_database() {
    let sandbox = Sandbox::new();
    settings(&sandbox);
    let bad = sandbox.root().join("bad-agents.yaml");
    std::fs::write(
        &bad,
        "version: 1\nagents:\n  - name: Broken\n    role: boss\n    description: D\n    skills: [rust]\n",
    )
    .unwrap();
    sandbox
        .run(&["--load-preset", bad.to_str().unwrap()])
        .assert_failure(4, "role is invalid");
    assert!(!sandbox.data_dir().exists());
}

#[test]
fn the_loader_requires_settings_but_not_a_laboratory() {
    let sandbox = Sandbox::new();
    let outcome = sandbox.run(&["--load-preset", catalog().to_str().unwrap()]);
    outcome.assert_failure(2, "settings");
    assert!(!sandbox.data_dir().exists());
}
