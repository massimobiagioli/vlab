# CLI Reference

VLab is a single-binary, text-only CLI. Results go to stdout; errors, warnings,
and prompts go to stderr. Every invocation selects exactly one primary action.
Run `vlab` or `vlab --help` for the grouped help.

## Settings

| Command | Behavior |
|---|---|
| `--show-settings` | Print the resolved settings path, provider, model, source-control provider, and whether the DeepSeek/GitHub keys in `settings.yaml` are set. No key value is shown. |
| `--edit-settings` | Edit a private copy in `VISUAL`/`EDITOR`, validate it, and atomically replace the file only on success. |

## Laboratories

| Command | Behavior |
|---|---|
| `--list-labs` | ID, name, agent count, project count, validation result, active marker. |
| `--new-lab <name>` | Create a laboratory; print its ID. The active selection is unchanged. |
| `--open-lab <lab-id>` | Select the active laboratory; show warnings for an incomplete laboratory. |
| `--check-lab <lab-id>` | One line per structural rule; exit 4 if invalid. |
| `--delete-lab <lab-id>` | Confirm, cascade projects/requests/stories/memories/memberships, preserve agents, clear a matching selection. |

## Projects

| Command | Behavior |
|---|---|
| `--list-projects` | Projects in the active laboratory with request/story/state counts. |
| `--new-project <name> [--github-source <url>]` | Create a project; the optional URL must be `https://github.com/<owner>/<repository>` and requires `credentials.github_api_key` in `settings.yaml`. |
| `--show-project-kanban <project-id>` | Read-only five-column board plus counts and pending intake. |
| `--delete-project <project-id>` | Confirm, then cascade requests/stories/memories. |
| `--new-request <project-id> [text]` | Plan a request from literal text or piped stdin. |

## Memories

| Command | Behavior |
|---|---|
| `--new-memory <text> --for-lab\|--for-project\|--for-agent <id>` | Create a bounded, immutable memory. |
| `--list-memories --for-lab\|--for-project\|--for-agent <id>` | List ID, scope, owner, preview, creation time. |
| `--delete-memory <memory-id>` | Confirm and delete one memory. |

Exactly one `--for-*` selector is required. `--for-project` requires the project
to belong to the active laboratory. `--yes` is valid on deletions only.

## Agents

| Command | Behavior |
|---|---|
| `--list-agents` | Active members of the active laboratory with project/open-story counts. |
| `--new-agent` | Guided interactive profile creation; non-TTY exits 4. |
| `--show-agent-detail <agent-id>` | Profile, skills, CV, memberships, projects, assignments, memory count; unavailable metrics show `N/A`. |
| `--delete-agent <agent-id>` | Confirm; block unfinished stories; archive the profile, remove memberships, preserve history/memories. |
| `--ask <question> [--to-agent <agent-id>]` | Ask a current member or the single lead; the answer is advisory text. |

## Modifiers

- `--yes`: skip confirmation for `--delete-lab`, `--delete-project`,
  `--delete-agent`, `--delete-memory`.
- `--for-lab`, `--for-project`, `--for-agent`: memory scope for
  `--new-memory`/`--list-memories`.
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
