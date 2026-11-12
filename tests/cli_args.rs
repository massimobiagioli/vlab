//! One-primary-action rule and argument validation (WP1-AC3).

mod support;

use support::Sandbox;

const ID: &str = "a3f91c07";

#[test]
fn invalid_combinations_are_usage_errors() {
    let cases: &[(&str, &[&str])] = &[
        ("two actions", &["--list-labs", "--list-agents"]),
        ("two valued actions", &["--new-lab", "X", "--open-lab", ID]),
        ("unknown flag", &["--bogus"]),
        ("stray positional", &["--list-labs", "extra"]),
        ("to-agent alone", &["--to-agent", ID]),
        ("to-agent without ask", &["--list-labs", "--to-agent", ID]),
        ("yes alone", &["--yes"]),
        ("yes without deletion", &["--list-labs", "--yes"]),
        ("yes with ask", &["--ask", "Why?", "--yes"]),
        ("ask without question", &["--ask"]),
        ("new-lab without name", &["--new-lab"]),
        ("open-lab without id", &["--open-lab"]),
        ("open-project without id", &["--open-project"]),
        ("assign-agent without id", &["--assign-agent"]),
        (
            "new-request with three values",
            &["--new-request", "text", "more"],
        ),
        ("removed memory command", &["--list-memories"]),
        ("repeated action", &["--list-labs", "--list-labs"]),
    ];

    let sandbox = Sandbox::new();
    for (name, args) in cases {
        let outcome = sandbox.run(args);
        assert_eq!(outcome.code, 2, "{name}: {outcome:#?}");
        outcome
            .assert_failure(2, "Run `vlab --help`")
            .assert_error_contract()
            .assert_no_escape_bytes();
    }
    assert!(sandbox.is_untouched());
}
