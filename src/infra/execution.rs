//! Confined filesystem and command tools for one worktree (ADR-0050).
//!
//! Every path is resolved against the worktree root and rejected when it
//! escapes. Commands run with a cleared environment, a private `HOME`, a
//! timeout, and capped output. The model never sees these details.

use std::fs::{self, OpenOptions};
use std::io::{self, Read};
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use crate::app::execution::{Action, ActionError, ActionRunner, MAX_COMMAND_CHARS, MAX_FILE_BYTES};
use crate::app::worktrees::{WorkflowEnvironment, Worktree, WorktreeError};
use crate::domain::ids::{ProjectId, StoryId};

const COMMAND_TIMEOUT: Duration = Duration::from_secs(60);
const GIT_TIMEOUT: Duration = Duration::from_secs(60);
const MAX_OUTPUT_BYTES: usize = 64 * 1024;
const MAX_READ_BYTES: u64 = 64 * 1024;
const MAX_LIST_ENTRIES: usize = 200;
const MAX_DIFF_BYTES: usize = 32 * 1024;

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Runs actions inside one worktree.
pub struct WorktreeActions {
    root: PathBuf,
}

impl WorktreeActions {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    fn resolve_existing(&self, path: &str) -> Result<PathBuf, ActionError> {
        let relative = Path::new(path);
        if relative.is_absolute() {
            return Err(ActionError::Confined);
        }
        let candidate = if path.trim().is_empty() || path == "." {
            self.root.clone()
        } else {
            self.root.join(relative)
        };
        let canonical = fs::canonicalize(&candidate).map_err(map_io)?;
        if !canonical.starts_with(&self.root) {
            return Err(ActionError::Confined);
        }
        Ok(canonical)
    }

    fn resolve_for_write(&self, path: &str) -> Result<PathBuf, ActionError> {
        let relative = Path::new(path);
        if relative.as_os_str().is_empty() || relative.is_absolute() {
            return Err(ActionError::Confined);
        }
        for component in relative.components() {
            if !matches!(component, Component::Normal(_)) {
                return Err(ActionError::Confined);
            }
        }
        let candidate = self.root.join(relative);
        let parent = candidate.parent().ok_or(ActionError::Confined)?;
        fs::create_dir_all(parent).map_err(map_io)?;
        let parent = fs::canonicalize(parent).map_err(map_io)?;
        if !parent.starts_with(&self.root) {
            return Err(ActionError::Confined);
        }
        let name = candidate.file_name().ok_or(ActionError::Confined)?;
        let destination = parent.join(name);
        if fs::symlink_metadata(&destination)
            .map(|metadata| metadata.file_type().is_symlink())
            .unwrap_or(false)
        {
            return Err(ActionError::Confined);
        }
        Ok(destination)
    }
}

impl ActionRunner for WorktreeActions {
    fn execute(&self, action: &Action) -> Result<String, ActionError> {
        match action {
            Action::ListFiles { path } => {
                let directory = self.resolve_existing(path)?;
                if !directory.is_dir() {
                    return Err(ActionError::NotFound);
                }
                let mut names = Vec::new();
                for entry in fs::read_dir(&directory).map_err(map_io)? {
                    let entry = entry.map_err(map_io)?;
                    let mut name = entry.file_name().to_string_lossy().into_owned();
                    if entry.path().is_dir() {
                        name.push('/');
                    }
                    names.push(name);
                }
                names.sort();
                names.truncate(MAX_LIST_ENTRIES);
                Ok(names.join("\n"))
            }
            Action::ReadFile { path } => {
                let file = self.resolve_existing(path)?;
                let metadata = fs::metadata(&file).map_err(map_io)?;
                if !metadata.is_file() {
                    return Err(ActionError::NotFound);
                }
                if metadata.len() > MAX_READ_BYTES {
                    return Err(ActionError::TooLarge);
                }
                let mut bytes = Vec::new();
                fs::File::open(&file)
                    .map_err(map_io)?
                    .take(MAX_READ_BYTES + 1)
                    .read_to_end(&mut bytes)
                    .map_err(map_io)?;
                String::from_utf8(bytes).map_err(|_| ActionError::Output)
            }
            Action::WriteFile { path, content } => {
                if content.len() > MAX_FILE_BYTES {
                    return Err(ActionError::TooLarge);
                }
                let file = self.resolve_for_write(path)?;
                fs::write(&file, content.as_bytes()).map_err(map_io)?;
                Ok(format!("wrote {} bytes", content.len()))
            }
            Action::RunCommand { command } => {
                if command.chars().count() > MAX_COMMAND_CHARS {
                    return Err(ActionError::TooLarge);
                }
                let (code, stdout, stderr) =
                    run_shell(&self.root, command, COMMAND_TIMEOUT, MAX_OUTPUT_BYTES)?;
                let mut text = format!("exit {code}\n");
                if !stdout.is_empty() {
                    text.push_str(&stdout);
                    if !text.ends_with('\n') {
                        text.push('\n');
                    }
                }
                if !stderr.is_empty() {
                    text.push_str(&stderr);
                    if !text.ends_with('\n') {
                        text.push('\n');
                    }
                }
                Ok(text)
            }
        }
    }

