//! End-to-end `--ask` against a mock DeepSeek endpoint (WP6-AC2).

mod support;

use vlab::app::context::save_active_lab;
use vlab::app::directory::Directory;
use vlab::domain::agent::{AgentProfile, Description, Role};
use vlab::domain::ids::{AgentId, LabId};
use vlab::domain::lab::Lab;
use vlab::domain::name::Name;
use vlab::domain::settings::{ModelId, ProviderId};
use vlab::domain::skills;
use vlab::obs::RunId;

use support::{Outcome, Sandbox};

fn seed(sandbox: &Sandbox) -> AgentId {
    let db = sandbox.seed_database();
    db.migrate(RunId::generate()).unwrap();
    let lab = LabId::parse("a3f91c07").unwrap();
    db.insert_lab(&Lab::new(lab.clone(), Name::parse("Payments").unwrap()))
        .unwrap();
    save_active_lab(&db, &lab).unwrap();
    let lead = AgentId::parse("1c2d3e4f").unwrap();
    db.insert_agent(
        &AgentProfile::new(
            lead.clone(),
            Name::parse("Ada").unwrap(),
            Role::Lead,
            Description::parse("Coordinates work").unwrap(),
            skills::parse_list("planning").unwrap(),
            ProviderId::DeepSeek,
            ModelId::parse("deepseek-chat").unwrap(),
            None,
        )
        .unwrap(),
        std::slice::from_ref(&lab),
    )
    .unwrap();
    lead
}

#[test]
fn a_question_targets_the_lead_and_is_sanitized() {
    let sandbox = Sandbox::new();
    let lead = seed(&sandbox);
    sandbox.write_settings("provider: deepseek\nmodel: deepseek-chat\n");
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let server = runtime.block_on(wiremock::MockServer::start());
    let envelope = serde_json::json!({
        "id": "req-1",
        "model": "deepseek-chat",
        "choices": [{"message": {"content": "evil\u{1b}[31m answer"}, "finish_reason": "stop"}]
    });
    runtime.block_on(async {
        wiremock::Mock::given(wiremock::matchers::method("POST"))
            .respond_with(wiremock::ResponseTemplate::new(200).set_body_json(envelope))
            .mount(&server)
            .await;
    });
    let outcome = Outcome::from(
        sandbox
            .command()
            .env("DEEPSEEK_API_KEY", support::SECRET_MARKER)
            .env(
                "VLAB_DEEPSEEK_ENDPOINT",
                format!("{}/chat/completions", server.uri()),
            )
            .args(["--ask", "What next?"])
            .output()
            .unwrap(),
    );
    outcome
        .assert_success()
        .assert_no_escape_bytes()
        .assert_absent(support::SECRET_MARKER);
    assert!(
        outcome
            .stdout
            .contains("Agent Ada (1c2d3e4f) — model deepseek-chat")
    );
    assert!(outcome.stdout.contains("evil answer"), "{}", outcome.stdout);

    let db = sandbox.seed_database();
    let count: i64 = db.block_on(async {
        sqlx::query_scalar("SELECT count(*) FROM interactions WHERE kind = 'question'")
            .fetch_one(db.pool())
            .await
            .unwrap()
    });
    assert_eq!(count, 1);
    let _ = lead;
}

#[test]
fn a_non_member_target_is_not_found_and_missing_key_is_configuration() {
    let sandbox = Sandbox::new();
    seed(&sandbox);
    sandbox.write_settings("provider: deepseek\nmodel: deepseek-chat\n");
    sandbox
        .command()
        .env("DEEPSEEK_API_KEY", "dummy")
        .args(["--ask", "Why?", "--to-agent", "ffffffff"])
        .output()
        .map(Outcome::from)
        .unwrap()
        .assert_failure(3, "no agent with identifier `ffffffff` exists.");
    sandbox
        .command()
        .args(["--ask", "Why?"])
        .output()
        .map(Outcome::from)
        .unwrap()
        .assert_failure(2, "DEEPSEEK_API_KEY");
}
