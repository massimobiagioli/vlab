//! VLab: a local command-line application for virtual software laboratories.
//!
//! Layers and allowed dependencies (ADR-0001):
//!
//! ```text
//! cli -> app -> domain
//!         ^
//!         |
//!       infra (implements ports declared in app/domain)
//! ```
//!
//! `obs` provides diagnostics and audit events to every layer and depends on
//! no domain policy.

#![forbid(unsafe_code)]
// Expected failures are typed errors; tests may unwrap.
#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used))]

pub mod app;
pub mod cli;
pub mod domain;
pub mod infra;
pub mod obs;
