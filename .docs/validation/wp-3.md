# WP-3 Validation Report

Date: 2026-11-12
Package: WP-3 — Laboratories and Agent Profiles
Environment: macOS (aarch64), Rust 1.99.1, bundled SQLite 3.54.1
Commit: working tree, no tag

## Executed commands

```sh
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo audit
cargo deny check
```

`make check` runs the same five commands in order.

Results:

| Command | Result |
|---|---|
| `cargo fmt --all -- --check` | pass |
| `cargo clippy --locked --all-targets -- -D warnings` | pass |
| `cargo test --locked` | 185 passed, 0 failed |
| `cargo audit` | no findings (1471 advisories loaded) |
| `cargo deny check` | advisories ok, bans ok, licenses ok, sources ok |

## Walkthrough (mock/local, no provider)

Executed through the real binary in an isolated sandbox:

1. `vlab --new-lab "Payments Platform"` prints `Laboratory created.`, the short
   identifier, and the Tech Lead requirement. Exactly one `labs` row exists;
   `app_state` is unchanged; one `lab_created` event is recorded.
2. `vlab --open-lab <lab-id>` prints the summary to stdout and warnings for the
   incomplete laboratory to stderr; `app_state.active_lab_id` persists across
   invocations.
3. `vlab --check-lab <lab-id>` prints one line per rule and `Result: invalid`,
   exits 4, and explains the next action on stderr.
4. A complete laboratory (one `lead`, one `qa` with skills) yields
   `Result: valid` and exit 0 without a settings file or credential.
5. `vlab --list-labs` shows ID, name, member count, validation, and the active
   marker; the empty state names the create command.
6. `vlab --new-agent` from a pipe exits 4 with a hint and creates no database;
   the interactive flow is covered by scripted-prompter unit tests.
7. `vlab --list-agents` requires an active laboratory, excludes archived
   agents, and renders `N/A` for project and story counts.
8. `vlab --show-agent-detail <agent-id>` shows every stored field, skills, CV,
   memberships, archived status, and literal `N/A` metrics.

## Acceptance criteria

| ID | Evidence | Result |
|---|---|---|
| WP3-AC1 | Domain unit tests in `src/domain/{name,skills,agent,lab}.rs` | pass |
| WP3-AC2 | ID round-trip, collision retry, exhaustion, and spy-repository malformed-ID test | pass |
| WP3-AC3 | `tests/db_labs_agents.rs` constraints, cascade, migration preservation, atomic insert | pass |
| WP3-AC4 | `tests/cli_labs.rs` (10 tests) | pass |
| WP3-AC5 | `src/app/lab_validation.rs` (6 tests) and `tests/cli_labs.rs` report tests | pass |
| WP3-AC6 | `src/app/agents/wizard.rs` (8 tests) and `tests/cli_agent_noninteractive.rs` | pass |
| WP3-AC7 | `tests/cli_agents.rs` (9 tests) | pass |
| WP3-AC8 | `src/cli/render.rs` (7 tests) at widths 40, 80, 120 | pass |
| WP3-AC9 | `tests/provider_scope.rs`; no HTTP dependency before WP-5 | pass |

## Assumptions confirmed

- A-07 (operational roles need at least one skill; the lead may have none).
- A-09 (`--new-agent` is interactive-only; piped input exits 4).

Both confirmed by the product owner on 2026-11-12 and recorded in ADR-0020.

## Limitations and unverified items

- Linux was not run locally; hosted CI is deferred to Go-Live (ADR-0013).
- The wizard was exercised through the scripted `Prompter` port and the
  non-TTY refusal, not through a real PTY session.
- The CV prompt is single-line; a multiline input flow is not implemented.
- `--check-lab` validates agent provider/model copies, not the current
  `settings.yaml` (agents own copies per §5.4). Project references become
  checkable in WP-4.
- Terminal width is injected in tests and defaults to 100 columns when output
  is redirected; a live narrow terminal was not manually inspected.
- The one-Tech-Lead rule is enforced by the application, not by SQL; a direct
  database edit can violate it and `--check-lab` reports it.
