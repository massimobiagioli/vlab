//! Diagnostics, redaction, and audit-event emission (ADR-0011).
//!
//! May depend on: `tracing` APIs.
//! Must not depend on: domain policy.
//!
//! Diagnostics go to stderr only, are off unless `RUST_LOG` is set, and never
//! carry ANSI sequences. Request text, provider bodies, CVs, and credentials
//! are never accepted as fields: audit events are typed (see [`events`]) and
//! secrets travel inside [`Redacted`].

pub mod events;

use std::fmt;

use tracing_subscriber::EnvFilter;

const LOG_VAR: &str = "RUST_LOG";

/// Installs the stderr diagnostics subscriber when `RUST_LOG` is set.
pub fn init_diagnostics() {
    let Some(directives) = std::env::var(LOG_VAR)
        .ok()
        .filter(|value| !value.trim().is_empty())
    else {
        return;
    };
    let filter = match EnvFilter::try_new(directives) {
        Ok(filter) => filter,
        Err(_) => {
            eprintln!("Warning: {LOG_VAR} is invalid; diagnostics are disabled.");
            return;
        }
    };
    // Fails only if a subscriber is already installed, which is harmless.
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .try_init();
}

/// A value that must never be displayed: every format prints `[REDACTED]`.
#[derive(Clone, PartialEq, Eq)]
pub struct Redacted<T>(T);

impl<T> Redacted<T> {
    pub fn new(value: T) -> Self {
        Self(value)
    }

    /// The wrapped value, for the single place that legitimately uses it.
    pub fn expose(&self) -> &T {
        &self.0
    }
}

impl<T> fmt::Debug for Redacted<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[REDACTED]")
    }
}

impl<T> fmt::Display for Redacted<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[REDACTED]")
    }
}

/// Identifies one process invocation; generated once in `main`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RunId(uuid::Uuid);

impl RunId {
    pub fn generate() -> Self {
        Self(uuid::Uuid::new_v4())
    }
}

impl fmt::Display for RunId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.simple().fmt(f)
    }
}

/// Identifies one discrete operation within a run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActionId(uuid::Uuid);

impl ActionId {
    pub fn generate() -> Self {
        Self(uuid::Uuid::new_v4())
    }
}

impl fmt::Display for ActionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.simple().fmt(f)
    }
}
