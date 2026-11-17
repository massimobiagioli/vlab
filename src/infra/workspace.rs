//! Git checkout adapter for project workspaces (ADR-0032).
//!
//! The adapter runs `git` directly, without a shell. The GitHub token is
//! provided through a short-lived askpass script and the child environment; it
//! never appears in arguments, URLs, output, or errors.

use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use crate::app::workspace::{ImportedFolder, ProjectWorkspace, WorkspaceError};
use crate::domain::ids::ProjectId;
use crate::domain::project::{FolderSource, FolderSourceError, GithubSource};
use crate::infra::paths::{PathError, create_private_file, ensure_private_dir};

/// Default upper bound for one clone attempt.
pub const DEFAULT_CLONE_TIMEOUT: Duration = Duration::from_secs(300);

/// Default upper bound for one fetch or push attempt (ADR-0070).
pub const DEFAULT_REMOTE_TIMEOUT: Duration = Duration::from_secs(120);

/// Upper bound for one local folder import.
const MAX_IMPORT_ENTRIES: u64 = 100_000;
const MAX_IMPORT_BYTES: u64 = 1024 * 1024 * 1024;
const MAX_IMPORT_DEPTH: u32 = 64;
const GIT_IMPORT_TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Default)]
struct CopyBudget {
    entries: u64,
    bytes: u64,
}

