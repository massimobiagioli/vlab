//! Application use cases, orchestration, transaction boundaries, and policy.
//!
//! May depend on: `domain` and the ports it declares.
//! Must not depend on: `cli`.
//!
//! Ports (traits) are declared here, where they are consumed, and implemented
//! in `infra`.

pub mod agents;
pub mod ask;
pub mod context;
pub mod deletions;
pub mod directory;
pub mod documents;
mod errors;
pub mod execution;
pub mod floor;
pub mod interactions;
pub mod invocation;
pub mod kanban;
pub mod lab_validation;
pub mod labs;
pub mod memories;
pub mod metrics;
pub mod planning;
pub mod populate;
pub mod presets;
pub mod projects;
pub mod provider_retry;
pub mod requests;
pub mod schedules;
pub mod settings;
pub mod settings_edit;
pub mod storage;
pub mod workflow;

pub use errors::{AppError, EntityKind, ProviderFailure};

pub mod workspace;
pub mod worktrees;
