#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used))]

use std::process::ExitCode;

fn main() -> ExitCode {
    ExitCode::from(vlab::cli::main_entry())
}
