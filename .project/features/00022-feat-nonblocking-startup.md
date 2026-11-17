# Feature 00022 — Non-blocking startup resume

| Field | Value |
|---|---|
| Identifier | `00022` |
| Branch | `feat/nonblocking-startup` |
| Status | `Done` |
| Date | 2026-11-17 |
| Related ADRs | ADR-0064 (startup), ADR-0049 (resume), ADR-0061 (worker) |
| Scope | Open the dashboard immediately and run startup schedules/workflow in the background worker |

## Goal

Remove the synchronous `Resuming pending workflow...` startup block. The
dashboard must open at once and display background progress while due schedules
and pending story execution run in the existing worker.

## Acceptance criteria

1. Bare `vlab` in a terminal opens the dashboard immediately; it never writes
   `Resuming pending workflow...` to stderr before the terminal starts.
2. The worker performs one startup pass before selecting queued requests:
   due schedules for the current project, then pending `IN_PROGRESS`/`IN_REVIEW`
   stories with the existing bounded workflow.
3. The status line shows `Background:` progress and warnings; the dashboard
   remains responsive while the pass runs.
4. Missing active project, settings, or provider skips the pass with a status or
   warning and does not block or crash the dashboard.
5. Quitting cancels the shared token; no daemon, timer, or second worker is
   introduced.
6. No schema, dependency, provider-contract, or trust-boundary change.
7. Tests cover the worker status event and the absence of the pre-TUI blocking
   call; `make check` passes and hosted CI is green.

## Non-goals

- Running schedules or workflows in a separate process or daemon.
- Parallel startup work and queued request planning.
- Changing workflow budgets or provider behavior.
- Adding a new command for startup resume.

## Risks

- A long startup workflow delays queued requests until it finishes; this is
  intentional and visible in the status line.
- Cancellation must reach the startup pass; the worker shares its token with
  the existing workflow.
- The dashboard status can be overwritten by a later worker event; the most
  recent operation is the useful one.

## Validation

- Local gate: `make check` passed (fmt-check, clippy, tests, audit, deny).
- `src/cli/tui/mod.rs` test covers the `Background:` status event.
- The pre-TUI call is removed from `src/cli/mod.rs`; startup work is invoked
  only from `src/cli/tui/worker.rs`.
- Hosted CI is green on commit `b4c2371`.

## Task checklist

- [x] **F00-01** Write ADR-0064; update the registry and ADR-0049 amendment.
- [x] **F01-01** Remove the synchronous pre-TUI resume call.
- [x] **F02-01** Run startup schedules and workflow in the worker with shared
  cancellation and `Background:` events.
- [x] **F03-01** Show background status in the dashboard.
- [x] **F04-01** Update the specification, planning, CLI reference, README,
  playbook, and skill guidance.
- [x] **F05-01** Add the status-event test and verify `make check`.
- [x] **F05-02** Push, confirm hosted CI is green, and set the status to `Done`.
