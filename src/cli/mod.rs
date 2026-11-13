//! Argument parsing, input handling, rendering, and exit-code mapping.
//!
//! May depend on: `app`, `domain`, `obs`, and the composition of `infra`
//! adapters behind `app` ports.
//! Must not depend on: `infra` internals, `reqwest`, or `sqlx`.

pub mod args;
pub mod confirm;
pub mod dispatch;
pub mod exit;
pub mod input;
pub mod prompt;
pub mod render;
pub mod tui;

use std::ffi::OsString;
use std::io::{self, IsTerminal, Write};
use std::panic::{self, UnwindSafe};

use clap::error::ErrorKind;
use clap::{CommandFactory, Parser};

use crate::app::AppError;
use crate::obs;
use args::Cli;

/// Process entry point: returns the exit code for `main`.
pub fn main_entry() -> u8 {
    install_panic_hook();
    obs::init_diagnostics();
    let run_id = obs::RunId::generate();
    let _run = tracing::debug_span!("run", %run_id).entered();

    // Do not hold process-wide stream locks for the whole run: diagnostics
    // may be emitted from the SQLite worker thread, and a held stdout lock
    // can deadlock the tracing writer. Each write locks briefly instead.
    let mut stdout = io::stdout();
    let mut errors = io::stderr();
    let mut panic_errors = io::stderr();
    guard(
        std::panic::AssertUnwindSafe(|| {
            run_with_run_id(std::env::args_os(), &mut stdout, &mut errors, run_id)
        }),
        &mut panic_errors,
    )
}

/// Parses `argv`, runs the action, and writes the outcome to the streams.
pub fn run<I, T>(argv: I, stdout: &mut dyn Write, stderr: &mut dyn Write) -> u8
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    run_with_run_id(argv, stdout, stderr, obs::RunId::generate())
}

fn run_with_run_id<I, T>(
    argv: I,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
    run_id: obs::RunId,
) -> u8
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    match execute(argv, stdout, stderr, run_id) {
        Ok(()) => exit::SUCCESS,
        Err(error) => {
            tracing::debug!(code = error.code(), "command failed");
            // Nothing more can be reported if stderr itself is unwritable.
            let _ = stderr.write_all(exit::render(&error).as_bytes());
            exit::exit_code(&error)
        }
    }
}

fn execute<I, T>(
    argv: I,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
    run_id: obs::RunId,
) -> Result<(), AppError>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let cli = match Cli::try_parse_from(argv) {
        Ok(cli) => cli,
        Err(error) => {
            return match error.kind() {
                ErrorKind::DisplayHelp | ErrorKind::DisplayVersion => {
                    write_stdout(stdout, &error.to_string())
                }
                _ => Err(usage_error(&error)),
            };
        }
    };
    match cli.into_action()? {
        None => {
            if io::stdin().is_terminal() && io::stdout().is_terminal() {
                crate::cli::tui::run_home(run_id)
            } else {
                write_stdout(stdout, &Cli::command().render_help().to_string())
            }
        }
        Some(action) => {
            tracing::debug!(action = action.flag(), "dispatching");
            if !matches!(action, args::Action::Dashboard { .. }) && io::stderr().is_terminal() {
                write_stderr(
                    stderr,
                    "Warning: action flags are deprecated; use `vlab labs`, `vlab projects`, `vlab agents`, or `vlab settings`.\n",
                )?;
            }
            dispatch::dispatch(action, stdout, stderr, run_id)
        }
    }
}

/// Keeps clap's problem statement, including indented detail lines such as
/// missing arguments, and drops its usage block and tips, so parse errors
/// follow the same two-line contract as every other error.
fn usage_error(error: &clap::Error) -> AppError {
    let rendered = error.to_string();
    let message = rendered
        .lines()
        .take_while(|line| !line.trim().is_empty())
        .map(str::trim)
        .collect::<Vec<_>>()
        .join(" ");
    let message = message.strip_prefix("error: ").unwrap_or(&message);
    AppError::Usage {
        message: message.to_owned(),
    }
}

pub(crate) fn write_stdout(stdout: &mut dyn Write, text: &str) -> Result<(), AppError> {
    stdout
        .write_all(text.as_bytes())
        .and_then(|()| stdout.flush())
        .map_err(|error| AppError::Io {
            context: "writing to standard output",
            kind: error.kind(),
        })
}

pub(crate) fn write_stderr(stderr: &mut dyn Write, text: &str) -> Result<(), AppError> {
    stderr
        .write_all(text.as_bytes())
        .and_then(|()| stderr.flush())
        .map_err(|error| AppError::Io {
            context: "writing to standard error",
            kind: error.kind(),
        })
}

/// Runs `body`, converting a panic into an internal error with exit code 1.
pub fn guard(body: impl FnOnce() -> u8 + UnwindSafe, stderr: &mut dyn Write) -> u8 {
    match panic::catch_unwind(body) {
        Ok(code) => code,
        Err(_) => {
            let error = AppError::Internal;
            let _ = stderr.write_all(exit::render(&error).as_bytes());
            exit::exit_code(&error)
        }
    }
}

/// Replaces the default hook, which prints the panic payload. Payloads may
/// embed user data, so only the location and, on request, a backtrace are
/// shown.
fn install_panic_hook() {
    panic::set_hook(Box::new(|info| {
        if std::env::var_os("RUST_BACKTRACE").is_some_and(|value| value != "0") {
            if let Some(location) = info.location() {
                eprintln!("panic at {}:{}", location.file(), location.line());
            }
            eprintln!("{}", std::backtrace::Backtrace::force_capture());
        }
    }));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run_args(args: &[&str]) -> (u8, String, String) {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let argv = std::iter::once("vlab").chain(args.iter().copied());
        let code = run(argv, &mut stdout, &mut stderr);
        (
            code,
            String::from_utf8(stdout).unwrap(),
            String::from_utf8(stderr).unwrap(),
        )
    }

    #[test]
    fn help_goes_to_stdout_only() {
        let (code, stdout, stderr) = run_args(&["--help"]);
        assert_eq!(code, 0);
        assert!(stdout.contains("Laboratories:"));
        assert!(stderr.is_empty());
    }

    #[test]
    fn usage_errors_go_to_stderr_only() {
        let (code, stdout, stderr) = run_args(&["--bogus"]);
        assert_eq!(code, 2);
        assert!(stdout.is_empty());
        assert!(
            stderr.starts_with("Error: unexpected argument '--bogus' found\n"),
            "{stderr}"
        );
    }

    #[test]
    fn unwritable_stdout_is_an_io_failure() {
        struct Closed;
        impl Write for Closed {
            fn write(&mut self, _: &[u8]) -> io::Result<usize> {
                Err(io::ErrorKind::BrokenPipe.into())
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        let mut stderr = Vec::new();
        let code = run(["vlab", "--help"], &mut Closed, &mut stderr);
        assert_eq!(code, 1);
        assert!(
            String::from_utf8(stderr)
                .unwrap()
                .contains("writing to standard output failed")
        );
    }

    #[test]
    fn panics_become_internal_errors_with_exit_code_1() {
        let mut stderr = Vec::new();
        let code = guard(|| panic!("payload with private data"), &mut stderr);
        let stderr = String::from_utf8(stderr).unwrap();
        assert_eq!(code, 1);
        assert!(stderr.starts_with("Error: an internal error occurred.\n"));
        assert!(!stderr.contains("private data"));
    }
}
