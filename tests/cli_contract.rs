//! Final CLI contract: implemented handlers, streams, and exit codes
//! (WP6-AC5).

mod support;

use support::{Outcome, Sandbox};

#[test]
fn the_dispatch_table_has_no_scaffold_branch() {
    let source = include_str!("../src/cli/dispatch.rs");
    assert!(!source.contains("NotImplemented"));
    assert!(!source.contains("not available in this build"));
}

#[test]
fn help_version_and_bare_invocation_are_clean() {
    let sandbox = Sandbox::new();
    for args in [&[][..], &["--help"], &["-h"], &["--version"], &["-V"]] {
        let outcome = sandbox.run(args);
        outcome.assert_success().assert_no_escape_bytes();
        assert!(outcome.stderr.is_empty());
    }
    assert!(sandbox.is_untouched());
}

#[test]
fn the_exit_code_contract_covers_each_failure_class() {
    let sandbox = Sandbox::new();
    sandbox
        .run(&["--bogus"])
        .assert_failure(2, "unexpected argument");
    sandbox
        .run(&["--new-lab"])
        .assert_failure(2, "a value is required");
    sandbox
        .run(&["--list-agents"])
        .assert_failure(3, "no active laboratory");
    sandbox
        .run(&["--new-agent"])
        .assert_failure(4, "requires an interactive terminal");
    sandbox.write_settings("provider: deepseek\nmodel: deepseek-chat\n");
    sandbox
        .run(&["--ask", "Why?"])
        .assert_failure(2, "credentials.deepseek_api_key");
}

#[test]
fn destructive_and_list_commands_keep_stdout_and_stderr_separate() {
    let sandbox = Sandbox::new();
    let created = sandbox.run(&["--new-lab", "Payments"]);
    created.assert_success();
    let lab = created
        .stdout
        .lines()
        .find_map(|line| line.strip_prefix("ID:   "))
        .unwrap()
        .trim()
        .to_owned();
    let listed = sandbox.run(&["--list-labs"]);
    listed.assert_success().assert_no_escape_bytes();
    assert!(listed.stderr.is_empty());
    let opened = Outcome::from(
        sandbox
            .command()
            .arg("--open-lab")
            .arg(&lab)
            .output()
            .unwrap(),
    );
    assert_eq!(opened.code, 0);
    assert!(opened.stdout.contains("Active laboratory"));
    assert!(opened.stderr.contains("Warning:"));
}

#[test]
fn a_command_never_waits_for_input_when_stdin_is_not_a_terminal() {
    let sandbox = Sandbox::new();
    sandbox.write_settings("provider: deepseek\nmodel: deepseek-chat\n");
    // These return a usage/configuration error instead of blocking.
    sandbox
        .run(&["--ask", "Why?"])
        .assert_failure(2, "credentials.deepseek_api_key");
    sandbox
        .run(&["--new-request"])
        .assert_failure(2, "must contain non-empty text");
}
