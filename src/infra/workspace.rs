//! Git checkout adapter for project workspaces (ADR-0032).
//!
//! The adapter runs `git` directly, without a shell. The GitHub token is
//! provided through a short-lived askpass script and the child environment; it
//! never appears in arguments, URLs, output, or errors.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use crate::app::workspace::{ProjectWorkspace, WorkspaceError};
use crate::domain::ids::ProjectId;
use crate::domain::project::GithubSource;
use crate::infra::paths::{PathError, create_private_file, ensure_private_dir};

/// Default upper bound for one clone attempt.
pub const DEFAULT_CLONE_TIMEOUT: Duration = Duration::from_secs(300);

const ASKPASS: &str = r#"#!/bin/sh
case "$1" in
  *Username*) printf '%s\n' "x-access-token" ;;
  *Password*) printf '%s\n' "$VLAB_GIT_TOKEN" ;;
  *) exit 1 ;;
esac
"#;

/// Clones projects under a private workspace root.
pub struct GitWorkspace {
    root: PathBuf,
    token: Option<String>,
    timeout: Duration,
}

impl GitWorkspace {
    pub fn new(root: PathBuf, token: Option<String>, timeout: Duration) -> Self {
        Self {
            root,
            token,
            timeout,
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    fn run_clone(
        &self,
        url: &str,
        destination: &Path,
        askpass: Option<&Path>,
    ) -> Result<(), WorkspaceError> {
        let mut command = Command::new("git");
        command
            .arg("-c")
            .arg("credential.helper=")
            .arg("clone")
            .arg("--quiet")
            .arg("--")
            .arg(url)
            .arg(destination)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        if let Some(askpass) = askpass {
            command
                .env("GIT_ASKPASS", askpass)
                .env("GIT_ASKPASS_REQUIRE", "force")
                .env("GIT_TERMINAL_PROMPT", "0")
                .env("VLAB_GIT_TOKEN", self.token.as_deref().unwrap_or_default());
        }
        let status = wait_with_timeout(&mut command, self.timeout)?;
        if status.success() {
            Ok(())
        } else {
            Err(WorkspaceError::Failed)
        }
    }
}

impl ProjectWorkspace for GitWorkspace {
    fn clone_project(
        &self,
        source: &GithubSource,
        id: &ProjectId,
    ) -> Result<PathBuf, WorkspaceError> {
        ensure_private_dir(&self.root).map_err(map_path_error)?;
        let destination = self.root.join(id.as_str());
        match fs::symlink_metadata(&destination) {
            Ok(_) => return Err(WorkspaceError::DestinationExists),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(WorkspaceError::Io(error.kind())),
        }

        let temporary = self
            .root
            .join(format!(".tmp-{}", uuid::Uuid::new_v4().simple()));
        ensure_private_dir(&temporary).map_err(map_path_error)?;
        let result = self.clone_into(source, &temporary, &destination);
        let _ = fs::remove_dir_all(&temporary);
        result
    }

    fn remove_project(&self, path: &Path) {
        let _ = fs::remove_dir_all(path);
    }
}

impl GitWorkspace {
    fn clone_into(
        &self,
        source: &GithubSource,
        temporary: &Path,
        destination: &Path,
    ) -> Result<PathBuf, WorkspaceError> {
        let askpass = match self.token {
            Some(_) => {
                let path = temporary.join("askpass.sh");
                let mut file = create_private_file(&path).map_err(map_path_error)?;
                use std::io::Write;
                file.write_all(ASKPASS.as_bytes())
                    .map_err(|error| WorkspaceError::Io(error.kind()))?;
                file.sync_all()
                    .map_err(|error| WorkspaceError::Io(error.kind()))?;
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    fs::set_permissions(&path, fs::Permissions::from_mode(0o700))
                        .map_err(|error| WorkspaceError::Io(error.kind()))?;
                }
                Some(path)
            }
            None => None,
        };
        let checkout = temporary.join("checkout");
        self.run_clone(source.as_str(), &checkout, askpass.as_deref())?;
        fs::rename(&checkout, destination).map_err(|error| WorkspaceError::Io(error.kind()))?;
        Ok(destination.to_path_buf())
    }
}

fn wait_with_timeout(
    command: &mut Command,
    timeout: Duration,
) -> Result<std::process::ExitStatus, WorkspaceError> {
    let mut child = command.spawn().map_err(|error| {
        if error.kind() == io::ErrorKind::NotFound {
            WorkspaceError::GitUnavailable
        } else {
            WorkspaceError::Io(error.kind())
        }
    })?;
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Ok(status),
            Ok(None) => {}
            Err(error) => return Err(WorkspaceError::Io(error.kind())),
        }
        if started.elapsed() >= timeout {
            let _ = child.kill();
            let _ = child.wait();
            return Err(WorkspaceError::Timeout);
        }
        thread::sleep(Duration::from_millis(50));
    }
}

