# Feature 00008 — Simulated automatic story workflow

| Field | Value |
|---|---|
| Identifier | `00008` |
| Branch | `feat/simulated-workflow` |
| Status | `In progress` |
| Date | 2026-11-13 |
| Related ADRs | ADR-0048 (workflow simulation), ADR-0047 (assigned starts), ADR-0003 (no tools), ADR-0026 (provider contract), ADR-0043 (Kanban detail) |
| Scope | Automatic work/review simulation and DONE outcomes |

## Goal

After the human supplies a request, assigned stories advance automatically:
the assignee produces a textual work outcome, the QA (or Tech Lead) produces a
review outcome, and the story becomes `DONE`. The `DONE` card detail shows both
outcomes and the reviewer. No code is executed and the model has no tools.

## Acceptance criteria

1. After planning, every `IN_PROGRESS` assigned story without an outcome runs
   one work call and one review call.
2. The reviewer is the first active QA in the project roster, or in the
   laboratory roster when the project roster is empty; if there is no QA, the
   Tech Lead reviews.
3. Work and review outputs are strict JSON with one bounded text field, parsed
   locally; unknown fields, wrong types, control characters, or oversized text
   are rejected without repair.
4. A successful story is persisted as `DONE` with one outcome row (work text,
   review text, reviewer, completion time) in the same transaction.
5. `StoryDetail` for a `DONE` story shows the work outcome, review outcome,
   reviewer, and completion time; a non-DONE story shows no outcome.
6. Provider or validation failure leaves the story in progress, records a
   content-free workflow failure, and does not fail the planning request.
7. Calls are bounded: two per story, one attempt per call, 60-second timeout,
   512 output tokens, 240-second total workflow deadline, sequential stories.
8. No tools, shell, filesystem, network, or code execution is added. Outcomes
   are sanitized, never executed, and never logged as content.
9. Schema v9 adds `story_outcomes` and the `workflow` interaction kind; events
   are content-free.
10. Tests cover success, reviewer fallback, failures, bounds, persistence, and
    detail rendering; `make check` and hosted CI pass.

## Non-goals

- Real code execution, shell, filesystem, network, or repository mutation.
- A rejection/rework loop; the review is advisory and bounded.
- Manual story transitions from the board.
- Changing assignment rules or the initial state policy from ADR-0047.

## Risks

- `DONE` can be mistaken for real delivery. The UI, help, and documentation must
  call it a no-tool simulation.
- Two calls per story add cost and latency; the total deadline and 512-token cap
  bound it, but many stories can still be expensive.
- Provider failures leave stories in progress; a later planning run retries
  them. There is no separate retry command.
- The review is advisory; a low-quality work outcome can still be marked DONE.

## Validation

- Local gate (2026-11-13): `make check` passed (fmt, clippy, tests, audit,
  deny).
- Scripted-provider unit tests for work/review success, QA and lead reviewer
  selection, invalid JSON, provider failure, and pending-story selection.
- Database tests for the outcome row, state transition, and schema v9.
- Kanban detail test for a DONE card with outcomes.
- Hosted CI is required before the feature is marked `Done`.

## Task checklist

- [x] **F00-01** Write ADR-0048, update the registry and `AGENTS.md`, and align
  the specification, architecture, security, and observability documents.
- [x] **F01-01** Add the strict work/review schemas, prompts, and validators
  with tests.
- [x] **F01-02** Add schema v9 (`story_outcomes`, `workflow` interaction kind)
  and the repository operations with migration tests.
- [x] **F01-03** Implement the bounded workflow use case, reviewer selection,
  failure handling, and tests.
- [x] **F02-01** Trigger the workflow after planning from the flag and dashboard
  paths; update the outcome messages.
- [x] **F02-02** Add the `DONE` outcome to the Kanban detail and render it at
  supported sizes.
- [x] **F03-01** Update the planning, CLI reference, README, playbook, data
  model, retention, and feature docs.
- [x] **F04-01** Run `make check` and fix every failure.
- [ ] **F04-02** Push, confirm hosted CI is green, and set the status to `Done`.