pub(crate) const ASKPASS: &str = r#"#!/bin/sh
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

    fn import_folder(
        &self,
        source: &FolderSource,
        id: &ProjectId,
    ) -> Result<ImportedFolder, WorkspaceError> {
        self.import_folder_impl(source, id)
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

    fn import_folder_impl(
        &self,
        source: &FolderSource,
        id: &ProjectId,
    ) -> Result<ImportedFolder, WorkspaceError> {
        ensure_private_dir(&self.root).map_err(map_path_error)?;
        let root =
            fs::canonicalize(&self.root).map_err(|error| WorkspaceError::Io(error.kind()))?;
        let source_path = fs::canonicalize(source.path()).map_err(|error| match error.kind() {
            io::ErrorKind::NotFound => WorkspaceError::SourceNotFound,
            _ => WorkspaceError::Io(error.kind()),
        })?;
        let metadata = fs::metadata(&source_path).map_err(|error| match error.kind() {
            io::ErrorKind::NotFound => WorkspaceError::SourceNotFound,
            _ => WorkspaceError::Io(error.kind()),
        })?;
        if !metadata.is_dir() {
            return Err(WorkspaceError::SourceNotDirectory);
        }
        if source_path.file_name() == Some(OsStr::new(".git")) {
            return Err(WorkspaceError::UnsupportedEntry);
        }
        // A source that contains the workspace, or lives inside it, would copy
        // the destination into itself or duplicate VLab data.
        if source_path.starts_with(&root) || root.starts_with(&source_path) {
            return Err(WorkspaceError::SourceOverlapsWorkspace);
        }
        let canonical = source_path.to_str().ok_or(WorkspaceError::SourceNotUtf8)?;
        let canonical = FolderSource::parse(canonical).map_err(|error| match error {
            FolderSourceError::TooLong => WorkspaceError::SourceTooLarge,
            _ => WorkspaceError::SourceNotUtf8,
        })?;
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
        let result = self
            .copy_folder(&source_path, &checkout)
            .and_then(|()| self.init_repository(&checkout))
            .and_then(|()| {
                fs::rename(&checkout, &destination)
                    .map_err(|error| WorkspaceError::Io(error.kind()))?;
                Ok(ImportedFolder {
                    source: canonical,
                    path: destination,
                })
            });
        let _ = fs::remove_dir_all(&temporary);
        result
    }

    fn copy_folder(&self, source: &Path, destination: &Path) -> Result<(), WorkspaceError> {
        let mut budget = CopyBudget::default();
        self.copy_dir(source, destination, 0, &mut budget)
    }

    fn copy_dir(
        &self,
        source: &Path,
        destination: &Path,
        depth: u32,
        budget: &mut CopyBudget,
    ) -> Result<(), WorkspaceError> {
        if depth > MAX_IMPORT_DEPTH {
            return Err(WorkspaceError::SourceTooLarge);
        }
        fs::create_dir_all(destination).map_err(|error| WorkspaceError::Io(error.kind()))?;
        let entries = fs::read_dir(source).map_err(|error| WorkspaceError::Io(error.kind()))?;
        for entry in entries {
            let entry = entry.map_err(|error| WorkspaceError::Io(error.kind()))?;
            // A nested repository is not copied; the imported copy gets its
            // own fresh repository below.
            if entry.file_name() == OsStr::new(".git") {
                continue;
            }
            let file_type = entry
                .file_type()
                .map_err(|error| WorkspaceError::Io(error.kind()))?;
            budget.entries = budget.entries.saturating_add(1);
            if budget.entries > MAX_IMPORT_ENTRIES {
                return Err(WorkspaceError::SourceTooLarge);
            }
            let target = destination.join(entry.file_name());
            if file_type.is_dir() {
                self.copy_dir(&entry.path(), &target, depth + 1, budget)?;
            } else if file_type.is_file() {
                let length = entry
                    .metadata()
                    .map_err(|error| WorkspaceError::Io(error.kind()))?
                    .len();
                budget.bytes = budget.bytes.saturating_add(length);
                if budget.bytes > MAX_IMPORT_BYTES {
                    return Err(WorkspaceError::SourceTooLarge);
                }
                fs::copy(entry.path(), &target)
                    .map_err(|error| WorkspaceError::Io(error.kind()))?;
            } else {
                return Err(WorkspaceError::UnsupportedEntry);
            }
        }
        Ok(())
    }

    /// Creates the internal repository that backs worktrees. Force-adding keeps
    /// files hidden by a copied `.gitignore` in the initial checkout.
    fn init_repository(&self, root: &Path) -> Result<(), WorkspaceError> {
        self.run_git(root, &["init", "--quiet"])?;
        self.run_git(root, &["add", "-A", "-f"])?;
        self.run_git(
            root,
            &[
                "-c",
                "user.name=VLab",
                "-c",
                "user.email=vlab@localhost",
                "commit",
                "--no-gpg-sign",
                "--allow-empty",
                "-m",
                "vlab: import folder source",
            ],
        )
    }

    fn run_git(&self, root: &Path, args: &[&str]) -> Result<(), WorkspaceError> {
        let mut command = Command::new("git");
        command
            .args(args)
            .current_dir(root)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        let status =
            wait_with_timeout(&mut command, GIT_IMPORT_TIMEOUT).map_err(|error| match error {
                WorkspaceError::GitUnavailable => WorkspaceError::GitUnavailable,
                _ => WorkspaceError::ImportFailed,
            })?;
        if status.success() {
            Ok(())
        } else {
            Err(WorkspaceError::ImportFailed)
        }
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

    fn folder_source(path: &Path) -> FolderSource {
        FolderSource::parse(path.to_str().unwrap()).unwrap()
    }

    #[test]
    fn a_folder_source_is_copied_and_initialized_as_a_local_repository() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source");
        fs::create_dir_all(source.join("src")).unwrap();
        fs::write(source.join("README.md"), "hello").unwrap();
        fs::write(source.join("src/main.rs"), "fn main() {}").unwrap();
        fs::create_dir_all(source.join(".git")).unwrap();
        fs::write(source.join(".git/MARKER"), "not copied").unwrap();
        let workspace_root = private_dir(root.path());

        let imported = workspace(&workspace_root)
            .import_folder(&folder_source(&source), &project("a3f91c07"))
            .unwrap();
        assert_eq!(imported.path, workspace_root.join("a3f91c07"));
        assert_eq!(
            imported.source.as_str(),
            fs::canonicalize(&source).unwrap().to_str().unwrap()
        );
        assert_eq!(
            fs::read_to_string(imported.path.join("README.md")).unwrap(),
            "hello"
        );
        assert!(imported.path.join("src/main.rs").is_file());
        assert!(imported.path.join(".git").is_dir());
        assert!(!imported.path.join(".git/MARKER").exists());
        let log = Command::new("git")
            .args([
                "-C",
                imported.path.to_str().unwrap(),
                "log",
                "--format=%s",
                "-1",
            ])
            .output()
            .unwrap();
        assert!(log.status.success());
        assert!(
            String::from_utf8_lossy(&log.stdout).contains("import folder source"),
            "{:?}",
            log.stdout
        );
    }

    #[test]
    fn an_empty_folder_gets_an_empty_initial_commit() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("empty");
        fs::create_dir(&source).unwrap();
        let workspace_root = private_dir(root.path());
        let imported = workspace(&workspace_root)
            .import_folder(&folder_source(&source), &project("a3f91c07"))
            .unwrap();
        let log = Command::new("git")
            .args([
                "-C",
                imported.path.to_str().unwrap(),
                "log",
                "--format=%s",
                "-1",
            ])
            .output()
            .unwrap();
        assert!(log.status.success());
    }

    #[test]
    fn missing_files_and_overlapping_sources_are_rejected() {
        let root = tempfile::tempdir().unwrap();
        let workspace_root = private_dir(root.path());
        let error = workspace(&workspace_root)
            .import_folder(
                &folder_source(&root.path().join("missing")),
                &project("a3f91c07"),
            )
            .unwrap_err();
        assert_eq!(error, WorkspaceError::SourceNotFound);

        let file = root.path().join("file.txt");
        fs::write(&file, "x").unwrap();
        let error = workspace(&workspace_root)
            .import_folder(&folder_source(&file), &project("a3f91c07"))
            .unwrap_err();
        assert_eq!(error, WorkspaceError::SourceNotDirectory);

        for source in [workspace_root.clone(), root.path().to_path_buf()] {
            let error = workspace(&workspace_root)
                .import_folder(&folder_source(&source), &project("a3f91c07"))
                .unwrap_err();
            assert_eq!(error, WorkspaceError::SourceOverlapsWorkspace);
        }
    }

    #[cfg(unix)]
    #[test]
    fn a_symbolic_link_in_the_source_is_rejected_and_cleaned_up() {
        use std::os::unix::fs::symlink;

        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source");
        fs::create_dir(&source).unwrap();
        symlink("/etc/passwd", source.join("link")).unwrap();
        let workspace_root = private_dir(root.path());
        let error = workspace(&workspace_root)
            .import_folder(&folder_source(&source), &project("a3f91c07"))
            .unwrap_err();
        assert_eq!(error, WorkspaceError::UnsupportedEntry);
        assert!(!workspace_root.join("a3f91c07").exists());
        assert_eq!(fs::read_dir(&workspace_root).unwrap().count(), 0);
    }

    #[test]
    fn an_existing_destination_is_never_overwritten_for_a_folder_import() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source");
        fs::create_dir(&source).unwrap();
        let workspace_root = private_dir(root.path());
        fs::create_dir(workspace_root.join("a3f91c07")).unwrap();
        let error = workspace(&workspace_root)
            .import_folder(&folder_source(&source), &project("a3f91c07"))
            .unwrap_err();
        assert_eq!(error, WorkspaceError::DestinationExists);
    }

    #[test]
    fn a_folder_deeper_than_the_limit_is_rejected() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("deep");
        fs::create_dir(&source).unwrap();
        let mut path = source.clone();
        for _ in 0..=MAX_IMPORT_DEPTH + 1 {
            path.push("d");
            fs::create_dir(&path).unwrap();
        }
        let workspace_root = private_dir(root.path());
        let error = workspace(&workspace_root)
            .import_folder(&folder_source(&source), &project("a3f91c07"))
            .unwrap_err();
        assert_eq!(error, WorkspaceError::SourceTooLarge);
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
