//! Pseudo-terminal tests for the interactive dashboards (ADR-0038).

mod support;

use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use portable_pty::{CommandBuilder, PtySize, native_pty_system};
use vlab::app::context::save_active_lab;
use vlab::app::directory::Directory;
use vlab::app::projects::ProjectStore;
use vlab::app::storage::AppStateStore;
use vlab::domain::agent::{AgentProfile, Description, Role};
use vlab::domain::ids::{AgentId, LabId, ProjectId};
use vlab::domain::lab::Lab;
use vlab::domain::name::Name;
use vlab::domain::project::Project;
use vlab::domain::settings::{ModelId, ProviderId};
use vlab::domain::skills;

use support::Sandbox;

fn seed(sandbox: &Sandbox) -> (LabId, ProjectId, AgentId) {
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
    let agent = AgentId::parse("1c2d3e4f").unwrap();
    db.insert_agent(
        &AgentProfile::new(
            agent.clone(),
            Name::parse("Ada").unwrap(),
            Role::Dev,
            Description::parse("Writes code").unwrap(),
            skills::parse_list("rust").unwrap(),
            ProviderId::DeepSeek,
            ModelId::parse("deepseek-chat").unwrap(),
            None,
        )
        .unwrap(),
        std::slice::from_ref(&lab),
    )
    .unwrap();
    (lab, project, agent)
}

/// Runs `vlab <args>` in a 100x30 pseudo-terminal and sends timed key input.
fn run_tui(sandbox: &Sandbox, args: &[&str], keys: &[(&str, u64)]) -> (String, bool) {
    run_tui_with_env(sandbox, args, keys, &[])
}

