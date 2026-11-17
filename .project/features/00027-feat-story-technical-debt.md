# Feature 00027 — Close exhausted stories with technical debt

| Field | Value |
|---|---|
| Identifier | `00027` |
| Branch | `feat/story-technical-debt` |
| Status | `Done` |
| Date | 2026-11-17 |
| Related ADRs | ADR-0069 (closing decision), ADR-0067 (superseded), ADR-0050 (workflow), ADR-0068 (memories) |
| Scope | No automatic `BLOCKED`, Tech Lead compromise closure, linked debt card |

## Goal

When a story exhausts its execution and remediation budgets, the Tech Lead
closes it with a recorded compromise and opens one linked technical-debt card.
The card never moves between `IN_PROGRESS` and `BLOCKED`, and the autonomous
flow reaches a conclusion without a human.

## Acceptance criteria

1. No automatic workflow branch sets `BLOCKED`; historical `BLOCKED` rows are
   remediated or closed like any other candidate.
2. A story gets three execution attempts per cycle and three remediation cycles.
3. After the last cycle the Tech Lead returns strict JSON with `decision` and
   `debt_context`; invalid or unavailable output uses a bounded fallback.
4. The original story closes as `DONE` with the last work/branch evidence and a
   review outcome recording the `Tech Lead compromise:` decision.
5. One unassigned `TODO` debt card is linked through `debt_of_story_id` with the
   decision, residual work, criteria, priority, complexity, and due date.
6. Schema v17 adds `debt_of_story_id` and its index without changing the state
   check or unrelated tables.
7. The Tech Lead receives a markdown closure recap memory and a content-free
   `story_debt_created` event is recorded.
8. A transient provider/environment failure stops the pass without consuming the
   remediation budget or closing the card.
9. Tests cover the closing path, the debt card, no `BLOCKED` rows, parser and
   fallback, memory, and transient failures; `make check` passes and hosted CI is
   green.

## Non-goals

- Executing debt cards automatically in the same pass.
- A manual remediation or closure command.
- Removing `BLOCKED` from historical data or the state check.
- Unbounded retries, permission changes, or automatic merges.

## Risks

- Closing with a compromise marks a card `DONE` without a normal accepted
  review; the review text and debt card keep the decision auditable.
- The extra closure call spends provider budget; timeout and output limits bound
  it and the fallback keeps progress deterministic.
- A debt card can accumulate if the same work keeps failing; it stays `TODO`
  and unassigned so it never chains automatically.

## Validation

- `tests/workflow.rs` covers the compromise closure, linked debt card, no
  `BLOCKED` rows, Tech Lead memory, and transient provider failure.
- `src/app/workflow.rs` unit tests cover the closure prompt and parser.
- Migration tests cover schema v17.
- `make check` passed; hosted CI is green on `390d04d`.

## Task checklist

- [x] **F00-01** Write ADR-0069, supersede ADR-0067, and update the registry
  and AGENTS links.
- [x] **F01-01** Add migration v17 for `stories.debt_of_story_id` and bump the
  supported schema version.
- [x] **F02-01** Remove every automatic `BLOCKED` transition and stop the pass
  on transient failures.
- [x] **F03-01** Add the closing decision prompt, strict parser, fallback, and
  closing outcome.
- [x] **F04-01** Create the linked unassigned debt card in the closure
  transaction and record its event.
- [x] **F05-01** Write the Tech Lead closure recap memory.
- [x] **F06-01** Update tests, documentation, retention notes, and the Tech Lead
  skill.
- [x] **F07-01** Run `make check`.
- [x] **F07-02** Push, confirm hosted CI is green, and set the status to `Done`.
