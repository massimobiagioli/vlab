//! Markdown document import and relative references (ADR-0059).

mod support;

use support::Sandbox;
use vlab::app::directory::Directory;
use vlab::app::documents;
use vlab::app::projects::ProjectStore;
use vlab::domain::agent::{AgentProfile, Description, Role};
use vlab::domain::ids::{AgentId, LabId, ProjectId};
use vlab::domain::lab::Lab;
use vlab::domain::name::Name;
use vlab::domain::project::Project;
use vlab::domain::settings::{ModelId, ProviderId};
use vlab::domain::skills;
use vlab::infra::db::Database;

struct Fixture {
    sandbox: Sandbox,
    db: Database,
    lab: LabId,
    agent: AgentId,
}

fn fixture() -> Fixture {
    let sandbox = Sandbox::new();
    let db = sandbox.seed_database();
    let lab = LabId::parse("a3f91c07").unwrap();
    db.insert_lab(&Lab::new(lab.clone(), Name::parse("Payments").unwrap()))
        .unwrap();
    let project = ProjectId::parse("0b1c2d3e").unwrap();
    db.insert_project(&Project::new(
        project.clone(),
        lab.clone(),
        Name::parse("Portal").unwrap(),
        None,
    ))
    .unwrap();
    let agent = AgentId::parse("10000001").unwrap();
    db.insert_agent(
        &AgentProfile::new(
            agent.clone(),
            Name::parse("Ada").unwrap(),
            Role::Dev,
            Description::parse("Works").unwrap(),
            skills::parse_list("rust").unwrap(),
            ProviderId::DeepSeek,
            ModelId::parse("deepseek-chat").unwrap(),
            None,
        )
        .unwrap(),
        std::slice::from_ref(&lab),
    )
    .unwrap();
    Fixture {
        sandbox,
        db,
        lab,
        agent,
    }
}

fn source(fixture: &Fixture, name: &str, content: &str) -> std::path::PathBuf {
    let path = fixture.sandbox.root().join(name);
    std::fs::write(&path, content).unwrap();
    path
}

fn read_document(fixture: &Fixture, path: &str) -> String {
    std::fs::read_to_string(fixture.sandbox.data_dir().join(path)).unwrap()
}

#[test]
fn a_lab_document_is_copied_and_only_a_relative_reference_is_stored() {
    let fixture = fixture();
    let source = source(
        &fixture,
        "security.md",
        "# Security review\n\nKeep it small.",
    );
    let files = fixture.db.files().clone();
    let document = documents::import(&fixture.db, &files, &source, "lab/Payments", None).unwrap();
    assert!(
        document.path.starts_with("documents/lab/payments/"),
        "{}",
        document.path
    );
    assert!(document.path.ends_with(".md"));
    assert_eq!(
        read_document(&fixture, &document.path),
        "# Security review\n\nKeep it small."
    );
    let row: (String, String, Option<String>) = fixture
        .db
        .block_on(
            sqlx::query_as("SELECT path, scope_type, lab_id FROM documents WHERE id = ?1")
                .bind(document.id.as_str())
                .fetch_one(fixture.db.pool()),
        )
        .unwrap();
    assert_eq!(row.0, document.path);
    assert_eq!(row.1, "lab");
    assert_eq!(row.2.as_deref(), Some(fixture.lab.as_str()));
    assert_eq!(
        std::fs::read_to_string(&source).unwrap(),
        "# Security review\n\nKeep it small."
    );
}

#[test]
fn project_and_agent_destinations_resolve_by_name() {
    let fixture = fixture();
    let files = fixture.db.files().clone();
    let project_source = source(&fixture, "design.md", "# Design");
    let project_doc =
        documents::import(&fixture.db, &files, &project_source, "project/Portal", None).unwrap();
    assert!(
        project_doc.path.starts_with("documents/projects/portal/"),
        "{}",
        project_doc.path
    );

    let agent_source = source(&fixture, "notes.md", "# Agent notes");
    let agent_doc =
        documents::import(&fixture.db, &files, &agent_source, "agent", Some("Ada")).unwrap();
    assert!(
        agent_doc.path.starts_with("documents/agents/ada/"),
        "{}",
        agent_doc.path
    );
    let scope: (String, Option<String>) = fixture
        .db
        .block_on(
            sqlx::query_as("SELECT scope_type, agent_id FROM documents WHERE id = ?1")
                .bind(agent_doc.id.as_str())
                .fetch_one(fixture.db.pool()),
        )
        .unwrap();
    assert_eq!(scope.0, "agent");
    assert_eq!(scope.1.as_deref(), Some(fixture.agent.as_str()));
}

#[test]
fn agent_import_requires_an_agent_and_rejects_non_markdown() {
    let fixture = fixture();
    let files = fixture.db.files().clone();
    let markdown = source(&fixture, "notes.md", "# Notes");
    let error = documents::import(&fixture.db, &files, &markdown, "agent", None).unwrap_err();
    assert!(
        matches!(error, vlab::app::AppError::Usage { .. }),
        "{error:?}"
    );

    let text = source(&fixture, "notes.txt", "# Notes");
    let error = documents::import(&fixture.db, &files, &text, "lab/Payments", None).unwrap_err();
    assert!(
        matches!(error, vlab::app::AppError::Usage { .. }),
        "{error:?}"
    );
}

#[test]
fn the_cli_imports_into_the_requested_scope() {
    let fixture = fixture();
    let source = source(&fixture, "cli.md", "# CLI import");
    let outcome = fixture.sandbox.run(&[
        "--import",
        "--src",
        source.to_str().unwrap(),
        "--into",
        "lab/Payments",
    ]);
    outcome.assert_success().assert_no_escape_bytes();
    assert!(
        outcome.stdout.contains("Document imported."),
        "{}",
        outcome.stdout
    );
    let db = fixture.sandbox.seed_database();
    let row: (String, String) = db
        .block_on(
            sqlx::query_as(
                "SELECT path, scope_type FROM documents ORDER BY created_at DESC LIMIT 1",
            )
            .fetch_one(db.pool()),
        )
        .unwrap();
    assert_eq!(row.1, "lab");
    assert!(row.0.starts_with("documents/lab/payments/"));
    assert_eq!(
        std::fs::read_to_string(fixture.sandbox.data_dir().join(&row.0)).unwrap(),
        "# CLI import"
    );
}

#[test]
fn an_unknown_destination_is_rejected() {
    let fixture = fixture();
    let files = fixture.db.files().clone();
    let markdown = source(&fixture, "notes.md", "# Notes");
    assert!(matches!(
        documents::import(&fixture.db, &files, &markdown, "lab/Missing", None),
        Err(vlab::app::AppError::Precondition { .. })
    ));
    assert!(matches!(
        documents::import(&fixture.db, &files, &markdown, "project/Missing", None),
        Err(vlab::app::AppError::Precondition { .. })
    ));
    assert!(matches!(
        documents::import(&fixture.db, &files, &markdown, "elsewhere", None),
        Err(vlab::app::AppError::Usage { .. })
    ));
}
