//! Pseudo-terminal tests for the interactive dashboards (ADR-0038).

mod support;

use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use portable_pty::{CommandBuilder, PtySize, native_pty_system};
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
    let db = sandbox.seed_database();
    let archived: Option<String> = db.block_on(async {
        sqlx::query_scalar("SELECT archived_at FROM agents WHERE id = '1c2d3e4f'")
            .fetch_one(db.pool())
            .await
            .unwrap()
    });
    assert!(archived.is_some());
}
