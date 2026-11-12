//! `--edit-settings` (specification §5.3; WP2-AC4).

mod support;

use std::fs;
use std::path::{Path, PathBuf};

use support::{Outcome, SECRET_MARKER, Sandbox};

const VALID: &str = "provider: deepseek\nmodel: deepseek-chat\n";
const OTHER_VALID: &str = "provider: deepseek\nmodel: deepseek-reasoner\n";

/// Runs `--edit-settings` with the test editor as `VISUAL`.
struct Edit<'a> {
    sandbox: &'a Sandbox,
    visual: String,
    env: Vec<(&'static str, String)>,
}

impl<'a> Edit<'a> {
    fn new(sandbox: &'a Sandbox) -> Self {
        let editor = sandbox.install_editor();
        Self {
            sandbox,
            visual: format!("'{}'", editor.display()),
            env: Vec::new(),
        }
    }

    fn visual(mut self, value: String) -> Self {
        self.visual = value;
        self
    }

    fn env(mut self, key: &'static str, value: impl Into<String>) -> Self {
        self.env.push((key, value.into()));
        self
    }

    /// The editor replaces the file with `content`.
    fn writes(self, content: &str) -> Self {
        let source = self.sandbox.root().join("editor-source.yaml");
        fs::write(&source, content).unwrap();
        self.env("EDITOR_SOURCE", source.display().to_string())
    }

    fn run(self) -> Outcome {
        let mut command = self.sandbox.command();
        command.env("VISUAL", &self.visual).arg("--edit-settings");
        for (key, value) in &self.env {
            command.env(key, value);
        }
        Outcome::from(command.output().unwrap())
    }
}

