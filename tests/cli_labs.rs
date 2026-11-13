//! Laboratory commands (WP3-AC4, WP3-AC5).

mod support;

use vlab::app::directory::Directory;
use vlab::app::storage::{AppStateKey, AppStateStore};
use vlab::domain::agent::{AgentProfile, Description, Role};
use vlab::domain::ids::{AgentId, LabId};
use vlab::domain::lab::Lab;
use vlab::domain::name::Name;
use vlab::domain::settings::{ModelId, ProviderId};
use vlab::domain::skills;

use support::Sandbox;

fn id(raw: &str) -> LabId {
    LabId::parse(raw).unwrap()
}

fn seed_lab(sandbox: &Sandbox, raw: &str, name: &str) {
    let db = sandbox.seed_database();
    db.insert_lab(&Lab::new(id(raw), Name::parse(name).unwrap()))
        .unwrap();
}

fn seed_agent(sandbox: &Sandbox, raw: &str, lab: &LabId, role: Role, skills_list: &str) {
    let db = sandbox.seed_database();
    let profile = AgentProfile::new(
        AgentId::parse(raw).unwrap(),
        Name::parse("Member").unwrap(),
        role,
        Description::parse("Works on the project").unwrap(),
        skills::parse_list(skills_list).unwrap(),
        ProviderId::DeepSeek,
        ModelId::parse("deepseek-chat").unwrap(),
        None,
    )
    .unwrap();
    db.insert_agent(&profile, std::slice::from_ref(lab))
        .unwrap();
}

#[test]
fn creating_a_laboratory_prints_its_identifier_and_persists_one_row() {
    let sandbox = Sandbox::new();
    let outcome = sandbox.run(&["--new-lab", "  Payments Platform  "]);
    outcome.assert_success().assert_no_escape_bytes();
    assert!(outcome.stdout.starts_with("Laboratory created.\n"));
    assert!(outcome.stdout.contains("Name: Payments Platform\n"));
    assert!(
        outcome
            .stdout
            .contains("A Tech Lead is required before requests")
    );
    let printed = outcome
        .stdout
        .lines()
        .find_map(|line| line.strip_prefix("ID:   "))
        .expect("identifier line");
    assert!(LabId::parse(printed.trim()).is_some(), "{printed:?}");

    let db = sandbox.seed_database();
    assert_eq!(db.labs().unwrap().len(), 1);
    assert_eq!(db.get(AppStateKey::ActiveLabId).unwrap(), None);
    let events: i64 = db.block_on(async {
        sqlx::query_scalar("SELECT count(*) FROM events WHERE kind = 'lab_created'")
            .fetch_one(db.pool())
            .await
            .unwrap()
    });
    assert_eq!(events, 1);
}

#[test]
fn empty_names_exit_2_and_write_no_row() {
    let sandbox = Sandbox::new();
    let outcome = sandbox.run(&["--new-lab", "   "]);
    outcome
        .assert_failure(2, "a name is required.")
        .assert_error_contract();
    assert!(
        !sandbox.data_dir().exists(),
        "invalid input must not create a database"
    );
}

#[test]
fn an_empty_listing_is_actionable() {
    let sandbox = Sandbox::new();
    let outcome = sandbox.run(&["--list-labs"]);
    outcome.assert_success();
    assert_eq!(
        outcome.stdout,
        "No laboratories yet.\nHint: Run `vlab labs` to create the first one.\n"
    );
}

#[test]
fn listing_shows_agent_counts_validation_and_the_active_marker() {
    let sandbox = Sandbox::new();
    let alpha = id("a3f91c07");
    seed_lab(&sandbox, "a3f91c07", "Alpha");
    seed_lab(&sandbox, "1c2d3e4f", "Beta");
    seed_agent(&sandbox, "0b1c2d3e", &alpha, Role::Lead, "");
    seed_agent(&sandbox, "2d3e4f50", &alpha, Role::Dev, "rust");
    sandbox.run(&["--open-lab", "a3f91c07"]).assert_success();

    let outcome = sandbox.run(&["--list-labs"]);
    outcome.assert_success().assert_no_escape_bytes();
    let alpha_line = outcome
        .stdout
        .lines()
        .find(|line| line.contains("Alpha"))
        .expect("alpha row");
    assert!(alpha_line.contains("a3f91c07"), "{alpha_line}");
    assert!(alpha_line.contains('2'), "{alpha_line}");
    assert!(alpha_line.contains("yes"), "{alpha_line}");
    assert!(alpha_line.contains('*'), "{alpha_line}");

    let beta_line = outcome
        .stdout
        .lines()
        .find(|line| line.contains("Beta"))
        .expect("beta row");
    assert!(beta_line.contains("1c2d3e4f"), "{beta_line}");
    assert!(beta_line.contains("no"), "{beta_line}");
    assert!(!beta_line.contains('*'), "{beta_line}");
}

