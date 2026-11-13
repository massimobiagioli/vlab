//! Group subcommands, plain fallback, and non-TTY behavior (ADR-0038).

mod support;

use vlab::app::context::save_active_lab;
use vlab::app::directory::Directory;
use vlab::domain::ids::LabId;
use vlab::domain::lab::Lab;
use vlab::domain::name::Name;

use support::Sandbox;

fn seed_lab(sandbox: &Sandbox) {
    let db = sandbox.seed_database();
    let lab = LabId::parse("a3f91c07").unwrap();
    db.insert_lab(&Lab::new(lab.clone(), Name::parse("Payments").unwrap()))
        .unwrap();
    save_active_lab(&db, &lab).unwrap();
}

#[test]
fn labs_plain_lists_the_laboratories() {
    let sandbox = Sandbox::new();
    seed_lab(&sandbox);
    let outcome = sandbox.run(&["labs", "--plain"]);
    outcome.assert_success().assert_no_escape_bytes();
    assert!(outcome.stdout.contains("Payments"), "{}", outcome.stdout);
    assert!(outcome.stdout.contains("a3f91c07"), "{}", outcome.stdout);
}

#[test]
fn non_tty_group_commands_fall_back_to_plain_output() {
    let sandbox = Sandbox::new();
    seed_lab(&sandbox);
    let outcome = sandbox.run(&["labs"]);
    outcome.assert_success().assert_no_escape_bytes();
    assert!(outcome.stdout.contains("Payments"), "{}", outcome.stdout);
}

#[test]
fn settings_plain_uses_the_settings_report() {
    let sandbox = Sandbox::new();
    sandbox.write_settings("provider: deepseek\nmodel: deepseek-chat\n");
    let outcome = sandbox.run(&["settings", "--plain"]);
    outcome.assert_success().assert_no_escape_bytes();
    assert!(
        outcome.stdout.contains("Settings file:"),
        "{}",
        outcome.stdout
    );
    assert!(outcome.stdout.contains("Model:             deepseek-chat"));
}

#[test]
fn project_and_agent_plain_lists_are_stable() {
    let sandbox = Sandbox::new();
    seed_lab(&sandbox);
    let projects = sandbox.run(&["projects", "--plain"]);
    projects.assert_success().assert_no_escape_bytes();
    assert!(
        projects.stdout.contains("No projects"),
        "{}",
        projects.stdout
    );
    let agents = sandbox.run(&["agents", "--plain"]);
    agents.assert_success().assert_no_escape_bytes();
    assert!(agents.stdout.contains("No agents"), "{}", agents.stdout);
}

#[test]
fn group_subcommands_reject_action_flags() {
    let sandbox = Sandbox::new();
    for args in [
        &["labs", "--list-labs"][..],
        &["projects", "--new-project", "Portal"],
        &["agents", "--yes"],
        &["settings", "--show-settings"],
    ] {
        let outcome = sandbox.run(args);
        assert_eq!(outcome.code, 2, "{args:?}: {outcome:#?}");
        assert!(outcome.stdout.is_empty(), "{args:?}: {outcome:#?}");
        assert!(!outcome.stderr.is_empty(), "{args:?}: {outcome:#?}");
    }
}

#[test]
fn bare_invocation_without_a_terminal_prints_help() {
    let sandbox = Sandbox::new();
    let outcome = sandbox.run(&[]);
    outcome.assert_success().assert_no_escape_bytes();
    assert!(outcome.stdout.contains("Usage: vlab"), "{}", outcome.stdout);
    assert!(outcome.stdout.contains("labs"), "{}", outcome.stdout);
}
