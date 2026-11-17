//! `develop` integration and testable sandbox checkouts (ADR-0070).
//!
//! The adapter runs `git` without a shell. Remote operations reuse the private
//! askpass mechanism: the token lives only in the child environment and never
//! appears in arguments, URLs, output, or errors.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use crate::app::worktrees::{
    DevelopIntegration, ProjectRepository, RepositoryError, SandboxCheckout,
};
use crate::domain::ids::{ProjectId, StoryId};
use crate::infra::paths::{PathError, create_private_file, ensure_private_dir};
use crate::infra::workspace::ASKPASS;

const MAX_OUTPUT_CHARS: usize = 64 * 1024;
const INTEGRATION_DIR: &str = ".integrate";

/// Integrates story commits into `develop` and refreshes `.sandbox` checkouts.
#[derive(Clone)]
pub struct GitRepository {
    workspace_root: PathBuf,
    sandbox_root: PathBuf,
    token: Option<String>,
    timeout: Duration,
}

impl GitRepository {
    pub fn new(
        workspace_root: PathBuf,
        sandbox_root: PathBuf,
        token: Option<String>,
        timeout: Duration,
    ) -> Self {
        Self {
            workspace_root,
            sandbox_root,
            token,
            timeout,
        }
    }

    pub fn workspace_root(&self) -> &Path {
        &self.workspace_root
    }

    pub fn sandbox_root(&self) -> &Path {
        &self.sandbox_root
    }

    fn base(&self, project: &ProjectId) -> PathBuf {
        self.workspace_root.join(project.as_str())
    }

    fn sandbox_path(&self, project: &ProjectId) -> PathBuf {
        self.sandbox_root.join(project.as_str())
    }

    fn git(&self, cwd: &Path, args: &[&str]) -> Result<(bool, String), RepositoryError> {
        let output = Command::new("git")
            .args(args)
            .current_dir(cwd)
            .stdin(Stdio::null())
            .output()
            .map_err(map_spawn_error)?;
        let text: String = String::from_utf8_lossy(&output.stdout)
            .chars()
            .take(MAX_OUTPUT_CHARS)
            .collect();
        Ok((output.status.success(), text))
    }

    fn git_remote(&self, cwd: &Path, args: &[&str]) -> Result<bool, RepositoryError> {
        let askpass = self.askpass()?;
        let mut command = Command::new("git");
        command
            .current_dir(cwd)
            .arg("-c")
            .arg("credential.helper=")
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .env("GIT_TERMINAL_PROMPT", "0");
        if let Some(path) = &askpass.path {
            command
                .env("GIT_ASKPASS", path)
                .env("GIT_ASKPASS_REQUIRE", "force")
                .env("VLAB_GIT_TOKEN", self.token.as_deref().unwrap_or_default());
        }
        wait_with_timeout(&mut command, self.timeout)
    }

