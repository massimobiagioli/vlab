//! Launching the user's editor (specification §5.3, ADR-0015).
//!
//! `VISUAL`, then `EDITOR`, is split into a program and arguments with POSIX
//! shell-word rules (quotes and backslashes; no expansion, substitution, or
//! operators) and run directly. No shell is ever involved.

use std::ffi::OsString;
use std::io::{self, BufRead, IsTerminal, Read, Write};
use std::path::Path;
use std::process::Command;

use crate::app::AppError;
use crate::app::agents::wizard::{Prompt, Prompter};
use crate::app::settings::Editor;

const EDITOR_VARS: [&str; 2] = ["VISUAL", "EDITOR"];

/// Largest accepted line from the wizard.
const MAX_PROMPT_BYTES: u64 = 64 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalEditor {
    program: String,
    args: Vec<String>,
}

impl TerminalEditor {
    /// Resolves the editor from `lookup` (environment variables). An unset,
    /// empty, or blank variable is skipped.
    pub fn resolve(lookup: impl Fn(&str) -> Option<OsString>) -> Result<Self, AppError> {
        for variable in EDITOR_VARS {
            let Some(value) = lookup(variable) else {
                continue;
            };
            let value = value.into_string().map_err(|_| AppError::Configuration {
                problem: format!("{variable} is not valid UTF-8."),
                hint: format!("Set {variable} to the editor's path and arguments."),
            })?;
            if value.trim().is_empty() {
                continue;
            }
            let mut words = shell_words::split(&value)
                .map_err(|_| AppError::Configuration {
                    problem: format!("{variable} has an unterminated quote."),
                    hint: format!(
                        "Quote paths with spaces, for example {variable}='\"/Applications/My Editor\" --wait'."
                    ),
                })?
                .into_iter();
            let Some(program) = words.next() else {
                continue;
            };
            return Ok(Self {
                program,
                args: words.collect(),
            });
        }
        Err(AppError::Precondition {
            problem: "no editor is configured.".to_owned(),
            hint: "Set VISUAL or EDITOR, for example `export EDITOR=nano`.".to_owned(),
        })
    }

    pub fn from_environment() -> Result<Self, AppError> {
        Self::resolve(|key| std::env::var_os(key))
    }
}

impl Editor for TerminalEditor {
    fn edit(&self, file: &Path) -> Result<(), AppError> {
        let status = Command::new(&self.program)
            .args(&self.args)
            .arg(file)
            .status()
            .map_err(|error| match error.kind() {
                io::ErrorKind::NotFound | io::ErrorKind::PermissionDenied => {
                    AppError::Precondition {
                        problem: format!(
                            "the editor `{}` could not be started: {}.",
                            self.program,
                            error.kind()
                        ),
                        hint: "Set VISUAL or EDITOR to an installed editor.".to_owned(),
                    }
                }
                kind => AppError::Io {
                    context: "starting the editor",
                    kind,
                },
            })?;
        if status.success() {
            Ok(())
        } else {
            tracing::debug!(code = status.code(), "editor exited unsuccessfully");
            Err(AppError::Cancelled)
        }
    }
}

/// The terminal-backed prompter used by `--new-agent`.
///
/// Prompts go to stderr so stdout stays the result stream. Input is bounded
/// and must be UTF-8; EOF cancels (exit 5).
#[derive(Debug, Default)]
pub struct TerminalPrompter;

impl TerminalPrompter {
    fn read_line(&self) -> Result<Option<String>, AppError> {
        let mut line = String::new();
        let mut stdin = io::stdin().lock().take(MAX_PROMPT_BYTES + 1);
        let read = stdin
            .read_line(&mut line)
            .map_err(|error| match error.kind() {
                io::ErrorKind::InvalidData => AppError::Usage {
                    message: "standard input is not valid UTF-8.".to_owned(),
                },
                kind => AppError::Io {
                    context: "reading from standard input",
                    kind,
                },
            })?;
        if read == 0 {
            return Ok(None);
        }
        if line.len() as u64 > MAX_PROMPT_BYTES {
            return Err(AppError::Usage {
                message: "an answer is limited to 64 KiB.".to_owned(),
            });
        }
        Ok(Some(line.trim_end_matches(['\r', '\n']).to_owned()))
    }
}

impl Prompter for TerminalPrompter {
    fn is_interactive(&self) -> bool {
        io::stdin().is_terminal()
    }

    fn ask(&mut self, prompt: &Prompt<'_>) -> Result<String, AppError> {
        let mut stderr = io::stderr();
        match prompt.default {
            Some(default) => write!(stderr, "{} [{}]: ", prompt.label, default),
            None => write!(stderr, "{}: ", prompt.label),
        }
        .and_then(|()| stderr.flush())
        .map_err(|error| AppError::Io {
            context: "writing a prompt",
            kind: error.kind(),
        })?;
        self.read_line()?.ok_or(AppError::Cancelled)
    }

    fn confirm(&mut self, question: &str) -> Result<bool, AppError> {
        let mut stderr = io::stderr();
        write!(stderr, "{question} ")
            .and_then(|()| stderr.flush())
            .map_err(|error| AppError::Io {
                context: "writing a prompt",
                kind: error.kind(),
            })?;
        let answer = self.read_line()?.ok_or(AppError::Cancelled)?;
        Ok(matches!(
            answer.trim().to_ascii_lowercase().as_str(),
            "y" | "yes"
        ))
    }

    fn show_error(&mut self, message: &str) {
        eprintln!("  {}", crate::cli::render::sanitize(message));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resolve(pairs: &[(&str, &str)]) -> Result<TerminalEditor, AppError> {
        let pairs: Vec<(String, String)> = pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect();
        TerminalEditor::resolve(move |key| {
            pairs
                .iter()
                .find(|(k, _)| k == key)
                .map(|(_, v)| OsString::from(v))
        })
    }

    fn editor(program: &str, args: &[&str]) -> TerminalEditor {
        TerminalEditor {
            program: program.to_owned(),
            args: args.iter().map(|arg| (*arg).to_owned()).collect(),
        }
    }

    #[test]
    fn visual_takes_precedence_over_editor() {
        let resolved = resolve(&[("VISUAL", "code --wait"), ("EDITOR", "vi")]).unwrap();
        assert_eq!(resolved, editor("code", &["--wait"]));
        assert_eq!(
            resolve(&[("VISUAL", "  "), ("EDITOR", "vi")]).unwrap(),
            editor("vi", &[])
        );
    }

    #[test]
    fn quotes_group_words_and_nothing_is_expanded() {
        let resolved =
            resolve(&[("EDITOR", r#""/opt/my editor/bin" -n 'a b' $HOME `x` ;"#)]).unwrap();
        assert_eq!(
            resolved,
            editor("/opt/my editor/bin", &["-n", "a b", "$HOME", "`x`", ";"])
        );
    }

    #[test]
    fn missing_or_malformed_editors_are_reported() {
        let missing = resolve(&[]).unwrap_err();
        assert!(
            matches!(missing, AppError::Precondition { .. }),
            "{missing:?}"
        );
        let unterminated = resolve(&[("VISUAL", "'vi")]).unwrap_err();
        assert_eq!(
            unterminated.to_string(),
            "VISUAL has an unterminated quote."
        );
    }
}
