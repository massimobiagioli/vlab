# Dependency Register

Every direct dependency, with its purpose, license, and the evidence used to
approve it. Policy: [`STACK.md`](STACK.md) and
[ADR-0008](adr/adr-0008.md). A dependency is added only by the work package
that first uses it.

Verification method for every entry: latest release on crates.io checked with
`cargo info`; `cargo audit` (RustSec database, 1471 advisories loaded) and
`cargo deny check` (advisories including unmaintained and unsound, licenses,
bans, sources) pass on the resulting `Cargo.lock`. A clean scan is evidence at
a point in time, not proof of absence of vulnerabilities.

## Toolchain

| Item | Version | Verified | Evidence |
|---|---|---|---|
| Rust | 1.99.1, edition 2024 | 2026-11-12 | `rustup check` reports 1.99.1 as the current stable release. Pinned in `rust-toolchain.toml`. |
| Git | 2.54.0 verified locally; requires a maintained version on `PATH` | 2026-11-12 | Used only for project clones (ADR-0032). Hosted Ubuntu and macOS runners include git. Tests use a fake executable or a local `file://` repository. |
| SQLite engine | 3.54.1, bundled | 2026-11-12 | `libsqlite3-sys` 0.39.1 bundles SQLite 3.54.1 (`SQLITE_VERSION` in the vendored `sqlite3.h`). The bundled engine is what the build runs, so no system SQLite affects behavior. `sqlx-sqlite` 0.9.0 is vendored with its upper bound widened so this fixed release can be selected (ADR-0017). |

## Runtime dependencies

| Crate | Version | Features | Purpose | License | Verified |
|---|---|---|---|---|---|
| `clap` | 4.6.7 | `std`, `derive`, `help`, `usage`, `error-context`, `suggestions` (no `color`) | Argument parsing and help (`STACK.md`). Without `color`, clap cannot emit ANSI sequences (ADR-0005). | MIT OR Apache-2.0 | 2026-11-12 |
| `ctrlc` | 3.5.2 | default | SIGINT handler that cancels in-flight planning (ADR-0026). | MIT OR Apache-2.0 | 2026-11-12 |
| `directories` | 6.0.0 | default | Per-user configuration and data directories (`STACK.md`). RUSTSEC-2020-0054 (unmaintained) was withdrawn in 2021; 6.0.0 was released 2025-01-12. | MIT OR Apache-2.0 | 2026-11-12 |
| `serde` | 1.0.229 | `std`, `derive` (default features off) | Typed deserialization for settings (ADR-0015) and provider payloads in WP-5. | MIT OR Apache-2.0 | 2026-11-12 |
| `serde_json` | 1.0.154 | default | Strict planning JSON and bounded JSON columns (WP-5, ADR-0026). | MIT OR Apache-2.0 | 2026-11-12 |
| `serde-saphyr` | 2.1.1 | `deserialize` (default features off) | Strict YAML parsing for `settings.yaml` (ADR-0008, ADR-0015). | MIT OR Apache-2.0 | 2026-11-12 |
| `shell-words` | 1.1.1 | default | Splits `VISUAL`/`EDITOR` into executable and arguments without a shell (ADR-0015). | MIT OR Apache-2.0 | 2026-11-12 |
| `ratatui` | 0.30.2 | `crossterm`, `std` (default features off) | Interactive dashboards: layout, widgets, terminal lifecycle, and the re-exported `crossterm` backend (ADR-0038). | MIT | 2026-11-13 |
| `reqwest` | 0.13.5 | `blocking`, `json`, `rustls` (default features off) | Outbound DeepSeek HTTPS client with bounded timeout/size (ADR-0026). | MIT OR Apache-2.0 | 2026-11-12 |
| `sqlx` | 0.9.0 | `runtime-tokio`, `sqlite`, `migrate`, `macros` (default features off) | SQLite access and embedded migrations (`STACK.md`, ADR-0002, ADR-0016). | MIT OR Apache-2.0 | 2026-11-12 |
| `thiserror` | 2.0.21 | default | Typed errors (`STACK.md`). | MIT OR Apache-2.0 | 2026-11-12 |
| `tokio` | 1.54.1 | `rt`, `time` (default features off) | Runtime for the SQLite driver and its busy timeout (ADR-0016). | MIT | 2026-11-12 |
| `time` | 0.3.55 | `macros` | UTC calendar arithmetic for locally computed story due dates (ADR-0026). | MIT OR Apache-2.0 | 2026-11-12 |
| `terminal_size` | 0.4.4 | default | Detects the terminal width for table layout (ADR-0021). | MIT OR Apache-2.0 | 2026-11-12 |
| `tracing` | 0.1.44 | `std` | Structured diagnostics (`STACK.md`). | MIT | 2026-11-12 |
| `tracing-subscriber` | 0.3.23 | `std`, `fmt`, `env-filter` (no `ansi`) | Stderr diagnostics filtered by `RUST_LOG`. | MIT | 2026-11-12 |
| `unicode-width` | 0.2.3 | default | Display width for aligned tables, including wide characters (ADR-0021). | MIT OR Apache-2.0 | 2026-11-12 |
| `uuid` | 1.29.2 | `std`, `v4` | Run and action identifiers; entity identifiers in WP-3 (ADR-0004). | Apache-2.0 OR MIT | 2026-11-12 |

