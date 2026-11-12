# Vendored dependencies

## `sqlx-sqlite` 0.9.0

- Purpose: temporary patch for RUSTSEC-2026-0459.
- Provenance: the published crate `sqlx-sqlite` 0.9.0 from crates.io
  (checksum `488e99c397a62007e4229aec669a179816339afc6d2620ca6fa420dbee2e982c`),
  unpacked unchanged except as noted below.
- Modification: in `Cargo.toml`, the `libsqlite3-sys` requirement is widened
  from `>=0.30.1, <0.38.0` to `>=0.30.1, <0.40.0`. No Rust source file is
  changed.
- Why: `libsqlite3-sys` 0.38.2 and earlier bundle SQLite releases affected by
  CVE-2026-119375. The first fixed release is 0.39.0 (SQLite 3.54.0), but
  `sqlx` 0.9.0 constrains the dependency below it. Upstream `sqlx` `main`
  already declares `<0.40.0`; this patch applies the same bound until a
  release contains it.
- Removed files: `Cargo.lock`, `Cargo.toml.orig`, and `.cargo-ok` from the
  published archive. They do not affect the build.
- License: MIT OR Apache-2.0, unchanged. The license files are included.
- Exit condition: when a published `sqlx`/`sqlx-sqlite` release requires
  `libsqlite3-sys >=0.39`, delete this directory, remove the
  `[patch.crates-io]` entry from the root `Cargo.toml`, and run the gates.
- Decision record: [ADR-0017](../.docs/adr/adr-0017.md).
