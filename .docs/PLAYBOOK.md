# VLab Playbook

A short step-by-step guide to configure VLab and run your first planning cycle.

## 1. What you need

- Rust via `rustup` (the pinned toolchain is in `rust-toolchain.toml`).
- A DeepSeek API key in `DEEPSEEK_API_KEY`.
- Optional: a GitHub key in `GITHUB_API_KEY` if a project points at a repository.

```sh
git clone https://github.com/massimobiagioli/vlab.git
cd vlab
make build-debug
```

The binary is `./target/debug/vlab`. `make run ARGS="..."` runs it with data in
`.data/` instead of your per-user directories.

## 2. Set the credentials

Use environment variables; never put keys in `settings.yaml`.

```sh
export DEEPSEEK_API_KEY="sk-..."
export GITHUB_API_KEY="ghp_..."   # optional
```

## 3. Create settings

```sh
vlab --edit-settings
```

Your editor opens a private copy. Save a valid file like:

```yaml
provider: deepseek
model: deepseek-chat
# Optional; GitHub is the only source-control provider.
source_control:
  provider: github
```

Invalid edits are rejected and the previous file is kept. Check it with:

```sh
vlab --show-settings
```

It shows `DEEPSEEK_API_KEY: set` and `GITHUB_API_KEY: set` (never the values).

## 4. Create and open a laboratory

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

```sh
vlab --new-project "Customer Portal"
```

With a GitHub repository (requires `GITHUB_API_KEY`):

```sh
vlab --new-project "Customer Portal" --github-source https://github.com/acme/portal
```

List projects and open the read-only board:

```sh
vlab --list-projects
vlab --show-project-kanban <project-id>
```

## 7. Add memories (optional)

Memories give the Tech Lead or an agent extra context. One scope per command.

```sh
vlab --new-memory "Prefer small pull requests." --for-lab <lab-id>
vlab --new-memory "The API is versioned." --for-project <project-id>
vlab --new-memory "Ask before changing shared schemas." --for-agent <agent-id>

vlab --list-memories --for-lab <lab-id>
```

List and delete:

```sh
vlab --list-memories --for-project <project-id>
vlab --delete-memory <memory-id>          # asks for confirmation
vlab --delete-memory <memory-id> --yes    # non-interactive
```

## 8. Submit a request

```sh
vlab --new-request <project-id> "Add password reset"
```

Or pipe a file:

```sh
vlab --new-request <project-id> < request.md
```

VLab stores the request first, calls DeepSeek with bounded retries, validates
the JSON proposal, assigns only real laboratory members, computes due dates
locally, and saves all stories in one transaction. It prints the request ID,
status, and story table. If it returns questions, the status is
`NEEDS_CLARIFICATION`; submit a revised request if needed.

## 9. Read the result

```sh
vlab --show-project-kanban <project-id>
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

`make run` uses `.data/` inside the project. To reset development data:

```sh
make clean-data
```

There is no backup command; stop VLab before copying the database. SQLite is
not encrypted at rest; use OS disk encryption for sensitive data.

## 12. Common errors

| Code | Meaning | Action |
|---|---|---|
| 2 | Invalid input/settings | Read the hint; run `--show-settings` or fix the argument. |
| 3 | Missing active laboratory or entity | Run `--list-labs`, then `--open-lab <lab-id>`. |
| 4 | Laboratory not ready / validation failed / confirmation needed | Run `--check-lab`; add a lead/skills; use a terminal or `--yes` for deletion. |
| 1 | Provider or storage failure | Check the key, network, and hint; retry later. |
| 130 | Interrupted | Run the command again; the request is preserved. |

## Quick start

```sh
export DEEPSEEK_API_KEY="sk-..."
vlab --edit-settings                 # provider: deepseek, model: deepseek-chat
vlab --new-lab "My Lab"
vlab --open-lab <lab-id>
vlab --new-agent                     # create the lead, then other agents
vlab --check-lab <lab-id>
vlab --new-project "My Project"
vlab --new-request <project-id> "Add the first feature"
vlab --show-project-kanban <project-id>
vlab --ask "What should we do next?"
```

See [`cli-reference.md`](cli-reference.md), [`configuration.md`](configuration.md),
and [`planning.md`](planning.md) for details.
