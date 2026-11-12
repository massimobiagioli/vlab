//! Project commands (WP4-AC2).

mod support;

use vlab::app::context::save_active_lab;
use vlab::app::directory::Directory;
use vlab::app::projects::ProjectStore;
use vlab::domain::ids::LabId;
use vlab::domain::lab::Lab;
use vlab::domain::name::Name;

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

#[test]
fn project_commands_require_an_active_laboratory() {
    let sandbox = Sandbox::new();
    sandbox
        .run(&["--list-projects"])
        .assert_failure(3, "no active laboratory is selected.");
    sandbox
        .run(&["--new-project", "Portal"])
        .assert_failure(3, "no active laboratory is selected.");
}

#[test]
fn creating_a_project_persists_and_prints_its_identifier() {
    let sandbox = Sandbox::new();
    let lab = seed_lab(&sandbox, "a3f91c07", "Payments");
    activate(&sandbox, &lab);
    let outcome = sandbox.run(&["--new-project", "  Customer Portal  "]);
    outcome.assert_success().assert_no_escape_bytes();
    assert!(outcome.stdout.starts_with("Project created.\n"));
    assert!(outcome.stdout.contains("Name: Customer Portal\n"));
    let id = outcome
        .stdout
        .lines()
        .find_map(|line| line.strip_prefix("ID:   "))
        .expect("id line");
    assert!(vlab::domain::ids::ProjectId::parse(id.trim()).is_some());

    let listed = sandbox.run(&["--list-projects"]);
    listed.assert_success();
    assert!(listed.stdout.contains("Customer Portal"));
    assert!(listed.stdout.contains(id.trim()));
}

#[test]
fn empty_project_names_exit_2_without_creating_a_database() {
    let sandbox = Sandbox::new();
    let lab = seed_lab(&sandbox, "a3f91c07", "Payments");
    activate(&sandbox, &lab);
    sandbox
        .run(&["--new-project", "   "])
        .assert_failure(2, "a name is required.");
    let db = sandbox.seed_database();
    assert!(db.projects(&lab).unwrap().is_empty());
}

#[test]
fn an_empty_project_list_is_actionable() {
    let sandbox = Sandbox::new();
    let lab = seed_lab(&sandbox, "a3f91c07", "Payments");
    activate(&sandbox, &lab);
    let outcome = sandbox.run(&["--list-projects"]);
    outcome.assert_success();
    assert_eq!(
        outcome.stdout,
        "No projects in the active laboratory.\nHint: Run `vlab --new-project \"Name\"` to create one.\n"
    );
}

#[test]
fn a_github_source_requires_a_settings_file_and_a_global_key() {
    let sandbox = Sandbox::new();
    let lab = seed_lab(&sandbox, "a3f91c07", "Payments");
    activate(&sandbox, &lab);
    let args = [
        "--new-project",
        "Portal",
        "--github-source",
        "https://github.com/acme/portal",
    ];
    sandbox.run(&args).assert_failure(2, "does not exist.");
    sandbox.write_settings("provider: deepseek\nmodel: deepseek-chat\n");
    sandbox
        .run(&args)
        .assert_failure(2, "`credentials.github_api_key` is not set.");
}

