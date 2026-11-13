# Feature 00001 — Interactive dashboards and readable terminal output

| Field | Value |
|---|---|
| Identifier | `00001` |
| Branch | `feat/interactive-dashboards` |
| Status | `In progress` |
| Date | 2026-11-13 |
| Related ADRs | ADR-0005 (to supersede), ADR-0009 (to amend), ADR-0021 (to amend), ADR-0038 (to write) |
| Scope | Presentation, command surface, terminal interaction; no domain or schema change |

## Goal

Replace the flag-only, plain-text output with a coherent Unix-style terminal
experience:

- `vlab labs`, `vlab projects`, `vlab agents`, and `vlab settings` open
  full-screen dashboards for each group.
- Dashboards are keyboard-driven, paginated, colored, and readable at a glance.
- A persistent bottom bar shows the available actions and key hints.
- Destructive actions use explicit modals and preserve the current safeguards.
- Non-interactive use remains plain, stable, and scriptable.

The visual target is a modern text UI in the spirit of `lazygit`, `htop`, and
`ranger`: calm, dense, legible, and clearly organized, not a decorated CLI.

## Non-goals

- No mouse-first interaction; mouse support is optional and deferred.
- No graphical interface or web UI.
- No change to the domain model, SQLite schema, provider contract, or
  authorization rules.
- No removal of the existing non-interactive safety behavior.
- No real-time collaboration, background refresh, or agent execution loop.

## Desired experience

### Entry points

```text
vlab                 # home dashboard with group tabs and recent context
vlab labs            # laboratories dashboard
vlab projects        # projects dashboard for the active laboratory
vlab agents          # agents dashboard for the active laboratory
vlab settings        # settings dashboard
vlab labs --plain    # plain table for scripts and pipes (no cursor control)
```

The flags `--show-settings`, `--list-labs`, `--list-projects`, `--list-agents`,
and the related action flags are replaced by subcommands. A temporary
compatibility shim may remain during the migration, but the final state has one
documented command family.

### Common screen anatomy

```text
┌ Labs ─────────────────────────────────────────────── Active: Demo Lab ┐
│ ID        Name             Agents  Projects  Valid  Active             │
│ 9135a362  Demo Lab              5         1  ok     *                  │
│ 7c2d1e4f  Payments Platform     3         4  warn                       │
│                                                                        │
│                        Page 1/2 · 8 labs · 4 per page                  │
├────────────────────────────────────────────────────────────────────────┤
│ n new   c check   a set active   d delete   / filter   ? help   q quit │
└────────────────────────────────────────────────────────────────────────┘
```

- Header: group name, context (active laboratory/project), global status.
- Body: selection-aware table with a highlighted row, pagination, and clear
  empty states.
- Footer: context actions, key hints, transient status/toast area.
- Modal layer: forms, confirmations, validation errors, help overlay.
- Minimum tested size: 80×24; usable fallback at 60×20; below that, a clear
  “terminal too small” screen with the required minimum.

### Key conventions

| Key | Action |
|---|---|
| `j` / `↓`, `k` / `↑` | Move selection |
| `h` / `←`, `l` / `→` | Previous/next page |
| `g`, `G` | First/last page |
| `Enter` | Open/confirm the selected item |
| `n` | New item in the current group |
| `c` | Check (labs only) |
| `a` | Set active / assign (context-dependent) |
| `d` | Delete with a bound confirmation modal |
| `/` | Filter the current list |
| `?` | Help overlay |
| `q` / `Esc` | Go back or quit without saving |

Actions that mutate data must show the exact target and scope in the modal.
`Esc` closes a modal and never confirms a destructive action.

### Group actions

| Dashboard | Bottom-bar actions |
|---|---|
| Labs | new, check, set active, delete |
| Projects | new (with optional GitHub source), open, assign agent, Kanban, delete |
| Agents | new (guided profile), detail, assign to current project, delete |
| Settings | edit in external editor, reload, show path, show credential set/unset, quit |

