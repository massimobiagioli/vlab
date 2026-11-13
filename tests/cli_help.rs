//! Help and version contract (WP1-AC1, WP1-AC2).

mod support;

use support::Sandbox;

#[test]
fn help_exposes_only_help_and_version() {
    let sandbox = Sandbox::new();
    let outcome = sandbox.run(&["--help"]);
    outcome.assert_success().assert_no_escape_bytes();
    assert!(outcome.stdout.contains("-h, --help"), "{}", outcome.stdout);
    assert!(
        outcome.stdout.contains("-V, --version"),
        "{}",
        outcome.stdout
    );
    assert!(!outcome.stdout.contains("Commands:"), "{}", outcome.stdout);
    assert!(!outcome.stdout.contains("labs"), "{}", outcome.stdout);
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
