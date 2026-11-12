//! Secret and privacy audit across storage and outputs (WP6-AC4).

mod support;

use sqlx::Row;
use vlab::app::context::save_active_lab;
use vlab::app::directory::Directory;
use vlab::app::projects::ProjectStore;
use vlab::domain::agent::{AgentProfile, Description, Role};
use vlab::domain::ids::{AgentId, LabId, ProjectId};
use vlab::domain::lab::Lab;
use vlab::domain::name::Name;
use vlab::domain::project::Project;
use vlab::domain::settings::{ModelId, ProviderId};
use vlab::domain::skills;

use support::{Outcome, SECRET_MARKER, Sandbox};

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
    for (id, role, text) in [
        ("1c2d3e4f", Role::Lead, ""),
        ("2d3e4f50", Role::Dev, "rust"),
    ] {
        db.insert_agent(
            &AgentProfile::new(
                AgentId::parse(id).unwrap(),
                Name::parse("Member").unwrap(),
                role,
                Description::parse("Works").unwrap(),
                skills::parse_list(text).unwrap(),
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

fn outcomes(sandbox: &Sandbox, args: &[&str]) -> Outcome {
    Outcome::from(
        sandbox
            .command()
            .env("DEEPSEEK_API_KEY", SECRET_MARKER)
            .env("RUST_LOG", "trace")
            .args(args)
            .output()
            .unwrap(),
    )
}

#[test]
fn a_secret_in_a_memory_is_rejected_and_never_stored() {
    let sandbox = Sandbox::new();
    seed(&sandbox);
    sandbox.write_settings("provider: deepseek\nmodel: deepseek-chat\n");
    let outcome = outcomes(
        &sandbox,
        &["--new-memory", SECRET_MARKER, "--for-lab", "a3f91c07"],
    );
    outcome
        .assert_failure(4, "provider credential")
        .assert_absent(SECRET_MARKER);
    let db = sandbox.seed_database();
    let count: i64 = db.block_on(async {
        sqlx::query_scalar("SELECT count(*) FROM memories")
            .fetch_one(db.pool())
            .await
            .unwrap()
    });
    assert_eq!(count, 0);
}

#[test]
fn a_secret_in_a_request_is_rejected_before_the_request_row() {
    let sandbox = Sandbox::new();
    let project = seed(&sandbox);
    sandbox.write_settings("provider: deepseek\nmodel: deepseek-chat\n");
    let outcome = Outcome::from(
        sandbox
            .command()
            .env("DEEPSEEK_API_KEY", SECRET_MARKER)
            .arg("--new-request")
            .arg(project.as_str())
            .write_stdin(SECRET_MARKER)
            .output()
            .unwrap(),
    );
    outcome
        .assert_failure(4, "provider credential")
        .assert_absent(SECRET_MARKER);
    let db = sandbox.seed_database();
    let count: i64 = db.block_on(async {
        sqlx::query_scalar("SELECT count(*) FROM requests")
            .fetch_one(db.pool())
            .await
            .unwrap()
    });
    assert_eq!(count, 0);
}

#[test]
fn the_marker_is_absent_from_stored_metadata_and_settings() {
    let sandbox = Sandbox::new();
    let project = seed(&sandbox);
    sandbox.write_settings("provider: deepseek\nmodel: deepseek-chat\n");
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let server = runtime.block_on(wiremock::MockServer::start());
    runtime.block_on(async {
        wiremock::Mock::given(wiremock::matchers::method("POST"))
            .respond_with(wiremock::ResponseTemplate::new(500).set_body_string("failure"))
            .mount(&server)
            .await;
    });
    let outcome = Outcome::from(
        sandbox
            .command()
            .env("DEEPSEEK_API_KEY", SECRET_MARKER)
            .env(
                "VLAB_DEEPSEEK_ENDPOINT",
                format!("{}/chat/completions", server.uri()),
            )
            .env("RUST_LOG", "trace")
            .arg("--new-request")
            .arg(project.as_str())
            .write_stdin("A harmless request")
            .output()
            .unwrap(),
    );
    outcome.assert_absent(SECRET_MARKER);

    let db = sandbox.seed_database();
    db.block_on(async {
        for table in ["requests", "events", "interactions"] {
            let sql = format!("SELECT * FROM {table}");
            let rows = sqlx::query(sqlx::AssertSqlSafe(sql))
                .fetch_all(db.pool())
                .await
                .unwrap();
            for row in rows {
                for index in 0..row.len() {
                    if let Ok(value) = row.try_get::<String, _>(index) {
                        assert!(!value.contains(SECRET_MARKER), "{table} index {index}");
                    }
                }
            }
        }
    });
    let settings = std::fs::read_to_string(sandbox.settings_file()).unwrap();
    assert!(!settings.contains(SECRET_MARKER));
}
