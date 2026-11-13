//! Help and version contract (WP1-AC1, WP1-AC2).

mod support;

use support::Sandbox;

/// Public group subcommands, in help order.
const COMMANDS: &[&str] = &["labs", "projects", "agents", "settings"];

/// Lines of the help section that starts with `heading:`.
fn section<'a>(help: &'a str, heading: &str) -> Vec<&'a str> {
    help.lines()
        .skip_while(|line| *line != format!("{heading}:"))
        .skip(1)
        .take_while(|line| !line.is_empty())
        .collect()
}

#[test]
fn help_lists_the_group_subcommands_in_order() {
    let sandbox = Sandbox::new();
    let outcome = sandbox.run(&["--help"]);
    outcome.assert_success().assert_no_escape_bytes();
    let lines = section(&outcome.stdout, "Commands");
    let listed: Vec<&str> = lines
        .iter()
        .filter_map(|line| {
            COMMANDS
                .iter()
                .copied()
                .find(|command| line.trim_start().starts_with(command))
        })
        .collect();
    assert_eq!(listed, COMMANDS, "{}", outcome.stdout);
    assert!(
        !outcome.stdout.contains("--list-labs"),
        "{}",
        outcome.stdout
    );
}

#[test]
fn help_matches_the_reviewed_snapshot() {
    let outcome = Sandbox::new().run(&["--help"]);
    outcome.assert_success();
    assert_eq!(outcome.stdout, include_str!("snapshots/help.txt"));
}

#[test]
fn no_arguments_prints_the_same_help_and_touches_nothing() {
    let sandbox = Sandbox::new();
    let bare = sandbox.run(&[]);
    bare.assert_success();
    assert_eq!(bare.stdout, sandbox.run(&["--help"]).stdout);
    assert!(sandbox.is_untouched());
}

#[test]
fn version_prints_the_crate_version() {
    let sandbox = Sandbox::new();
    let outcome = sandbox.run(&["--version"]);
    outcome.assert_success();
    assert_eq!(
        outcome.stdout,
        format!("vlab {}\n", env!("CARGO_PKG_VERSION"))
    );
    assert!(sandbox.is_untouched());
}

#[test]
fn help_and_version_create_no_files() {
    let sandbox = Sandbox::new();
    for args in [&["--help"][..], &["-h"], &["--version"], &["-V"]] {
        sandbox.run(args).assert_success();
    }
    assert!(sandbox.is_untouched());
}