    fn askpass(&self) -> Result<Askpass, RepositoryError> {
        let Some(_) = self.token.as_deref().filter(|token| !token.is_empty()) else {
            return Ok(Askpass { path: None });
        };
        let path =
            std::env::temp_dir().join(format!("vlab-git-{}.sh", uuid::Uuid::new_v4().simple()));
        let mut file = create_private_file(&path).map_err(map_path_error)?;
        use std::io::Write;
        file.write_all(ASKPASS.as_bytes())
            .map_err(|error| RepositoryError::Io(error.kind()))?;
        file.sync_all()
            .map_err(|error| RepositoryError::Io(error.kind()))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o700))
                .map_err(|error| RepositoryError::Io(error.kind()))?;
        }
        Ok(Askpass { path: Some(path) })
    }

    fn is_repository(&self, path: &Path) -> Result<bool, RepositoryError> {
        if !path.is_dir() {
            return Ok(false);
        }
        let (ok, output) = self.git(path, &["rev-parse", "--git-dir"])?;
        Ok(ok && !output.trim().is_empty())
    }

    fn ref_exists(&self, cwd: &Path, reference: &str) -> Result<bool, RepositoryError> {
        let (ok, output) = self.git(cwd, &["rev-parse", "--verify", "--quiet", reference])?;
        Ok(ok && !output.trim().is_empty())
    }

    fn is_ancestor(
        &self,
        cwd: &Path,
        ancestor: &str,
        descendant: &str,
    ) -> Result<bool, RepositoryError> {
        let (ok, _) = self.git(cwd, &["merge-base", "--is-ancestor", ancestor, descendant])?;
        Ok(ok)
    }

    fn rev_parse(&self, cwd: &Path, reference: &str) -> Result<String, RepositoryError> {
        let (ok, output) = self.git(cwd, &["rev-parse", reference])?;
        if !ok || output.trim().is_empty() {
            return Err(RepositoryError::Git);
        }
        Ok(output.trim().to_owned())
    }

    fn has_origin(&self, base: &Path) -> Result<bool, RepositoryError> {
        let (ok, output) = self.git(base, &["remote"])?;
        Ok(ok && output.lines().any(|line| line.trim() == "origin"))
    }

    fn fetch_origin(&self, base: &Path, branch: Option<&str>) {
        let mut args = vec!["fetch", "--quiet", "origin"];
        if let Some(branch) = branch {
            args.push(branch);
        }
        let _ = self.git_remote(base, &args);
    }

    fn default_ref(&self, base: &Path) -> Result<String, RepositoryError> {
        if let Ok((true, output)) = self.git(
            base,
            &["symbolic-ref", "--short", "refs/remotes/origin/HEAD"],
        ) {
            let reference = output.trim();
            if !reference.is_empty() {
                return Ok(reference.to_owned());
            }
        }
        for candidate in ["refs/heads/main", "refs/heads/master"] {
            if self.ref_exists(base, candidate)? {
                return Ok(candidate.to_owned());
            }
        }
        Ok("HEAD".to_owned())
    }

    fn ensure_develop(&self, base: &Path, remote: bool) -> Result<(), RepositoryError> {
        if self.ref_exists(base, "refs/heads/develop")? {
            return Ok(());
        }
        let source = if remote && self.ref_exists(base, "refs/remotes/origin/develop")? {
            "origin/develop".to_owned()
        } else {
            self.default_ref(base)?
        };
        let (ok, _) = self.git(base, &["branch", "develop", &source])?;
        if ok {
            Ok(())
        } else {
            Err(RepositoryError::NoDevelop)
        }
    }

    fn reconcile_develop(&self, base: &Path, remote: bool) -> Result<(), RepositoryError> {
        if !remote || !self.ref_exists(base, "refs/remotes/origin/develop")? {
            return Ok(());
        }
        if !self.ref_exists(base, "refs/heads/develop")? {
            let (ok, _) = self.git(base, &["branch", "develop", "origin/develop"])?;
            return if ok {
                Ok(())
            } else {
                Err(RepositoryError::NoDevelop)
            };
        }
        if self.is_ancestor(base, "develop", "origin/develop")? {
            let (ok, _) = self.git(base, &["branch", "-f", "develop", "origin/develop"])?;
            if !ok {
                return Err(RepositoryError::Git);
            }
        } else if !self.is_ancestor(base, "origin/develop", "develop")? {
            return Err(RepositoryError::Conflict);
        }
        Ok(())
    }

    fn merge_develop(
        &self,
        base: &Path,
        story: &StoryId,
        commit: &str,
    ) -> Result<String, RepositoryError> {
        let directory = self.workspace_root.join(INTEGRATION_DIR);
        fs::create_dir_all(&directory).map_err(|error| RepositoryError::Io(error.kind()))?;
        let temporary = directory.join(story.as_str());
        let _ = fs::remove_dir_all(&temporary);
        let temporary_text = temporary.to_string_lossy().into_owned();
        let _ = self.git(base, &["worktree", "prune"]);
        let (added, _) = self.git(
            base,
            &[
                "worktree",
                "add",
                "--quiet",
                "--force",
                &temporary_text,
                "develop",
            ],
        )?;
        if !added {
            let _ = fs::remove_dir_all(&temporary);
            return Err(RepositoryError::Git);
        }
        let message = format!("vlab: integrate story {story}");
        let (merged, _) = self.git(
            &temporary,
            &[
                "-c",
                "user.name=VLab",
                "-c",
                "user.email=vlab@localhost",
                "merge",
                "--no-ff",
                "--no-edit",
                "-m",
                &message,
                commit,
            ],
        )?;
        if !merged {
            let (_, conflicts) = self
                .git(&temporary, &["diff", "--name-only", "--diff-filter=U"])
                .unwrap_or((false, String::new()));
            let _ = self.git(&temporary, &["merge", "--abort"]);
            let _ = self.git(base, &["worktree", "remove", "--force", &temporary_text]);
            let _ = fs::remove_dir_all(&temporary);
            return Err(if conflicts.trim().is_empty() {
                RepositoryError::Git
            } else {
                RepositoryError::Conflict
            });
        }
        let merge_commit = match self.rev_parse(&temporary, "HEAD") {
            Ok(commit) => commit,
            Err(error) => {
                let _ = self.git(base, &["worktree", "remove", "--force", &temporary_text]);
                let _ = fs::remove_dir_all(&temporary);
                return Err(error);
            }
        };
        let _ = self.git(base, &["worktree", "remove", "--force", &temporary_text]);
        let _ = fs::remove_dir_all(&temporary);
        Ok(merge_commit)
    }

    fn push_develop(&self, base: &Path, remote: bool) -> Result<bool, RepositoryError> {
        if !remote {
            return Ok(false);
        }
        let pushed = self.git_remote(
            base,
            &[
                "push",
                "--quiet",
                "origin",
                "refs/heads/develop:refs/heads/develop",
            ],
        )?;
        Ok(pushed)
    }

    fn clone_sandbox(
        &self,
        base: &Path,
        sandbox: &Path,
    ) -> Result<SandboxCheckout, RepositoryError> {
        ensure_private_dir(&self.sandbox_root).map_err(map_path_error)?;
        let temporary = self
            .sandbox_root
            .join(format!(".tmp-{}", uuid::Uuid::new_v4().simple()));
        ensure_private_dir(&temporary).map_err(map_path_error)?;
        let checkout = temporary.join("checkout");
        let base_text = base.to_string_lossy().into_owned();
        let checkout_text = checkout.to_string_lossy().into_owned();
        let (cloned, _) = self.git(
            &self.sandbox_root,
            &[
                "clone",
                "--quiet",
                "--branch",
                "develop",
                "--single-branch",
                "--",
                &base_text,
                &checkout_text,
            ],
        )?;
        if !cloned {
            let _ = fs::remove_dir_all(&temporary);
            return Err(RepositoryError::Git);
        }
        if let Err(error) = fs::rename(&checkout, sandbox) {
            let _ = fs::remove_dir_all(&temporary);
            return Err(RepositoryError::Io(error.kind()));
        }
        let _ = fs::remove_dir_all(&temporary);
        Ok(SandboxCheckout {
            path: sandbox.to_path_buf(),
        })
    }
}

