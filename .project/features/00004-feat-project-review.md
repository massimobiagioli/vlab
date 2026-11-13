# Feature 00004 — Project review

| Field | Value |
|---|---|
| Identifier | `00004` |
| Branch | `feat/project-review` |
| Status | `In review` |
| Date | 2026-11-13 |
| Related ADRs | ADR-0043 (project edit, Kanban screen), ADR-0042 (form modal), ADR-0038 (dashboards), ADR-0032 (checkouts) |
| Scope | Projects dashboard editing and Kanban presentation |

## Goal

- Let the user rename a project from the Projects dashboard.
- Replace the Kanban dialog with a dedicated, full-screen, Trello-style board
  that matches the current dashboard look: lists side by side, stories as
  cards, keyboard navigation, and a card detail view.

## Acceptance criteria

1. `e` in the Projects dashboard opens the form with the selected project's
   Name; `Enter` saves, `Esc` cancels; the table shows the new name.
2. Rename rejects empty names and projects outside the active laboratory;
   validation errors do not echo field values.
3. A successful rename records `project_updated` with the project identifier only.
4. `v` opens a full-screen Kanban screen (not a modal) with header, five lists,
   and a persistent Actions bar.
5. Cards show only the story's short description (title), wrapped to at most
   three lines with `…` when cut; card height follows the text. Empty lists
   show guidance.
6. `←/→`/`h/l` change list, `↑/↓`/`j/k` change card, `PgUp/PgDn` page,
   `Home/End`/`g/G` first/last, `Enter` opens the detail dialog, `r` reloads,
   `Esc` returns to Projects, `q` quits.
7. Each list scrolls on its own, keeps its position when focus moves, and shows
   `↑N`, `↓N more`, and `position/total`; resizing re-fits the offsets.
8. The detail dialog shows every story field with aligned labels, scrolls
   long content, and closes with `Esc`/`Enter`.
9. Narrow terminals show a window of lists around the focused one; 80×24 and
   60×20 render without panicking.
10. Stored text is sanitized; the board never writes to the database.
11. Unit, TestBackend, and PTY tests cover the above; `make check` passes.

## Non-goals

- No editing of the GitHub source (ADR-0043 §2).
- No story state transitions or card moves.
- No mouse support.

## Risks

- Narrow terminals truncate titles heavily; mitigated by the list window and
  card detail.

## Validation

Local gate (2026-11-13): `make check` passed (fmt, clippy, tests, audit, deny).
Hosted CI: pending; the branch has not been pushed.

## Task checklist

- [x] **F00-01** Write ADR-0043 and update the registry and `AGENTS.md`.
- [x] **F01-01** Add `projects::rename` with the `ProjectStore` port, SQLite
  implementation, and unit/integration tests.
- [x] **F01-02** Add the `project_updated` audit event.
- [x] **F02-01** Add `e` Edit to the Projects dashboard using the form component.
- [x] **F03-01** Add `kanban::story` detail use case and SQLite query with tests.
- [x] **F03-02** Add the full-screen Kanban screen: header, lists, cards,
  scrolling, list window, and Actions bar.
- [x] **F03-03** Add board navigation, card detail modal, reload, and Esc back.
- [x] **F04-01** Add TestBackend tests for the board at 100×30, 80×24, 60×20,
  and PTY tests for edit and the Kanban flow.
- [x] **F05-01** Update specification, CLI reference, playbook, README,
  observability, and run `make check`.
- [x] **F06-01** Product-owner review: cards show only the short description
  with variable height; the detail dialog shows every field and scrolls.
- [x] **F06-02** Per-list scrolling with height-aware offsets, `↑N`/`↓N more`
  indicators, paging, first/last keys, and resize re-fit; tests.
- [x] **F06-03** Extend the `vlab-tui` skill with board/scrolling rules and a
  visual-review procedure; apply it to this screen.
- [ ] **F05-02** Push the branch, confirm hosted CI is green, and set the
  status to `Done`.
