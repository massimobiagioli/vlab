# Feature 00006 — Agent interaction floor

| Field | Value |
|---|---|
| Identifier | `00006` |
| Branch | `feat/agent-interaction` |
| Status | `Done` |
| Date | 2026-11-13 |
| Related ADRs | ADR-0045 (laboratory floor), ADR-0046 (manual memories), ADR-0038 (dashboards), ADR-0039 (home navigation), ADR-0034 (project roster), ADR-0028 (content-free interactions), ADR-0022/0035/0036 (memories) |
| Scope | Laboratory floor screen and laboratory/project memory management |

## Goal

Show the active laboratory as a friendly two-dimensional floor where agents
working on the same project sit together, the Tech Lead coordinates from a
central desk, an agent with no open work shows a `(zzz)` speech bubble, two
agents exchanging information sit close with conversation bubbles, and an
agent focused on several stories isolates at a desk with work bubbles. From
the same screen, manage laboratory and project memories.

The view is an overview of stored assignments, not a live simulation.

## Acceptance criteria

1. The home dashboard lists `Lab floor`; `Enter` or `5` opens it, `Esc`
   returns home, and `q` quits.
2. The floor uses the active laboratory and shows active agents only. Archived
   agents are hidden and counted in the header.
3. Each project in the laboratory has a pod built from its assigned agents. A
   project with no assignments uses the whole active laboratory, matching the
   planning roster. Agents assigned to no pod appear in a `Lounge` pod.
4. The first active `lead` is the Tech Lead, shown at a central coordination
   desk with cooperation connectors to every pod; the lead is not duplicated
   in the pods.
5. The scene is deterministic, MSX-inspired, and legible: double outer frame,
   single-line pods, `(◕‿◕)` sprites, project accent colors, and `★` lead
   marker. An agent with zero open stories shows `(zzz)`; two working agents
   of different roles in one project sit adjacent with `( ? )`, `( ! )`, and
   `⇅`; an agent with two or more open stories isolates at a `focus desk` with
   `(///)` or `(</>)`. Color is never the only signal. Unicode falls back to
   ASCII.
6. The status line names the selected agent and its role, project count, open
   stories, and skills. `Enter` opens the existing agent detail dialog.
7. `↑/↓` select agents, `←/→` page pods, `r` reloads, `m` opens the memory
   screen, and `?` shows help. Floor navigation never writes, calls the
   provider, or renders model content.
8. The memory screen has laboratory and project tabs. It lists memories newest
   first, shows the full content on `Enter`, creates with `n` (multiline
   editor), edits with `e`, and deletes with `d` behind the bound confirmation.
   The project tab requires a current project. Content is validated (1–2,000
   characters, no forbidden controls), the configured credential is rejected,
   and the per-scope cap of 20 is enforced on create. Edits preserve the
   identifier, scope, and creation time. No agent memories are exposed.
9. Layout works at 60×20, 80×24, and 100×30. Pods that do not fit paginate with
   a `page/total` indicator; members that do not fit show `+N more`.
10. Stored text is sanitized and width-aware. No credential, CV, or model text
    is rendered. The global terminal-too-small screen still applies.
11. Unit tests cover the read model and memory mutations; `TestBackend` tests
    cover the scene, idle/exchange/focus bubbles, pagination, ASCII fallback,
    selection, and the memory screens. `make check` passes.

## Non-goals

- No animation, timers, walking sprites, or live simulation.
- No inter-agent messages, new tables, migrations, or provider calls.
- No editing or assignment changes from the floor.
- No agent-memory management; only laboratory and project memories.
- No CLI memory flags; the memory screen is part of the floor.
- No mouse, web, image, or pixel canvas.
- No plain/non-TTY equivalent; the screens are interactive-only.

## Risks

- "Cooperation" is inferred from project membership and the Tech Lead role.
  The legend and documentation must call the view an overview, not a live
  trace.
- Many projects or members can crowd the terminal. Pagination and `+N more`
  keep it bounded, at the cost of hiding some content behind keys.
- Unicode width and font support vary. The ASCII fallback and char-width
  handling are tested.
- Adding a fifth home item changes the direct-key range and help text; existing
  dashboard tests must be updated.
- Manual memory content is user-authored and enters prompts; the credential
  and owner checks must stay in the application layer, not only in the UI.

## Validation

- Local gate (2026-11-13): `make check` passed (fmt, clippy, tests, audit,
  deny).
- Hosted CI run `43431288813` passed on Ubuntu, macOS, and audit/deny.
- `TestBackend` renders at 60×20, 80×24, and 100×30.
- Read-model tests for assignments, whole-lab fallback, lounge, lead, archived
  agents, activity states, exchange pairs, and open-story counts.
- Memory tests for create, edit, delete, ownership, the credential rule, the
  editor, the detail dialog, and the list at supported sizes.

## Task checklist

- [x] **F00-01** Write ADR-0045 and update the registry and `AGENTS.md`.
- [x] **F00-02** Write ADR-0046, amend ADR-0022/0035/0036, and update the
  registry and `AGENTS.md`.
- [x] **F01-01** Add the `app::floor` read model with assignment, whole-lab,
  lounge, lead, archived, activity, exchange, and count tests.
- [x] **F01-02** Add memory update, checked deletion, the repository update
  statement, and `memory_updated` events with tests.
- [x] **F02-01** Add the `Screen::Floor` entry, `FloorView` state, keymap, help,
  and home item.
- [x] **F02-02** Implement the deterministic scene renderer with pods,
  connectors, sprites, idle `(zzz)`, exchange bubbles, focus desks,
  pagination, and ASCII fallback; add `TestBackend` tests.
- [x] **F02-03** Add the `Screen::Memories` list, editor, detail, delete
  confirmation, floor action, and tests.
- [x] **F03-01** Update the CLI reference, technical specification, planning,
  retention, observability, README, playbook, and feature docs.
- [x] **F04-01** Run `make check` and fix every failure.
- [x] **F04-02** Push the branch, confirm hosted CI is green, and set the status
  to `Done`.