The Projects and Agents dashboards require an active laboratory; they show an
actionable empty state when none is selected. Project actions require a current
project and offer an “open project” action when none is selected.

### Color and typography

- Semantic palette: accent, success, warning, danger, muted, selection,
  border, and focus.
- 256-color when supported, 16-color fallback, monochrome when `NO_COLOR` is
  set, `TERM=dumb`, or output is redirected.
- Never use color alone to convey state: pair it with text, symbols, or a
  label.
- Unicode box drawing when the locale and terminal support it; ASCII fallback
  otherwise.
- No color or cursor sequences in `--plain` output, errors, or logs.

## Technical direction

- Add a presentation layer under `src/tui/` (or `src/cli/tui/`) that consumes
  existing application use cases. Domain and application layers remain free of
  terminal I/O.
- Candidate stack: `ratatui` for layout/widgets and `crossterm` for terminal
  capability, raw mode, and events. Confirm license, maintenance, MSRV, and
  supply-chain impact in an ADR before adding them.
- Keep a built-in plain renderer for non-TTY output and tests. The plain path
  reuses the existing width-aware table logic.
- Introduce a typed screen router and a per-screen state machine:
  `Home`, `Labs`, `Projects`, `Agents`, `Settings`, `Modal`, `Help`.
- Restore the terminal on normal exit, error, panic, and signal using an RAII
  guard and panic hook. Alternate screen and cursor visibility are never left
  dirty.
- Never open a provider connection from a dashboard until an explicit action
  requests it. Listing screens are read-only.

## Acceptance criteria

1. `vlab labs` opens a paginated laboratories dashboard with the actions
   `new`, `check`, `set active`, and `delete` in the bottom bar.
2. Equivalent dashboards and actions exist for `projects`, `agents`, and
   `settings`.
3. `vlab` with no arguments opens a home dashboard; `vlab <group>` opens that
   group directly.
4. Keyboard navigation, pagination, filtering, help, and modal confirmations
   work without a mouse.
5. The interface uses semantic color and box drawing, with `NO_COLOR`,
   monochrome, ASCII, and narrow-terminal fallbacks.
6. Redirected output and `--plain` contain no cursor-control or ANSI color
   sequences and remain snapshot-testable.
7. A destructive action never executes without the same bound confirmation
   required by the current CLI; `Esc` cancels.
8. No credential value is ever displayed in a dashboard, modal, header, or
   output.
9. The terminal is restored after success, error, panic, and SIGINT.
10. All existing domain behavior, exit codes, and security tests remain valid.

## Risks and open questions

- **Dependency weight.** `ratatui` + `crossterm` add a UI stack. If the ADR
  rejects them, the fallback is a smaller ANSI/crossterm renderer with manual
  layout; the UX must not regress.
- **Terminal diversity.** macOS Terminal, iTerm2, VS Code, tmux, and CI differ
  in color depth, Unicode width, and resize behavior. Test at least tmux and
  non-TTY CI.
- **Old-command removal.** Existing scripts may depend on flags. Decide the
  deprecation window and document it before removal.
- **Scope of “settings”.** The dashboard must not become a secret editor.
  Editing stays in `$VISUAL`/`$EDITOR`; the dashboard only shows `set`/`unset`.
- **Performance.** Lists are small today; the plan still bounds rendering at
  10,000 rows and tests pagination without loading everything into one screen.
- **Accessibility.** Color is never the only signal; verify with monochrome and
  a screen reader where practical.

## Task checklist

Check a task only when its deliverable and verification exist. The feature is
`Done` only when every box below is checked.

### Phase 0 — Decisions and specifications

- [x] **F00-01** Write ADR-0038 “Interactive colored terminal dashboards” that
  supersedes ADR-0005, amends ADR-0009 for subcommands, and amends ADR-0021 for
  the dashboard layout. Record the color policy, TTY detection, dependency
  choice, and terminal-restore rule.