impl ProjectRepository for GitRepository {
    fn integrate_story(
        &self,
        project: &ProjectId,
        story: &StoryId,
        _branch: &str,
        commit: &str,
    ) -> Result<DevelopIntegration, RepositoryError> {
        let base = self.base(project);
        if !self.is_repository(&base)? {
            return Err(RepositoryError::NotCloned);
        }
        let revision = format!("{commit}^{{commit}}");
        if !self.ref_exists(&base, &revision)? {
            return Err(RepositoryError::Git);
        }
        let remote = self.has_origin(&base)?;
        if remote {
            self.fetch_origin(&base, None);
        }
        self.ensure_develop(&base, remote)?;
        self.reconcile_develop(&base, remote)?;
        let already_integrated = self.is_ancestor(&base, commit, "develop")?;
        let merge_commit = if already_integrated {
            self.rev_parse(&base, "develop")?
        } else {
            self.merge_develop(&base, story, commit)?
        };
        let pushed = self.push_develop(&base, remote)?;
        Ok(DevelopIntegration {
            merge_commit,
            pushed,
            remote,
            already_integrated,
        })
    }

    fn sync_sandbox(&self, project: &ProjectId) -> Result<SandboxCheckout, RepositoryError> {
        let base = self.base(project);
        if !self.is_repository(&base)? {
            return Err(RepositoryError::NotCloned);
        }
        ensure_private_dir(&self.sandbox_root).map_err(map_path_error)?;
        let remote = self.has_origin(&base)?;
        if remote {
            self.fetch_origin(&base, Some("develop"));
        }
        if !self.ref_exists(&base, "refs/heads/develop")? {
            self.ensure_develop(&base, remote)?;
        }
        if remote {
            let _ = self.reconcile_develop(&base, true);
        }
        let sandbox = self.sandbox_path(project);
        match fs::symlink_metadata(&sandbox) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(RepositoryError::Confined);
            }
            Ok(metadata) if metadata.is_dir() => {}
            Ok(_) => return Err(RepositoryError::Git),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return self.clone_sandbox(&base, &sandbox);
            }
            Err(error) => return Err(RepositoryError::Io(error.kind())),
        }
        if !self.is_repository(&sandbox)? {
            return Err(RepositoryError::Git);
        }
        let _ = self.git(&sandbox, &["fetch", "--quiet", "origin"]);
        let target = if self.ref_exists(&sandbox, "refs/remotes/origin/develop")? {
            "origin/develop"
        } else {
            "develop"
        };
        let (checked_out, _) = self.git(
            &sandbox,
            &["checkout", "--quiet", "--force", "-B", "develop", target],
        )?;
        if !checked_out {
            return Err(RepositoryError::Git);
        }
        let (reset, _) = self.git(&sandbox, &["reset", "--hard", "--quiet", target])?;
        if !reset {
            return Err(RepositoryError::Git);
        }
        let _ = self.git(&sandbox, &["clean", "-fdq"]);
        Ok(SandboxCheckout { path: sandbox })
    }
}

