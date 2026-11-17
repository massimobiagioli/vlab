//! Real worktree execution workflow (ADR-0050).

mod support;

use std::process::Command;
use std::time::Duration;

use vlab::app::context::{save_active_lab, save_active_project};
use vlab::app::directory::Directory;
use vlab::app::kanban;
use vlab::app::memories::MemoryStore;
use vlab::app::projects::ProjectStore;
use vlab::app::workflow;
use vlab::domain::agent::{AgentProfile, Description, Role};
use vlab::domain::budget::{CancelToken, ExecutionLimits};
use vlab::domain::clock::TestClock;
use vlab::domain::ids::{AgentId, LabId, ProjectId};
use vlab::domain::lab::Lab;
use vlab::domain::memory::MemoryScope;
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
            r#"{"review_outcome":"The file exists and is correct.","accepted":true}"#,
        )),
    ]);
    let report = run(&fixture, &provider);
    assert_eq!(
        report,
        workflow::WorkflowReport {
            completed: 1,
            failed: 0,
            remaining: 0,
            remediated: 0,
            debt: 0
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
    let agent = AgentId::parse("2d3e4f50").unwrap();
    let memories = fixture.db.memories(&MemoryScope::Agent(agent)).unwrap();
    assert_eq!(memories.len(), 1);
    let recap =
        std::fs::read_to_string(fixture.db.files().base().join(fixture.db.block_on(async {
            sqlx::query_scalar::<_, String>("SELECT path FROM memories LIMIT 1")
                .fetch_one(fixture.db.pool())
                .await
                .unwrap()
        })))
        .unwrap();
    assert!(recap.starts_with("# Card completed"), "{recap}");
    assert!(recap.contains("## Work"), "{recap}");
    assert!(recap.contains("## Review"), "{recap}");
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
        Ok(response(r#"{"review_outcome":"OK.","accepted":true}"#)),
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
fn an_exhausted_story_is_remediated_by_the_tech_lead_and_finishes() {
    let fixture = fixture(true);
    fixture
        .db
        .execute("UPDATE stories SET execution_attempts = 3 WHERE id = '5a617283'")
        .unwrap();
    let provider = ScriptedProvider::new(vec![
        Ok(response(
            r#"{"decision":"Retry with the lead and make the missing file explicit.","context":"Create README.md, then verify it exists.","assignee_id":"1c2d3e4f"}"#,
        )),
        Ok(response(
            r#"{"actions":[{"tool":"write_file","path":"README.md","content":"changed\n"}],"done":false}"#,
        )),
        Ok(response(
            r#"{"actions":[],"done":true,"summary":"Updated the README."}"#,
        )),
        Ok(response(r#"{"review_outcome":"OK.","accepted":true}"#)),
    ]);
    let report = run(&fixture, &provider);
    assert_eq!(report.remediated, 1);
    assert_eq!(report.completed, 1);
    let detail = story(&fixture);
    assert_eq!(detail.state, kanban::StoryState::Done);
    assert_eq!(detail.assignee.as_deref(), Some("Grace"));
    assert!(
        detail
            .tech_lead_context
            .as_deref()
            .unwrap_or_default()
            .contains("Create README.md")
    );
    assert_eq!(provider.requests().len(), 4);
}

#[test]
fn an_assigned_todo_story_is_executed() {
    let fixture = fixture(true);
    fixture
        .db
        .execute("UPDATE stories SET state = 'TODO' WHERE id = '5a617283'")
        .unwrap();
    let provider = ScriptedProvider::new(vec![
        Ok(response(
            r#"{"actions":[{"tool":"write_file","path":"README.md","content":"changed\n"}],"done":false}"#,
        )),
        Ok(response(
            r#"{"actions":[],"done":true,"summary":"Updated the README."}"#,
        )),
        Ok(response(r#"{"review_outcome":"OK.","accepted":true}"#)),
    ]);
    let report = run(&fixture, &provider);
    assert_eq!(report.completed, 1);
    assert_eq!(story(&fixture).state, kanban::StoryState::Done);
}

#[test]
fn a_rejected_review_is_reworked_by_the_same_assignee_and_finishes() {
    let fixture = fixture(true);
    let provider = ScriptedProvider::new(vec![
        Ok(response(
            r#"{"actions":[{"tool":"write_file","path":"README.md","content":"changed\n"}],"done":false}"#,
        )),
        Ok(response(
            r#"{"actions":[],"done":true,"summary":"Updated the README."}"#,
        )),
        Ok(response(
            r#"{"review_outcome":"The README is incomplete.","accepted":false,"findings":["Add the missing section."],"retest":"Only the missing section and the README rendering."}"#,
        )),
        Ok(response(
            r#"{"actions":[{"tool":"write_file","path":"README.md","content":"complete\n","append":true}],"done":false}"#,
        )),
        Ok(response(
            r#"{"actions":[],"done":true,"summary":"Completed the README."}"#,
        )),
        Ok(response(
            r#"{"review_outcome":"Now complete.","accepted":true}"#,
        )),
    ]);
    let report = run(&fixture, &provider);
    assert_eq!(report.remediated, 1);
    assert_eq!(report.completed, 1);
    let detail = story(&fixture);
    assert_eq!(detail.state, kanban::StoryState::Done);
    assert_eq!(detail.assignee.as_deref(), Some("Ada"));
    assert!(
        detail
            .tech_lead_context
            .as_deref()
            .unwrap_or_default()
            .contains("Rework only"),
        "the rework context names the bounded scope"
    );
    assert!(
        detail
            .tech_lead_context
            .as_deref()
            .unwrap_or_default()
            .contains("Add the missing section."),
        "the rework context carries the reviewer finding"
    );
    let outcome = detail.outcome.unwrap();
    assert!(outcome.accepted);
    assert!(outcome.commit_sha.is_some());
    assert_eq!(
        provider.requests().len(),
        6,
        "no Tech Lead call on the first failure"
    );
}

#[test]
fn a_second_failure_escalates_to_a_different_agent() {
    let fixture = fixture(true);
    let provider = ScriptedProvider::new(vec![
        Ok(response(
            r#"{"actions":[{"tool":"write_file","path":"README.md","content":"first\n"}],"done":false}"#,
        )),
        Ok(response(
            r#"{"actions":[],"done":true,"summary":"First attempt."}"#,
        )),
        Ok(response(
            r#"{"review_outcome":"Missing a section.","accepted":false,"findings":["Add the missing section."]}"#,
        )),
        Ok(response(
            r#"{"actions":[{"tool":"write_file","path":"README.md","content":"second\n","append":true}],"done":false}"#,
        )),
        Ok(response(
            r#"{"actions":[],"done":true,"summary":"Rework attempt."}"#,
        )),
        Ok(response(
            r#"{"review_outcome":"Still missing the section.","accepted":false,"findings":["The section is still missing."]}"#,
        )),
        Ok(response(
            r#"{"decision":"The previous assignee could not fix it; reassign.","context":"Add the missing section and run the checks.","assignee_id":"1c2d3e4f"}"#,
        )),
        Ok(response(
            r#"{"actions":[{"tool":"write_file","path":"README.md","content":"final\n"}],"done":false}"#,
        )),
        Ok(response(
            r#"{"actions":[],"done":true,"summary":"Completed after escalation."}"#,
        )),
        Ok(response(
            r#"{"review_outcome":"Complete.","accepted":true}"#,
        )),
    ]);
    let report = run(&fixture, &provider);
    assert_eq!(report.remediated, 2);
    assert_eq!(report.completed, 1);
    let detail = story(&fixture);
    assert_eq!(detail.state, kanban::StoryState::Done);
    assert_eq!(detail.assignee.as_deref(), Some("Grace"));
    let (remediation, rework): (i64, i64) = fixture.db.block_on(async {
        sqlx::query_as(
            "SELECT remediation_attempts, rework_attempts FROM stories \
             WHERE id = '5a617283'",
        )
        .fetch_one(fixture.db.pool())
        .await
        .unwrap()
    });
    assert_eq!(remediation, 1);
    assert_eq!(rework, 2);
    assert_eq!(provider.requests().len(), 10);
}

#[test]
fn a_work_limit_is_reworked_before_any_review() {
    let fixture = fixture(true);
    let mut outcomes: Vec<Result<ProviderResponse, ProviderError>> = (1..24)
        .map(|_| {
            Ok(response(
                r#"{"actions":[{"tool":"list_files","path":"."}],"done":false}"#,
            ))
        })
        .collect();
    outcomes.push(Ok(response(
        r#"{"actions":[{"tool":"write_file","path":"README.md","content":"partial\n"}],"done":false}"#,
    )));
    outcomes.push(Ok(response(
        r#"{"actions":[{"tool":"write_file","path":"README.md","content":"complete\n"}],"done":false}"#,
    )));
    outcomes.push(Ok(response(
        r#"{"actions":[],"done":true,"summary":"Finished the rework."}"#,
    )));
    outcomes.push(Ok(response(
        r#"{"review_outcome":"Complete.","accepted":true}"#,
    )));
    let provider = ScriptedProvider::new(outcomes);
    let report = run(&fixture, &provider);
    assert_eq!(report.completed, 1);
    let requests = provider.requests();
    assert_eq!(requests.len(), 27);
    assert!(
        requests[24].user.contains("Rework only"),
        "the work-limit attempt is followed by a bounded rework"
    );
    assert!(
        requests[24].system.contains("developer"),
        "the rework is work, not a review"
    );
    assert!(
        requests[26].system.contains("reviewer"),
        "the review happens only after the rework completes"
    );
    let detail = story(&fixture);
    assert_eq!(detail.state, kanban::StoryState::Done);
}

#[test]
fn an_exhausted_remediation_closes_the_card_with_technical_debt() {
    let fixture = fixture(true);
    fixture
        .db
        .execute(
            "UPDATE stories SET execution_attempts = 3, remediation_attempts = 0, \
             rework_attempts = 3 WHERE id = '5a617283'",
        )
        .unwrap();
    fixture
        .db
        .execute(
            "INSERT INTO story_outcomes (story_id, work_outcome, review_outcome, \
             reviewer_agent_id, review_accepted, branch, worktree_path, commit_sha, \
             review_findings_json, review_retest, completed_at) \
             VALUES ('5a617283', 'Added the reset endpoint.', 'The audit log is missing.', \
             '3e4f5061', 0, 'vlab/5a617283', '.worktrees/5a617283', 'abc1234', \
             '[\"Add the missing audit log.\"]', 'Only the audit log and the reset endpoint.', \
             '2026-01-02T00:00:00Z')",
        )
        .unwrap();
    let provider = ScriptedProvider::new(vec![Ok(response(
        r#"{"decision":"Accept the reset endpoint and ship it without the audit log.","debt_context":"Add the missing audit log and its integration test in a follow-up card."}"#,
    ))]);
    let report = run(&fixture, &provider);
    assert_eq!(report.failed, 0);
    assert_eq!(report.remediated, 1);
    assert_eq!(report.debt, 1);
    assert_eq!(report.remaining, 0);
    let detail = story(&fixture);
    assert_eq!(detail.state, kanban::StoryState::Done);
    let outcome = detail.outcome.unwrap();
    assert!(outcome.accepted);
    assert!(
        outcome.review.contains("Tech Lead compromise"),
        "{}",
        outcome.review
    );
    assert!(outcome.review.contains("without the audit log"));
    assert_eq!(outcome.reviewer, "Grace");
    assert_eq!(outcome.branch.as_deref(), Some("vlab/5a617283"));
    let debt: Vec<(String, String, String, Option<String>, String, String)> =
        fixture.db.block_on(async {
            sqlx::query_as(
                "SELECT id, title, state, assigned_agent_id, debt_of_story_id, description \
                 FROM stories WHERE debt_of_story_id = '5a617283'",
            )
            .fetch_all(fixture.db.pool())
            .await
            .unwrap()
        });
    assert_eq!(debt.len(), 1);
    let (debt_id, debt_title, debt_state, assignee, parent, debt_description) = &debt[0];
    assert!(debt_title.starts_with("Technical debt:"), "{debt_title}");
    assert_eq!(debt_state, "TODO");
    assert!(assignee.is_none(), "a debt card waits for triage");
    assert_eq!(parent.as_str(), "5a617283");
    assert!(
        debt_description.contains("Add the missing audit log."),
        "{debt_description}"
    );
    let blocked: i64 = fixture.db.block_on(async {
        sqlx::query_scalar("SELECT COUNT(*) FROM stories WHERE state = 'BLOCKED'")
            .fetch_one(fixture.db.pool())
            .await
            .unwrap()
    });
    assert_eq!(blocked, 0, "automatic remediation never parks a card");
    let lead = AgentId::parse("1c2d3e4f").unwrap();
    let memories = fixture.db.memories(&MemoryScope::Agent(lead)).unwrap();
    assert_eq!(memories.len(), 1);
    let path: String = fixture.db.block_on(async {
        sqlx::query_scalar("SELECT path FROM memories WHERE id = ?1")
            .bind(memories[0].memory.id.as_str())
            .fetch_one(fixture.db.pool())
            .await
            .unwrap()
    });
    let recap = std::fs::read_to_string(fixture.db.files().base().join(path)).unwrap();
    assert!(
        recap.starts_with("# Card closed with technical debt"),
        "{recap}"
    );
    assert!(recap.contains(debt_id), "{recap}");
    assert_eq!(provider.requests().len(), 1);
    let _ = fixture.dir;
}

#[test]
fn an_accepted_story_is_integrated_and_the_sandbox_is_materialized() {
    let fixture = fixture(true);
    let sandbox_root = fixture.dir.path().join("sandbox");
    let environment = WorkspaceEnvironment::with_repository(
        fixture.dir.path().join("workspace"),
        sandbox_root.clone(),
        None,
        Duration::from_secs(30),
    );
    let provider = ScriptedProvider::new(vec![
        Ok(response(
            r##"{"actions":[{"tool":"write_file","path":"Playbook.md","content":"# Playbook\n"}],"done":false}"##,
        )),
        Ok(response(
            r#"{"actions":[],"done":true,"summary":"Wrote Playbook.md."}"#,
        )),
        Ok(response(
            r#"{"review_outcome":"Complete.","accepted":true}"#,
        )),
    ]);
    let report = workflow::run(
        &fixture.db,
        &fixture.db,
        &fixture.project,
        &settings(),
        &provider,
        &environment,
        ExecutionLimits::default(),
        &TestClock::default(),
        CancelToken::default(),
        RunId::generate(),
    )
    .unwrap();
    assert_eq!(report.completed, 1);
    let base = fixture.dir.path().join("workspace/0b1c2d3e");
    let commit = story(&fixture).outcome.unwrap().commit_sha.unwrap();
    let ancestor = Command::new("git")
        .current_dir(&base)
        .args(["merge-base", "--is-ancestor", &commit, "develop"])
        .status()
        .unwrap();
    assert!(ancestor.success(), "story commit is on develop");
    assert_eq!(
        std::fs::read_to_string(sandbox_root.join("0b1c2d3e/Playbook.md")).unwrap(),
        "# Playbook\n"
    );
    let (integrated, push_ok): (Option<String>, i64) = fixture.db.block_on(async {
        sqlx::query_as(
            "SELECT integrated_at, push_ok FROM story_outcomes WHERE story_id = '5a617283'",
        )
        .fetch_one(fixture.db.pool())
        .await
        .unwrap()
    });
    assert!(integrated.is_some());
    assert_eq!(push_ok, 1, "a project without a remote needs no push");
    let _ = fixture.dir;
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
