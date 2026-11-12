//! Help and version contract (WP1-AC1, WP1-AC2).

mod support;

use support::Sandbox;

/// Command groups and their flags, in the order of specification §4.1.
const GROUPS: &[(&str, &[&str])] = &[
    ("Settings", &["--show-settings", "--edit-settings"]),
    (
        "Laboratories",
        &[
            "--list-labs",
            "--new-lab <name>",
            "--open-lab <lab-id>",
            "--delete-lab <lab-id>",
            "--check-lab <lab-id>",
        ],
    ),
    (
        "Projects",
        &[
            "--list-projects",
            "--new-project <name>",
            "--github-source <url>",
            "--open-project <project-id>",
            "--show-project-kanban",
            "--delete-project",
            "--new-request [<request-text>]",
            "--assign-agent <agent-id>",
        ],
    ),
    (
        "Agents",
        &[
            "--list-agents",
            "--new-agent",
            "--delete-agent <agent-id>",
            "--show-agent-detail <agent-id>",
            "--ask <question>",
            "--to-agent <agent-id>",
        ],
    ),
];

/// Lines of the help section that starts with `heading:`.
fn section<'a>(help: &'a str, heading: &str) -> Vec<&'a str> {
    help.lines()
        .skip_while(|line| *line != format!("{heading}:"))
        .skip(1)
        .take_while(|line| !line.is_empty())
        .collect()
}

#[test]
fn help_lists_every_command_under_its_group_in_order() {
    let sandbox = Sandbox::new();
    let outcome = sandbox.run(&["--help"]);
    outcome.assert_success().assert_no_escape_bytes();

    for (heading, flags) in GROUPS {
        let lines = section(&outcome.stdout, heading);
        let listed: Vec<&str> = lines
            .iter()
            .filter_map(|line| {
                flags
                    .iter()
                    .copied()
                    .find(|flag| line.trim_start().starts_with(flag))
            })
            .collect();
        assert_eq!(&listed, flags, "group {heading} in:\n{}", outcome.stdout);
    }
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
