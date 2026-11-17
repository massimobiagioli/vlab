# VLab Playbook

A short step-by-step guide to configure VLab and run your first planning cycle.

## 1. What you need

- Rust via `rustup` (the pinned toolchain is in `rust-toolchain.toml`).
- A DeepSeek API key.
- Optional: a GitHub token if a project points at a repository.

```sh
git clone https://github.com/massimobiagioli/vlab.git
cd vlab
make build-debug
```

The binary is `./target/debug/vlab`. A debug build from this checkout uses
`.data/` automatically; `make run ARGS="..."` does the same explicitly. An
installed or release binary uses per-user directories.

## 2. Create settings with the credentials

Credentials live in `settings.yaml`, never in environment variables. The file
is private, outside git, and written with mode `0600`.

From this checkout, the debug binary keeps settings in `.data/settings.yaml`.
Create it with the editor:

```sh
./target/debug/vlab --edit-settings
```

Or start from the tracked example with private permissions:

```sh
install -m 600 .data/settings.example.yaml .data/settings.yaml
```

An installed or release binary uses the per-user settings path instead; the
same `--edit-settings` command creates it there.

A valid file looks like:

```yaml
provider: deepseek
# Current DeepSeek models: deepseek-flash (recommended) or deepseek-v4-pro.
model: deepseek-flash
# Optional; GitHub is the only source-control provider.
source_control:
  provider: github
credentials:
  deepseek_api_key: "sk-..."
  github_api_key: "ghp-..."   # optional
```

`github_api_key` is needed only for projects created with `--github-source`.
Invalid edits are rejected and the previous file is kept.

## 3. Verify the settings

```sh
vlab --show-settings
```

It shows `DeepSeek API key: set` and `GitHub API key: set` (never the values).

## 4. Create and open a laboratory

Interactive route:

```sh
vlab
```

Choose **Laboratories**, then use `n` to create (a bordered Name + optional
Description form), `e` to edit, `a` to set active, `c` to check, and `p` to open
the active laboratory's projects. `Esc` returns to the home dashboard. The
non-interactive commands are:

```sh
vlab --new-lab "Payments Platform"
vlab --open-lab <lab-id>
```

`--new-lab` prints the ID. Opening an incomplete laboratory is allowed.

## 5. Add agents

The wizard is interactive and needs a terminal.

```sh
vlab --new-agent
```

Create the **Tech Lead** first (role `lead`), then at least one `dev`,
`analyst`, or `qa` agent with one or more skills. The active laboratory is
pre-filled; press Enter to accept it.

To reuse a profile, open **Agents**, select it and press `c`. The form asks for
a new name and whether to `Preserve memory`; copied memories are bounded by the
normal agent-memory limits. A duplicated Tech Lead is created without
memberships because the source laboratory already has a lead.

To create one agent from a markdown definition:

```sh
make agent-create-from-file SRC=agents/example.md
```

The file has strict YAML frontmatter (`name`, `role`, `description`, `skills`);
the markdown body becomes the CV. The active laboratory is joined when
available, otherwise the profile is created in the catalog.

Faster route: load the tracked fictional catalog, then populate a laboratory
from the Laboratories dashboard.

```sh
make load-preset          # create the thirty preset profiles in .data/
vlab                      # open the dashboard, choose Laboratories
```

Select the laboratory and press `o` (or open it and press `o` on the Lab
floor). Use `v` to read the selected agent's full profile, move the Tech Lead
and at least one other agent to **In this laboratory** with `Enter`, then press
`s` to save.

Check the laboratory:

```sh
vlab --check-lab <lab-id>
```

Exit 0 means it is ready. Exit 4 prints one line per failed rule.

## 6. Create a project

Interactive route:

```sh
vlab
```

Choose **Projects**, then use `n` to create, `e` to rename, `Enter` to open,
`a` to assign an agent, `k` for the full-screen Kanban board, which refreshes
in place while background work runs (`←/→` lists,
`↑/↓` cards with per-list scrolling, `PgUp/PgDn` pages, `Enter` opens the
story detail dialog, `Esc` back), and `d` to delete. The New project form offers
a source choice (`None`, `GitHub`, or `Folder`) and one source field. The
non-interactive command is:

