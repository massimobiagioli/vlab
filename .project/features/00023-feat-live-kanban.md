# Feature 00023 — Live Kanban and initial-based view keys

| Field | Value |
|---|---|
| Identifier | `00023` |
| Branch | `feat/live-kanban` |
| Status | `In progress` |
| Date | 2026-11-17 |
| Related ADRs | ADR-0065 (refresh and keys), ADR-0038 (dashboards), ADR-0062 (floor link) |
| Scope | In-place Kanban refresh and consistent `k`/`f` shortcuts |

## Goal

Keep the Kanban current while background planning and workflow run, without
leaving the screen, and make the Kanban shortcut `k` everywhere it is an entry
point.

## Acceptance criteria

1. The Kanban refreshes in place about once per second while visible; selection,
   column, and scroll are preserved.
2. Worker completion events still trigger an immediate refresh.
3. Refreshing makes no provider call, write, or navigation change.
4. `k` opens the Kanban from the Projects dashboard and from the floor.
5. `f` opens the floor from the Kanban; `Esc` preserves the return path.
6. The old `v` Kanban binding and action label are removed; help and action bars
   show `k`/`f` consistently.
7. Projects and floor keep arrow navigation; `k` is not an up key there.
8. Tests cover the live refresh, both links, the removed `v` binding, and the
   help text; `make check` passes and hosted CI is green.

## Non-goals

- A push/websocket feed or a background daemon.
- Refreshing while a form or prompt is open.
- Changing story-state semantics or workflow cadence.
- Adding a second Kanban shortcut.

## Risks

- Periodic reads can compete with the worker's SQLite writes; the existing busy
  timeout and short read-only queries bound this.
- A refresh can race a user's selection; `KanbanView::replace` preserves the
  selected column/cards and scroll offsets.
- Removing `v` is a keyboard contract change; documentation and tests are
  updated in the same feature.

## Validation

- `make check` passed: fmt-check, clippy, tests, audit, deny.
- `the_kanban_refreshes_without_leaving_the_screen` inserts a story and calls
  the live refresh without changing screen.
- `the_kanban_and_floor_link_both_ways` covers `k`/`f` and `Esc`.
- Hosted CI has not run; the plan stays `In progress` until it is green.

## Task checklist

- [x] **F00-01** Write ADR-0065 and update the registry/AGENTS links.
- [x] **F01-01** Add the periodic read-only refresh for Kanban/floor/metrics.
- [x] **F02-01** Switch Projects and floor to `k` for Kanban; keep `f` for the
  floor from Kanban.
- [x] **F03-01** Update action bars, help, tests, and documentation.
- [x] **F04-01** Run `make check`.
- [ ] **F04-02** Push, confirm hosted CI is green, and set the status to `Done`.