    fn diff(&self) -> Result<String, ActionError> {
        let (code, stdout, _) = run_process(
            "git",
            &["show", "--format=", "--patch", "HEAD"],
            &self.root,
            GIT_TIMEOUT,
            MAX_DIFF_BYTES,
        )?;
        if code != 0 {
            return Err(ActionError::Git);
        }
        Ok(stdout)
    }

    fn finalize(&self, _summary: &str) -> Result<Option<String>, ActionError> {
        let (code, _, _) = run_process(
            "git",
            &["add", "-A"],
            &self.root,
            GIT_TIMEOUT,
            MAX_OUTPUT_BYTES,
        )?;
        if code != 0 {
            return Err(ActionError::Git);
        }
        let (code, status, _) = run_process(
            "git",
            &["status", "--porcelain"],
            &self.root,
            GIT_TIMEOUT,
            MAX_OUTPUT_BYTES,
        )?;
        if code != 0 {
            return Err(ActionError::Git);
        }
        if status.trim().is_empty() {
            return Ok(None);
        }
        let (code, _, _) = run_process(
            "git",
            &[
                "-c",
                "user.name=VLab",
                "-c",
                "user.email=vlab@localhost",
                "commit",
                "--no-gpg-sign",
                "-m",
                "vlab: complete story",
            ],
            &self.root,
            GIT_TIMEOUT,
            MAX_OUTPUT_BYTES,
        )?;
        if code != 0 {
            return Err(ActionError::Git);
        }
        let (code, hash, _) = run_process(
            "git",
            &["rev-parse", "HEAD"],
            &self.root,
            GIT_TIMEOUT,
            MAX_OUTPUT_BYTES,
        )?;
        if code != 0 {
            return Err(ActionError::Git);
        }
        let hash = hash.trim().to_owned();
        if hash.is_empty() {
            return Err(ActionError::Git);
        }
        Ok(Some(hash))
    }
}

/// Prepares worktrees under `<workspace>/.worktrees/<story-id>`.
pub struct WorkspaceEnvironment {
    root: PathBuf,
}

impl WorkspaceEnvironment {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    fn clone_dir(&self, project: &ProjectId) -> Result<PathBuf, WorktreeError> {
        let root = fs::canonicalize(&self.root).map_err(|error| match error.kind() {
            io::ErrorKind::NotFound => WorktreeError::NotCloned,
            _ => WorktreeError::Io,
        })?;
        let clone = root.join(project.as_str());
        let canonical = fs::canonicalize(&clone).map_err(|error| match error.kind() {
            io::ErrorKind::NotFound => WorktreeError::NotCloned,
            _ => WorktreeError::Io,
        })?;
        if !canonical.is_dir() || !canonical.starts_with(&root) {
            return Err(WorktreeError::Confined);
        }
        Ok(canonical)
    }
}

impl WorkflowEnvironment for WorkspaceEnvironment {
    fn prepare(&self, project: &ProjectId, story: &StoryId) -> Result<Worktree, WorktreeError> {
        let clone = self.clone_dir(project)?;
        let root = clone.parent().ok_or(WorktreeError::Confined)?.to_path_buf();
        let tree_root = root.join(".worktrees");
        fs::create_dir_all(&tree_root).map_err(|_| WorktreeError::Io)?;
        let path = tree_root.join(story.as_str());
        let branch = format!("vlab/{}", story.as_str());

        if path.exists() {
            let _ = run_git(
                &clone,
                &["worktree", "remove", "--force", &path.to_string_lossy()],
            );
            let _ = run_git(&clone, &["branch", "-D", &branch]);
            let _ = fs::remove_dir_all(&path);
        }
        if path.exists() {
            return Err(WorktreeError::Occupied);
        }

        let (code, _, _) = run_git(
            &clone,
            &[
                "worktree",
                "add",
                "-b",
                &branch,
                &path.to_string_lossy(),
                "HEAD",
            ],
        )?;
        if code != 0 {
            return Err(WorktreeError::Git);
        }
        let canonical = fs::canonicalize(&path).map_err(|_| WorktreeError::Io)?;
        if !canonical.starts_with(&root) {
            return Err(WorktreeError::Confined);
        }
        let relative = format!(".worktrees/{}", story.as_str());
        Ok(Worktree {
            path: canonical,
            relative_path: relative,
            branch,
        })
    }

    fn actions(&self, worktree: &Worktree) -> Box<dyn ActionRunner + '_> {
        Box::new(WorktreeActions::new(worktree.path.clone()))
    }
}

fn run_git(cwd: &Path, args: &[&str]) -> Result<(i32, String, String), WorktreeError> {
    match run_process("git", args, cwd, GIT_TIMEOUT, MAX_OUTPUT_BYTES) {
        Ok(output) => Ok(output),
        Err(ActionError::NotFound) => Err(WorktreeError::GitUnavailable),
        Err(_) => Err(WorktreeError::Git),
    }
}

