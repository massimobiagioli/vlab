//! Real worktree execution workflow (ADR-0050).

mod support;

use std::process::Command;

use vlab::app::context::{save_active_lab, save_active_project};
use vlab::app::directory::Directory;
use vlab::app::kanban;
use vlab::app::projects::ProjectStore;
use vlab::app::workflow;
use vlab::domain::agent::{AgentProfile, Description, Role};
use vlab::domain::budget::{CancelToken, ExecutionLimits};
use vlab::domain::clock::TestClock;
use vlab::domain::ids::{AgentId, LabId, ProjectId};
use vlab::domain::lab::Lab;
use vlab::domain::name::Name;
use vlab::domain::project::Project;
use vlab::domain::provider::{ProviderError, ProviderResponse};
use vlab::domain::settings::{ModelId, ProviderId, Settings};
use vlab::domain::skills;
use vlab::infra::db::Database;
use vlab::infra::execution::WorkspaceEnvironment;
use vlab::infra::provider::scripted::ScriptedProvider;
use vlab::obs::RunId;

struct Fixture {
    dir: tempfile::TempDir,
    db: Database,
    project: ProjectId,
}

fn git(dir: &std::path::Path, args: &[&str]) {
    let status = Command::new("git")
        .current_dir(dir)
        .args(args)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_TERMINAL_PROMPT", "0")
        .status()
        .unwrap();
    assert!(status.success(), "git {args:?}");
}

