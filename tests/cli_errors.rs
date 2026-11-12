//! Error rendering at the process boundary (WP1-AC4 complements the unit
//! table in `src/cli/exit.rs`).

mod support;

use support::Sandbox;

#[test]
fn usage_errors_name_the_problem_without_clap_decoration() {
    let outcome = Sandbox::new().run(&["--list-labs", "--list-agents"]);
    outcome
        .assert_failure(2, "cannot be used with")
        .assert_error_contract();
    assert_eq!(outcome.stderr.lines().count(), 2, "{outcome:#?}");
}

#[test]
fn control_sequences_in_arguments_are_not_echoed() {
    let outcome = Sandbox::new().run(&["--list-labs", "bad\u{1b}[2Jvalue\u{7}"]);
    outcome
        .assert_failure(2, "Error: ")
        .assert_no_escape_bytes();
    assert!(!outcome.stderr.contains('\u{7}'), "{outcome:#?}");
}