fn run_tui_with_env(
    sandbox: &Sandbox,
    args: &[&str],
    keys: &[(&str, u64)],
    env: &[(&str, String)],
) -> (String, bool) {
    let pty = native_pty_system();
    let pair = pty
        .openpty(PtySize {
            rows: 30,
            cols: 100,
            pixel_width: 0,
            pixel_height: 0,
        })
        .unwrap();
    let mut command = CommandBuilder::new(env!("CARGO_BIN_EXE_vlab"));
    command.args(args);
    command.env("VLAB_CONFIG_DIR", sandbox.config_dir());
    command.env("VLAB_DATA_DIR", sandbox.data_dir());
    command.env("HOME", sandbox.home());
    command.env("TERM", "xterm-256color");
    command.env_remove("NO_COLOR");
    command.env_remove("CLICOLOR");
    for (key, value) in env {
        command.env(key, value);
    }
    command.cwd(sandbox.home());
    let mut child = pair.slave.spawn_command(command).unwrap();
    drop(pair.slave);
    let mut writer = pair.master.take_writer().unwrap();
    let mut reader = pair.master.try_clone_reader().unwrap();
    let captured = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&captured);
    let reader_thread = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = reader.read_to_end(&mut bytes);
        *sink.lock().unwrap() = bytes;
    });

    let started = Instant::now();
    while started.elapsed() < Duration::from_secs(3) {
        let ready = captured
            .lock()
            .unwrap()
            .windows(6)
            .any(|window| window == b"\x1b[?1049h");
        if ready {
            break;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    std::thread::sleep(Duration::from_millis(100));
    for (keys, delay) in keys {
        writer.write_all(keys.as_bytes()).unwrap();
        writer.flush().unwrap();
        std::thread::sleep(Duration::from_millis(*delay));
    }
    let deadline = Instant::now() + Duration::from_secs(10);
    let success = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status.success();
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            break false;
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    drop(writer);
    let _ = reader_thread.join();
    let bytes = captured.lock().unwrap().clone();
    (String::from_utf8_lossy(&bytes).into_owned(), success)
}

fn assert_restored(output: &str) {
    assert!(
        output.contains("\u{1b}[?1049l") && output.contains("\u{1b}[?25h"),
        "terminal was not restored: {output:?}"
    );
}

#[test]
fn the_labs_dashboard_creates_checks_activates_and_deletes() {
    let sandbox = Sandbox::new();
    let (output, success) = run_tui(
        &sandbox,
        &[],
        &[
            ("\r", 300),
            ("n", 200),
            ("Pty Lab", 200),
            ("\r", 300),
            ("a", 200),
            ("c", 200),
            ("x", 200),
            ("d", 200),
            ("y", 400),
            ("q", 200),
        ],
    );
    assert!(success, "dashboard did not exit cleanly: {output:?}");
    assert_restored(&output);
    assert!(output.contains("Laboratories"), "{output:?}");
    assert!(output.contains("Laboratory check"), "{output:?}");
    let plain = sandbox.run(&["--list-labs"]);
    plain.assert_success();
    assert!(plain.stdout.contains("No laboratories"), "{}", plain.stdout);
}

#[test]
fn the_labs_dashboard_paginates_and_cancels_a_new_form() {
    let sandbox = Sandbox::new();
    let db = sandbox.seed_database();
    for index in 1..=30 {
        let id = LabId::parse(&format!("{index:08x}")).unwrap();
        db.insert_lab(&Lab::new(id, Name::parse(&format!("Lab {index}")).unwrap()))
            .unwrap();
    }
    let (output, success) = run_tui(&sandbox, &[], &[("\r", 300), ("l", 300), ("q", 200)]);
    assert!(success, "dashboard did not exit cleanly: {output:?}");
    assert!(output.contains("Page 2"), "{output:?}");

    let sandbox = Sandbox::new();
    let (output, success) = run_tui(
        &sandbox,
        &[],
        &[
            ("\r", 300),
            ("n", 200),
            ("Cancelled Lab", 200),
            ("\u{1b}", 300),
            ("q", 200),
        ],
    );
    assert!(success, "dashboard did not exit cleanly: {output:?}");
    let plain = sandbox.run(&["--list-labs"]);
    plain.assert_success();
    assert!(plain.stdout.contains("No laboratories"), "{}", plain.stdout);
}

#[test]
fn the_labs_dashboard_edits_the_description_and_opens_projects() {
    let sandbox = Sandbox::new();
    let (lab, _, _) = seed(&sandbox);
    let (output, success) = run_tui(
        &sandbox,
        &[],
        &[
            ("1", 300),
            ("e", 200),
            ("\t", 200),
            ("Platform team", 200),
            ("\r", 400),
            ("p", 400),
            ("q", 200),
        ],
    );
    assert!(success, "dashboard did not exit cleanly: {output:?}");
    assert_restored(&output);
    assert!(output.contains("Projects"), "{output:?}");
    let db = sandbox.seed_database();
    let description: String = db.block_on(async {
        sqlx::query_scalar("SELECT description FROM labs WHERE id = ?1")
            .bind(lab.as_str())
            .fetch_one(db.pool())
            .await
            .unwrap()
    });
    assert_eq!(description, "Platform team");
    assert_eq!(
        db.get(vlab::app::storage::AppStateKey::ActiveLabId)
            .unwrap()
            .as_deref(),
        Some(lab.as_str())
    );
}

#[test]
fn the_projects_dashboard_creates_opens_assigns_and_deletes() {
    let sandbox = Sandbox::new();
    let (_, _, _) = seed(&sandbox);
    let (output, success) = run_tui(
        &sandbox,
        &[],
        &[
            ("2", 300),
            ("n", 200),
            ("Pty Project", 200),
            ("\r", 300),
            ("G", 200),
            ("\r", 200),
            ("a", 200),
            ("\r", 300),
            ("q", 200),
        ],
    );
    assert!(success, "dashboard did not exit cleanly: {output:?}");
    assert_restored(&output);
    assert!(output.contains("Kanban"), "{output:?}");
    let db = sandbox.seed_database();
    let assigned: i64 = db.block_on(async {
        sqlx::query_scalar("SELECT count(*) FROM project_agents")
            .fetch_one(db.pool())
            .await
            .unwrap()
    });
    assert_eq!(assigned, 1);

    let (output, success) = run_tui(
        &sandbox,
        &[],
        &[("2", 300), ("G", 200), ("d", 200), ("y", 400), ("q", 200)],
    );
    assert!(success, "dashboard did not exit cleanly: {output:?}");
    assert_restored(&output);
    let count: i64 = db.block_on(async {
        sqlx::query_scalar("SELECT count(*) FROM project_agents")
            .fetch_one(db.pool())
            .await
            .unwrap()
    });
    assert_eq!(count, 0);
    let plain = sandbox.run(&["--list-projects"]);
    plain.assert_success();
    assert!(plain.stdout.contains("Portal"), "{}", plain.stdout);
}

#[test]
fn project_and_agent_dashboards_start_without_context() {
    let sandbox = Sandbox::new();
    for key in ["2", "3"] {
        let (output, success) = run_tui(&sandbox, &[], &[(key, 300), ("q", 200)]);
        assert!(success, "{key} did not exit cleanly: {output:?}");
        assert_restored(&output);
    }
}

#[test]
fn the_agents_dashboard_creates_an_agent_through_the_wizard() {
    let sandbox = Sandbox::new();
    let (_, _, _) = seed(&sandbox);
    sandbox.write_settings("provider: deepseek\nmodel: deepseek-chat\n");
    let (output, success) = run_tui(
        &sandbox,
        &[],
        &[
            ("3", 300),
            ("n", 500),
            ("Pty Agent\r", 200),
            ("dev\r", 200),
            ("Writes code\r", 200),
            ("rust\r", 200),
            ("\r", 200),
            ("\r", 200),
            ("\r", 200),
            ("\r", 200),
            ("y\r", 400),
            ("q", 200),
        ],
    );
    assert!(success, "dashboard did not exit cleanly: {output:?}");
    assert_restored(&output);
    let db = sandbox.seed_database();
    let count: i64 = db.block_on(async {
        sqlx::query_scalar("SELECT count(*) FROM agents")
            .fetch_one(db.pool())
            .await
            .unwrap()
    });
    assert_eq!(count, 2);
}

#[test]
fn the_agents_dashboard_shows_detail_and_archives() {
    let sandbox = Sandbox::new();
    let (_, _, _) = seed(&sandbox);
    let (output, success) = run_tui(
        &sandbox,
        &[],
        &[
            ("3", 300),
            ("\r", 200),
            ("x", 200),
            ("d", 200),
            ("n", 200),
            ("d", 200),
            ("y", 400),
            ("q", 200),
        ],
    );
    assert!(success, "dashboard did not exit cleanly: {output:?}");
    assert_restored(&output);
    assert!(output.contains("Agent 1c2d3e4f"), "{output:?}");
    assert!(output.contains("Memories:"), "{output:?}");
    let db = sandbox.seed_database();
    let archived: Option<String> = db.block_on(async {
        sqlx::query_scalar("SELECT archived_at FROM agents WHERE id = '1c2d3e4f'")
            .fetch_one(db.pool())
            .await
            .unwrap()
    });
    assert!(archived.is_some());
}

#[test]
fn the_projects_dashboard_renames_and_opens_the_full_screen_kanban() {
    let sandbox = Sandbox::new();
    let (_, project, _) = seed(&sandbox);
    let db = sandbox.seed_database();
    db.execute(
        "INSERT INTO requests (id, project_id, content, status, created_at, updated_at) \
         VALUES ('5e6f7a8b', '0b1c2d3e', 'Add reset', 'PLANNED', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
    )
    .unwrap();
    db.execute(
        "INSERT INTO stories (id, project_id, request_id, title, description, acceptance_criteria_json, priority, complexity, required_skills_json, state, due_date, created_at, updated_at) \
         VALUES ('6f7a8b9c', '0b1c2d3e', '5e6f7a8b', 'Reset password', 'Send a link', '[\"Link expires\"]', 'high', 'small', '[\"rust\"]', 'TODO', '2026-01-03', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
    )
    .unwrap();
    drop(db);
    let (output, success) = run_tui(
        &sandbox,
        &[],
        &[
            ("2", 300),
            ("e", 200),
            (" Web", 200),
            ("\r", 400),
            ("v", 400),
            ("\r", 300),
            ("\u{1b}", 300),
            ("l", 200),
            ("\r", 300),
            ("\u{1b}", 400),
            ("q", 200),
        ],
    );
    assert!(success, "dashboard did not exit cleanly: {output:?}");
    assert_restored(&output);
    assert!(output.contains("Edit project"), "{output:?}");
    assert!(output.contains("TODO 1"), "{output:?}");
    assert!(output.contains("Story 6f7a8b9c"), "{output:?}");
    assert!(output.contains("Acceptance criteria"), "{output:?}");
    assert!(output.contains("Stories are read-only"), "{output:?}");
    assert!(output.contains("No stories in IN PROGRESS"), "{output:?}");
    let db = sandbox.seed_database();
    let name: String = db.block_on(async {
        sqlx::query_scalar("SELECT name FROM projects WHERE id = ?1")
            .bind(project.as_str())
            .fetch_one(db.pool())
            .await
            .unwrap()
    });
    assert_eq!(name, "Portal Web");
    let events: i64 = db.block_on(async {
        sqlx::query_scalar("SELECT count(*) FROM events WHERE kind = 'project_updated'")
            .fetch_one(db.pool())
            .await
            .unwrap()
    });
    assert_eq!(events, 1);
}

#[test]
fn the_projects_dashboard_submits_a_request_through_the_prompt() {
    let sandbox = Sandbox::new();
    let (lab, project, _) = seed(&sandbox);
    let db = sandbox.seed_database();
    db.insert_agent(
        &AgentProfile::new(
            AgentId::parse("2d3e4f50").unwrap(),
            Name::parse("Grace").unwrap(),
            Role::Lead,
            Description::parse("Plans work").unwrap(),
            skills::parse_list("").unwrap(),
            ProviderId::DeepSeek,
            ModelId::parse("deepseek-chat").unwrap(),
            None,
        )
        .unwrap(),
        std::slice::from_ref(&lab),
    )
    .unwrap();
    drop(db);
    sandbox.write_settings(
        "provider: deepseek\nmodel: deepseek-chat\ncredentials:\n  deepseek_api_key: \"sk-vlab-SECRET-MARKER\"\n",
    );

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let server = runtime.block_on(wiremock::MockServer::start());
    let plan = serde_json::json!({
        "schema_version": 1,
        "clarification_questions": [],
        "stories": [{
            "title": "Send reset email",
            "description": "Email a single-use link",
            "acceptance_criteria": ["Link expires"],
            "priority": "high",
            "complexity": "small",
            "required_skills": ["rust"],
            "assigned_agent_id": "1c2d3e4f",
            "assignment_rationale": "Rust developer."
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

    let (output, success) = run_tui_with_env(
        &sandbox,
        &[],
        &[
            ("2", 300),
            ("r", 300),
            ("\r", 200),
            ("Add password reset", 200),
            ("\r", 2000),
            ("x", 300),
            ("v", 400),
            ("q", 200),
        ],
        &[(
            "VLAB_DEEPSEEK_ENDPOINT",
            format!("{}/chat/completions", server.uri()),
        )],
    );
    assert!(success, "dashboard did not exit cleanly: {output:?}");
    assert_restored(&output);
    assert!(output.contains("New request"), "{output:?}");
    assert!(output.contains("Write a request first."), "{output:?}");
    assert!(output.contains("planned"), "{output:?}");
    assert!(output.contains("Send res"), "{output:?}");
    assert!(output.contains("IN PROGRESS 1"), "{output:?}");
    assert!(output.contains("TODO 0"), "{output:?}");
    assert!(!output.contains("SECRET-MARKER"), "{output:?}");

    let db = sandbox.seed_database();
    let (status, stories, content, state): (String, i64, String, String) = db.block_on(async {
        let status = sqlx::query_scalar("SELECT status FROM requests")
            .fetch_one(db.pool())
            .await
            .unwrap();
        let stories = sqlx::query_scalar("SELECT count(*) FROM stories")
            .fetch_one(db.pool())
            .await
            .unwrap();
        let content = sqlx::query_scalar("SELECT content FROM requests")
            .fetch_one(db.pool())
            .await
            .unwrap();
        let state = sqlx::query_scalar("SELECT state FROM stories")
            .fetch_one(db.pool())
            .await
            .unwrap();
        (status, stories, content, state)
    });
    assert_eq!(
        (status.as_str(), stories, content.as_str(), state.as_str()),
        ("PLANNED", 1, "Add password reset", "IN_PROGRESS")
    );
    assert_eq!(
        db.get(vlab::app::storage::AppStateKey::ActiveProjectId)
            .unwrap()
            .as_deref(),
        Some(project.as_str())
    );
}