#[test]
fn opening_persists_the_selection_and_warns_about_incomplete_laboratories() {
    let sandbox = Sandbox::new();
    seed_lab(&sandbox, "a3f91c07", "Payments");

    let outcome = sandbox.run(&["--open-lab", "a3f91c07"]);
    assert_eq!(outcome.code, 0, "{outcome:#?}");
    assert!(outcome.stdout.starts_with("Active laboratory: a3f91c07"));
    assert!(
        outcome
            .stderr
            .contains("Warning: this laboratory is not ready")
    );
    assert!(
        outcome
            .stderr
            .contains("Hint: Run `vlab --check-lab a3f91c07`")
    );

    let db = sandbox.seed_database();
    assert_eq!(
        db.get(AppStateKey::ActiveLabId).unwrap().as_deref(),
        Some("a3f91c07")
    );
    let listed = sandbox.run(&["--list-labs"]);
    assert!(
        listed
            .stdout
            .lines()
            .any(|line| line.contains("Payments") && line.contains('*')),
        "{}",
        listed.stdout
    );
}

#[test]
fn unknown_and_malformed_laboratory_ids_exit_3_and_2() {
    let sandbox = Sandbox::new();
    sandbox
        .run(&["--open-lab", "a3f91c07"])
        .assert_failure(3, "no laboratory with identifier `a3f91c07` exists.")
        .assert_failure(3, "Run `vlab --list-labs`");
    sandbox
        .run(&["--open-lab", "NOT-AN-ID"])
        .assert_failure(2, "must be 8 lowercase hexadecimal characters");
}

#[test]
fn malformed_identifiers_create_no_database() {
    for args in [
        &["--open-lab", "BAD"][..],
        &["--check-lab", "BAD"],
        &["--open-lab", "a3f91c0"],
    ] {
        let sandbox = Sandbox::new();
        sandbox
            .run(args)
            .assert_failure(2, "must be 8 lowercase hexadecimal characters");
        assert!(
            !sandbox.data_dir().exists(),
            "malformed input must not create a database: {args:?}"
        );
    }
}

#[test]
fn check_lab_reports_every_rule_and_uses_exit_4_when_invalid() {
    let sandbox = Sandbox::new();
    seed_lab(&sandbox, "a3f91c07", "Payments");
    let outcome = sandbox.run(&["--check-lab", "a3f91c07"]);
    assert_eq!(outcome.code, 4, "{outcome:#?}");
    assert!(outcome.stdout.contains("Laboratory a3f91c07: Payments"));
    for rule in [
        "laboratory exists",
        "at least one agent",
        "exactly one Tech Lead",
        "roles are valid",
        "profiles are well-formed",
        "provider and model are valid",
        "operational roles have skills",
        "memberships resolve",
        "project references resolve",
    ] {
        assert!(
            outcome.stdout.contains(rule),
            "missing {rule}:\n{}",
            outcome.stdout
        );
    }
    assert!(outcome.stdout.contains("[fail] at least one agent"));
    assert!(outcome.stdout.ends_with("Result: invalid\n"));
    assert!(
        outcome
            .stderr
            .contains("the laboratory did not pass validation.")
    );
    assert!(outcome.stderr.contains("Hint: Fix the failed rules"));
}

#[test]
fn check_lab_passes_a_complete_laboratory_without_a_provider_key() {
    let sandbox = Sandbox::new();
    let lab = id("a3f91c07");
    seed_lab(&sandbox, "a3f91c07", "Payments");
    seed_agent(&sandbox, "0b1c2d3e", &lab, Role::Lead, "");
    seed_agent(&sandbox, "2d3e4f50", &lab, Role::Qa, "testing");
    let outcome = sandbox.run(&["--check-lab", "a3f91c07"]);
    outcome.assert_success();
    assert!(
        outcome.stdout.ends_with("Result: valid\n"),
        "{}",
        outcome.stdout
    );
    assert!(outcome.stderr.is_empty());
}

#[test]
fn check_lab_handles_missing_and_malformed_identifiers() {
    let sandbox = Sandbox::new();
    sandbox
        .run(&["--check-lab", "a3f91c07"])
        .assert_failure(3, "no laboratory with identifier `a3f91c07` exists.");
    sandbox
        .run(&["--check-lab", "bad"])
        .assert_failure(2, "must be 8 lowercase hexadecimal characters");
}
