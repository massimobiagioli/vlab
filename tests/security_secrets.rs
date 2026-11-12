//! Secret and privacy audit across storage and outputs (WP6-AC4).

mod support;

use sqlx::Row;
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
    save_active_project(&db, &project).unwrap();
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

fn settings_with_key(sandbox: &Sandbox) {
    sandbox.write_settings(&format!(
        "provider: deepseek\nmodel: deepseek-chat\ncredentials:\n  deepseek_api_key: \"{SECRET_MARKER}\"\n"
    ));
}

#[test]
fn a_secret_in_a_request_is_rejected_before_the_request_row() {
    let sandbox = Sandbox::new();
    seed(&sandbox);
    settings_with_key(&sandbox);
    let outcome = Outcome::from(
        sandbox
            .command()
            .arg("--new-request")
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
    seed(&sandbox);
    settings_with_key(&sandbox);
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
            .env(
                "VLAB_DEEPSEEK_ENDPOINT",
                format!("{}/chat/completions", server.uri()),
            )
            .env("RUST_LOG", "trace")
            .arg("--new-request")
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
    // The key is allowed in the private settings file, but nowhere else.
    let settings = std::fs::read_to_string(sandbox.settings_file()).unwrap();
    assert!(settings.contains(SECRET_MARKER));
}
