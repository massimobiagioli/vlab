//! Scoped memory commands (WP4-AC10).

mod support;

use vlab::app::context::save_active_lab;
use vlab::app::directory::Directory;
use vlab::app::memories::MemoryStore;
use vlab::app::projects::ProjectStore;
use vlab::domain::ids::{LabId, MemoryId, ProjectId};
use vlab::domain::lab::Lab;
use vlab::domain::memory::{Memory, MemoryScope};
use vlab::domain::name::Name;
use vlab::domain::project::Project;

use support::Sandbox;

fn seed_lab(sandbox: &Sandbox, raw: &str, name: &str) -> LabId {
    let db = sandbox.seed_database();
    let id = LabId::parse(raw).unwrap();
    db.insert_lab(&Lab::new(id.clone(), Name::parse(name).unwrap()))
        .unwrap();
    id
}

fn activate(sandbox: &Sandbox, lab: &LabId) {
    let db = sandbox.seed_database();
    save_active_lab(&db, lab).unwrap();
}

fn seed_project(sandbox: &Sandbox, raw: &str, lab: &LabId) -> ProjectId {
    let db = sandbox.seed_database();
    let id = ProjectId::parse(raw).unwrap();
    db.insert_project(&Project::new(
        id.clone(),
        lab.clone(),
        Name::parse("Portal").unwrap(),
        None,
    ))
    .unwrap();
    id
}

fn created_id(stdout: &str) -> String {
    stdout
        .lines()
        .find_map(|line| line.strip_prefix("ID:    "))
        .expect("memory id")
        .trim()
        .to_owned()
}

#[test]
fn lab_memories_can_be_created_listed_and_deleted() {
    let sandbox = Sandbox::new();
    let lab = seed_lab(&sandbox, "a3f91c07", "Payments");
    let created = sandbox.run(&[
        "--new-memory",
        "Prefer small pull requests.",
        "--for-lab",
        lab.as_str(),
    ]);
    created.assert_success();
    assert!(created.stdout.contains("Scope: lab a3f91c07"));
    let id = created_id(&created.stdout);

    let listed = sandbox.run(&["--list-memories", "--for-lab", lab.as_str()]);
    listed.assert_success();
    assert!(listed.stdout.contains(&id));
    assert!(listed.stdout.contains("Prefer small pull requests."));

    sandbox
        .run(&["--delete-memory", &id])
        .assert_failure(4, "requires confirmation");
    sandbox
        .run(&["--delete-memory", &id, "--yes"])
        .assert_success();
    let empty = sandbox.run(&["--list-memories", "--for-lab", lab.as_str()]);
    empty.assert_success();
    assert!(empty.stdout.contains("No memories for this scope."));
}

#[test]
fn project_and_agent_scopes_are_checked() {
    let sandbox = Sandbox::new();
    let lab = seed_lab(&sandbox, "a3f91c07", "Payments");
    let project = seed_project(&sandbox, "0b1c2d3e", &lab);

    sandbox
        .run(&["--new-memory", "x", "--for-project", project.as_str()])
        .assert_failure(3, "no active laboratory is selected.");
    activate(&sandbox, &lab);
    sandbox
        .run(&["--new-memory", "x", "--for-project", project.as_str()])
        .assert_success();
    sandbox
        .run(&["--new-memory", "x", "--for-agent", "ffffffff"])
        .assert_failure(3, "no agent with identifier `ffffffff` exists.");
}

#[test]
fn exactly_one_scope_selector_is_required() {
    let sandbox = Sandbox::new();
    sandbox
        .run(&["--new-memory", "x"])
        .assert_failure(2, "needs exactly one of");
    sandbox
        .run(&["--list-memories"])
        .assert_failure(2, "needs exactly one of");
    sandbox
        .run(&[
            "--new-memory",
            "x",
            "--for-lab",
            "a3f91c07",
            "--for-project",
            "0b1c2d3e",
        ])
        .assert_failure(2, "needs exactly one of");
    sandbox
        .run(&["--new-memory", "x", "--for-agent", "BAD"])
        .assert_failure(2, "must be 8 lowercase hexadecimal characters");
}

#[test]
fn malformed_scope_ids_are_usage_errors_before_storage() {
    for args in [
        &["--new-memory", "x", "--for-lab", "BAD"][..],
        &["--list-memories", "--for-project", "BAD"],
        &["--delete-memory", "BAD"],
    ] {
        let sandbox = Sandbox::new();
        sandbox
            .run(args)
            .assert_failure(2, "must be 8 lowercase hexadecimal characters");
        assert!(!sandbox.data_dir().exists(), "{args:?}");
    }
}

#[test]
fn memory_content_is_bounded_and_control_free() {
    let sandbox = Sandbox::new();
    let lab = seed_lab(&sandbox, "a3f91c07", "Payments");
    sandbox
        .run(&[
            "--new-memory",
            &"a".repeat(2_001),
            "--for-lab",
            lab.as_str(),
        ])
        .assert_failure(4, "at most 2000 characters");
    sandbox
        .run(&["--new-memory", "bad\u{7}value", "--for-lab", lab.as_str()])
        .assert_failure(4, "must not contain control characters");
    sandbox
        .run(&[
            "--new-memory",
            &"a".repeat(2_000),
            "--for-lab",
            lab.as_str(),
        ])
        .assert_success();
}

#[test]
fn the_twenty_first_memory_is_rejected() {
    let sandbox = Sandbox::new();
    let lab = seed_lab(&sandbox, "a3f91c07", "Payments");
    let db = sandbox.seed_database();
    let scope = MemoryScope::Lab(lab.clone());
    for index in 0..20 {
        db.insert_memory(
            &Memory::new(
                MemoryId::generate(),
                scope.clone(),
                &format!("note {index}"),
            )
            .unwrap(),
        )
        .unwrap();
    }
    sandbox
        .run(&["--new-memory", "one too many", "--for-lab", lab.as_str()])
        .assert_failure(4, "already has 20 memories");
}

#[test]
fn memory_content_never_appears_in_audit_events() {
    let sandbox = Sandbox::new();
    let lab = seed_lab(&sandbox, "a3f91c07", "Payments");
    let secret = support::SECRET_MARKER;
    let created = sandbox.run(&["--new-memory", secret, "--for-lab", lab.as_str()]);
    created.assert_success();
    let id = created_id(&created.stdout);
    let db = sandbox.seed_database();
    let events: Vec<(String, String, String)> = db.block_on(async {
        sqlx::query_as(
            "SELECT kind, entity_type, entity_id FROM events WHERE kind = 'memory_created'",
        )
        .fetch_all(db.pool())
        .await
        .unwrap()
    });
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].0, "memory_created");
    assert_eq!(events[0].1, "memory");
    assert_eq!(events[0].2, id);
    assert!(
        !events
            .iter()
            .any(|row| row.0.contains(secret) || row.2.contains(secret))
    );
}