#[test]
fn a_github_backed_project_stores_the_canonical_source() {
    let sandbox = Sandbox::new();
    let lab = seed_lab(&sandbox, "a3f91c07", "Payments");
    activate(&sandbox, &lab);
    sandbox.write_settings(&format!(
        "provider: deepseek\nmodel: deepseek-chat\ncredentials:\n  github_api_key: \"{}\"\n",
        support::SECRET_MARKER
    ));
    let outcome = support::Outcome::from(
        sandbox
            .command()
            .args([
                "--new-project",
                "Portal",
                "--github-source",
                " https://github.com/acme/portal.git/ ",
            ])
            .output()
            .unwrap(),
    );
    outcome
        .assert_success()
        .assert_absent(support::SECRET_MARKER);
    assert!(
        outcome
            .stdout
            .contains("GitHub: https://github.com/acme/portal")
    );
    let id = outcome
        .stdout
        .lines()
        .find_map(|line| line.strip_prefix("ID:   "))
        .expect("id line")
        .trim();
    let workspace = sandbox.data_dir().join("workspace").join(id);
    assert!(workspace.join(".git").is_dir(), "workspace was not cloned");
    assert!(
        outcome
            .stdout
            .contains(&format!("Workspace: {}", workspace.display()))
    );
    assert!(!sandbox.git_log().contains(support::SECRET_MARKER));
    assert!(sandbox.git_log().contains("token=set"));
    let listed = sandbox.run(&["--list-projects"]);
    listed.assert_success();
    assert!(listed.stdout.contains("https://github.com/acme/portal"));
    let db = sandbox.seed_database();
    let project = db.projects(&lab).unwrap();
    assert_eq!(
        project[0].github_source.as_deref(),
        Some("https://github.com/acme/portal")
    );
}

#[test]
fn a_failed_clone_creates_no_project_row_or_checkout() {
    let sandbox = Sandbox::new();
    let lab = seed_lab(&sandbox, "a3f91c07", "Payments");
    activate(&sandbox, &lab);
    sandbox.write_settings(&format!(
        "provider: deepseek\nmodel: deepseek-chat\ncredentials:\n  github_api_key: \"{}\"\n",
        support::SECRET_MARKER
    ));
    sandbox.fail_git();
    let outcome = sandbox.run(&[
        "--new-project",
        "Portal",
        "--github-source",
        "https://github.com/acme/portal",
    ]);
    outcome
        .assert_failure(1, "could not be cloned")
        .assert_absent(support::SECRET_MARKER);
    let db = sandbox.seed_database();
    assert!(db.projects(&lab).unwrap().is_empty());
    let workspace = sandbox.data_dir().join("workspace");
    assert!(
        !workspace.exists() || std::fs::read_dir(&workspace).unwrap().next().is_none(),
        "clone failure left a checkout"
    );
    assert!(!sandbox.git_log().contains(support::SECRET_MARKER));
}

#[test]
fn an_invalid_github_url_or_modifier_is_a_usage_error() {
    let sandbox = Sandbox::new();
    sandbox
        .run(&[
            "--new-project",
            "Portal",
            "--github-source",
            "https://gitlab.com/a/b",
        ])
        .assert_failure(2, "must be `https://github.com/<owner>/<repository>`");
    sandbox
        .run(&["--github-source", "https://github.com/a/b"])
        .assert_failure(2, "can only be used with `--new-project`");
}

#[test]
fn malformed_project_ids_are_usage_errors_before_storage() {
    let sandbox = Sandbox::new();
    sandbox
        .run(&["--show-project-kanban", "BAD"])
        .assert_failure(2, "must be 8 lowercase hexadecimal characters");
    assert!(!sandbox.data_dir().exists());
}

#[test]
fn a_project_from_another_laboratory_is_not_found_or_shown() {
    let sandbox = Sandbox::new();
    let first = seed_lab(&sandbox, "a3f91c07", "First");
    let second = seed_lab(&sandbox, "1c2d3e4f", "Second");
    activate(&sandbox, &first);
    sandbox
        .run(&["--new-project", "First Project"])
        .assert_success();

    // Select the other lab: the first project must not appear.
    activate(&sandbox, &second);
    let listed = sandbox.run(&["--list-projects"]);
    listed.assert_success();
    assert!(
        !listed.stdout.contains("First Project"),
        "{}",
        listed.stdout
    );

    let db = sandbox.seed_database();
    let first_id = db.projects(&first).unwrap()[0].id.clone();
    sandbox
        .run(&["--show-project-kanban", first_id.as_str()])
        .assert_failure(3, "no project with identifier");
}
