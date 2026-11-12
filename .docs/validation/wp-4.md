# WP-4 Validation Report

Date: 2026-11-12
Package: WP-4 — Projects, Kanban, Safe Deletion, and Scoped Memories
Environment: macOS (aarch64), Rust 1.99.1, bundled SQLite 3.54.1
Commit: working tree, no tag

## Executed commands

```sh
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo audit
cargo deny check
```

`make check` runs the same five commands in order.

| Command | Result |
|---|---|
| `cargo fmt --all -- --check` | pass |
| `cargo clippy --locked --all-targets -- -D warnings` | pass |
| `cargo test --locked` | 249 passed, 0 failed |
| `cargo audit` | no findings |
| `cargo deny check` | advisories ok, bans ok, licenses ok, sources ok |

## Walkthrough (local, no provider)

1. Create a laboratory, open it, create a project, and see it in
   `--list-projects` with zero request/story counts.
2. `--show-project-kanban` prints the project and laboratory names, the Tech
   Lead, five state counts, completed count, pending intake, and an actionable
   empty state. Fixtures with stories in all five states render as five
   side-by-side columns at width 100; widths below 100 use stacked sections.
3. `--new-memory "…" --for-lab|--for-project|--for-agent` creates a memory;
   `--list-memories` shows ID, scope, owner, a sanitized preview, and creation
   time; `--delete-memory` requires confirmation or `--yes`.
4. A project memory is rejected when the project belongs to another
   laboratory. An archived agent's existing memories remain listable, but new
   ones are rejected.
5. `--delete-project --yes` removes the project, its requests, stories, and
   memories, and preserves agent profiles.
6. `--delete-lab --yes` removes projects, requests, stories, memories, and
   memberships, preserves agents, and clears the active selection.
7. `--delete-agent --yes` blocks while an assigned story is not `DONE`;
   otherwise it archives the profile, removes memberships, preserves story
   references and memories, and warns when the agent is a Tech Lead.
8. A fresh `PLANNING` request blocks laboratory, project, and agent deletion;
   a planning lease older than 240 seconds does not.
9. Without a terminal and without `--yes`, every destructive command exits 4
   and changes nothing.

## Acceptance criteria

| ID | Evidence | Result |
|---|---|---|
| WP4-AC1 | `tests/db_projects.rs` | pass |
| WP4-AC2 | `tests/cli_projects.rs` | pass |
| WP4-AC3 | `tests/cli_kanban.rs`, `src/cli/render.rs` | pass |
| WP4-AC4 | `tests/cli_kanban.rs` | pass |
| WP4-AC5 | `src/cli/confirm.rs`, deletion CLI tests | pass |
| WP4-AC6 | `tests/cli_deletions.rs` | pass |
| WP4-AC7 | `tests/cli_deletions.rs` | pass |
| WP4-AC8 | `tests/cli_deletions.rs` | pass |
| WP4-AC9 | `tests/cli_labs.rs`, `tests/cli_agents.rs` | pass |
| WP4-AC10 | `tests/db_memories.rs`, `tests/cli_memories.rs` | pass |

## Limitations and unverified items

- Linux was not run locally; hosted CI is deferred to Go-Live (ADR-0013).
- The interactive `y/N` path is unit-tested through the prompt port; the
  non-interactive refusal and `--yes` are exercised end to end. A real PTY
  session was not recorded.
- Active planning is simulated with inserted `PLANNING` rows; the provider
  integration that creates them arrives in WP-5.
- Snapshot binding compares names and affected counts, not a byte-for-byte
  database hash.
- The Kanban is read-only and only `TODO` stories are created by the initial
  planner; state transitions are not implemented.
- Memory content is stored unencrypted with the rest of the database; OS disk
  encryption remains the only at-rest protection.
- Agent deletion archives rather than removes the profile; an archived agent's
  memories are retained and excluded from future prompts by design.
