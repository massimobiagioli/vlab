//! Infrastructure adapters: filesystem, configuration files, SQLite, and the
//! provider. Implements ports declared in `app` and `domain`.
//!
//! May depend on: `domain`, and the port traits and error types of `app`.
//! Must not depend on: `cli`, or `app` use-case logic.

pub mod agent_definition;
pub mod config;
pub mod db;
pub mod execution;
pub mod files;
pub mod git_repository;
pub mod paths;
pub mod presets;
pub mod provider;

pub mod workspace;
