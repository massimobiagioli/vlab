# Feature 00010 — Startup workflow resume and application logs

| Field | Value |
|---|---|
| Identifier | `00010` |
| Branch | `feat/startup-workflow-logs` |
| Status | `Done` |
| Date | 2026-11-13 |
| Related ADRs | ADR-0049 (startup resume, `--logs`), ADR-0048 (workflow), ADR-0011 (audit events), ADR-0009 (flag surface) |
| Scope | Automatic workflow resume and audit-log view |

## Goal

Remove the manual workflow command and resume pending simulated stories
automatically when the application starts. Add `vlab --logs` so the operator
can read the newest content-free audit events without SQLite.

## Acceptance criteria

1. Bare `vlab` in a terminal resumes pending `IN_PROGRESS`/`IN_REVIEW` stories
   for the active project before opening the dashboard.
2. A missing active project, settings, or provider configuration skips the
   resume without blocking the dashboard; a workflow failure prints a warning.
3. `--run-workflow` no longer exists in the parsed surface or the published
   CLI reference.
4. `vlab --logs` prints the newest 100 audit events, newest first, with time,
   kind, outcome, entity, and error code, and succeeds on an empty log.
5. The log view contains no laboratory, project, agent, story, request, or
   memory content and no credential.
6. Tests cover the log output and the absence of the removed flag; `make check`
   and hosted CI pass.

## Non-goals

- Changing the workflow bounds, schemas, or reviewer rules from ADR-0048.
- Exposing `RUST_LOG` tracing diagnostics through `--logs`.
- A dashboard log screen or a live log tail.
- Manual story transitions.

## Risks

- Startup can perform paid provider calls when stories are pending; ADR-0048
  bounds them, but the user should know that starting the app may continue
  work.
- `--logs` is content-free by design; operators expecting free-form messages
  must use `RUST_LOG`.
- Removing a flag is a breaking CLI change for scripts that used it.

## Validation

- Local gate (2026-11-13): `make check` passed (fmt, clippy, tests, audit,
  deny).
- Hosted CI run `43452010663` passed on Ubuntu, macOS, and audit/deny.
- `tests/cli_logs.rs` creates a laboratory, asserts `lab_created` and
  `memory_created` appear, and asserts the laboratory name and `sk-` marker do
  not.

## Task checklist

- [x] **F00-01** Write ADR-0049 and update the registry and `AGENTS.md`.
- [x] **F01-01** Remove `--run-workflow` and resume pending stories on
  interactive startup.
- [x] **F01-02** Add the `--logs` flag, the audit query, and the table
  rendering.
- [x] **F02-01** Add the log CLI test.
- [x] **F03-01** Update the CLI reference, planning, README, ADR-0048, and the
  feature docs.
- [x] **F04-01** Run `make check` and fix every failure.
- [x] **F04-02** Push, confirm hosted CI is green, and set the status to `Done`.