```sh
vlab --new-project "Customer Portal"
```

With a GitHub repository (requires `credentials.github_api_key`), VLab clones
it into `.workspace/<project-id>` before saving the project:

```sh
vlab --new-project "Customer Portal" --github-source https://github.com/acme/portal
```

If the clone fails, no project is created. The token is passed through a
private askpass helper, never in the URL or arguments.

With a local folder, VLab copies it into `.workspace/<project-id>` and
initializes a local git repository only inside the copy:

```sh
vlab --new-project "Local Portal" --folder-source "$PWD/local-portal"
```

The original folder is never modified and needs no version control. Symbolic
links and special files are rejected, and a failed import creates no project.

`--new-project` selects the new project. Assign the agents that should work on
it (repeat for each agent):

```sh
vlab --assign-agent <agent-id>
```

Use `--open-project <project-id>` to switch project later. List projects and
open the read-only board:

```sh
vlab --list-projects
vlab --show-project-kanban
```

## 7. Automatic memories

Memories are generated automatically when a laboratory, agent, or project is
created, and when a request is planned successfully. Laboratory and project
memories can also be viewed, created, edited, and deleted from the Lab floor
(`m`); agent memories have no manual surface. Existing memories remain in the
planning context within the 8 KiB bound.

## 8. Submit a request

Interactive route: in **Projects** select the project and press `r` (or press
`n` on its Kanban board). Type or paste the request in the dialog and press
`Enter`; `Esc` cancels. The request is queued as `TODO` immediately and a
`REQUEST` card appears in the Kanban `TODO` column. The background Tech Lead
picks up the oldest queued request, plans it, and runs the bounded workflow.
A second request waits in `TODO` without blocking the dashboard. Press `Enter`
on a request card to read its text.

The hidden legacy flags still work for scripts:

```sh
vlab --new-request "Add password reset"
vlab --run-queue
```

Or pipe a file:

```sh
vlab --new-request < request.md
vlab --run-queue
```

`--new-request` only queues. `--run-queue` calls DeepSeek with bounded retries,
validates the JSON proposal, assigns only the project's agents (or laboratory
members when none is assigned), computes due dates locally, and saves all
stories in one transaction. It then stores a markdown planning recap and prints
the request status. When an agent closes an accepted card, VLab adds a markdown
completion recap to that agent's memory. If planning returns questions, the Tech Lead
resolves them autonomously in up to two bounded follow-up calls; unresolved
questions fail with `clarification_unresolved`, never with a human prompt. A
missing provider
configuration leaves the request in `TODO`.

## 9. Read the result

```sh
vlab --show-project-kanban
```

The board shows `TODO`, `IN PROGRESS`, `IN REVIEW`, `BLOCKED`, and `DONE`,
with assignee, priority, due date, counts, and pending intake. It is read-only;
states are changed only by the application workflow. A failing or rejected
story is remediated by the Tech Lead: it gets bounded context, may be
reassigned, and is retried for up to three cycles. The workflow never parks it
in `BLOCKED`; after the last cycle the Tech Lead records a closing compromise,
the card becomes `DONE`, and one linked unassigned `TODO` card captures the
technical debt. A `DONE` card can be opened with `Enter` to read its work
summary, review, reviewer, branch, worktree, commit, and completion time. VLab
never merges the branch automatically.

### Interactions

In **Projects** select a project and press `i`, or press `i` on its Kanban
board. The read-only page lists planning, work, and review entries newest
first, with time, participants, reference, outcome, and a recap. `Enter` opens
the full scrollable recap, `r` reloads, and `Esc` returns. The page never calls
the provider or changes a story.

### Schedules

In **Projects** select a project and press `s`. Use `n` to create a recurring
task: title, single-line request, interval (daily/weekly/monthly), and owner
agent. `p` pauses/resumes, `d` deletes after confirmation, and `Enter` opens the
full task. Due schedules run in the background after the dashboard opens, or with:

```sh
make run-schedules
```

Each due schedule creates a normal bounded request; the Tech Lead still assigns
the work. VLab is not a background daemon.

## 9.1 Import a document

Memories and documents are markdown files under `data/memories` and
`data/documents`. Import a document into a lab, project, or agent folder:

