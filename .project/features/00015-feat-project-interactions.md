# Feature 00015 — Project interaction history

| Field | Value |
|---|---|
| Identifier | `00015` |
| Branch | `feat/project-interactions` |
| Status | `Done` |
| Date | 2026-11-16 |
| Related ADRs | ADR-0056 (interaction history), ADR-0050 (workflow), ADR-0028 (content-free metadata) |
| Scope | Read-only project page with planning, work, and review interactions and recaps |

## Goal

Give the operator a separate project-level page that shows every real agent
collaboration in the project — planning, work, and review — with participants,
context, outcome, time, and a recap. Each entry is derived from persisted
project data; no free-form chat is invented.

## Acceptance criteria

1. Schema v13 adds `requests.planned_by_agent_id`; planning records the actual
   Tech Lead and existing requests migrate with `NULL`.
2. A project interaction read model produces `planning`, `work`, and `review`
   entries from requests, stories, story outcomes, and agents.
3. Every entry has participants, a reference (request or story), an outcome
   (`success`, `failure`, `pending`), a timestamp when available, and a recap
   capped at 4,000 characters.
4. Planning recaps include the request and resulting stories/assignees, the
   clarification questions, or the failure code.
5. Work recaps use the stored work outcome or a deterministic state summary.
6. Review recaps use the stored review outcome and accepted/rejected decision or
   a deterministic pending/not-reviewed summary.
7. The Projects dashboard opens the page with `i` for the selected project;
   the Kanban board opens it with `i` for the current project.
8. The page lists entries newest first with time, kind, participants, reference,
   outcome, and a truncated recap; `Enter` opens a scrollable full recap.
9. The page is read-only, performs no provider call, writes nothing, and
   returns to the originating screen with `Esc`.
10. Tests cover the schema, read model, ordering, participants, recaps,
    dashboard rendering, detail scrolling, and entry points; `make check`
    passes.

## Non-goals

- Free-form agent-to-agent chat or a new orchestration loop.
- Storing duplicate recap text in a new table.
- Showing provider prompts, responses, file content, or command output.
- Cross-project or laboratory-wide interaction history.
- Editing or deleting interaction entries.

## Risks

- Historical requests have no planner attribution; their planning entries list
  assignees but no lead.
- Work and review recaps are model-generated project data and must be rendered
  sanitized.
- A large project history needs scrolling and a page-size clamp.
- Planning entries can be long; the 4,000-character cap keeps the detail modal
  bounded.

## Validation

- Local gate (2026-11-16): `make check` passed (fmt-check, clippy, tests,
  audit, deny).
- `tests/db_projects.rs` verifies schema v13 migration preservation and the
  planner column. `src/app/interactions.rs` unit tests cover planning, work,
  and review entries, participants, recaps, outcomes, ordering, and caps.
- `src/cli/tui/interactions.rs` tests cover the table, navigation, empty state,
  and full-recap detail.
- The dashboard test opens Interactions from Projects, opens a detail, and
  returns to the originating screen.
- Hosted CI is green on commit `6fad608`.

## Task checklist

- [x] **F00-01** Write ADR-0056; update the registry and `AGENTS.md`.
- [x] **F01-01** Add schema v13 and record the planner when planning starts.
- [x] **F02-01** Add the project interaction read model and SQLite query.
- [x] **F03-01** Add the Interactions screen, navigation, detail modal, help,
  and entry points.
- [x] **F04-01** Update the specification, CLI reference, README, playbook,
  data model, retention, and security docs.
- [x] **F05-01** Add database, read-model, migration, and dashboard tests.
- [x] **F06-01** Run `make check` and fix every failure.
- [x] **F06-02** Push the branch, confirm hosted CI is green, and set the
  status to `Done`.