fn map_io(error: io::Error) -> ActionError {
    match error.kind() {
        io::ErrorKind::NotFound => ActionError::NotFound,
        _ => ActionError::Io,
    }
}

fn run_shell(
    cwd: &Path,
    command: &str,
    timeout: Duration,
    max: usize,
) -> Result<(i32, String, String), ActionError> {
    run_process("sh", &["-c", command], cwd, timeout, max)
}

/// Runs a process with a cleared environment, output files, a timeout, and
/// bounded captured output.
fn run_process(
    program: &str,
    args: &[&str],
    cwd: &Path,
    timeout: Duration,
    max: usize,
) -> Result<(i32, String, String), ActionError> {
    let scratch = scratch_dir()?;
    let stdout_path = scratch.join("stdout");
    let stderr_path = scratch.join("stderr");
    let home = scratch.join("home");
    fs::create_dir_all(&home).map_err(map_io)?;
    let stdout_file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&stdout_path)
        .map_err(map_io)?;
    let stderr_file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&stderr_path)
        .map_err(map_io)?;
    let mut command = Command::new(program);
    command
        .args(args)
        .current_dir(cwd)
        .env_clear()
        .env(
            "PATH",
            std::env::var("PATH").unwrap_or_else(|_| "/usr/bin:/bin".to_owned()),
        )
        .env("HOME", &home)
        .env("TMPDIR", &scratch)
        .env("LANG", "C.UTF-8")
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout_file))
        .stderr(Stdio::from(stderr_file));
    let mut child = command.spawn().map_err(|error| match error.kind() {
        io::ErrorKind::NotFound => ActionError::NotFound,
        _ => ActionError::Io,
    })?;
    let started = Instant::now();
    let status = loop {
        match child.try_wait().map_err(map_io)? {
            Some(status) => break status,
            None if started.elapsed() >= timeout => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = fs::remove_dir_all(&scratch);
                return Err(ActionError::Timeout);
            }
            None => thread::sleep(Duration::from_millis(20)),
        }
    };
    let stdout = read_capped(&stdout_path, max)?;
    let stderr = read_capped(&stderr_path, max)?;
    let _ = fs::remove_dir_all(&scratch);
    Ok((status.code().unwrap_or(-1), stdout, stderr))
}

fn read_capped(path: &Path, max: usize) -> Result<String, ActionError> {
    let bytes = fs::read(path).map_err(map_io)?;
    let truncated = bytes.len() > max;
    let slice = &bytes[..bytes.len().min(max)];
    let mut text = String::from_utf8_lossy(slice).into_owned();
    if truncated {
        text.push_str("\n… truncated");
    }
    Ok(text)
}

fn scratch_dir() -> Result<PathBuf, ActionError> {
    let unique = TEMP_COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("vlab-exec-{}-{unique}", std::process::id()));
    fs::create_dir_all(&dir).map_err(map_io)?;
    Ok(dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root() -> (tempfile::TempDir, WorktreeActions) {
        let dir = tempfile::tempdir().unwrap();
        let canonical = fs::canonicalize(dir.path()).unwrap();
        (dir, WorktreeActions::new(canonical))
    }

    #[test]
    fn writes_are_confined_to_the_worktree() {
        let (_dir, actions) = root();
        assert!(matches!(
            actions.execute(&Action::WriteFile {
                path: "../escape.md".to_owned(),
                content: "x".to_owned()
            }),
            Err(ActionError::Confined)
        ));
        assert!(matches!(
            actions.execute(&Action::WriteFile {
                path: "/tmp/escape.md".to_owned(),
                content: "x".to_owned()
            }),
            Err(ActionError::Confined)
        ));
        actions
            .execute(&Action::WriteFile {
                path: "docs/note.md".to_owned(),
                content: "hello".to_owned(),
            })
            .unwrap();
        assert_eq!(
            actions
                .execute(&Action::ReadFile {
                    path: "docs/note.md".to_owned()
                })
                .unwrap(),
            "hello"
        );
    }

    #[cfg(unix)]
    #[test]
    fn symlink_escapes_are_rejected() {
        let (dir, actions) = root();
        let outside = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink(outside.path(), dir.path().join("link")).unwrap();
        assert!(matches!(
            actions.execute(&Action::ListFiles {
                path: "link".to_owned()
            }),
            Err(ActionError::Confined)
        ));
    }

    #[test]
    fn commands_run_in_the_worktree_with_bounded_output() {
        let (_dir, actions) = root();
        let output = actions
            .execute(&Action::RunCommand {
                command: "pwd && printf 'ok'".to_owned(),
            })
            .unwrap();
        assert!(output.contains("exit 0"), "{output}");
        assert!(output.contains("ok"), "{output}");
        let failed = actions
            .execute(&Action::RunCommand {
                command: "exit 7".to_owned(),
            })
            .unwrap();
        assert!(failed.contains("exit 7"), "{failed}");
    }
}
