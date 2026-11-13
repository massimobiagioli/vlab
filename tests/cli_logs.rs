//! `--logs` prints content-free application audit events.

mod support;

use support::{Outcome, Sandbox};

#[test]
fn logs_show_recent_events_without_content_or_credentials() {
    let sandbox = Sandbox::new();
    let created = Outcome::from(
        sandbox
            .command()
            .args(["--new-lab", "Log Lab"])
            .output()
            .unwrap(),
    );
    created.assert_success();

    let logs = Outcome::from(sandbox.command().arg("--logs").output().unwrap());
    logs.assert_success();
    let output = &logs.stdout;
    for expected in ["Application logs", "lab_created", "memory_created"] {
        assert!(output.contains(expected), "{expected}: {output}");
    }
    assert!(!output.contains("sk-"), "{output}");
    assert!(!output.contains("Log Lab"), "{output}");
}
