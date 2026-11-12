//! Settings lifecycle through the real binary (WP-2 exit gate): create, show,
//! recover from an invalid edit, show again.

mod support;

use std::fs;

use support::Sandbox;

#[test]
fn settings_lifecycle() {
    let sandbox = Sandbox::new();
    let editor = sandbox.install_editor();
    let source = sandbox.root().join("source.yaml");
    let edit = |content: &str| {
        fs::write(&source, content).unwrap();
        support::Outcome::from(
            sandbox
                .command()
                .env("EDITOR", format!("'{}'", editor.display()))
                .env("EDITOR_SOURCE", &source)
                .arg("--edit-settings")
                .output()
                .unwrap(),
        )
    };

    sandbox
        .run(&["--show-settings"])
        .assert_failure(2, "does not exist.");

    edit("provider: deepseek\nmodel: deepseek-chat\n").assert_success();
    let shown = sandbox.run(&["--show-settings"]);
    shown.assert_success();
    assert!(shown.stdout.contains("Model:             deepseek-chat\n"));

    edit("provider: deepseek\nmodel: \"\"\n").assert_failure(2, "the previous settings were kept.");
    assert_eq!(sandbox.run(&["--show-settings"]).stdout, shown.stdout);

    assert!(
        !sandbox.data_dir().exists(),
        "settings commands open no database"
    );
}