fn map_path_error(error: PathError) -> WorkspaceError {
    match error {
        PathError::Io { kind, .. } => WorkspaceError::Io(kind),
        PathError::NotPrivate { .. } | PathError::NotADirectory { .. } => {
            WorkspaceError::Io(io::ErrorKind::PermissionDenied)
        }
        _ => WorkspaceError::Io(io::ErrorKind::Other),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn workspace(root: &Path) -> GitWorkspace {
        GitWorkspace::new(root.to_path_buf(), None, Duration::from_secs(30))
    }

    fn source() -> GithubSource {
        GithubSource::parse("https://github.com/acme/portal").unwrap()
    }

    fn project(id: &str) -> ProjectId {
        ProjectId::parse(id).unwrap()
    }

    fn private_dir(root: &Path) -> PathBuf {
        let path = root.join("workspace");
        ensure_private_dir(&path).unwrap();
        path
    }

    #[test]
    fn a_local_repository_is_cloned_into_the_project_directory() {
        let root = tempfile::tempdir().unwrap();
        let repository = root.path().join("source.git");
        let init = Command::new("git")
            .args(["init", "--bare", "--quiet"])
            .arg(&repository)
            .status()
            .unwrap();
        assert!(init.success());
        let root = private_dir(root.path());
        let cloned = workspace(&root)
            .clone_url_for_test(
                &format!("file://{}", repository.display()),
                &project("a3f91c07"),
            )
            .unwrap();
        assert_eq!(cloned, root.join("a3f91c07"));
        assert!(cloned.join(".git").is_dir());
    }

    #[test]
    fn a_failed_clone_leaves_no_temporary_or_project_directory() {
        let root = tempfile::tempdir().unwrap();
        let root = private_dir(root.path());
        let error = workspace(&root)
            .clone_url_for_test("file:///does/not/exist", &project("a3f91c07"))
            .unwrap_err();
        assert_eq!(error, WorkspaceError::Failed);
        assert!(!root.join("a3f91c07").exists());
        assert_eq!(fs::read_dir(&root).unwrap().count(), 0);
    }

    #[test]
    fn an_existing_project_directory_is_never_overwritten() {
        let root = tempfile::tempdir().unwrap();
        let root = private_dir(root.path());
        let existing = root.join("a3f91c07");
        fs::create_dir(&existing).unwrap();
        let error = workspace(&root)
            .clone_project(&source(), &project("a3f91c07"))
            .unwrap_err();
        assert_eq!(error, WorkspaceError::DestinationExists);
        assert!(existing.exists());
    }

    impl GitWorkspace {
        fn clone_url_for_test(&self, url: &str, id: &ProjectId) -> Result<PathBuf, WorkspaceError> {
            ensure_private_dir(&self.root).map_err(map_path_error)?;
            let destination = self.root.join(id.as_str());
            match fs::symlink_metadata(&destination) {
                Ok(_) => return Err(WorkspaceError::DestinationExists),
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(WorkspaceError::Io(error.kind())),
            }
            let temporary = self
                .root
                .join(format!(".tmp-{}", uuid::Uuid::new_v4().simple()));
            ensure_private_dir(&temporary).map_err(map_path_error)?;
            let checkout = temporary.join("checkout");
            let result = self.run_clone(url, &checkout, None).and_then(|()| {
                fs::rename(&checkout, &destination)
                    .map_err(|error| WorkspaceError::Io(error.kind()))?;
                Ok(destination.clone())
            });
            let _ = fs::remove_dir_all(&temporary);
            result
        }
    }
}
