//! Develop integration and sandbox checkouts (ADR-0070).
//!
//! Every test uses local repositories or local `file://` remotes. No network
//! and no GitHub credential are required.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use vlab::app::worktrees::{ProjectRepository, RepositoryError, WorkflowEnvironment};
use vlab::domain::ids::{ProjectId, StoryId};
use vlab::infra::execution::WorkspaceEnvironment;
use vlab::infra::git_repository::GitRepository;

const PROJECT: &str = "0b1c2d3e";

fn git(cwd: &Path, args: &[&str]) {
    let status = Command::new("git")
        .current_dir(cwd)
        .args(args)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_TERMINAL_PROMPT", "0")
        .status()
        .unwrap();
    assert!(status.success(), "git {args:?} in {}", cwd.display());
}

fn commit(cwd: &Path, message: &str) {
    git(
        cwd,
        &[
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@localhost",
            "commit",
            "--no-gpg-sign",
            "-q",
            "-m",
            message,
        ],
    );
}

fn rev(cwd: &Path, reference: &str) -> String {
    let output = Command::new("git")
        .current_dir(cwd)
        .args(["rev-parse", reference])
        .output()
        .unwrap();
    assert!(output.status.success(), "rev-parse {reference}");
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

fn path_text(path: &Path) -> &str {
    path.to_str().unwrap()
}

struct Fixture {
    dir: tempfile::TempDir,
    base: PathBuf,
    remote: PathBuf,
    sandbox: PathBuf,
}

impl Fixture {
    fn new(with_remote: bool) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let remote = dir.path().join("remote.git");
        let seed = dir.path().join("seed");
        if with_remote {
            git(
                dir.path(),
                &["init", "--bare", "--quiet", path_text(&remote)],
            );
            git(dir.path(), &["init", "--quiet", path_text(&seed)]);
        } else {
            git(dir.path(), &["init", "--quiet", path_text(&seed)]);
        }
        fs::write(seed.join("README.md"), "base\n").unwrap();
        git(&seed, &["add", "."]);
        commit(&seed, "init");
        git(&seed, &["branch", "-M", "main"]);
        if with_remote {
            git(&seed, &["remote", "add", "origin", path_text(&remote)]);
            git(&seed, &["push", "--quiet", "origin", "main"]);
            git(&remote, &["symbolic-ref", "HEAD", "refs/heads/main"]);
        }

        let workspace = dir.path().join("workspace");
        fs::create_dir_all(&workspace).unwrap();
        let base = workspace.join(PROJECT);
        if with_remote {
            git(
                dir.path(),
                &["clone", "--quiet", path_text(&remote), path_text(&base)],
            );
        } else {
            git(
                dir.path(),
                &["clone", "--quiet", path_text(&seed), path_text(&base)],
            );
            git(&base, &["remote", "remove", "origin"]);
        }
        let sandbox = dir.path().join("sandbox");
        Self {
            dir,
            base,
            remote,
            sandbox,
        }
    }

    fn repository(&self) -> GitRepository {
        GitRepository::new(
            self.dir.path().join("workspace"),
            self.sandbox.clone(),
            None,
            Duration::from_secs(30),
        )
    }

    fn story(&self, branch: &str, file: &str, content: &str) -> String {
        git(&self.base, &["checkout", "-q", "main"]);
        git(&self.base, &["checkout", "-q", "-b", branch]);
        fs::write(self.base.join(file), content).unwrap();
        git(&self.base, &["add", "."]);
        commit(&self.base, "story");
        let commit = rev(&self.base, "HEAD");
        git(&self.base, &["checkout", "-q", "main"]);
        commit
    }
}

fn story_id() -> StoryId {
    StoryId::parse("5a617283").unwrap()
}

fn project_id() -> ProjectId {
    ProjectId::parse(PROJECT).unwrap()
}

#[test]
fn a_done_commit_is_merged_into_develop_pushed_and_materialized() {
    let fixture = Fixture::new(true);
    let commit = fixture.story("vlab/5a617283", "story.txt", "story\n");
    let repository = fixture.repository();

    let first = repository
        .integrate_story(&project_id(), &story_id(), "vlab/5a617283", &commit)
        .unwrap();
    assert!(first.remote);
    assert!(first.pushed);
    assert!(!first.already_integrated);
    assert_eq!(rev(&fixture.base, "develop"), first.merge_commit);
    let ancestor = Command::new("git")
        .current_dir(&fixture.base)
        .args(["merge-base", "--is-ancestor", &commit, "develop"])
        .status()
        .unwrap();
    assert!(ancestor.success());
    assert_eq!(
        rev(&fixture.remote, "refs/heads/develop"),
        first.merge_commit
    );

    let checkout = repository.sync_sandbox(&project_id()).unwrap();
    assert_eq!(checkout.path, fixture.sandbox.join(PROJECT));
    assert_eq!(
        fs::read_to_string(checkout.path.join("story.txt")).unwrap(),
        "story\n"
    );

    let second = repository
        .integrate_story(&project_id(), &story_id(), "vlab/5a617283", &commit)
        .unwrap();
    assert!(second.already_integrated);
    assert!(second.pushed);
    assert_eq!(second.merge_commit, first.merge_commit);
    let _ = fixture.dir;
}