struct Askpass {
    path: Option<PathBuf>,
}

impl Drop for Askpass {
    fn drop(&mut self) {
        if let Some(path) = &self.path {
            let _ = fs::remove_file(path);
        }
    }
}

fn wait_with_timeout(command: &mut Command, timeout: Duration) -> Result<bool, RepositoryError> {
    let mut child = command.spawn().map_err(map_spawn_error)?;
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Ok(status.success()),
            Ok(None) => {}
            Err(error) => return Err(RepositoryError::Io(error.kind())),
        }
        if started.elapsed() >= timeout {
            let _ = child.kill();
            let _ = child.wait();
            return Err(RepositoryError::Timeout);
        }
        thread::sleep(Duration::from_millis(50));
    }
}

fn map_spawn_error(error: io::Error) -> RepositoryError {
    if error.kind() == io::ErrorKind::NotFound {
        RepositoryError::GitUnavailable
    } else {
        RepositoryError::Io(error.kind())
    }
}

fn map_path_error(error: PathError) -> RepositoryError {
    match error {
        PathError::Io { kind, .. } => RepositoryError::Io(kind),
        PathError::NotPrivate { .. } | PathError::NotADirectory { .. } => {
            RepositoryError::Io(io::ErrorKind::PermissionDenied)
        }
        _ => RepositoryError::Io(io::ErrorKind::Other),
    }
}
