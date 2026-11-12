# Continuous Integration

Policy: [ADR-0013](adr/adr-0013.md), which defers hosted CI to Go-Live, and
[ADR-0012](adr/adr-0012.md), which defines the configuration that Go-Live
activates. See also [`STACK.md`](STACK.md) (Quality and Security Tooling).

## Status: disabled until Go-Live

The workflow and Dependabot files exist, but every line is commented out.
Until the Go-Live phase of the implementation plan, the quality gates run
locally, and a change is complete only when all of them pass:

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

## Workflows (active after Go-Live)

| File | Trigger | Runs |
|---|---|---|
| `.github/workflows/ci.yml` | every push and pull request | On Linux and macOS: `cargo fmt --all -- --check`, `cargo clippy --locked --all-targets --all-features -- -D warnings`, `cargo test --locked`. On Linux: `cargo audit`, `cargo deny check`. |
| `.github/workflows/security.yml` | daily at 05:17 UTC, and on demand | `cargo audit` and `cargo deny check advisories` with fresh advisory data. |
| `.github/dependabot.yml` | weekly | Update proposals for Cargo and GitHub Actions. |

To activate a file, delete its four-line `DISABLED` header and remove the
leading `# ` from every remaining line. The result is the reviewed
configuration, unchanged.

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

## Manual repository settings (Go-Live)

These settings cannot be stored in the repository. Apply them at Go-Live,
after the workflows are active:

- Protect the default branch: require a pull request, require the status
  checks `fmt, clippy, test (ubuntu-latest)`, `fmt, clippy, test
  (macos-latest)`, and `audit and deny`, and forbid force pushes.
- Set the default workflow token permission to read-only.
- Enable Dependabot alerts and security updates.
- Enable private vulnerability reporting (see A-21 in the implementation
  plan).
