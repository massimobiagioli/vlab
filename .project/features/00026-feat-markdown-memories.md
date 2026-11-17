# Feature 00026 — Markdown recap memories

| Field | Value |
|---|---|
| Identifier | `00026` |
| Branch | `feat/markdown-memories` |
| Status | `Done` |
| Date | 2026-11-17 |
| Related ADRs | ADR-0068 (recaps), ADR-0022 (memories), ADR-0059 (file store) |
| Scope | Structured project planning recaps and agent card-completion memories |

## Goal

Automatically create readable markdown recaps for project planning and for each
card an agent completes, instead of raw `Planning summary:` snippets.

## Acceptance criteria

1. A successful plan writes a project memory file with a markdown title, date,
   project, request ID, request text, stories, and optional Tech Lead summary.
2. The project recap is written even when the optional model summary is empty.
3. An accepted review writes an agent memory file with card, project, assignee,
   branch, commit, work, review, and Tech Lead context.
4. Both recaps are bounded to 2,000 characters and use the existing memory file
   store; a full scope or write error never fails planning or workflow.
5. No schema, dependency, provider-contract, or retention change.
6. Existing raw automatic files remain readable; a local backfill can wrap them
   without touching user-authored memories.
7. Tests cover project recap content, agent completion memory, bounds, and the
   existing best-effort behavior; `make check` passes and hosted CI is green.

## Non-goals

- Rewriting all historical memory content automatically.
- A new memory table or a new file format.
- Model-generated recaps beyond the existing optional planning summary.
- Manual agent-memory editing.

## Risks

- Recaps contain request, work, and review text; they remain untrusted project
  data and are sanitized when displayed.
- The 2,000-character cap can truncate long recaps; the marker and title remain.
- A full memory scope silently skips the recap by design.

## Validation

- `tests/planning_flow.rs` checks the project recap markdown and summary.
- `tests/workflow.rs` checks the agent completion recap.
- `make check` passed; hosted CI is green on `390d04d`.

## Task checklist

- [x] **F00-01** Write ADR-0068 and update the registry/AGENTS links.
- [x] **F01-01** Build and persist the markdown project planning recap.
- [x] **F01-02** Build and persist the markdown agent card-completion recap.
- [x] **F02-01** Bound both recaps and keep them best effort.
- [x] **F03-01** Update tests, docs, and the Tech Lead skill.
- [x] **F04-01** Run `make check`.
- [x] **F04-02** Push, confirm hosted CI is green, and set the status to `Done`.
