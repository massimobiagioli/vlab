//! Shared helpers for CLI integration tests.
//!
//! Every test runs the real `vlab` binary inside a [`Sandbox`]: a temporary
//! root with an empty `HOME`, isolated `VLAB_CONFIG_DIR`/`VLAB_DATA_DIR`, and a
//! cleared environment, so no test can read the developer's settings, reach a
//! provider, or write to the real user profile.
//!
//! Extension point for WP-5: provider-backed tests will add a sandbox method
//! that points the binary at a scripted provider or a local `wiremock` server.
//! No provider types are introduced here.

// Each integration-test crate compiles this module separately and uses a
// different subset of it; unused helpers in one crate are expected.
#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};

use assert_cmd::Command;
use tempfile::TempDir;

/// Synthetic credential used to prove that secrets never reach output.
pub const SECRET_MARKER: &str = "sk-vlab-SECRET-MARKER";

/// A temporary directory with mode `0700`, suitable as a database parent:
/// private storage refuses directories other users can read (ADR-0010).
pub fn private_tempdir() -> TempDir {
    let mut builder = tempfile::Builder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        builder.permissions(fs::Permissions::from_mode(0o700));
    }
    builder.tempdir().expect("create private temp dir")
}

pub struct Sandbox {
    root: TempDir,
}

impl Sandbox {
    pub fn new() -> Self {
        let root = tempfile::tempdir().expect("create sandbox root");
        fs::create_dir(root.path().join("home")).expect("create sandbox home");
        let bin = root.path().join("bin");
        fs::create_dir(&bin).expect("create sandbox bin");
        let git = bin.join("git");
        fs::write(
            &git,
            "#!/bin/sh\n             root=$(CDPATH= cd -- \"$(dirname -- \"$0\")/..\" && pwd)\n             { printf '%s\\n' \"$@\"; \n               if [ -n \"${VLAB_GIT_TOKEN:-}\" ]; then printf '%s\\n' 'token=set'; \n               else printf '%s\\n' 'token=unset'; fi; } >> \"$root/git.log\"\n             if [ -f \"$root/git.fail\" ]; then exit 1; fi\n             dest=\n             for arg in \"$@\"; do dest=$arg; done\n             mkdir -p \"$dest/.git\"\n",
        )
        .expect("write fake git");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&git, fs::Permissions::from_mode(0o755))
                .expect("make fake git executable");
        }
        Self { root }
    }

    /// Arguments recorded by the fake `git` executable.
    pub fn git_log(&self) -> String {
        fs::read_to_string(self.root().join("git.log")).unwrap_or_default()
    }

    /// Makes the next fake `git` invocation fail.
    pub fn fail_git(&self) {
        fs::write(self.root().join("git.fail"), b"").expect("write git failure marker");
    }

    pub fn root(&self) -> &Path {
        self.root.path()
    }

    pub fn home(&self) -> PathBuf {
        self.root().join("home")
    }

    pub fn config_dir(&self) -> PathBuf {
        self.root().join("config")
    }

    pub fn data_dir(&self) -> PathBuf {
        self.root().join("data")
    }

    pub fn settings_file(&self) -> PathBuf {
        self.config_dir().join("settings.yaml")
    }

    pub fn database_path(&self) -> PathBuf {
        self.data_dir().join("vlab.db")
    }

    /// Opens (creating and migrating) the sandbox database so a test can seed
    /// rows before invoking the binary. The data directory is made private,
    /// as the binary itself requires.
    pub fn seed_database(&self) -> vlab::infra::db::Database {
        fs::create_dir_all(self.data_dir()).expect("create data dir");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(self.data_dir(), fs::Permissions::from_mode(0o700))
                .expect("make data dir private");
        }
        let (database, _) = vlab::infra::db::Database::open_and_migrate(
            self.database_path(),
            vlab::obs::RunId::generate(),
        )
        .expect("open sandbox database");
        database
    }

    /// Writes `settings.yaml` in a private configuration directory.
    pub fn write_settings(&self, content: &str) {
        fs::create_dir_all(self.config_dir()).expect("create config dir");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(self.config_dir(), fs::Permissions::from_mode(0o700))
                .expect("make config dir private");
        }
        fs::write(self.settings_file(), content).expect("write settings");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(self.settings_file(), fs::Permissions::from_mode(0o600))
                .expect("make settings private");
        }
    }

    /// Installs `tests/fixtures/editors/editor.sh` in a directory whose name
    /// contains a space, and returns its path.
    pub fn install_editor(&self) -> PathBuf {
        let dir = self.root().join("editor dir");
        fs::create_dir_all(&dir).expect("create editor dir");
        let editor = dir.join("editor.sh");
        let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/editors/editor.sh");
        fs::copy(source, &editor).expect("install editor");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&editor, fs::Permissions::from_mode(0o755))
                .expect("make editor executable");
        }
        editor
    }

    /// A `vlab` command with a cleared environment and sandboxed paths.
    pub fn command(&self) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_vlab"));
        command
            .env_clear()
            .env("HOME", self.home())
            .env("VLAB_CONFIG_DIR", self.config_dir())
            .env("VLAB_DATA_DIR", self.data_dir())
            .env(
                "PATH",
                format!("{}:/usr/bin:/bin", self.root().join("bin").display()),
            )
            // Child processes such as the test editor need basic tools.
            .current_dir(self.home());
        command
    }

    pub fn run(&self, args: &[&str]) -> Outcome {
        Outcome::from(self.command().args(args).output().expect("run vlab"))
    }

    /// True when the binary created nothing: only the empty `home` exists.
    pub fn is_untouched(&self) -> bool {
        let mut entries: Vec<_> = fs::read_dir(self.root())
            .expect("read sandbox root")
            .map(|entry| entry.expect("read entry").file_name())
            .collect();
        entries.sort();
        let home_is_empty = fs::read_dir(self.home())
            .expect("read sandbox home")
            .next()
            .is_none();
        // `bin` is harness infrastructure, not something vlab created.
        entries == ["bin", "home"] && home_is_empty
    }
}

