//! `--new-agent` refuses a non-interactive stdin (A-09, WP3-AC6).

mod support;

use support::Sandbox;

#[test]
fn a_piped_invocation_exits_4_without_hanging_or_creating_data() {
    let sandbox = Sandbox::new();
    sandbox.write_settings("provider: deepseek\nmodel: deepseek-chat\n");
    let outcome = sandbox.run(&["--new-agent"]);
    outcome
        .assert_failure(4, "`--new-agent` requires an interactive terminal.")
        .assert_failure(4, "Hint: Run `vlab --new-agent` from a terminal")
        .assert_error_contract();
    assert!(
        !sandbox.data_dir().exists(),
        "no database work is attempted"
    );
}

#[test]
fn stdin_without_a_terminal_is_detected_before_settings_are_read() {
    let sandbox = Sandbox::new();
    // No settings file either: the terminal check must win so the user is
    // told about the real blocker.
    let outcome = sandbox.run(&["--new-agent"]);
    outcome.assert_failure(4, "requires an interactive terminal.");
    assert!(!sandbox.data_dir().exists());
    assert!(!sandbox.config_dir().exists());
}
