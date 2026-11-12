//! Secrets never reach stdout, stderr, or rendered errors (WP1-AC6).

mod support;

use support::{SECRET_MARKER, Sandbox};
use vlab::app::AppError;
use vlab::cli::exit::render;
use vlab::obs::Redacted;

#[test]
fn provider_key_in_settings_never_reaches_output_even_with_tracing() {
    let sandbox = Sandbox::new();
    sandbox.write_settings(&format!(
        "provider: deepseek\nmodel: deepseek-chat\ncredentials:\n  deepseek_api_key: \"{SECRET_MARKER}\"\n"
    ));
    let cases: &[&[&str]] = &[
        &[],
        &["--help"],
        &["--list-labs"],
        &["--ask", "What next?"],
        &["--bogus"],
    ];
    for args in cases {
        let output = sandbox
            .command()
            .env("RUST_LOG", "trace")
            .args(*args)
            .output()
            .expect("run vlab");
        support::Outcome::from(output).assert_absent(SECRET_MARKER);
    }
}

#[test]
fn redacted_values_hide_their_content_in_every_format() {
    let secret = Redacted::new(SECRET_MARKER.to_owned());
    for rendered in [
        format!("{secret}"),
        format!("{secret:?}"),
        format!("{secret:#?}"),
    ] {
        assert!(!rendered.contains(SECRET_MARKER), "{rendered}");
        assert_eq!(rendered, "[REDACTED]");
    }
    assert_eq!(secret.expose(), SECRET_MARKER);
}

#[test]
fn errors_built_from_redacted_context_do_not_leak() {
    let secret = Redacted::new(SECRET_MARKER);
    let error = AppError::Configuration {
        problem: format!("provider rejected credential {secret}"),
        hint: format!("check the key {secret:?}"),
    };
    let rendered = render(&error);
    assert!(!rendered.contains(SECRET_MARKER), "{rendered}");
    assert!(!format!("{error:?}").contains(SECRET_MARKER));
}

#[test]
fn diagnostics_are_silent_unless_rust_log_is_set() {
    let sandbox = Sandbox::new();
    let quiet = sandbox.run(&["--list-projects"]);
    assert_eq!(quiet.stderr.lines().count(), 2, "{quiet:#?}");

    let verbose = support::Outcome::from(
        sandbox
            .command()
            .env("RUST_LOG", "debug")
            .arg("--list-projects")
            .output()
            .expect("run vlab"),
    );
    assert!(verbose.stderr.lines().count() > 2, "{verbose:#?}");
    verbose.assert_no_escape_bytes();
    assert!(verbose.stdout.is_empty());
}
