# Feature 00021 — Three-room MSX laboratory floor

| Field | Value |
|---|---|
| Identifier | `00021` |
| Branch | `feat/msx-three-room-floor` |
| Status | `Done` |
| Date | 2026-11-17 |
| Related ADRs | ADR-0062 (floor), ADR-0045 (superseded), ADR-0038 (dashboards), ADR-0043 (Kanban) |
| Scope | Fixed Project Lab, War Room, and Lounge; task context; deterministic movement; bidirectional Kanban link |

## Goal

Turn the laboratory floor into a fixed, recognizable MSX laboratory with three
rooms, desks and decorations, agents that visibly move and interact, real task
context, and a two-way link with the active project's Kanban board.

## Acceptance criteria

1. The floor always shows three fixed rooms — Project Lab, War Room, Lounge —
   and a corridor at 60×20, 80×24, and 100×30; the T plan is used where the
   scene fits and a compact row is the narrow fallback. Rooms do not paginate.
2. The Project Lab shows the active project's task-state dots without mixing
   task text into the scene; with no active project the header gives the next
   action.
3. The Tech Lead is in the War Room, focused agents use focus desks, active
   project workers are in the Project Lab, and every other active agent is in
   the Lounge; each appears once and archived agents are counted.
4. Room interiors contain titles, furniture, icons, and each agent's face with
   its name below and a state bubble above. Task titles stay in the `Enter`
   detail; `t` opens the first task's read-only story detail.
5. Agents move deterministically between room spots and along the corridor,
   with `(zzz)` idle, exchange markers, and no wall-clock or random input.
6. `f` on the floor opens the active project's Kanban; `f` on the Kanban opens
   the floor; `Esc` returns to the screen that opened the current view.
7. The Kanban action bar and help expose the floor link; the floor action bar
   exposes the board link.
8. The scene is read-only, makes no provider call or database write, sanitizes
   stored text, and honors color, ASCII, narrow, and empty states.
9. Tests cover rooms, task data, movement, detail/task actions, both link
   directions, 60×20/80×24/100×30, ASCII, and empty states; `make check`
   passes and hosted CI is green.

## Non-goals

- A game engine, ECS, pixel canvas, or new TUI dependency.
- Cross-project rooms or a room per project.
- Persistent animation, inter-agent messages, or live execution tracing.
- Editing stories, tasks, or assignments from the floor.
- Rendering prompts, provider responses, CVs, or credentials.

## Risks

- Animation can become distracting or imply fake activity; destinations must
  follow stored state and the scene must state that it is read-only.
- Three rooms at 60×20 are tight; text must truncate predictably and every
  symbol needs a text fallback.
- The floor and Kanban now return to different screens; the TUI must preserve
  both return paths without resetting the other view.
- Task titles are user/model data; they must be sanitized and width-aware.
- A worker finish can reload the floor mid-animation; selection and geometry
  must remain stable.

## Validation

- Local gate (2026-11-17): `make check` passed (fmt-check, clippy, tests,
  audit, deny).
- `src/app/floor.rs` tests cover the active project, attached tasks, fixed room
  assignment, task detail, and empty states.
- `src/cli/tui/floor.rs` tests cover the T plan and row fallback, three rooms,
  furniture/decoration icons, task-state dots, agent names and bubbles,
  60×20/80×24/100×30, ASCII, deterministic slower movement through doors,
  exchanges, selection, empty states, and the selected task.
- `tests/db_tasks.rs` covers the project and agent open-task queries, ordering,
  state, and assignee.
- `src/cli/tui/mod.rs` tests cover the bidirectional Kanban link, preserved
  return paths, the agent detail `Working on` section, and `t` opening the
  story.
- `AGENTS.md`, `vlab-tech-lead`, and ADR-0063 record the mandatory post-feature
  skill update.
- Hosted CI is green on commit `efbd2f5` (agent names, bubbles, doors, and
  slower movement).

## Task checklist

- [x] **F00-01** Write ADR-0062; supersede ADR-0045; update the registry.
- [x] **F00-02** Add the `msx-computer-senior-software-architect` skill and
  register it in `AGENTS.md`.
- [x] **F00-03** Record ADR-0063 and update `vlab-tech-lead` and `AGENTS.md`
  with the post-feature skill-maintenance rule.
- [x] **F01-01** Add active-project task data to the floor read model.
- [x] **F02-01** Rebuild the floor as three fixed rooms with furniture and
  decorations.
- [x] **F03-01** Add deterministic agent tours, corridor movement, and
  interaction markers.
- [x] **F04-01** Add task status/detail/task actions and the bidirectional
  Kanban link with preserved return paths.
- [x] **F05-01** Update the specification, CLI reference, README, playbook,
  observability/security notes, and help.
- [x] **F06-01** Add read-model, database, renderer, movement, and TUI tests.
- [x] **F07-01** Run `make check` and fix every failure.
- [x] **F07-02** Push the branch, confirm hosted CI is green, and set the
  status to `Done`.
