# CLI Reference

VLab is a single-binary, text-only CLI. Results go to stdout; errors, warnings,
and prompts go to stderr. Every invocation selects exactly one primary action.
Run `vlab` or `vlab --help` for the grouped help.

## Group dashboards

| Command | Behavior |
|---|---|
| `vlab` | Home dashboard in a terminal; help otherwise. |
| `-h`, `--help` | Print help. |
| `-V`, `--version` | Print the version. |

The Laboratories dashboard shows ID, name, description, agents, projects, and
the active marker. Its actions are New, Check, Edit, Set active, Projects,
Delete, Filter, Help, Back, and Quit. New/Edit use a bordered form: `Tab` or
arrows move fields, the focused field shows a cursor, `Enter` saves, and `Esc`
cancels. **Projects** activates the selected
laboratory and opens its Projects dashboard.

The Projects dashboard actions are New, Edit, Request, Open, Assign, Kanban,
Delete, Filter, Help, Back, and Quit. **Edit** renames the selected project; its
GitHub source cannot be changed. **Request** (`r`, or `n` on the Kanban board)
makes the project current and opens a prompt dialog: type or paste the request
(`Alt+Enter` adds a line break), `Enter` submits, `Esc` cancels. While the Tech
Lead plans, a `Planning…` dialog is shown and `Esc` or `Ctrl-C` cancels; the
outcome dialog lists the new stories, the clarification questions, or the
failure code and hint. **Kanban** (`v`) opens the selected project's board
as a full-screen view: the header shows project, laboratory, Tech Lead,
optional GitHub source, story counts, and pending intake; the five states are
side-by-side lists of cards, each showing only the story's short description.
Every list scrolls on its own and shows `↑N`, `↓N more`, and `position/total`
in its border. Use `←/→` or `h/l` for lists, `↑/↓` or `j/k` for cards,
`PgUp/PgDn` to scroll a page, `Home/End` or `g/G` for the first/last card,
`Enter` for the detail dialog (every story field; `↑/↓` scroll, `Esc` close),
`n` for a new request, `r` to reload, `?` for help, `Esc` to return to Projects, and `q` to quit. The
board is read-only; state changes come from the application workflow. After
planning, assigned stories are simulated through `IN_REVIEW` to `DONE` (work by
the assignee, review by the first active QA or the Tech Lead). A `DONE` card
detail shows the work outcome, review outcome, reviewer, and completion time.
This is a no-tool simulation: no code is executed.

The home dashboard lists Laboratories, Projects, Agents, Settings, and Lab
floor. Use `Enter` to open the selected group, `1`–`5` to jump directly, `Esc`
to return home, and `q` to quit. Inside a group use `j/k` or arrows, `h/l` for
pages, the action keys in the bottom bar, `/` to filter, and `?` for help. The
**Lab floor** is a read-only 2D scene of the active laboratory: project pods,
the Tech Lead desk, `(zzz)` for idle agents, `( ? )`/`( ! )` exchange bubbles
for two working agents of different roles, and `(///)` at an isolated focus
desk for an agent with two or more open stories. Use `↑/↓` for agents,
`←/→` for pods, `Enter` for agent detail, `m` for memories, `r` to reload,
`?` for help, `Esc` home, and `q` to quit.

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
| `--new-project <name> [--github-source <url>]` | Create a project and select it; the optional URL must be `https://github.com/<owner>/<repository>` and requires `credentials.github_api_key`. A source is cloned into `.workspace/<project-id>` before the project row is saved; a failed clone creates no project. |
| `--open-project <project-id>` | Select a project in the active laboratory as the current project. |
| `--assign-agent <agent-id>` | Assign an active laboratory member to the current project; repeated assignment is idempotent. |
| `--show-project-kanban` | Read-only five-column board for the current project, plus counts and pending intake. |
| `--delete-project` | Confirm, then delete the current project and cascade requests/stories/memories/assignments. |
| `--new-request [text]` | Plan a request for the current project from literal text or piped stdin, then run the bounded no-tool work/review simulation. |

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
| `--show-agent-detail <agent-id>` | Profile, skills, CV, memberships, projects, assignments, memory count; unavailable metrics show `N/A`. |
| `--delete-agent <agent-id>` | Confirm; block unfinished stories; archive the profile, remove memberships, preserve history/memories. |
| `--ask <question> [--to-agent <agent-id>]` | Ask a current member or the single lead; the answer is advisory text. |

## Modifiers

- `--yes`: skip confirmation for `--delete-lab`, `--delete-project`, and
  `--delete-agent`.
- `--to-agent`: target for `--ask`.
- `--github-source`: optional project GitHub URL.

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
