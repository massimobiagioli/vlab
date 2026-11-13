# Feature 00002 — Laboratory description, edit, and open projects

| Field | Value |
|---|---|
| Identifier | `00002` |
| Branch | `feat/lab-description` |
| Status | `Done` |
| Date | 2026-11-13 |
| Related ADRs | ADR-0041 (schema v7), ADR-0038/0039 (dashboards) |
| Scope | Laboratory domain, schema v7, Labs dashboard actions |

## Goal

Give each laboratory an optional description and make the Labs dashboard
actionable for editing and for moving directly into the active laboratory's
projects.

## Acceptance criteria

1. The Laboratory model and schema store an optional bounded description.
2. The Labs dashboard shows the description as its own aligned column.
3. The `Edit` action opens a form prefilled with the selected laboratory's name
   and description and persists validated changes.
4. The `New` form accepts an optional description.
5. The `Projects` action sets the selected laboratory active and opens the
   Projects dashboard for it.
6. Existing laboratories migrate with an empty description and keep working.
7. PTY and unit tests cover the new column, create/edit, and Projects action.
8. `make check` and hosted CI pass.

## Non-goals

- No change to project/agent ownership rules.
- No rich text or multi-line descriptions; the field is single-line text.
- No lab description in prompts unless a later feature adds it.

## Validation

Local gate (2026-11-13): `make check` passed (fmt, clippy, tests, audit, deny).
Hosted CI run `43402547220` passed on Ubuntu, macOS, and audit/deny.

## Task checklist

- [x] **F00-01** Write ADR-0041 for the optional laboratory description,
  schema v7, and dashboard actions; update the registry.
- [x] **F01-01** Add a bounded `LabDescription` domain value and optional field
  on `Lab`; keep `Lab::new` compatible.
- [x] **F01-02** Add migration `0007_lab_description.sql`, raise the supported
  schema version, and update schema tests.
- [x] **F02-01** Persist and read the description in the database adapter;
  extend `LabOverview` and `Directory::update_lab`.
- [x] **F03-01** Add the Description column to the Labs dashboard with dynamic
  widths and invalid-lab marking.
- [x] **F04-01** Replace the new-lab input with a name/description form and add
  a prefilled edit form for the selected laboratory.
- [x] **F04-02** Persist create/update through `app::labs`, with validation,
  audit events, and status feedback.
- [x] **F05-01** Add the `Projects` action: activate the selected laboratory,
  clear the prior current project, and open the Projects dashboard.
- [x] **F06-01** Add domain/database tests for trimming, bounds, empty
  description, and update.
- [x] **F06-02** Add TestBackend and PTY tests for the column, create/edit, and
  Projects navigation.
- [x] **F07-01** Update specification, data model, retention, README, playbook,
  CLI reference, and dashboard docs.
- [x] **F07-02** Run `make check`, hosted CI, and set the status to `Done`.
