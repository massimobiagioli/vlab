//! Domain rules, validation, and value types.
//!
//! May depend on: the standard library, `thiserror`, and time/ID value types.
//! Must not depend on: I/O, the filesystem, the network, `sqlx`, `reqwest`, or
//! `clap`. Domain code performs no I/O.

pub mod agent;
pub mod budget;
pub mod clock;
pub mod ids;
pub mod lab;
pub mod memory;
pub mod name;
pub mod project;
pub mod provider;
pub mod settings;
pub mod skills;