fn config_entries(sandbox: &Sandbox) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(sandbox.config_dir())
        .map(|entries| {
            entries
                .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    names
}

#[cfg(unix)]
fn mode(path: &Path) -> u32 {
    use std::os::unix::fs::PermissionsExt;
    fs::metadata(path).unwrap().permissions().mode() & 0o777
}

#[test]
fn a_valid_first_edit_creates_private_settings() {
    let sandbox = Sandbox::new();
    let capture = sandbox.root().join("received.yaml");
    let outcome = Edit::new(&sandbox)
        .env("EDITOR_CAPTURE", capture.display().to_string())
        .writes(VALID)
        .run();
    outcome.assert_success();
    assert!(
        outcome
            .stdout
            .starts_with("Settings saved.\n\nSettings file:     ")
    );
    assert!(
        outcome
            .stdout
            .contains("Model:             deepseek-chat\n")
    );

    assert_eq!(fs::read_to_string(sandbox.settings_file()).unwrap(), VALID);
    assert_eq!(config_entries(&sandbox), ["settings.yaml"]);
    let received = fs::read_to_string(capture).unwrap();
    assert!(
        received.contains("provider: deepseek\nmodel: \"<supported-model-id>\"\n"),
        "{received}"
    );
    #[cfg(unix)]
    {
        assert_eq!(mode(&sandbox.settings_file()), 0o600);
        assert_eq!(mode(&sandbox.config_dir()), 0o700);
    }
    assert!(!sandbox.data_dir().exists(), "no database work");
}

#[test]
fn the_editor_receives_the_current_file() {
    let sandbox = Sandbox::new();
    sandbox.write_settings(VALID);
    let capture = sandbox.root().join("received.yaml");
    Edit::new(&sandbox)
        .env("EDITOR_CAPTURE", capture.display().to_string())
        .writes(OTHER_VALID)
        .run()
        .assert_success();
    assert_eq!(fs::read_to_string(capture).unwrap(), VALID);
    assert_eq!(
        fs::read_to_string(sandbox.settings_file()).unwrap(),
        OTHER_VALID
    );
}

#[test]
fn saving_the_unchanged_template_is_rejected() {
    let sandbox = Sandbox::new();
    let outcome = Edit::new(&sandbox).run();
    outcome
        .assert_failure(2, "the edited settings are invalid: `model` must be")
        .assert_failure(2, "the previous settings were kept.")
        .assert_error_contract();
    assert!(
        config_entries(&sandbox).is_empty(),
        "{:?}",
        config_entries(&sandbox)
    );
}

#[test]
fn an_invalid_edit_keeps_the_previous_bytes_and_leaves_no_secret_behind() {
    let sandbox = Sandbox::new();
    sandbox.write_settings(VALID);
    let before = fs::read(sandbox.settings_file()).unwrap();

    let outcome = Edit::new(&sandbox)
        .writes(&format!("{VALID}api_key: {SECRET_MARKER}\n"))
        .run();
    outcome
        .assert_failure(2, "it contains an unknown field")
        .assert_error_contract()
        .assert_absent(SECRET_MARKER);

    assert_eq!(fs::read(sandbox.settings_file()).unwrap(), before);
    assert_eq!(config_entries(&sandbox), ["settings.yaml"]);
    for name in config_entries(&sandbox) {
        let content = fs::read_to_string(sandbox.config_dir().join(name)).unwrap();
        assert!(!content.contains(SECRET_MARKER));
    }
}

#[test]
fn an_unsuccessful_editor_cancels_without_changes() {
    let sandbox = Sandbox::new();
    sandbox.write_settings(VALID);
    let outcome = Edit::new(&sandbox)
        .writes(OTHER_VALID)
        .env("EDITOR_EXIT", "3")
        .run();
    outcome
        .assert_failure(5, "the operation was cancelled; no changes were made.")
        .assert_error_contract();
    assert_eq!(fs::read_to_string(sandbox.settings_file()).unwrap(), VALID);
    assert_eq!(config_entries(&sandbox), ["settings.yaml"]);
}

#[test]
fn editor_arguments_are_split_without_a_shell() {
    let sandbox = Sandbox::new();
    let editor = sandbox.install_editor();
    let log = sandbox.root().join("args.log");
    let pwned = sandbox.root().join("pwned");
    let visual = format!(
        "'{}' --flag \"two words\" ; touch {} $HOME",
        editor.display(),
        pwned.display()
    );
    Edit::new(&sandbox)
        .visual(visual)
        .env("EDITOR_LOG", log.display().to_string())
        .writes(VALID)
        .run()
        .assert_success();

    let args: Vec<String> = fs::read_to_string(&log)
        .unwrap()
        .lines()
        .map(str::to_owned)
        .collect();
    let pwned_arg = pwned.display().to_string();
    assert_eq!(
        args[..6],
        [
            "--flag",
            "two words",
            ";",
            "touch",
            pwned_arg.as_str(),
            "$HOME"
        ]
    );
    let draft = PathBuf::from(&args[6]);
    assert_eq!(draft.parent(), Some(sandbox.config_dir().as_path()));
    let name = draft.file_name().unwrap().to_string_lossy().into_owned();
    assert!(
        name.starts_with(".settings.yaml.") && name.ends_with(".tmp"),
        "{name}"
    );
    assert!(!pwned.exists(), "a shell interpreted the editor value");
}

#[test]
fn a_missing_or_unknown_editor_is_a_precondition_failure() {
    let sandbox = Sandbox::new();
    let outcome = sandbox.run(&["--edit-settings"]);
    outcome
        .assert_failure(4, "no editor is configured.")
        .assert_error_contract();
    assert!(sandbox.is_untouched());

    let unknown = Edit::new(&sandbox)
        .visual("vlab-no-such-editor --wait".to_owned())
        .run();
    unknown
        .assert_failure(4, "the editor `vlab-no-such-editor` could not be started")
        .assert_error_contract();
    assert!(config_entries(&sandbox).is_empty());

    let unterminated = Edit::new(&sandbox).visual("'vi".to_owned()).run();
    unterminated.assert_failure(2, "VISUAL has an unterminated quote.");
}

#[test]
fn stale_drafts_from_an_interrupted_edit_are_removed() {
    let sandbox = Sandbox::new();
    sandbox.write_settings(VALID);
    let stale = sandbox.config_dir().join(".settings.yaml.0123abcd.tmp");
    fs::write(&stale, format!("api_key: {SECRET_MARKER}\n")).unwrap();
    Edit::new(&sandbox)
        .writes(OTHER_VALID)
        .run()
        .assert_success();
    assert!(!stale.exists());
    assert_eq!(config_entries(&sandbox), ["settings.yaml"]);
}

#[cfg(unix)]
#[test]
fn a_configuration_directory_open_to_others_is_refused() {
    use std::os::unix::fs::PermissionsExt;
    let sandbox = Sandbox::new();
    sandbox.write_settings(VALID);
    fs::set_permissions(sandbox.config_dir(), fs::Permissions::from_mode(0o755)).unwrap();
    let outcome = Edit::new(&sandbox).writes(OTHER_VALID).run();
    outcome
        .assert_failure(4, "is accessible by other users.")
        .assert_error_contract();
    assert_eq!(fs::read_to_string(sandbox.settings_file()).unwrap(), VALID);
}
