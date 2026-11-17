# Feature 00019 — Markdown file store and document import

| Field | Value |
|---|---|
| Identifier | `00019` |
| Branch | `feat/file-store` |
| Status | `Done` |
| Date | 2026-11-17 |
| Related ADRs | ADR-0059 (file store), ADR-0022 (memories), ADR-0059 |
| Scope | File-backed memories, relative references, documents import, tracked skeleton |

## Goal

Move memory content to markdown files, keep only relative paths in SQLite, add
a documents store with agent/project/lab folders, and provide
`make import SRC=<file.md> INTO=<dest>`.

## Acceptance criteria

1. `.data/memories/.gitkeep` and `.data/documents/.gitkeep` are tracked; the
   rest of both trees is ignored.
2. The data directory contains `memories` and `documents` roots with `agents`,
   `projects`, and `lab` subfolders.
3. Existing memories are materialized into markdown files; `memories.path`
   stores a relative reference and legacy content is cleared. Failed writes are
   retried on a later open.
4. The memory API returns file content; create, update, and delete operate on
   the file and the reference.
5. Documents are copied from a UTF-8 `.md` source into the matching folder; a
   `documents` row stores scope, title, and relative path.
6. `make import SRC=<file.md> INTO=<dest>` accepts `agent` (with `AGENT`),
   `project/<name>`, and `lab/<name>`; ambiguity and non-markdown input fail
   clearly.
7. Paths are confined to the data directory; absolute paths, `..`, symlinks,
   and oversized files are rejected.
8. Tests cover the file store, memory migration, document import, and CLI/
   Makefile behavior; `make check` passes.

## Non-goals

- A documents browser/editor in this change.
- Dropping the legacy `content` column.
- Encryption or backup of the file store.
- Symlinks or non-markdown files.

## Validation

- Local gate (2026-11-17): `make check` passed (fmt-check, clippy, tests,
  audit, deny).
- `src/infra/files.rs` tests cover confined reads/writes, traversal and
  absolute paths, symlink destinations, markdown-only imports, and entity
  folders.
- `tests/db_memories.rs` covers markdown content, the path-only database row,
  and v14-to-v15 materialization.
- `tests/documents.rs` covers lab/project/agent imports, ambiguity, non-markdown
  input, and the CLI path.
- `tests/cli_memories.rs` and `tests/planning_flow.rs` read content from the
  referenced files.
- Hosted CI is green on commit `6fad608`.

## Task checklist

- [x] **F00-01** Write ADR-0059; update the registry and `AGENTS.md`.
- [x] **F01-01** Add the confined file store with tests.
- [x] **F02-01** Add schema v15, entity folders, and memory materialization.
- [x] **F03-01** Make the memory adapter file-backed without changing callers.
- [x] **F04-01** Add the documents store and import use case.
- [x] **F05-01** Add the hidden CLI import and Makefile target.
- [x] **F06-01** Add the tracked `.gitkeep` skeleton and ignore rules.
- [x] **F07-01** Update the specification, CLI reference, README, playbook,
  data model, retention, and security docs.
- [x] **F08-01** Run `make check` and fix every failure.
- [x] **F08-02** Push the branch, confirm hosted CI is green, and set the
  status to `Done`.