#[test]
fn a_sandbox_refresh_discards_local_edits_and_untracked_files() {
    let fixture = Fixture::new(true);
    let commit = fixture.story("vlab/5a617283", "story.txt", "story\n");
    let repository = fixture.repository();
    repository
        .integrate_story(&project_id(), &story_id(), "vlab/5a617283", &commit)
        .unwrap();
    let checkout = repository.sync_sandbox(&project_id()).unwrap();
    fs::write(checkout.path.join("story.txt"), "local edit\n").unwrap();
    fs::write(checkout.path.join("scratch.tmp"), "scratch\n").unwrap();

    repository.sync_sandbox(&project_id()).unwrap();
    assert_eq!(
        fs::read_to_string(checkout.path.join("story.txt")).unwrap(),
        "story\n"
    );
    assert!(!checkout.path.join("scratch.tmp").exists());
    let _ = fixture.dir;
}

#[test]
fn a_sandbox_is_created_from_the_default_branch_before_any_done() {
    let fixture = Fixture::new(true);
    let repository = fixture.repository();
    let checkout = repository.sync_sandbox(&project_id()).unwrap();
    assert_eq!(rev(&fixture.base, "develop"), rev(&fixture.base, "main"));
    assert_eq!(
        fs::read_to_string(checkout.path.join("README.md")).unwrap(),
        "base\n"
    );
    let _ = fixture.dir;
}

#[test]
fn a_conflicting_story_is_rejected_without_touching_develop() {
    let fixture = Fixture::new(true);
    let first = fixture.story("vlab/11111111", "README.md", "first\n");
    let second = fixture.story("vlab/22222222", "README.md", "second\n");
    let repository = fixture.repository();
    let integrated = repository
        .integrate_story(
            &project_id(),
            &StoryId::parse("11111111").unwrap(),
            "vlab/11111111",
            &first,
        )
        .unwrap();
    let error = repository
        .integrate_story(
            &project_id(),
            &StoryId::parse("22222222").unwrap(),
            "vlab/22222222",
            &second,
        )
        .unwrap_err();
    assert_eq!(error, RepositoryError::Conflict);
    assert_eq!(rev(&fixture.base, "develop"), integrated.merge_commit);
    let _ = fixture.dir;
}

#[test]
fn a_project_without_a_remote_integrates_locally() {
    let fixture = Fixture::new(false);
    let commit = fixture.story("vlab/5a617283", "story.txt", "story\n");
    let repository = fixture.repository();
    let integration = repository
        .integrate_story(&project_id(), &story_id(), "vlab/5a617283", &commit)
        .unwrap();
    assert!(!integration.remote);
    assert!(!integration.pushed);
    assert_eq!(rev(&fixture.base, "develop"), integration.merge_commit);
    let checkout = repository.sync_sandbox(&project_id()).unwrap();
    assert!(checkout.path.join("story.txt").is_file());
    let _ = fixture.dir;
}

#[test]
fn a_push_failure_keeps_the_local_merge() {
    let fixture = Fixture::new(true);
    let commit = fixture.story("vlab/5a617283", "story.txt", "story\n");
    git(
        &fixture.base,
        &["remote", "set-url", "origin", "/does/not/exist.git"],
    );
    let repository = fixture.repository();
    let integration = repository
        .integrate_story(&project_id(), &story_id(), "vlab/5a617283", &commit)
        .unwrap();
    assert!(integration.remote);
    assert!(!integration.pushed);
    assert_eq!(rev(&fixture.base, "develop"), integration.merge_commit);
    let _ = fixture.dir;
}

#[test]
fn a_retry_reuses_the_story_worktree_and_branch() {
    let fixture = Fixture::new(false);
    let environment = WorkspaceEnvironment::new(fixture.dir.path().join("workspace"));
    let project = project_id();
    let story = story_id();
    let first = environment.prepare(&project, &story).unwrap();
    fs::write(first.path.join("story.txt"), "first\n").unwrap();
    git(&first.path, &["add", "."]);
    commit(&first.path, "first attempt");
    let head = rev(&first.path, "HEAD");

    let second = environment.prepare(&project, &story).unwrap();
    assert_eq!(second.path, first.path);
    assert_eq!(rev(&second.path, "HEAD"), head);
    assert_eq!(
        fs::read_to_string(second.path.join("story.txt")).unwrap(),
        "first\n"
    );
    let _ = fixture.dir;
}

#[test]
fn a_missing_checkout_is_reported() {
    let fixture = Fixture::new(true);
    let repository = GitRepository::new(
        fixture.dir.path().join("missing"),
        fixture.sandbox.clone(),
        None,
        Duration::from_secs(30),
    );
    assert_eq!(
        repository.integrate_story(
            &project_id(),
            &story_id(),
            "vlab/5a617283",
            "0000000000000000000000000000000000000000",
        ),
        Err(RepositoryError::NotCloned)
    );
    let _ = fixture.dir;
}
