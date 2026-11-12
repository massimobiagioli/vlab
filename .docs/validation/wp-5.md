# WP-5 Validation Report

Date: 2026-11-12
Package: WP-5 — Request Intake and Tech Lead Planning
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

| Command | Result |
|---|---|
| `cargo fmt --all -- --check` | pass |
| `cargo clippy --locked --all-targets -- -D warnings` | pass |
| `cargo test --locked` | 301 passed, 0 failed |
| `cargo audit` | no findings |
| `cargo deny check` | advisories ok, bans ok, licenses ok, sources ok |

## Mock-based evidence

- `src/domain/budget.rs` and `src/domain/clock.rs`: deadline, attempt,
  cancellation, and zero-budget boundaries.
- `src/app/provider_retry.rs`: retryable failures produce three attempts with
  500 ms/1 s backoff; non-retryable auth produces one; cancellation prevents
  dispatch; `Retry-After` exceeding the deadline yields `budget_exceeded`.
- `src/app/planning/prompt.rs`: roster, no CV, escaped request, memory order,
  8 KiB whole-memory cut-off.
- `src/app/planning/schema.rs`: valid plan; unknown fields/due dates, wrong
  schema, both/neither arrays, empty criteria, bad skills, 21 stories.
- `src/app/planning/rules.rs`: due dates including leap year and year boundary;
  out-of-lab and archived assignees; unassigned explanation.
- `tests/planning_flow.rs`: success, clarification, invalid output, provider
  failure, stale recovery, and request-before-provider.
- `tests/planning_cancel.rs`: cancellation before/during dispatch, timeout
  retries, budget exhaustion.
- `tests/security_planning.rs`: injected request text, out-of-lab assignment,
  CV/secret markers, memory delimiting, inert tool-like text, no request echo.
- `tests/provider_deepseek.rs`: wiremock success, 401, 429, 500, malformed,
  truncated/oversized body, timeout, key only in header.
- `tests/cli_new_request.rs`: input matrix and end-to-end mock planning.

## Acceptance criteria

| ID | Evidence | Result |
|---|---|---|
| WP5-AC1 | budget/clock/retry tests | pass |
| WP5-AC2 | `tests/provider_deepseek.rs` | pass |
| WP5-AC3 | retry tests | pass |
| WP5-AC4 | prompt golden tests | pass |
| WP5-AC5 | schema tests | pass |
| WP5-AC6 | rules tests | pass |
| WP5-AC7 | `tests/planning_flow.rs` | pass |
| WP5-AC8 | `tests/cli_new_request.rs` | pass |
| WP5-AC9 | `tests/security_planning.rs` | pass |
| WP5-AC10 | `tests/planning_cancel.rs` | pass |
| WP5-AC11 | `tests/cli_new_request.rs` end-to-end | pass |
| WP5-AC12 | prompt memory order/cut-off tests | pass |
| WP5-AC13 | WP-5-T13 tests and migration | pass |

## Limitations and unverified items

- The DeepSeek contract is verified against `wiremock`, not the live provider;
  GL-T2 confirms the endpoint/model and authentication mapping.
- The provider port is synchronous (`reqwest::blocking`) by ADR-0026; a future
  concurrent client would need a new decision.
- SIGINT handling is installed in the CLI, but a real PTY interrupt was not
  automated; cancellation is covered through the token and scripted providers.
- A literal request position on a real TTY is behaviorally implemented but not
  exercised by a PTY test; the literal validation path is unit-tested.
- Live retry charges, real-provider token usage, and model semantic quality are
  evaluation criteria for GL-T3, not deterministic proofs.
- No story state transitions exist; the planner creates `TODO` stories only.
