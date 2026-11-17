# CLI Reference

VLab is a single-binary, text-only CLI. Results go to stdout; errors, warnings,
and prompts go to stderr. Every invocation selects exactly one primary action.
Run `vlab` or `vlab --help` for the grouped help.

## Group dashboards

| Command | Behavior |
|---|---|
| `vlab` | Home dashboard opens immediately in a terminal; a background worker runs due schedules, resumes pending story execution, and drains the request queue while the status line reports `Background:` progress. Prints help otherwise. |
| `-h`, `--help` | Print help. |
| `-V`, `--version` | Print the version. |

## Diagnostics

| Command | Behavior |
|---|---|
| `--logs` | Print the newest 100 content-free application audit events, newest first: time, kind, outcome, entity, and error code. The view never contains request, story, memory, prompt, or credential content. |

The Laboratories dashboard shows ID, name, description, agents, projects, and
the active marker. Its actions are New, Check, Edit, Set active, Projects,
Populate, Delete, Filter, Help, Back, and Quit. New/Edit use a bordered form:
`Tab` or arrows move fields, the focused field shows a cursor, `Enter` saves,
and `Esc` cancels. **Projects** activates the selected
laboratory and opens its Projects dashboard. **Populate** (`o`) opens the
two-pane membership editor for the selected laboratory: **Available agents** on
the left and **In this laboratory** on the right. `Tab` or `←/→` switches pane,
`↑/↓` moves, `Enter` or `Space` moves the selected agent to the other pane, `v`
opens the selected agent's full profile, `s` saves, and `Esc` asks before
discarding unsaved moves. A saved roster needs at
least one agent and exactly one Tech Lead; archived profiles and archived
memberships are preserved. The catalog comes from `make load-preset`.

The Projects dashboard actions are New, Edit, Request, Open, Assign, Kanban,
Interactions, Schedules, Delete, Filter, Help, Back, and Quit. **New** opens a form with a source choice
(`None`, `GitHub`, `Folder`) and one source field; the source cannot be changed
after creation. **Edit** renames the selected project. **Request** (`r`, or `n` on the Kanban board)
makes the project current and opens a prompt dialog: type or paste the request
(`Alt+Enter` adds a line break), `Enter` submits, `Esc` cancels. Submission
persists the request as `TODO` and returns immediately; it never waits for
planning. A background Tech Lead worker plans queued requests one at a time and
runs the bounded workflow. **Kanban** (`k`) opens the selected project's board
as a full-screen view: the header shows project, laboratory, Tech Lead,
optional GitHub source, story counts, and pending intake; the five states are
side-by-side lists of cards, each showing only the story's short description.
Queued requests appear as `REQUEST` cards at the top of `TODO`; `Enter` opens a
read-only request detail. The request card disappears when planning creates its
stories.
Every list scrolls on its own and shows `↑N`, `↓N more`, and `position/total`
in its border. Use `←/→` or `h/l` for lists, `↑/↓` or `j/k` for cards,
`PgUp/PgDn` to scroll a page, `Home/End` or `g/G` for the first/last card,
`Enter` for the detail dialog (every story field; `↑/↓` scroll, `Esc` close),
`n` for a new request, `i` for the project interaction history, `r` to reload,
`?` for help, `Esc` to return to Projects, and `q` to quit. The board is
read-only; state changes come from the application workflow. After
planning, assigned stories are executed for real in a git worktree and then
moved through `IN_REVIEW` to `DONE` (work by the assignee, review by the first
active QA or the Tech Lead). A `DONE` card detail shows the work summary,
review, reviewer, branch, worktree, commit, and completion time. VLab never
merges the branch automatically.

**Interactions** (`i`) opens a read-only project history: planning, work, and
review entries with time, participants, reference, outcome, and a recap.
`Enter` opens a scrollable full recap, `r` reloads, and `Esc` returns to the
originating screen. It is available from the Projects dashboard for the
selected project and from the Kanban board for the current project.