fn fixture(with_qa: bool) -> Fixture {
    let dir = support::private_tempdir();
    let db = Database::open(dir.path().join("vlab.db")).unwrap();
    db.migrate(RunId::generate()).unwrap();
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
    for (id, name, role, skills_text) in [
        ("1c2d3e4f", "Grace", Role::Lead, ""),
        ("2d3e4f50", "Ada", Role::Dev, "rust"),
    ] {
        db.insert_agent(
            &AgentProfile::new(
                AgentId::parse(id).unwrap(),
                Name::parse(name).unwrap(),
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
    if with_qa {
        db.insert_agent(
            &AgentProfile::new(
                AgentId::parse("3e4f5061").unwrap(),
                Name::parse("Quinn").unwrap(),
                Role::Qa,
                Description::parse("Reviews").unwrap(),
                skills::parse_list("testing").unwrap(),
                ProviderId::DeepSeek,
                ModelId::parse("deepseek-chat").unwrap(),
                None,
            )
            .unwrap(),
            std::slice::from_ref(&lab),
        )
        .unwrap();
    }
    db.execute(
        "INSERT INTO requests (id, project_id, content, status, created_at, updated_at) \
         VALUES ('4f506172', '0b1c2d3e', 'Add reset', 'PLANNED', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
    )
    .unwrap();
    db.execute(
        "INSERT INTO stories (id, project_id, request_id, title, description, acceptance_criteria_json, priority, complexity, required_skills_json, assigned_agent_id, assignment_rationale, state, due_date, created_at, updated_at) \
         VALUES ('5a617283', '0b1c2d3e', '4f506172', 'Write Playbook.md', 'Write the playbook', '[\"File exists\"]', 'high', 'small', '[\"rust\"]', '2d3e4f50', 'Rust developer.', 'IN_PROGRESS', '2026-01-03', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
    )
    .unwrap();

    let repo = dir.path().join("workspace").join("0b1c2d3e");
    std::fs::create_dir_all(&repo).unwrap();
    git(&repo, &["init", "-q"]);
    std::fs::write(repo.join("README.md"), "# Portal\n").unwrap();
    git(&repo, &["add", "."]);
    git(
        &repo,
        &[
            "-c",
            "user.name=VLab",
            "-c",
            "user.email=vlab@localhost",
            "commit",
            "--no-gpg-sign",
            "-q",
            "-m",
            "init",
        ],
    );
    Fixture { dir, db, project }
}

fn settings() -> Settings {
    Settings::parse("deepseek", "deepseek-chat").unwrap()
}

fn response(content: &str) -> ProviderResponse {
    ProviderResponse {
        content: content.to_owned(),
        model: "deepseek-chat".to_owned(),
        usage: None,
        finish_reason: None,
        request_id: None,
    }
}

fn run(fixture: &Fixture, provider: &ScriptedProvider) -> workflow::WorkflowReport {
    let environment = WorkspaceEnvironment::new(fixture.dir.path().join("workspace"));
    workflow::run(
        &fixture.db,
        &fixture.db,
        &fixture.project,
        &settings(),
        provider,
        &environment,
        ExecutionLimits::default(),
        &TestClock::default(),
        CancelToken::default(),
        RunId::generate(),
    )
    .unwrap()
}

fn story(fixture: &Fixture) -> kanban::StoryDetail {
    kanban::story(
        &fixture.db,
        &fixture.db,
        &fixture.project,
        &vlab::domain::ids::StoryId::parse("5a617283").unwrap(),
    )
    .unwrap()
}

#[test]
fn the_agent_writes_a_file_commits_it_and_stores_the_outcome() {
    let fixture = fixture(true);
    let provider = ScriptedProvider::new(vec![
        Ok(response(
            r##"{"actions":[{"tool":"write_file","path":"Playbook.md","content":"# Playbook\n\nReal content.\n"}],"done":false}"##,
        )),
        Ok(response(
            r#"{"actions":[],"done":true,"summary":"Wrote Playbook.md."}"#,
        )),
        Ok(response(
            r#"{"review_outcome":"The file exists and is correct."}"#,
        )),
    ]);
    let report = run(&fixture, &provider);
    assert_eq!(
        report,
        workflow::WorkflowReport {
            completed: 1,
            failed: 0,
            remaining: 0
        }
    );
    let detail = story(&fixture);
    assert_eq!(detail.state, kanban::StoryState::Done);
    let outcome = detail.outcome.unwrap();
    assert_eq!(outcome.work, "Wrote Playbook.md.");
    assert_eq!(outcome.review, "The file exists and is correct.");
    assert_eq!(outcome.reviewer, "Quinn");
    assert_eq!(outcome.branch.as_deref(), Some("vlab/5a617283"));
    assert_eq!(
        outcome.worktree_path.as_deref(),
        Some(".worktrees/5a617283")
    );
    assert!(outcome.commit_sha.as_deref().unwrap().len() >= 7);
    let worktree = fixture
        .dir
        .path()
        .join("workspace/.worktrees/5a617283/Playbook.md");
    assert_eq!(
        std::fs::read_to_string(&worktree).unwrap(),
        "# Playbook\n\nReal content.\n"
    );
    let _ = fixture.dir;
}

#[test]
fn the_tech_lead_reviews_when_no_qa_exists() {
    let fixture = fixture(false);
    let provider = ScriptedProvider::new(vec![
        Ok(response(
            r#"{"actions":[{"tool":"write_file","path":"README.md","content":"changed\n"}],"done":false}"#,
        )),
        Ok(response(
            r#"{"actions":[],"done":true,"summary":"Updated the README."}"#,
        )),
        Ok(response(r#"{"review_outcome":"OK."}"#)),
    ]);
    run(&fixture, &provider);
    let requests = provider.requests();
    assert!(requests[2].system.contains("reviewer"));
    let detail = story(&fixture);
    assert_eq!(detail.outcome.unwrap().reviewer, "Grace");
}

#[test]
fn a_provider_failure_keeps_the_story_in_progress_and_counts_the_attempt() {
    let fixture = fixture(true);
    let provider = ScriptedProvider::new(vec![Err(ProviderError::Timeout)]);
    let report = run(&fixture, &provider);
    assert_eq!(report.failed, 1);
    let detail = story(&fixture);
    assert_eq!(detail.state, kanban::StoryState::InProgress);
    assert!(detail.outcome.is_none());
    let attempts: i64 = fixture.db.block_on(async {
        sqlx::query_scalar("SELECT execution_attempts FROM stories WHERE id = '5a617283'")
            .fetch_one(fixture.db.pool())
            .await
            .unwrap()
    });
    assert_eq!(attempts, 1);
}

#[test]
fn three_attempts_block_the_story_without_calling_the_provider() {
    let fixture = fixture(true);
    fixture
        .db
        .execute("UPDATE stories SET execution_attempts = 3 WHERE id = '5a617283'")
        .unwrap();
    let provider = ScriptedProvider::new(Vec::new());
    let report = run(&fixture, &provider);
    assert_eq!(report.failed, 1);
    assert!(provider.requests().is_empty());
    let detail = story(&fixture);
    assert_eq!(detail.state, kanban::StoryState::Blocked);
    assert!(detail.outcome.is_none());
}

#[test]
fn unassigned_and_completed_stories_are_not_processed() {
    let fixture = fixture(true);
    fixture
        .db
        .execute(
            "UPDATE stories SET state = 'TODO', assigned_agent_id = NULL WHERE id = '5a617283'",
        )
        .unwrap();
    let provider = ScriptedProvider::new(Vec::new());
    let report = run(&fixture, &provider);
    assert_eq!(report, workflow::WorkflowReport::default());
    assert!(provider.requests().is_empty());
}