`crossterm` 0.29.0 is a transitive runtime dependency of `ratatui-crossterm`; it
is not declared directly. It is MIT licensed and provides terminal setup, raw
mode, and key events for ADR-0038.

## Development dependencies

| Crate | Version | Purpose | License | Verified |
|---|---|---|---|---|
| `assert_cmd` | 2.2.2 | Run the binary and capture exit code and streams (`STACK.md`). | MIT OR Apache-2.0 | 2026-11-12 |
| `tempfile` | 3.27.0 | Isolated filesystem sandboxes (`STACK.md`). | MIT OR Apache-2.0 | 2026-11-12 |
| `wiremock` | 0.6.5 | Local HTTP test double for DeepSeek contract tests (`STACK.md`). | MIT OR Apache-2.0 | 2026-11-12 |

## Transitive license exception

| Crate | License | Reached through | Decision |
|---|---|---|---|
| `option-ext` 0.2.0 | MPL-2.0 | `directories` → `dirs-sys` | Allowed for this crate only, unmodified (ADR-0012). |
| TLS dependency graph | ISC, CDLA-Permissive-2.0 | `reqwest` → rustls/aws-lc-rs/webpki | Allowed explicitly for permissive TLS crates; no crate ignored (ADR-0027). |

## Patched dependencies

| Crate | Patch | Reason | Exit condition |
|---|---|---|---|
| `sqlx-sqlite` 0.9.0 | Vendored from crates.io; `Cargo.toml` only, `libsqlite3-sys` upper bound `<0.38.0` → `<0.40.0` | RUSTSEC-2026-0459; `libsqlite3-sys` 0.39.x bundles the fixed SQLite 3.54.x, but released SQLx 0.9.0 refuses it. Upstream `sqlx` `main` already uses `<0.40.0`. | Remove `vendor/sqlx-sqlite` and the `[patch.crates-io]` entry once a published SQLx release accepts `libsqlite3-sys >=0.39`. See ADR-0017 and `vendor/README.md`. |

## Deferred selections

These are required by later packages and are added, with fresh verification,
by the package that first uses them. Versions shown are the latest releases
observed on 2026-11-12, for planning only.

| Need | Candidate | Observed | First use |
|---|---|---|---|
| Contextual errors | `anyhow` | 1.0.104 | First app-boundary use |
| Output predicates | `predicates` | 3.1.4 | Only if plain assertions prove insufficient |

### YAML parser evaluation

The YAML ecosystem has several unmaintained options, so the choice is made
explicitly (ADR-0008):

| Crate | Status on 2026-11-12 | Decision |
|---|---|---|
| `serde_yaml` | Published as `0.9.34+deprecated` by its author. | Rejected. |
| `serde_yml` | RUSTSEC-2025-0068: unsound and unmaintained. | Rejected. |
| `serde_yaml_ng` | Fork of `serde_yaml`; last release 2024-05; depends on `unsafe-libyaml`, which is unmaintained (RUSTSEC-2024-0449). | Rejected. |
| `serde_norway` | Fork of `serde_yaml`; last release 2024-12; depends on `unsafe-libyaml-norway`, a fork of the same machine-translated C parser built on `unsafe` code. | Rejected. |
| `serde-saphyr` | Pure Rust, actively released (2.1.1, 2026-11-05), MIT OR Apache-2.0, no RustSec advisory. Several of its dependencies are optional integrations; WP-2 enables none beyond what settings parsing needs. | Selected for WP-2, subject to re-verification when added. |