**Schedules** (`s`) opens the selected project's recurring tasks. The page lists
title, owner, interval, next run, status, and last result; `n` creates a task,
`p` pauses/resumes, `d` deletes after confirmation, `Enter` opens the full task,
`r` reloads, and `Esc` returns. Due schedules run in the background after the
dashboard opens and with `--run-schedules`; the page itself never calls the
provider.

The home dashboard lists Laboratories, Projects, Agents, Settings, Lab
floor, and Metrics. Use `Enter` to open the selected group, `1`–`6` to jump
directly, `Esc` to return home, and `q` to quit. Inside a group use `j/k` or
arrows, `h/l` for pages, the action keys in the bottom bar, `/` to filter, and
`?` for help. **Metrics** is a read-only laboratory summary: project count,
per-state card counts, open, closed, total, completion percentage, pending
intake, and a per-project table. `r` reloads it. A card is open when its state
is `TODO`, `IN PROGRESS`, `IN REVIEW`, or `BLOCKED`; it is closed when `DONE`. The
**Lab floor** is a fixed three-room MSX scene: **Project Lab** for the active
project and its task board, **War Room** for the Tech Lead and focused agents,
and **Lounge** for idle agents, joined by a corridor. Agents move
deterministically between desks and rooms. `↑/↓` selects agents, `←/→` moves to
the next occupied room, `Enter` opens the agent detail with its current tasks,
`t` opens the selected agent's first task, `k` opens the active project's
Kanban, `o` populates the active laboratory, `m` opens memories, `r` reloads,
and the Kanban refreshes in place while open.
`?` shows help, `Esc` returns to home or the Kanban, and `q` quits. The floor is
read-only; it never calls the provider. On the Kanban, `f` opens the active
project's floor; `Esc` returns to the screen that opened the current view.

The **Memories** screen opened with `m` has `Lab` and `Project` tabs. `Tab` or
`←/→` switches scope, `↑/↓` moves, `Enter` shows the full memory, `n` creates
with a multiline editor (`Alt+Enter` line break, `Enter` save, `Esc` cancel),
`e` edits, `d` deletes after confirmation, `r` reloads, and `Esc` returns to the
floor. The project tab requires a current project. Content is 1–2,000
characters, credentials are rejected, and each scope holds at most 20
memories. Group
subcommands (`vlab labs`, `vlab projects`, `vlab agents`, `vlab settings`,
`vlab help`) are removed; hidden legacy flags remain for script compatibility
during the deprecation window. Stored and
model-generated text is sanitized before display. Credentials are never shown.

## Legacy flags (hidden, deprecated)

The following flag surface remains available for scripts during the migration
window but is hidden from `--help`. New work uses the group subcommands.

## Settings

| Command | Behavior |
|---|---|
| `--show-settings` | Print the resolved settings path, provider, model, source-control provider, and whether the DeepSeek/GitHub keys in `settings.yaml` are set. No key value is shown. |
| `--edit-settings` | Edit a private copy in `VISUAL`/`EDITOR`, validate it, and atomically replace the file only on success. |

## Laboratories

| Command | Behavior |
|---|---|
| `--list-labs` | ID, name, description, agent count, project count, validation result, active marker. |
| `--new-lab <name>` | Create a laboratory; print its ID. The active selection is unchanged. |
| `--open-lab <lab-id>` | Select the active laboratory; show warnings for an incomplete laboratory. |
| `--check-lab <lab-id>` | One line per structural rule; exit 4 if invalid. |
| `--delete-lab <lab-id>` | Confirm, cascade projects/requests/stories/memories/memberships, preserve agents, clear a matching selection. |

## Projects

