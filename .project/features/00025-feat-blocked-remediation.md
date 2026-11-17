# Feature 00025 — Autonomous remediation of blocked stories

| Field | Value |
|---|---|
| Identifier | `00025` |
| Branch | `feat/blocked-remediation` |
| Status | `Done` |
| Date | 2026-11-17 |
| Related ADRs | ADR-0067 (remediation), ADR-0050 (workflow), ADR-0026 (provider) |
| Scope | Tech Lead context, reassignment, retry, and bounded blocked-story recovery |

## Goal

When a story becomes `BLOCKED`, the Tech Lead must take it over, make a
technical decision, add context, reassign if useful, and put it back in
`IN_PROGRESS` automatically. The workflow must continue until the card is
`DONE` or the bounded remediation budget is exhausted.

## Acceptance criteria

1. A `BLOCKED` story triggers a Tech Lead remediation call with the request,
   story, criteria, previous outcome, previous context, and active roster.
2. The decision is strict JSON with a bounded decision/context and an active
   assignee; invalid output falls back safely.
3. Remediation persists `tech_lead_context`, may reassign the story, resets the
   execution attempts, increments `remediation_attempts`, clears the stale
   outcome, and sets `IN_PROGRESS`.
4. The next execution prompt includes the Tech Lead context as delimited,
   untrusted data.
5. The workflow remediates and retries in the same pass, up to three
   remediation attempts per story and the workflow deadline.
6. A successful path ends in `DONE`; an impossible story remains `BLOCKED` and
   is never falsely completed.
7. Schema v16 adds the two story columns; story detail shows the Tech Lead
   context; a content-free `story_remediated` event is recorded.
8. Tests cover exhausted attempts, rejected review, reassignment, context,
   prompt validation, migration, and normal completion; `make check` passes and
   hosted CI is green.

## Non-goals

- Unbounded retries or autonomous permission changes.
- Faking `DONE` without a reviewed commit.
- A manual remediation command.
- Changing execution tool budgets or the review contract.

## Risks

- Remediation adds provider calls and execution time; three bounded cycles and
  the workflow deadline cap it.
- A reassigned agent may lack the right skills; the Tech Lead must choose from
  the active roster and the existing assignment rules still apply.
- `tech_lead_context` is model output and must remain delimited untrusted data.
- An impossible story can still reach the remediation limit; it remains
  visible as `BLOCKED` with an auditable reason.

## Validation

- `make check` passed: fmt-check, clippy, tests, audit, deny.
- `tests/workflow.rs` covers exhausted attempts and rejected-review
  remediation to `DONE`, reassignment, context, and normal work.
- `src/app/workflow.rs` tests cover the remediation prompt and strict decision
  parsing.
- Hosted CI is green on commit `15007ed`.

## Task checklist

- [x] **F00-01** Write ADR-0067 and update the registry/AGENTS links.
- [x] **F01-01** Add schema v16, story remediation columns, and migration.
- [x] **F02-01** Add blocked-story queries and remediation persistence.
- [x] **F03-01** Add the Tech Lead remediation prompt, validation, fallback, and
  in-pass retry loop.
- [x] **F04-01** Show Tech Lead context in story detail and record the event.
- [x] **F05-01** Update tests and documentation.
- [x] **F06-01** Run `make check`.
- [x] **F06-02** Push, confirm hosted CI is green, and set the status to `Done`.
