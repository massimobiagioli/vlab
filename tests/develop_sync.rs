//! Active-laboratory develop reconciliation (ADR-0070).

mod support;

use std::cell::RefCell;
use std::path::PathBuf;

use vlab::app::context::save_active_lab;
use vlab::app::directory::Directory;
use vlab::app::integration;
use vlab::app::projects::ProjectStore;
use vlab::app::worktrees::{
    DevelopIntegration, ProjectRepository, RepositoryError, SandboxCheckout,
};
use vlab::domain::agent::{AgentProfile, Description, Role};
use vlab::domain::ids::{AgentId, LabId, ProjectId, StoryId};
use vlab::domain::lab::Lab;
use vlab::domain::name::Name;
use vlab::domain::project::{FolderSource, Project, ProjectSource};
use vlab::domain::settings::{ModelId, ProviderId};
use vlab::domain::skills;
use vlab::infra::db::Database;
use vlab::obs::RunId;

struct Fixture {
    _dir: tempfile::TempDir,
    db: Database,
    github: ProjectId,
    folder: ProjectId,
    empty: ProjectId,
}

fn fixture() -> Fixture {
    let dir = support::private_tempdir();
    let db = Database::open(dir.path().join("vlab.db")).unwrap();
    db.migrate(RunId::generate()).unwrap();
    let lab = LabId::parse("a3f91c07").unwrap();
    db.insert_lab(&Lab::new(lab.clone(), Name::parse("Payments").unwrap()))
        .unwrap();
    save_active_lab(&db, &lab).unwrap();
    let reviewer = AgentId::parse("2d3e4f50").unwrap();
    db.insert_agent(
        &AgentProfile::new(
            reviewer,
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

    let github = ProjectId::parse("0b1c2d3e").unwrap();
    db.insert_project(&Project::new(
        github.clone(),
        lab.clone(),
        Name::parse("Portal").unwrap(),
        Some(vlab::domain::project::GithubSource::parse("https://github.com/acme/portal").unwrap()),
    ))
    .unwrap();
    let folder = ProjectId::parse("1c2d3e4f").unwrap();
    db.insert_project(&Project::with_source(
        folder.clone(),
        lab.clone(),
        Name::parse("Local").unwrap(),
        ProjectSource::Folder(
            FolderSource::parse(&dir.path().join("source").to_string_lossy()).unwrap(),
        ),
    ))
    .unwrap();
    let empty = ProjectId::parse("2c3d4e5f").unwrap();
    db.insert_project(&Project::with_source(
        empty.clone(),
        lab.clone(),
        Name::parse("Blank").unwrap(),
        ProjectSource::None,
    ))
    .unwrap();

    for (project, story, branch, commit) in [
        (&github, "11111111", "vlab/11111111", "aaaaaaa1"),
        (&github, "22222222", "vlab/22222222", "aaaaaaa2"),
        (&github, "33333333", "vlab/33333333", "aaaaaaa3"),
        (&folder, "44444444", "vlab/44444444", "aaaaaaa4"),
    ] {
        let request = format!("req{}", &story[..6]);
        db.block_on(async {
            sqlx::query(
                "INSERT INTO requests (id, project_id, content, status, created_at, updated_at) \
                 VALUES (?1, ?2, 'R', 'PLANNED', '2026-01-01T00:00:00Z', \
                 '2026-01-01T00:00:00Z')",
            )
            .bind(&request)
            .bind(project.as_str())
            .execute(db.pool())
            .await
            .unwrap();
            sqlx::query(
                "INSERT INTO stories (id, project_id, request_id, title, description, \
                 acceptance_criteria_json, priority, complexity, required_skills_json, \
                 assigned_agent_id, assignment_rationale, state, due_date, created_at, updated_at) \
                 VALUES (?1, ?2, ?3, 'T', 'D', '[\"A\"]', 'high', 'small', '[]', '2d3e4f50', \
                 NULL, 'DONE', '2026-01-03', '2026-01-01T00:00:00Z', \
                 '2026-01-01T00:00:00Z')",
            )
            .bind(story)
            .bind(project.as_str())
            .bind(&request)
            .execute(db.pool())
            .await
            .unwrap();
            sqlx::query(
                "INSERT INTO story_outcomes (story_id, work_outcome, review_outcome, \
                 reviewer_agent_id, review_accepted, branch, worktree_path, commit_sha) \
                 VALUES (?1, 'W', 'R', '2d3e4f50', 1, ?2, ?3, ?4)",
            )
            .bind(story)
            .bind(branch)
            .bind(format!(".worktrees/{story}"))
            .bind(commit)
            .execute(db.pool())
            .await
            .unwrap();
        });
    }

    Fixture {
        _dir: dir,
        db,
        github,
        folder,
        empty,
    }
}

#[derive(Default)]
struct FakeRepository {
    integrations: RefCell<Vec<StoryId>>,
    sandboxes: RefCell<Vec<ProjectId>>,
}

impl ProjectRepository for FakeRepository {
    fn integrate_story(
        &self,
        _project: &ProjectId,
        story: &StoryId,
        _branch: &str,
        _commit: &str,
    ) -> Result<DevelopIntegration, RepositoryError> {
        self.integrations.borrow_mut().push(story.clone());
        match story.as_str() {
            "11111111" => Ok(DevelopIntegration {
                merge_commit: "bbbbbbb1".to_owned(),
                pushed: true,
                remote: true,
                already_integrated: false,
            }),
            "22222222" => Ok(DevelopIntegration {
                merge_commit: "bbbbbbb2".to_owned(),
                pushed: false,
                remote: true,
                already_integrated: false,
            }),
            "33333333" => Err(RepositoryError::Conflict),
            "44444444" => Ok(DevelopIntegration {
                merge_commit: "bbbbbbb4".to_owned(),
                pushed: false,
                remote: false,
                already_integrated: false,
            }),
            _ => Err(RepositoryError::Git),
        }
    }

    fn sync_sandbox(&self, project: &ProjectId) -> Result<SandboxCheckout, RepositoryError> {
        self.sandboxes.borrow_mut().push(project.clone());
        Ok(SandboxCheckout {
            path: PathBuf::from("/tmp/sandbox").join(project.as_str()),
        })
    }
}

#[test]
fn every_source_project_is_integrated_and_synced() {
    let fixture = fixture();
    let repository = FakeRepository::default();
    let report =
        integration::sync_active_lab(&fixture.db, &fixture.db, &repository, RunId::generate())
            .unwrap();
    assert_eq!(report.projects, 3);
    assert_eq!(report.integrated, 3);
    assert_eq!(report.push_failed, 1);
    assert_eq!(report.failed, 1);
    assert_eq!(report.synced, 2);
    assert_eq!(report.skipped, 1);
    assert_eq!(repository.integrations.borrow().len(), 4);
    assert_eq!(
        *repository.sandboxes.borrow(),
        vec![fixture.github.clone(), fixture.folder.clone()]
    );

    let state: Vec<(String, Option<String>, i64, Option<String>)> = fixture.db.block_on(async {
        sqlx::query_as(
            "SELECT story_id, integrated_at, push_ok, integration_error FROM story_outcomes \
             ORDER BY story_id",
        )
        .fetch_all(fixture.db.pool())
        .await
        .unwrap()
    });
    assert_eq!(state.len(), 4);
    assert!(state[0].1.is_some() && state[0].2 == 1 && state[0].3.is_none());
    assert!(state[1].1.is_some() && state[1].2 == 0);
    assert_eq!(state[1].3.as_deref(), Some("develop_push_failed"));
    assert!(state[2].1.is_none() && state[2].2 == 0);
    assert_eq!(state[2].3.as_deref(), Some("develop_conflict"));
    assert!(state[3].1.is_some() && state[3].2 == 1);

    let events: Vec<(String, String, Option<String>)> = fixture.db.block_on(async {
        sqlx::query_as(
            "SELECT kind, outcome, error_code FROM events \
             WHERE kind IN ('develop_integrated', 'develop_push_failed', \
             'develop_integration_failed', 'sandbox_synced') ORDER BY kind, error_code",
        )
        .fetch_all(fixture.db.pool())
        .await
        .unwrap()
    });
    assert!(
        events
            .iter()
            .any(|(kind, outcome, _)| { kind == "develop_integrated" && outcome == "success" })
    );
    assert!(events.iter().any(|(kind, _, code)| {
        kind == "develop_push_failed" && code.as_deref() == Some("develop_push_failed")
    }));
    assert!(events.iter().any(|(kind, _, code)| {
        kind == "develop_integration_failed" && code.as_deref() == Some("develop_conflict")
    }));
    assert_eq!(
        events
            .iter()
            .filter(|(kind, _, _)| kind == "sandbox_synced")
            .count(),
        2
    );
    let _ = fixture.empty;
}

#[test]
fn a_successful_integration_is_not_retried() {
    let fixture = fixture();
    let repository = FakeRepository::default();
    integration::sync_active_lab(&fixture.db, &fixture.db, &repository, RunId::generate()).unwrap();
    // Mark the pushed and local-only stories as complete; the conflict stays
    // pending and the push failure stays pending.
    fixture
        .db
        .execute(
            "UPDATE story_outcomes SET push_ok = 1 WHERE story_id IN \
             ('11111111', '44444444')",
        )
        .unwrap();
    let repository = FakeRepository::default();
    integration::sync_active_lab(&fixture.db, &fixture.db, &repository, RunId::generate()).unwrap();
    let calls: Vec<String> = repository
        .integrations
        .borrow()
        .iter()
        .map(|story| story.to_string())
        .collect();
    assert_eq!(calls, ["22222222", "33333333"]);
}
