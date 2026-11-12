//! `--show-settings` (specification §5.2; WP2-AC3, WP2-AC8).

mod support;

use std::fs;

use support::{SECRET_MARKER, Sandbox};

const VALID: &str = "provider: deepseek\nmodel: deepseek-chat\n";

fn listing(sandbox: &Sandbox) -> Vec<String> {
    let mut entries: Vec<String> = walk(sandbox.root());
    entries.sort();
    entries
}

fn walk(dir: &std::path::Path) -> Vec<String> {
    fs::read_dir(dir)
        .unwrap()
        .flat_map(|entry| {
            let path = entry.unwrap().path();
            let mut found = vec![path.display().to_string()];
            if path.is_dir() {
                found.extend(walk(&path));
            }
            found
        })
        .collect()
}

#[test]
fn valid_settings_are_shown_on_stdout() {
    let sandbox = Sandbox::new();
    sandbox.write_settings(VALID);
    let outcome = sandbox.run(&["--show-settings"]);
    outcome.assert_success().assert_no_escape_bytes();
    assert_eq!(
        outcome.stdout,
        format!(
            "Settings file:     {}\n\
             Status:            valid\n\
             Provider:          deepseek\n\
             Model:             deepseek-chat\n\
             Source control:    github\n\
             DeepSeek API key:  unset\n\
             GitHub API key:    unset\n",
            sandbox.settings_file().display()
        )
    );
}

#[test]
fn credentials_in_settings_are_reported_as_set_but_never_shown() {
    let sandbox = Sandbox::new();
    sandbox.write_settings(&format!(
        "{VALID}credentials:\n  deepseek_api_key: \"{SECRET_MARKER}\"\n  github_api_key: \"{SECRET_MARKER}\"\n"
    ));
    let output = sandbox
        .command()
        .env("RUST_LOG", "trace")
        .arg("--show-settings")
        .output()
        .unwrap();
    let outcome = support::Outcome::from(output);
    assert_eq!(outcome.code, 0, "{outcome:#?}");
    assert!(outcome.stdout.contains("DeepSeek API key:  set\n"));
    assert!(outcome.stdout.contains("GitHub API key:    set\n"));
    outcome.assert_absent(SECRET_MARKER);
}

#[test]
fn a_missing_credential_section_is_reported_as_unset() {
    let sandbox = Sandbox::new();
    sandbox.write_settings(VALID);
    let outcome = sandbox.run(&["--show-settings"]);
    assert!(outcome.stdout.contains("DeepSeek API key:  unset\n"));
    assert!(outcome.stdout.contains("GitHub API key:    unset\n"));
}

#[cfg(unix)]
#[test]
fn a_settings_file_readable_by_other_users_is_refused() {
    use std::os::unix::fs::PermissionsExt;

    let sandbox = Sandbox::new();
    sandbox.write_settings(&format!(
        "{VALID}credentials:\n  deepseek_api_key: \"{SECRET_MARKER}\"\n"
    ));
    fs::set_permissions(sandbox.settings_file(), fs::Permissions::from_mode(0o644)).unwrap();
    let outcome = sandbox.run(&["--show-settings"]);
    outcome
        .assert_failure(2, "is accessible by other users")
        .assert_failure(2, "chmod 600")
        .assert_absent(SECRET_MARKER);
}

#[test]
fn a_missing_file_exits_2_and_creates_nothing() {
    let sandbox = Sandbox::new();
    let outcome = sandbox.run(&["--show-settings"]);
    outcome
        .assert_failure(2, "does not exist.")
        .assert_failure(2, "Hint: Run `vlab --edit-settings` to create it.")
        .assert_error_contract();
    assert!(
        outcome
            .stderr
            .contains(&sandbox.settings_file().display().to_string())
    );
    assert!(sandbox.is_untouched());
}

#[test]
fn invalid_files_exit_2_with_a_reason_and_no_file_content() {
    let cases = [
        (
            format!("{VALID}api_key: {SECRET_MARKER}\n"),
            "it contains an unknown field (line 3)",
        ),
        (
            format!("provider: {SECRET_MARKER}\nmodel: m\n"),
            "`provider` must be `deepseek`",
        ),
        (
            "provider: deepseek\n".to_owned(),
            "the required field `model` is missing",
        ),
        ("provider: [\n".to_owned(), "it is not valid YAML"),
    ];
    for (content, reason) in cases {
        let sandbox = Sandbox::new();
        sandbox.write_settings(&content);
        let outcome = sandbox.run(&["--show-settings"]);
        outcome
            .assert_failure(2, reason)
            .assert_failure(2, "Hint: Run `vlab --edit-settings` to fix it.")
            .assert_error_contract()
            .assert_absent(SECRET_MARKER);
    }
}

#[test]
fn showing_settings_writes_nothing_and_opens_no_database() {
    let sandbox = Sandbox::new();
    sandbox.write_settings(VALID);
    let before = listing(&sandbox);
    sandbox.run(&["--show-settings"]).assert_success();
    assert_eq!(listing(&sandbox), before);
    assert!(!sandbox.data_dir().exists());
}

#[test]
fn control_characters_in_the_path_are_not_rendered() {
    let sandbox = Sandbox::new();
    let odd = sandbox.root().join("odd\u{1b}[2Jdir");
    let output = sandbox
        .command()
        .env("VLAB_CONFIG_DIR", &odd)
        .arg("--show-settings")
        .output()
        .unwrap();
    let outcome = support::Outcome::from(output);
    outcome
        // The complete CSI sequence is removed, leaving the visible text.
        .assert_failure(2, "odddir")
        .assert_no_escape_bytes();
}
