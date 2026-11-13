//! Single dashboard entry point and hidden legacy aliases (ADR-0039).

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
fn removed_group_subcommands_are_usage_errors() {
    let sandbox = Sandbox::new();
    for args in [
        &["labs"][..],
        &["projects"],
        &["agents"],
        &["settings"],
        &["help"],
        &["labs", "--plain"],
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
    assert!(outcome.stdout.contains("-h, --help"), "{}", outcome.stdout);
    assert!(!outcome.stdout.contains("Commands:"), "{}", outcome.stdout);
}

#[test]
fn hidden_legacy_flags_remain_available_for_scripts() {
    let sandbox = Sandbox::new();
    seed_lab(&sandbox);
    let outcome = sandbox.run(&["--list-labs"]);
    outcome.assert_success().assert_no_escape_bytes();
    assert!(outcome.stdout.contains("Payments"), "{}", outcome.stdout);
}
