# Technology Stack

## Architecture

The application is a Rust-based AI-agent CLI with a text-only interface
and SQLite persistence.

Architectural constraints:

- No web GUI.
- No HTTP server or application API exposed over the network.
- No PostgreSQL or external database service.
- SQLite is the application database.
- HTTPS is used only for outbound integrations when required.
- Start with one bounded agent or a deterministic workflow.

## Version and Maintenance Policy

All technologies listed below are candidates subject to verification before
adoption. Exact versions must be selected using current upstream release
information and security advisories; they must not be guessed.

Mandatory requirements:

- Use stable, supported releases from actively maintained projects.
- Do not use obsolete, deprecated, unsupported, or unmaintained releases.
- Do not use versions with known security vulnerabilities.
- Apply these requirements to direct and transitive dependencies,
  the Rust toolchain, and the SQLite engine.
- Check upstream support policies, RustSec, and relevant vendor advisories.
- Upgrade, replace, or remove dependencies that fail these requirements.
- Do not suppress vulnerability findings to pass release checks.
- Enable only required dependency features.

Record approved dependency versions in `Cargo.toml`, commit `Cargo.lock`,
and pin the verified Rust toolchain in `rust-toolchain.toml`.

Use locked builds in CI and release workflows. Reproducible builds do not
replace security verification.

Security status is time-dependent. A clean advisory scan is not proof that
a dependency contains no vulnerabilities.

## Core Technologies

| Area | Selection | Purpose |
| --- | --- | --- |
| Language | Rust | Memory-safe, typed application implementation |
| Toolchain management | rustup | Install and pin a supported stable toolchain |
| Build system | Cargo | Dependency management, builds, and tests |
| CLI framework | clap with derive | Commands, argument validation, and help |
| Async runtime | Tokio | Provider calls, cancellation, and bounded concurrency |
| Database | SQLite | Local persistent application state |
| Database library | SQLx with SQLite support | Async database access and migrations |
| Serialization | serde and serde_json | Typed provider payloads and structured CLI output |
| HTTP client | reqwest with rustls | Outbound HTTPS calls to AI providers |
| Typed errors | thiserror | Domain and reusable module errors |
| Application errors | anyhow | Contextual errors at application boundaries |
| Diagnostics | tracing and tracing-subscriber | Structured, redacted diagnostic events |

Prefer safe Rust. Justify, isolate, review, and test any necessary unsafe code.

## Textual User Interface

Use `clap` as the CLI framework.

The interface has two presentation modes (ADR-0038):

- plain, width-aware output for scripts, pipes, `--plain`, and non-TTY use;
- full-screen, keyboard-driven dashboards for interactive terminals.

`ratatui` with its re-exported `crossterm` backend is the only TUI stack.

Conventions:

- Send results to stdout and diagnostics to stderr.
- Provide consistent help text and actionable errors.
- Use documented, stable exit codes.
- Offer structured JSON output for scripting where useful.
- Avoid prompts when stdin is not a terminal.
- Disable decorative terminal output when output is redirected.
- Handle cancellation cleanly.
- Never accept secrets through command-line arguments when safer input
  mechanisms are available.

The dashboard requirement is recorded in ADR-0038. Do not add another TUI
framework or a second terminal backend.

## SQLite Persistence

Use SQLite through SQLx. Do not introduce a second database access library
or an ORM without a demonstrated need.

### Storage

- Store the database in the operating system's per-user application data
  directory, not implicitly in the current working directory.
- Use `directories` to resolve platform-appropriate locations, subject to
  the same dependency verification policy.
- Allow an explicit database-path override.
- Restrict database-directory access to the current user using appropriate
  platform permissions or ACLs.
- Do not store provider credentials in the database.

### Database Access

- Use parameterized queries.
- Maintain versioned schema migrations.
- Enable and verify foreign-key enforcement for every connection.
- Keep transactions short.
- Configure a bounded busy timeout and return actionable lock errors.
- Begin with a single connection unless concurrent access requirements
  justify a small pool.
- Enable WAL only when concurrency requirements justify it and the target
  filesystem supports its locking semantics.
- Do not assume WAL supports databases shared over network filesystems.

