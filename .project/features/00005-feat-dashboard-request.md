# Feature 00005 — Dashboard request intake

| Field | Value |
|---|---|
| Identifier | `00005` |
| Branch | `feat/dashboard-request` |
| Status | `In review` |
| Date | 2026-11-13 |
| Related ADRs | ADR-0044 (dashboard request intake), ADR-0043 (Kanban), ADR-0026 (provider port), ADR-0039 (dashboard entry point) |
| Scope | Projects and Kanban request submission |

## Goal

Submit a new request from the dashboards through a simple, clean prompt dialog,
replacing the Kanban hint that points to the hidden `--new-request` flag.

## Acceptance criteria

1. `r` in Projects and `n` in the Kanban open a prompt dialog for the project
   and make it current.
2. The dialog has one bordered text area with a cursor, wrapping, scrolling,
   editing keys, `Alt+Enter` line breaks, bracketed paste, a character count,
   `Enter` submit, and `Esc` cancel. Empty input is not submitted.
3. Submission uses `requests::plan` with the standard limits and provider; a
   `Planning…` dialog is shown; `Esc`/`Ctrl-C` cancels without retry.
4. The outcome dialog shows planned stories, questions, failure code and hint,
   or cancellation. Model text is sanitized; the board and table reload.
5. The Kanban empty state reads `Press n to submit a request.`
6. No second database connection; no request text in logs or events.
7. Unit, TestBackend, and PTY tests (with a loopback mock provider) cover the
   above; `make check` and hosted CI pass.

## Non-goals

- No retry command or editing of an existing request.
- No streaming or animated progress.
- Hidden `--new-request` behavior is unchanged.

## Risks

- Planning blocks redraws for up to 240 s; mitigated by the static dialog and
  `Esc` cancellation.

## Validation

Local gate (2026-11-13): `make check` passed (fmt, clippy, tests, audit, deny).
Hosted CI: pending; the branch has not been pushed.

## Task checklist

- [x] **F00-01** Write ADR-0044 and update the registry and `AGENTS.md`.
- [x] **F01-01** Add the prompt buffer (multi-line insert, paste
  normalization) with unit tests.
- [x] **F01-02** Render the prompt dialog with wrapping, cursor, scrolling,
  and footer; TestBackend tests.
- [x] **F02-01** Wire `r` in Projects and `n` in the Kanban; update hints,
  action bars, and help.
- [x] **F02-02** Run planning with the `Planning…` dialog, cancellation
  watcher, outcome dialog, and reload; enable bracketed paste.
- [x] **F03-01** PTY test against a loopback mock provider.
- [x] **F04-01** Update planning, specification, CLI reference, playbook,
  README; run `make check`.
- [ ] **F04-02** Push, confirm hosted CI is green, and set the status to `Done`.