/// Exit code and captured streams of one invocation.
#[derive(Debug)]
pub struct Outcome {
    pub code: i32,
    pub stdout: String,
    pub stderr: String,
}

impl From<std::process::Output> for Outcome {
    fn from(output: std::process::Output) -> Self {
        Self {
            code: output.status.code().expect("process exited with a code"),
            stdout: String::from_utf8(output.stdout).expect("stdout is UTF-8"),
            stderr: String::from_utf8(output.stderr).expect("stderr is UTF-8"),
        }
    }
}

impl Outcome {
    /// Exit 0 with nothing on stderr.
    pub fn assert_success(&self) -> &Self {
        assert_eq!(self.code, 0, "unexpected exit code: {self:#?}");
        assert!(self.stderr.is_empty(), "unexpected stderr: {self:#?}");
        self
    }

    /// Exit `code`, empty stdout, and stderr containing `needle`.
    pub fn assert_failure(&self, code: i32, needle: &str) -> &Self {
        assert_eq!(self.code, code, "unexpected exit code: {self:#?}");
        assert!(self.stdout.is_empty(), "unexpected stdout: {self:#?}");
        assert!(
            self.stderr.contains(needle),
            "stderr lacks {needle:?}: {self:#?}"
        );
        self
    }

    /// Stderr follows the `Error:` / `Hint:` contract.
    pub fn assert_error_contract(&self) -> &Self {
        let mut lines = self.stderr.lines();
        let first = lines.next().unwrap_or_default();
        let second = lines.next().unwrap_or_default();
        assert!(first.starts_with("Error: "), "bad error line: {self:#?}");
        assert!(second.starts_with("Hint: "), "bad hint line: {self:#?}");
        self
    }

    pub fn assert_no_escape_bytes(&self) -> &Self {
        assert!(!self.stdout.contains('\u{1b}'), "ANSI on stdout: {self:#?}");
        assert!(!self.stderr.contains('\u{1b}'), "ANSI on stderr: {self:#?}");
        self
    }

    pub fn assert_absent(&self, needle: &str) -> &Self {
        assert!(!self.stdout.contains(needle), "{needle:?} on stdout");
        assert!(!self.stderr.contains(needle), "{needle:?} on stderr");
        self
    }
}
