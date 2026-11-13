# Feature 00012 — Metrics dashboard

| Field | Value |
|---|---|
| Identifier | `00012` |
| Branch | `feat/metrics-dashboard` |
| Status | `In progress` |
| Date | 2026-11-13 |
| Related ADRs | ADR-0051 (metrics), ADR-0038 (dashboards), ADR-0043 (board counts) |
| Scope | Read-only laboratory metrics from the home menu |

## Goal

Add a **Metrics** item to the home menu that shows the active laboratory's card
counts: TODO, in progress, in review, blocked, done, open, closed, total,
completion percentage, pending intake, and a per-project breakdown.

## Acceptance criteria

1. The home menu lists Metrics as the sixth item; `Enter` or `6` opens it.
2. The screen shows the active laboratory name and project count.
3. It shows `TODO`, `IN_PROGRESS`, `IN_REVIEW`, `BLOCKED`, and `DONE` counts;
   open, closed, total, and completion percentage; and pending intake.
4. Open = TODO + IN_PROGRESS + IN_REVIEW + BLOCKED; closed = DONE; completion
   is closed/total or `—` when total is zero.
5. A project table lists every project with TODO, open, closed, and total;
   projects without stories show zeros.
6. `r` reloads, `?` shows help, `Esc` returns home, `q` quits; the screen is
   read-only and makes no provider call.
7. Without an active laboratory, an explicit empty state is shown.
8. Tests cover the aggregate query and the rendered screen; `make check` and
   hosted CI pass.

## Non-goals

- Live updates while a workflow runs.
- Charts, historical trends, or per-agent metrics.
- A CLI flag or JSON output.
- Changing story states from the metrics screen.

## Risks

- Many projects make the screen long; the current implementation clips to the
  terminal. Pagination can follow if needed.
- Counts are a snapshot and can be stale until `r`.
- `BLOCKED` counts as open, which is intentional but must be documented.

## Validation

- Local gate (2026-11-13): `make check` passed (fmt, clippy, tests, audit,
  deny).
- `tests/db_metrics.rs` covers the aggregate, an empty project, and an unknown
  laboratory.
- The TUI test opens Metrics from the home menu and asserts the labels and
  counts at 100×30.
- Hosted CI is required before the feature is marked `Done`.

## Task checklist

- [x] **F00-01** Write ADR-0051 and update the registry and `AGENTS.md`.
- [x] **F01-01** Add the `MetricsStore` port, aggregate query, and project
  breakdown with tests.
- [x] **F01-02** Add the Metrics screen, menu entry, key handling, reload, and
  empty state.
- [x] **F02-01** Add the TUI test and update the home dashboard test.
- [x] **F03-01** Update the CLI reference, README, playbook, specification, and
  feature docs.
- [x] **F04-01** Run `make check` and fix every failure.
- [ ] **F04-02** Push, confirm hosted CI is green, and set the status to `Done`.
