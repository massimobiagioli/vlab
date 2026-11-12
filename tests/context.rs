//! The missing-context contract of specification §3.4 (WP2-AC6).

use vlab::app::context::no_active_lab;
use vlab::cli::exit::{exit_code, render};

#[test]
fn the_missing_context_error_matches_the_specification_exactly() {
    let error = no_active_lab();
    assert_eq!(exit_code(&error), 3);
    assert_eq!(
        render(&error),
        "Error: no active laboratory is selected.\n\
         Run `vlab --list-labs`, then `vlab --open-lab <lab-id>`.\n"
    );
}