```sh
make import SRC=notes/security.md INTO=lab/Payments
make import SRC=notes/design.md INTO=project/Portal
make import SRC=notes/profile.md INTO=agent AGENT=Klaus
```

The source must be a regular UTF-8 `.md` file. The command copies it into the
matching folder, stores only the relative reference, and records a
`document_imported` event.

## 10. Ask a question

```sh
vlab --ask "What should we prioritize next?"
vlab --ask "Review this API" --to-agent <agent-id>
```

Without `--to-agent`, the single Tech Lead answers. The answer is advisory and
is never executed or stored.

## 11. Where data lives

| Purpose | Default | Override |
|---|---|---|
| Settings | per-user config dir + `vlab/settings.yaml` | `VLAB_CONFIG_DIR` |
| Database | per-user data dir + `vlab/vlab.db` | `VLAB_DATA_DIR`, `VLAB_DB_PATH` |
| Project checkouts | `<repo>/.workspace/` for the debug layout, otherwise `<data dir>/workspace/` | — |

A debug build from this checkout uses `.data/` automatically (ADR-0031). It
contains `settings.yaml` once you create it, and that file holds your keys;
`.data/settings.example.yaml` is only a tracked placeholder. To reset development data (keeping the example):

```sh
make clean-data
make clean-workspace
```

`make clean-workspace` deletes cloned checkouts; project deletion does not
remove them automatically yet. There is no backup command; stop VLab before copying the database. SQLite is
not encrypted at rest; use OS disk encryption for sensitive data. Protect
backups of `settings.yaml` as secrets.

## 12. Watch the lab floor and manage memories

From the home dashboard choose **Lab floor** (or press `5`). The fixed scene
has a **Project Lab** with the active project's task board, a **War Room** with
the Tech Lead and focused agents, and a **Lounge** for idle agents, connected
by a corridor and doors. Agents move deterministically; each face has its name
below and a state bubble above (`(zzz)`, `( ... )`, `(///)`, `(★)`, `(→)`), and
`⇅` marks an exchange. Room interiors contain titles, furniture, faces, names,
and bubbles; task text appears only in the `Enter` detail. `↑/↓` selects agents, `←/→` moves to the
next occupied room, `Enter` opens the agent detail with its current tasks, `t`
opens the selected task, `k` opens the active project's Kanban, and `r`
reloads. The Kanban `f` key returns to the floor; `Esc` preserves the screen
that opened the current view. Press `m` to open **Memories**: `Tab` switches
between laboratory and project scope, `n` creates, `Enter` views, `e` edits,
and `d` deletes after confirmation. Content is bounded and credentials are
rejected. Agent memories have no manual surface.

## 13. Read laboratory metrics

From the home dashboard choose **Metrics** (or press `6`). The screen shows the
active laboratory name, project count, card counts for `TODO`, `IN PROGRESS`,
`IN REVIEW`, `BLOCKED`, and `DONE`, plus open, closed, total, completion
percentage, pending intake, and a per-project table. A card is open until it is
`DONE`. Press `r` to reload, `Esc` to return home, and `q` to quit. The screen
is read-only and does not call the provider.

## 14. Common errors

| Code | Meaning | Action |
|---|---|---|
| 2 | Invalid input/settings | Read the hint; run `--show-settings` or fix the argument. |
| 3 | Missing active laboratory, project, or entity | Run `--list-labs`/`--list-projects`, then `--open-lab`/`--open-project`. |
| 4 | Laboratory not ready / validation failed / confirmation needed | Run `--check-lab`; add a lead/skills; use a terminal or `--yes` for deletion. |
| 1 | Provider or storage failure | Check the key, network, and hint; retry later. |
| 130 | Interrupted | Run the command again; the request is preserved. |

## Quick start

```sh
vlab --edit-settings                 # add provider, model, and keys
vlab --show-settings
vlab --new-lab "My Lab"
vlab --open-lab <lab-id>
vlab --new-agent                     # create the lead, then other agents
vlab --check-lab <lab-id>
vlab --new-project "My Project"
vlab --new-request "Add the first feature"
vlab --show-project-kanban
vlab --ask "What should we do next?"
```

See [`cli-reference.md`](cli-reference.md), [`configuration.md`](configuration.md),
and [`planning.md`](planning.md) for details.
