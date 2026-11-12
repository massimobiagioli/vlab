# Continuous Integration

Policy: [ADR-0029](adr/adr-0029.md), which activates hosted CI for the
pre-release build, and [ADR-0012](adr/adr-0012.md), which defines the
configuration. See also [`STACK.md`](STACK.md) (Quality and Security Tooling).

## Status: active

The CI, daily security scan, and Dependabot configuration are active. Every
push and pull request runs the hosted checks; the daily scan refreshes advisory
data. The local gate remains mandatory before every change:

```sh
make check
```

`make check` runs, in order:

```sh
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo audit
cargo deny check
```

Install the supply-chain tools once with `make tools`, or by hand:

```sh
cargo install --locked cargo-audit --version 0.23.0
cargo install --locked cargo-deny --version 0.20.2
```

## Workflows

| File | Trigger | Runs |
|---|---|---|
| `.github/workflows/ci.yml` | every push and pull request | On Linux and macOS: `cargo fmt --all -- --check`, `cargo clippy --locked --all-targets --all-features -- -D warnings`, `cargo test --locked`. On Linux: `cargo audit`, `cargo deny check`. |
| `.github/workflows/security.yml` | daily at 05:17 UTC, and on demand | `cargo audit` and `cargo deny check advisories` with fresh advisory data. |
| `.github/dependabot.yml` | weekly | Update proposals for Cargo and GitHub Actions. |

Hardening:

- Workflow token permissions are `contents: read`.
- Third-party actions are pinned to a full commit SHA, with the release tag in
  a comment. Only `actions/checkout` and `actions/cache` are used.
- `actions/checkout` runs with `persist-credentials: false`.
- The toolchain comes from `rust-toolchain.toml` through the runner's
  `rustup`. `cargo-audit` and `cargo-deny` are installed with
  `cargo install --locked` at the versions pinned in the workflow `env`.
- Caches are keyed by the toolchain file and `Cargo.lock`, or by tool
  versions.

## Manual repository settings

These settings cannot be stored in the repository. Apply them before the
release:

- Protect the default branch: require a pull request, require the status
  checks `fmt, clippy, test (ubuntu-latest)`, `fmt, clippy, test
  (macos-latest)`, and `audit and deny`, and forbid force pushes.
- Set the default workflow token permission to read-only.
- Enable Dependabot alerts and security updates.
- Enable private vulnerability reporting (see A-21 in the implementation
  plan).

## Notes

- Hosted CI is active before the release tag at the product owner's request;
  it does not replace the local gate evidence in package reports.
- A red hosted run is fixed with the normal TDD loop and a local gate run.
- The vendored `sqlx-sqlite` path dependency and bundled SQLite build are
  exercised on both hosted platforms.