- [x] **F00-02** Evaluate `ratatui` + `crossterm` against a minimal
  crossterm-only renderer. Record license, MSRV, transitive dependency, binary
  size, and testability evidence in `.docs/DEPENDENCIES.md` and
  `.docs/STACK.md`.
- [x] **F00-03** Approve the screen anatomy, keymap, bottom-bar actions, and
  color tokens in a short UX specification linked from the feature plan.
- [x] **F00-04** Decide the deprecation window for the existing flags and
  document the compatibility shim and removal task.

### Phase 1 — Command surface

- [x] **F01-01** Add clap subcommands `labs`, `projects`, `agents`, and
  `settings`, plus the bare-`vlab` home entry point. Keep one primary action
  per invocation where non-interactive.
- [x] **F01-02** Add a temporary compatibility shim mapping the old flags to
  the new subcommands, with a deprecation warning on stderr and identical
  behavior.
- [x] **F01-03** Route dashboard actions to existing application use cases
  without duplicating validation or authorization.
- [ ] **F01-04** Remove the old flags, update `tests/snapshots/help.txt`, and
  add a migration note to the CLI reference. All old-flag tests are replaced
  with subcommand tests.
- [x] **F01-05** Add `--plain` and `--no-color` behavior to every group and
  verify redirected output is free of escape sequences.

### Phase 2 — Terminal foundation

- [x] **F02-01** Implement terminal capability detection: TTY, `TERM`,
  `NO_COLOR`, `CLICOLOR`, `COLUMNS`, `LINES`, color depth, Unicode support.
- [x] **F02-02** Implement the RAII terminal guard: alternate screen, raw mode,
  cursor visibility, restore on success/error/panic/SIGINT.
- [x] **F02-03** Implement the render loop and key-event dispatch with a typed
  screen state machine.
- [x] **F02-04** Implement reusable layout components: header, context line,
  table with selection, pagination footer, action bar, toast, modal, help
  overlay.
- [x] **F02-05** Implement the semantic theme and ASCII/monochrome fallbacks.
- [x] **F02-06** Implement the “terminal too small” screen and resize handling.

### Phase 3 — Laboratories dashboard

- [x] **F03-01** Render the paginated laboratory list with ID, name, agents,
  projects, validation, and active marker.
- [x] **F03-02** Implement `new` with name validation and a result toast.
- [x] **F03-03** Implement `check` with the full rule report in a scrollable
  modal and the documented exit semantics for plain mode.
- [x] **F03-04** Implement `set active` with immediate active-marker refresh.
- [x] **F03-05** Implement `delete` with a bound confirmation modal, cascade
  warning, and active-selection refresh.
- [ ] **F03-06** Add PTY tests for navigation, pagination, new, check, active,
  delete, cancel, and empty state.

### Phase 4 — Projects dashboard

- [x] **F04-01** Render projects for the active laboratory with current-project
  marker, request/story counts, and pagination.
- [x] **F04-02** Implement `new`, including the optional GitHub source field
  and clone error presentation.
- [x] **F04-03** Implement `open` and refresh the current-project marker.
- [x] **F04-04** Implement `assign agent` with a selectable active-member list
  and duplicate-assignment feedback.
- [x] **F04-05** Implement `Kanban` as a scrollable read-only modal/screen with
  the five states, assignee, priority, due date, and pending intake.
- [x] **F04-06** Implement `delete` with bound confirmation and current-project
  clearing.
- [ ] **F04-07** Add PTY tests for every project action, including no active
  laboratory and no current project.

### Phase 5 — Agents dashboard

- [x] **F05-01** Render active agents with role, skills, project/open-story
  counts, and archived-status handling.
- [x] **F05-02** Implement detail view with profile, memberships, project
  assignments, story counts, and memory count.
- [x] **F05-03** Integrate the guided new-agent wizard into the dashboard
  without losing TTY validation or the one-Tech-Lead rule.
