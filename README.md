# VLab

VLab is a local, text-only command-line application for managing virtual
software laboratories staffed by AI agents. It stores data in a local SQLite
database and talks to a single outbound AI provider. It runs no server and
never executes model-generated content.

This is the initial deterministic release: all documented commands are
implemented. Live-provider verification, a real-user walkthrough, hosted CI,
and the release security gate are Go-Live activities; see
[`.docs/validation/final.md`](.docs/validation/final.md) for evidence and
remaining limitations.

## Build

The Rust toolchain is pinned in `rust-toolchain.toml`; `rustup` installs it on
first use.

```sh
make build-debug        # or: cargo build --locked
./target/debug/vlab --help
```

## Makefile

The `Makefile` at the project root wraps every everyday task. `make` on its
own prints the list.

| Command | What it does |
|---|---|
| `make` / `make help` | List the available targets. |
| `make build-debug` | Build the debug binary in `target/debug/vlab`. |
| `make build-release` | Build the optimized binary in `target/release/vlab`. |
| `make run ARGS="..."` | Build, then run `vlab` with its settings and database in `.data/` (see below). Without `ARGS`, shows `--help`. |
| `make test` | Run all tests. |
| `make lint` | Check formatting and run clippy with warnings as errors. |
| `make fmt` | Format the code. |
| `make fmt-check`, `make clippy` | Run each half of `lint` separately. |
| `make audit` | Check dependencies against RustSec advisories. |
| `make deny` | Check advisories, licenses, banned crates, and sources. |
| `make check` | Run every quality gate: `fmt-check`, `clippy`, `test`, `audit`, `deny`. Required before any change is complete. |
| `make tools` | Install `cargo-audit` and `cargo-deny` at the versions the gates expect. |
| `make snapshot` | Regenerate `tests/snapshots/help.txt` after an intended help change. |
| `make clean` | Remove build output. |
| `make clean-data` | Delete the development data in `.data/`, keeping the example settings. |

Examples:

```sh
make run ARGS="--version"
make run ARGS="--new-lab 'Payments Platform'"
```

### Development data in `.data/`

`make run` sets `VLAB_CONFIG_DIR` and `VLAB_DATA_DIR` to the project's
`.data/` folder, so `settings.yaml` and `vlab.db` are written there. Before
each run it creates the folder if needed and sets its mode to `0700`, because
VLab refuses directories other users can read. `.data/` is ignored by git,
except the tracked placeholder `.data/settings.example.yaml`. Copy it to
`.data/settings.yaml` and add your keys; `settings.yaml` itself is never
tracked. Running the binary directly, without `make run`, uses the per-user
directories below (ADR-0014).

## Local paths

VLab never writes to the current working directory by default.

| Purpose | Default location | Override |
|---|---|---|
| Settings | `<per-user config dir>/vlab/settings.yaml` | `VLAB_CONFIG_DIR` |
| Data | `<per-user data dir>/vlab/` | `VLAB_DATA_DIR` |
| Database | `<per-user data dir>/vlab/vlab.db`, or `$VLAB_DATA_DIR/vlab.db` when that override is set | `VLAB_DB_PATH` |

The per-user directories are `~/Library/Application Support` on macOS and
`$XDG_CONFIG_HOME` (default `~/.config`) and `$XDG_DATA_HOME` (default
`~/.local/share`) on Linux. Overrides must be absolute paths. VLab creates its
directories with mode `0700` and its files with mode `0600` on Unix. It
refuses an existing directory that grants group or other access and, on Unix,
a settings file readable by group or other users.
Overrides are operational facilities for tests and controlled deployments,
not product commands.

## Local walkthrough

WP-3 delivers laboratories and agent profiles; WP-4 adds projects, the
read-only Kanban, scoped memories, and safe deletion. Settings must be valid
before `--new-agent`, because new agents copy the provider and model from
`settings.yaml` (§5.4).