Treat destructive migrations as high-impact operations requiring explicit
approval and a tested recovery plan.

Use a SQLite-aware backup mechanism. Do not assume copying an active database
file produces a consistent backup.

SQLite does not encrypt application data at rest by default. Minimize stored
sensitive content and rely on approved storage protection; assess database
encryption separately if required.

Store scoped memory content as bounded text with an explicit scope and owner
foreign key. Apply the same migration, retention, and permission rules as other
project data. Store an optional project GitHub source as validated text
metadata; never store the GitHub credential, which is read from
`credentials.github_api_key` in the private `settings.yaml`.

Verify the actual SQLite engine used by the build, whether bundled or
system-provided. Auditing Rust crates alone is insufficient.

## AI Integration

Do not introduce an agent framework initially.

Implement a small provider integration module using `reqwest`, `serde`,
and `serde_json`. Support one provider first.

Outbound HTTPS requests do not require an HTTP server framework.
Axum, Tower server middleware, and web frontend frameworks are not part
of this stack.

### Execution Controls

- Define typed tool inputs, outputs, and errors.
- Validate model-generated arguments before execution.
- Treat model output, retrieved content, and tool responses as untrusted.
- Allowlist tools and grant minimum required permissions.
- Enforce deadlines, step limits, token budgets, concurrency limits, and a
  fixed memory-context budget.
- Include memory content only inside delimited data blocks and never as
  instructions or policy.
- Bound retries and retry only appropriate failures.
- Do not automatically retry non-idempotent actions without safeguards.
- Require explicit approval for destructive, irreversible, or high-impact actions.
- Do not provide unrestricted shell access.
- Keep TLS certificate validation enabled.
- Keep credentials out of source control except the private, git-ignored
  `settings.yaml`; keep them out of prompts, logs, and command output.
- Use the local `git` executable for project clones with a private askpass
  helper, no shell, a bounded timeout, and no token in arguments (ADR-0032).

Use supported provider APIs and supported model identifiers. Record model
configuration for evaluation and troubleshooting without recording secrets.

## Testing

| Tool | Purpose |
| --- | --- |
| Rust test framework | Unit and integration tests |
| assert_cmd | CLI process, output, and exit-code tests |
| tempfile | Isolated filesystem and SQLite test fixtures |
| wiremock | Local HTTP test doubles for outbound provider calls |

`wiremock` is a development dependency, not an application HTTP server.

Required tests:

- Argument parsing, help, and exit codes.
- stdout/stderr separation and JSON output.
- Non-interactive behavior.
- Database initialization, migrations, constraints, and lock handling.
- Provider timeouts, rate limits, and malformed responses.
- Cancellation and execution-budget exhaustion.
- Tool authorization and approval requirements.
- Prompt-injection attempts from untrusted content.
- Secret redaction.

Use temporary, file-backed SQLite databases for tests that exercise connection,
locking, or persistence behavior.

Keep normal CI tests deterministic with mocked providers. Live-provider tests
must be opt-in, isolated, and budget-limited.

## Quality and Security Tooling

- rustfmt for formatting.
- Clippy for static analysis.
- cargo-audit for RustSec advisory checks.
- cargo-deny for dependency, advisory, license, and source policies.
- Dependabot or Renovate for dependency-update proposals; choose one.

Configure checks to reject unmaintained dependencies and known vulnerable
versions. Supplement automated scans with upstream maintenance and vendor
advisory review.

Run on every proposed change:

    cargo fmt --all -- --check
    cargo clippy --locked --all-targets -- -D warnings
    cargo test --locked
    cargo audit
    cargo deny check

Add CI jobs for every shipped feature combination and supported platform.

Run daily security scans and refresh advisory data before releases.
Block releases when security checks fail or current advisory data cannot
be obtained.

## First Delivery Increment

Implement one CLI command that:

1. Validates a textual request.
2. Initializes or opens the SQLite database.
3. Calls one AI provider with a deadline and bounded output allowance.
4. Stores only the minimal required, non-secret execution metadata.
5. Prints a result to stdout and diagnostics to stderr.
6. Passes CLI, database, provider-mock, and dependency-security checks.

Add further tools and interaction features only through small, independently
testable changes.
