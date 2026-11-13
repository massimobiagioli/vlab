# Feature 00007 — Assigned stories start in progress

| Field | Value |
|---|---|
| Identifier | `00007` |
| Branch | `feat/assigned-progress` |
| Status | `In progress` |
| Date | 2026-11-13 |
| Related ADRs | ADR-0047 (assigned initial state), ADR-0023 (story state), ADR-0043 (Kanban), ADR-0034 (roster) |
| Scope | Story initial state after planning |

## Goal

When planning assigns a story to an active project member, the story appears in
`IN PROGRESS` on the Kanban board immediately. An unassigned story stays in
`TODO`. The board remains read-only; `IN PROGRESS` means "assigned and ready",
not "an agent process is running".

## Acceptance criteria

1. A planned story with an assignee is persisted as `IN_PROGRESS` in the same
   transaction as the rest of the plan.
2. A planned story without an assignee is persisted as `TODO`.
3. The project and Kanban counts reflect the stored state immediately.
4. The outcome dialog and documentation explain the two initial states and that
   the board is still read-only.
5. Schema v8 adds a one-time data migration for existing assigned `TODO`
   stories; there is no table or column change. No provider or dependency
   change is introduced.
6. Tests cover both initial states, the data migration, atomic persistence, and
   the board rendering.
7. `make check` passes and hosted CI is green.

## Non-goals

- No manual state transitions from the board.
- No execution engine or live agent activity.
- No new `READY` state or schema change.
- No change to unassigned-story assignment rules.

## Risks

- `IN PROGRESS` can be mistaken for live execution. The board hint, ADR, and
  documentation must state that it means assigned/ready.
- `TODO` may appear empty after planning. The request intake item remains
  visible separately.

## Validation

- Planning tests for assigned and unassigned stories.
- Kanban rendering test for an assigned story in the `IN PROGRESS` column.
- `make check` and hosted CI.

## Task checklist

- [ ] **F00-01** Write ADR-0047 and update the registry and `AGENTS.md`.
- [ ] **F01-01** Add the initial-state policy, bind it in the story insert, and
  add the schema v8 data migration for existing assigned stories.
- [ ] **F01-02** Update planning, Kanban, migration, and outcome tests for both
  states.
- [ ] **F02-01** Update the outcome dialog, specification, planning, CLI
  reference, README, and feature docs.
- [ ] **F03-01** Run `make check` and fix every failure.
- [ ] **F04-01** Push, confirm hosted CI is green, and set the status to `Done`.