```sh
# Create the private settings file and add provider/model and credentials.
vlab --edit-settings
vlab --show-settings

# Create a laboratory; the identifier is printed and is not selected.
vlab --new-lab "Payments Platform"

# Select it. Opening an incomplete laboratory is allowed, with warnings.
vlab --open-lab <lab-id>

# Check every structural rule. Exit 0 when valid, 4 when not.
vlab --check-lab <lab-id>

# Create the Tech Lead, then other agents. The active laboratory is pre-filled.
vlab --new-agent
vlab --new-agent

# Inspect the members of the active laboratory and one profile in detail.
vlab --list-agents
vlab --show-agent-detail <agent-id>
```

Projects belong to the active laboratory. Creating one is allowed even while
the laboratory is structurally incomplete, but planning will require a valid
laboratory.

```sh
# Optional GitHub source; requires credentials.github_api_key in settings.yaml.
vlab --new-project "Customer Portal" --github-source https://github.com/acme/portal
vlab --list-projects
vlab --show-project-kanban <project-id>
```

`--github-source` accepts only canonical `https://github.com/<owner>/<repository>`
URLs. The GitHub key is global (`credentials.github_api_key` in
`settings.yaml`) and is never stored in the project, database, logs, or output.
No GitHub network call is performed in this release; the source is
configuration metadata for a future operation.

The board is read-only: it shows the five states, cards, assignee or
`Unassigned`, priority, due date, story counts, completed count, and pending
intake. Only `TODO` stories are created initially; state transitions are not
implemented.

### Scoped memories

A memory is user-authored text attached to exactly one scope. It is bounded to
2,000 characters, at most 20 per scope, and treated as untrusted context. The
model cannot create, edit, or delete one.

```sh
vlab --new-memory "Prefer small pull requests." --for-lab <lab-id>
vlab --new-memory "The API is versioned." --for-project <project-id>
vlab --new-memory "Ask before changing shared schemas." --for-agent <agent-id>

vlab --list-memories --for-lab <lab-id>
vlab --list-memories --for-project <project-id>
vlab --list-memories --for-agent <agent-id>

vlab --delete-memory <memory-id>          # asks for confirmation
vlab --delete-memory <memory-id> --yes    # explicit non-interactive bypass
```

`--for-project` requires the project to belong to the active laboratory.
Planning and questions will include only the selected scoped memories, within
an 8 KiB budget, delimited as data (WP-5/WP-6).

### Destructive commands

`--delete-lab`, `--delete-project`, `--delete-agent`, and `--delete-memory`
show what will be affected and require confirmation. Without a terminal,
`--yes` is required. Deleting a laboratory cascades its projects, requests,
stories, memberships, and memories while preserving reusable agent profiles.
Agent deletion archives the profile, removes memberships, preserves story
references and memories, and warns when the agent is a Tech Lead. Active
planning blocks deletion.

`--new-agent` uses a guided procedure and requires an interactive terminal;
piped input exits 4 without creating anything (A-09). VLab emits no color, so
`NO_COLOR` is trivially satisfied; tables stack into `label: value` blocks on
narrow terminals.

### Validation rules (`--check-lab`)

The report contains one line per rule:

| Rule | Failure condition |
|---|---|
| Laboratory exists | The ID resolves to no laboratory (exit 3 before a report). |
| At least one agent | The laboratory has no active members. |
| Exactly one Tech Lead | Zero or more than one member has role `lead`. |
| Roles are valid | A stored role is not `lead`, `dev`, `analyst`, or `qa`. |
| Profiles are well-formed | A stored name, description, skills value, or CV is malformed. |
| Provider and model are valid | A stored provider or model is not a supported token. |
| Operational roles have skills | A `dev`, `analyst`, or `qa` member has no skill (A-07). |
| Memberships resolve | Reserved for referential integrity; always passes while foreign keys are enabled. |
| Project references resolve | Schema v3 keeps projects inside their laboratory; the rule reports the project count. |

`--check-lab` makes no provider call and does not require a provider key.
The package links no HTTP client; that arrives with WP-5.

### Exit codes for this package

