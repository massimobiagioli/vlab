//! Request input matrix and end-to-end planning against a mock DeepSeek
//! endpoint (WP5-AC8, WP5-AC11).

mod support;

use vlab::app::context::{save_active_lab, save_active_project};
use vlab::app::directory::Directory;
use vlab::app::projects::ProjectStore;
use vlab::domain::agent::{AgentProfile, Description, Role};
use vlab::domain::ids::{AgentId, LabId, ProjectId};
use vlab::domain::lab::Lab;
use vlab::domain::name::Name;
use vlab::domain::project::Project;
use vlab::domain::settings::{ModelId, ProviderId};
use vlab::domain::skills;

use support::{Outcome, Sandbox};

fn seed(sandbox: &Sandbox) -> ProjectId {
    let db = sandbox.seed_database();
    let lab = LabId::parse("a3f91c07").unwrap();
    db.insert_lab(&Lab::new(lab.clone(), Name::parse("Payments").unwrap()))
        .unwrap();
    save_active_lab(&db, &lab).unwrap();
    let project = ProjectId::parse("0b1c2d3e").unwrap();
    db.insert_project(&Project::new(
        project.clone(),
        lab.clone(),
        Name::parse("Portal").unwrap(),
        None,
    ))
    .unwrap();
    save_active_project(&db, &project).unwrap();
    for (id, role, skills_text) in [
        ("1c2d3e4f", Role::Lead, ""),
        ("2d3e4f50", Role::Dev, "rust"),
    ] {
        db.insert_agent(
            &AgentProfile::new(
                AgentId::parse(id).unwrap(),
                Name::parse("Member").unwrap(),
                role,
                Description::parse("Works").unwrap(),
                skills::parse_list(skills_text).unwrap(),
                ProviderId::DeepSeek,
                ModelId::parse("deepseek-chat").unwrap(),
                None,
            )
            .unwrap(),
            std::slice::from_ref(&lab),
        )
        .unwrap();
    }
    project
}

fn settings(sandbox: &Sandbox) {
    sandbox.write_settings(
        "provider: deepseek\nmodel: deepseek-chat\ncredentials:\n  deepseek_api_key: \"sk-vlab-SECRET-MARKER\"\n",
    );
}

#[test]
fn the_input_matrix_rejects_dual_empty_invalid_and_oversized_input() {
    let sandbox = Sandbox::new();
    seed(&sandbox);
    settings(&sandbox);

    // Literal text plus a non-terminal stdin is ambiguous.
    let both = Outcome::from(
        sandbox
            .command()
            .arg("--new-request")
            .arg("literal")
            .write_stdin("piped")
            .output()
            .unwrap(),
    );
    both.assert_failure(2, "not both");

    let empty = Outcome::from(
        sandbox
            .command()
            .arg("--new-request")
            .write_stdin("   ")
            .output()
            .unwrap(),
    );
    empty.assert_failure(2, "non-empty");

    let invalid = Outcome::from(
        sandbox
            .command()
            .arg("--new-request")
            .write_stdin(vec![0xff, 0xfe])
            .output()
            .unwrap(),
    );
    invalid.assert_failure(2, "not valid UTF-8");

    let too_large = Outcome::from(
        sandbox
            .command()
            .arg("--new-request")
            .write_stdin("a".repeat(64 * 1024 + 1))
            .output()
            .unwrap(),
    );
    too_large.assert_failure(2, "at most 64 KiB");
}

#[test]
fn an_end_to_end_request_creates_linked_stories() {
    let sandbox = Sandbox::new();
    seed(&sandbox);
    settings(&sandbox);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let server = runtime.block_on(wiremock::MockServer::start());
    let plan = serde_json::json!({
        "schema_version": 1,
        "clarification_questions": [],
        "stories": [{
            "title": "Add reset",
            "description": "Add password reset",
            "acceptance_criteria": ["Email sent"],
            "priority": "high",
            "complexity": "small",
            "required_skills": ["rust"],
            "assigned_agent_id": null,
            "assignment_rationale": "No suitable member."
        }]
    });
    let envelope = serde_json::json!({
        "id": "req-1",
        "model": "deepseek-chat",
        "choices": [{"message": {"content": plan.to_string()}, "finish_reason": "stop"}],
        "usage": {"prompt_tokens": 10, "completion_tokens": 20}
    });
    runtime.block_on(async {
        wiremock::Mock::given(wiremock::matchers::method("POST"))
            .and(wiremock::matchers::path("/chat/completions"))
            .respond_with(wiremock::ResponseTemplate::new(200).set_body_json(envelope))
            .mount(&server)
            .await;
    });

    let outcome = Outcome::from(
        sandbox
            .command()
            .env(
                "VLAB_DEEPSEEK_ENDPOINT",
                format!("{}/chat/completions", server.uri()),
            )
            .arg("--new-request")
            .write_stdin("Add password reset")
            .output()
            .unwrap(),
    );
    outcome
        .assert_success()
        .assert_absent(support::SECRET_MARKER);
    assert!(outcome.stdout.contains("Request "), "{}", outcome.stdout);
    assert!(outcome.stdout.contains("planned"), "{}", outcome.stdout);
    assert!(outcome.stdout.contains("Add reset"), "{}", outcome.stdout);

    let db = sandbox.seed_database();
    let rows: (String, i64) = db.block_on(async {
        let status: String = sqlx::query_scalar("SELECT status FROM requests LIMIT 1")
            .fetch_one(db.pool())
            .await
            .unwrap();
        let stories: i64 = sqlx::query_scalar("SELECT count(*) FROM stories")
            .fetch_one(db.pool())
            .await
            .unwrap();
        (status, stories)
    });
    assert_eq!(rows, ("PLANNED".to_owned(), 1));
}
