# Feature 00029 — Bounded rework and review escalation

| Field | Value |
|---|---|
| Identifier | `00029` |
| Branch | `feat/bounded-rework` |
| Status | `Done` |
| Date | 2026-11-17 |
| Related ADRs | ADR-0071 (rework policy), ADR-0050 (workflow), ADR-0069 (closing compromise), ADR-0070 (develop integration) |
| Scope | Preserve work on retry, structured findings, bounded rework, Tech Lead escalation |

## Goal

Stop the `IN_PROGRESS -> IN_REVIEW -> IN_PROGRESS` loop. A rejection keeps the
same assignee and the previous branch, the rework fixes only the reviewer
findings, and after a bounded number of failures the Tech Lead escalates or the
card closes with a technical-debt card.

## Acceptance criteria

1. A retry reuses the existing `vlab/<story-id>` worktree and branch; the
   previous commit and files are still present.
2. The reviewer returns strict JSON with `review_outcome`, `accepted`,
   `findings`, and an optional `retest` scope; a rejection always yields at
   least one actionable finding.
3. A work-limit or deadline stop is not sent to review; it records an incomplete
   outcome and increments the bounded rework counter.
4. Schema v20 adds `stories.rework_attempts`,
   `story_outcomes.review_findings_json`, and `story_outcomes.review_retest`.
5. First failure (`rework_attempts = 1`): same assignee, rework context with only
   the findings and retest scope, no Tech Lead call.
6. Second failure (`rework_attempts = 2`): Tech Lead escalation to a different
   active agent with explicit instructions; the previous assignee is rejected
   when another candidate exists.
7. Third failure (`rework_attempts >= 3`) or three remediation cycles: close the
   original `DONE` with a compromise and create one unassigned `TODO` debt card
   containing the remaining findings.
8. Invalid or unavailable review output is never accepted; it produces a
   content-free failure and a bounded remediation path.
9. Follow-up reviews see the latest fix commit and the previous findings, and
   verify only those items plus regressions.
10. Tests cover branch reuse, findings parsing, work-limit handling, direct
    rework, escalation, the hard close, and no false `DONE`; `make check` passes
    and hosted CI is green.

## Non-goals

- Unbounded rework or automatic conflict resolution.
- Letting the model choose the loop counters or transitions.
- Changing the provider contract beyond the review JSON fields.
- A new UI screen; the existing card detail and events show the outcome.

## Risks

- A reviewer outage can consume a bounded remediation cycle; it never marks the
  card `DONE`.
- The findings list is model output and remains untrusted, bounded data.
- A third failure closes the card with debt even if the work is partly usable;
  the debt card carries the exact remaining findings.
- Reusing a worktree preserves uncommitted files; the next agent sees them as
  its starting point.

## Validation

- `tests/workflow.rs` covers direct rework, escalation, the hard close,
  work-limit handling, and review findings.
- `src/app/execution.rs` tests cover the strict review parser and findings
  bounds.
- The git adapter test covers worktree/branch reuse.
- Migration tests cover schema v20.
- `make check` passed; hosted CI is green on `03b1f11`.

## Task checklist

- [x] **F00-01** Write ADR-0071, update the registry and AGENTS.
- [x] **F01-01** Add schema v20 counters and findings columns.
- [x] **F02-01** Preserve the story branch/worktree on retry.
- [x] **F03-01** Add structured review findings and retest scope.
- [x] **F04-01** Treat work-limit stops as incomplete, not review submissions.
- [x] **F05-01** Implement direct rework, Tech Lead escalation, and the hard
  close.
- [x] **F06-01** Include findings in prompts, debt cards, events, and docs.
- [x] **F07-01** Update and add tests.
- [x] **F08-01** Run `make check`.
- [x] **F08-02** Push, confirm hosted CI is green, and set the status to `Done`.