| Code | Meaning |
|---|---|
| 0 | Success. |
| 1 | Storage or internal failure. |
| 2 | Malformed identifier or invalid input, such as an empty name or a missing memory scope. |
| 3 | No active laboratory, or a well-formed identifier that resolves to nothing. |
| 4 | A validation or precondition failure, including an invalid `--check-lab` report, non-interactive `--new-agent`, a full memory scope, an unfinished assigned story, or active planning during deletion. |
| 5 | The user declined a confirmation or an editor exited unsuccessfully. |

`--show-settings` also reports the source-control provider and whether the
DeepSeek and GitHub keys in `settings.yaml` are set (`set`/`unset` only).

## Request planning

Planning requires a structurally valid active laboratory, a project, valid
settings, and `credentials.deepseek_api_key` in `settings.yaml`.

```sh
vlab --new-request <project-id> "Add password reset"
cat request.md | vlab --new-request <project-id>
vlab --show-project-kanban <project-id>
```

The request is persisted before any provider call. DeepSeek returns a strict
JSON proposal; VLab validates it, assigns only active laboratory members,
computes due dates locally in UTC (2/5/10 calendar days by complexity), and
inserts all stories in one transaction. Clarification, provider, validation,
budget, timeout, and cancellation outcomes preserve the request. Retries are
bounded and may incur additional provider charges; exactly-once remote
execution is not promised.

Planning prompts include laboratory, project, and Tech Lead agent memories
within 8 KiB, delimited as data. Request text, prompts, CVs, and provider
responses are never stored in events or shown in errors. See
[`.docs/planning.md`](.docs/planning.md).

## Direct questions

```sh
vlab --ask "What should we prioritize next?"
vlab --ask "Review this API" --to-agent <agent-id>
```

The target must be an active member of the active laboratory. Without
`--to-agent`, the single Tech Lead answers. The answer is advisory text, shown
with the agent and model identifiers; it is never stored as content and never
executed. Lab and responding-agent memories are included within the 8 KiB
budget.

## Limitations

- SQLite is unencrypted at rest; use OS disk encryption for sensitive data.
- No backup/restore command; stop VLab before copying the database file.
- Story state transitions and execution are not implemented; the board is
  read-only and planning creates `TODO` stories only.
- No JSON output mode, packaging/licensing, or Windows-specific hardening.
- Semantic quality of assignments and answers is an evaluation criterion, not
  a deterministic proof.
- Memories are immutable; there is no automatic capture, editing, or
  model-written memory.
- A live provider contract and cost/quality evaluation are Go-Live activities.

## Documentation

- [`AGENTS.md`](AGENTS.md) — contributor and agent entry point, quality gates.
- [`CONTRIBUTING.md`](CONTRIBUTING.md) — local setup and tests.
- [`.docs/`](.docs/) — authoritative project documentation and ADRs.
- [`.docs/cli-reference.md`](.docs/cli-reference.md) — every command, modifier,
  and exit code.
- [`.docs/PLAYBOOK.md`](.docs/PLAYBOOK.md) — step-by-step setup and first
  planning walkthrough.
- [`.docs/configuration.md`](.docs/configuration.md) — settings, credentials,
  paths, and the editor.
- [`.docs/planning.md`](.docs/planning.md) — request intake, provider bounds,
  outcomes, assignments, due dates, and memory context.
- [`.docs/security-review.md`](.docs/security-review.md) — trust boundaries and
  secret-handling evidence.
- [`.docs/data-retention.md`](.docs/data-retention.md) — what VLab stores,
  what it never stores, and the lack of encryption at rest.
- [`.docs/DATA-MODEL.md`](.docs/DATA-MODEL.md) — conceptual data model and
  relationships between laboratories, agents, projects, requests, stories,
  and memories.
- [`.docs/validation/`](.docs/validation/) — per-package validation reports.
- [`.project/IMPLEMENTATION-PLAN.md`](.project/IMPLEMENTATION-PLAN.md) —
  ordered implementation plan.