| Command | Behavior |
|---|---|
| `--list-projects` | Projects in the active laboratory with request/story/state counts. |
| `--new-project <name> [--github-source <url> \| --folder-source <path>]` | Create a project and select it. `--github-source` must be `https://github.com/<owner>/<repository>` and requires `credentials.github_api_key`; the repository is cloned into `.workspace/<project-id>`. `--folder-source` copies an absolute local folder, rejects symlinks/special files, and initializes a local git repository only inside `.workspace/<project-id>`. The modifiers are mutually exclusive; no modifier creates a legacy no-source project. A failed source preparation creates no project. |
| `--open-project <project-id>` | Select a project in the active laboratory as the current project. |
| `--assign-agent <agent-id>` | Assign an active laboratory member to the current project; repeated assignment is idempotent. |
| `--show-project-kanban` | Read-only five-column board for the current project, plus counts and pending intake. |
| `--delete-project` | Confirm, then delete the current project and cascade requests/stories/memories/assignments. |
| `--new-request [text]` | Validate a request for the current project from literal text or piped stdin and queue it as `TODO`; return immediately without a provider call. |
| `--run-queue` | Drain up to eight queued `TODO` requests for the active laboratory, oldest first, running planning and the bounded workflow; print a per-request and batch summary. |
| `--run-schedules` | Run up to three due schedules for the current project, creating bounded requests and executing planned work; print a run/failed/remaining summary. |
| `--import --src <file.md> --into <dest> [--agent <name-or-id>]` | Copy a UTF-8 markdown file into the data document store. `--into` is `lab/<name>`, `project/<name>`, or `agent` (with `--agent`). |

## Memories

Memories have no CLI commands. VLab writes structural memories when a
laboratory, agent, or project is created, and a bounded model summary after a
successful plan. Laboratory and project memories can be viewed, created,
edited, and deleted from the Lab floor (`m`); agent memories have no manual
surface. Deletion follows the owning laboratory or project.

## Agents

| Command | Behavior |
|---|---|
| `--list-agents` | Active members of the active laboratory with project/open-story counts. |
| `--new-agent` | Guided interactive profile creation; non-TTY exits 4. |
| `--create-agent-from-file <file>` | Read a markdown definition with strict YAML frontmatter (`name`, `role`, `description`, `skills`) and a markdown body as CV; create the profile and join the active laboratory when available. |
| `c` in the Agents dashboard | Duplicate the selected agent with a new name and an optional `Preserve memory` choice. |
| `--load-preset <file>` | Validate a tracked YAML catalog and create any missing profiles in one transaction. Creates no laboratory or membership, never calls the provider, and skips existing names case-insensitively. |
| `--show-agent-detail <agent-id>` | Profile, skills, CV, memberships, projects, assignments, memory count; unavailable metrics show `N/A`. |
| `--delete-agent <agent-id>` | Confirm; block unfinished stories; archive the profile, remove memberships, preserve history/memories. |
| `--ask <question> [--to-agent <agent-id>]` | Ask a current member or the single lead; the answer is advisory text. |

## Modifiers

- `--yes`: skip confirmation for `--delete-lab`, `--delete-project`, and
  `--delete-agent`.
- `--to-agent`: target for `--ask`.
- `--github-source`: optional project GitHub URL.
- `--folder-source`: optional project local folder path; mutually exclusive
  with `--github-source`.
- `--run-schedules`: run due project schedules instead of a dashboard action.
- `--run-queue`: process queued `TODO` requests instead of opening a dashboard.
- `--src`, `--into`, `--agent`: destination for `--import`.
- `--create-agent-from-file`: read an agent definition instead of the wizard.

## Exit codes

| Code | Meaning |
|---|---|
| 0 | Success, including a planning clarification. |
| 1 | Operation/provider/storage failure. |
| 2 | Invalid arguments, input, or configuration. |
| 3 | Missing entity or active context. |
| 4 | Validation/precondition failure, including non-interactive confirmation. |
| 5 | User cancellation/declined confirmation/editor failure. |
| 130 | Interrupted execution (SIGINT). |

## Notes

- No command emits ANSI color; `NO_COLOR` is trivially satisfied.
- Redirected output is plain text and sanitized.
- No command waits for optional input when stdin is not a terminal, except a
  required confirmation, which fails with exit 4.
- Memory content and prompts are never stored in events or interaction
  metadata.