- [x] **F05-04** Implement `assign to current project` with membership and
  archived checks.
- [x] **F05-05** Implement delete with the existing unfinished-story guard and
  confirmation.
- [ ] **F05-06** Add PTY tests for list, detail, new, assign, delete, cancel,
  and non-interactive rejection.

### Phase 6 — Settings dashboard

- [x] **F06-01** Render settings path, provider, model, source control, and
  credential `set`/`unset` indicators; never render values.
- [x] **F06-02** Implement `edit` by launching the existing external editor
  and reloading on success.
- [x] **F06-03** Implement `reload`, `show path`, and validation error display.
- [x] **F06-04** Add tests for missing/invalid settings, credential redaction,
  editor cancellation, and terminal restoration.

### Phase 7 — Cross-cutting behavior

- [x] **F07-01** Implement filtering with `/`, clear-filter, and a visible
  filter status.
- [x] **F07-02** Implement help overlay generated from the active keymap.
- [x] **F07-03** Implement consistent pagination for labs, projects, agents,
  and large Kanban boards.
- [x] **F07-04** Ensure all stored text is sanitized before rendering and that
  model/memory content cannot inject terminal sequences.
- [x] **F07-05** Verify no provider call occurs from list/detail screens and no
  database write occurs from navigation alone.
- [x] **F07-06** Add a plain-mode parity test proving every dashboard list and
  action result has a stable non-interactive representation where documented.

### Phase 8 — Testing and validation

- [x] **F08-01** Add unit tests for theme tokens, color degradation, layout
  math, pagination, and keymap dispatch.
- [x] **F08-02** Add render snapshot tests at 80×24, 120×40, 60×20, ASCII, and
  monochrome.
- [ ] **F08-03** Add PTY integration tests for every dashboard action and
  modal, including `Esc`, resize, and terminal restore.
- [x] **F08-04** Add non-TTY tests for `--plain`, `--no-color`, `NO_COLOR`,
  `TERM=dumb`, and redirected stdout/stderr.
- [x] **F08-05** Add security tests for escape-sequence injection, secret
  absence, and confirmation bypass attempts.
- [x] **F08-06** Add a performance test with 10,000 rows and verify bounded
  render time and memory.
- [ ] **F08-07** Run the full `make check` gate and the hosted CI matrix.

### Phase 9 — Documentation and closure

- [x] **F09-01** Update `TECHNICAL-SPECIFICATIONS.md` with the subcommand
  surface, dashboards, non-TTY behavior, and color policy.
- [x] **F09-02** Update `cli-reference.md`, `README.md`, `PLAYBOOK.md`,
  `configuration.md`, and `STACK.md` with screenshots/diagrams and keymaps.
- [x] **F09-03** Update ADR statuses and the registry; mark ADR-0005
  superseded and ADR-0009/0021 amended.
- [x] **F09-04** Update `AGENTS.md` only if the command surface or agent
  workflow changes beyond this plan.
- [ ] **F09-05** Verify every task above is checked, set the status to `Done`,
  and record the final `make check` and CI evidence in this file.

## Implementation notes (2026-11-13)

**Compatibility decision:** the legacy flags remain available for this
feature. They are not removed here because existing scripts and validation
evidence still use them; removal is a follow-up feature with its own
deprecation warning and migration note. The new subcommands are the documented
interactive path.

Core dashboards, subcommands, theme, plain fallback, actions, and render/key
tests are implemented. The feature remains `In progress` because the following
checklist items are still open:

- removal of the legacy flags after the deprecation window (F01-04);
- PTY-driven end-to-end tests (F03-06, F04-07, F05-06, F08-03);
- final gate and closure evidence (F08-07, F09-05).

## Completion rule

This feature is complete only when every checkbox in the task checklist is
checked, the status is `Done`, `make check` and hosted CI pass, the related
ADRs are accepted, and the documentation matches the implemented interface.
