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
vlab labs
```

Use `n` to create, `a` to set the active laboratory, `c` to check it, and `q` to
quit. The non-interactive commands are:

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

Check the laboratory:

```sh
vlab --check-lab <lab-id>
```

Exit 0 means it is ready. Exit 4 prints one line per failed rule.

## 6. Create a project

Interactive route:

```sh
vlab projects
```

Use `n` to create, `Enter` to open, `a` to assign an agent, `v` for Kanban, and
`d` to delete. The non-interactive command is:

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
created, and when a request is planned successfully. There are no memory
commands. Existing memories remain in the planning context within the 8 KiB
bound.

## 8. Submit a request

```sh
vlab --new-request "Add password reset"
```

Or pipe a file:

```sh
vlab --new-request < request.md
```

VLab stores the request first, calls DeepSeek with bounded retries, validates
the JSON proposal, assigns only the project's agents (or laboratory members
when none is assigned), computes due dates locally, and saves all stories in
one transaction. It then stores a short planning-summary memory. It prints the
request ID, status, and story table. If it returns questions, the status is
`NEEDS_CLARIFICATION`; submit a revised request if needed.

## 9. Read the result

```sh
vlab --show-project-kanban
```

The board shows `TODO`, `IN PROGRESS`, `IN REVIEW`, `BLOCKED`, and `DONE`,
with assignee, priority, due date, counts, and pending intake. It is read-only.

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

## 12. Common errors

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
