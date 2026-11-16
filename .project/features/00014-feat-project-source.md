# Feature 00014 — Project source types

| Field | Value |
|---|---|
| Identifier | `00014` |
| Branch | `feat/project-source` |
| Status | `In progress` |
| Date | 2026-11-16 |
| Related ADRs | ADR-0054 (project source types), ADR-0032 (GitHub clone), ADR-0050 (worktree execution) |
| Scope | Explicit GitHub/folder project source selection, isolated folder import, display, and tests |

## Goal

Let the operator choose a project source explicitly: a GitHub repository or a
local folder without version control. A folder source is copied into
`.workspace/<project-id>` and receives a local git repository only inside that
copy, so real story execution keeps using isolated worktrees without touching
the original folder.

## Acceptance criteria

1. The domain has `ProjectSource` (`none`, `github`, `folder`) and a validated
   `FolderSource` absolute path.
2. Schema v12 adds `projects.folder_source`; existing projects migrate to
   `none`, and no project stores both a GitHub and a folder source.
3. `--new-project` accepts `--folder-source <path>` or `--github-source <url>`,
   mutually exclusive, and keeps the legacy no-source form.
4. A folder source is canonicalized, must exist and be a directory, must not
   overlap `.workspace`, and rejects symlinks, special files, and `.git`
   entries during the copy.
5. Folder import is bounded (100,000 entries, 1 GiB, depth 64), copies into a
   private temporary directory, initializes git with an initial commit, and
   renames to `.workspace/<project-id>`.
6. A failed import or database insert leaves no project row and no workspace
   directory; the source folder is never modified.
7. Folder creation does not require settings or a GitHub credential, performs no
   network call, and the provider sees no source files.
8. The New project dashboard form offers `None`, `GitHub`, and `Folder` and one
   source field; validation matches the selected type.
9. Project creation, opening, listing, and the Kanban header show the source
   kind and detail.
10. Tests cover domain parsing, schema migration, workspace copy and git
    initialization, CLI modifiers and atomicity, and the dashboard form; `make
    check` passes.

## Non-goals

- Watching or re-syncing changes from the original folder after creation.
- Supporting symlinks, sockets, devices, or submodules inside folder sources.
- Using the original folder in place for execution.
- Changing the GitHub clone behavior or adding other source-control providers.
- Editing the source after project creation; only the name is editable.

## Risks

- Folder copies can be large; the bounds fail closed and no project is created.
- Force-adding ignored files can include build output or local secrets in the
  disposable workspace; the workspace is private and ignored by git, but users
  must choose folders deliberately.
- The stored source path can become stale if the original folder moves; the
  workspace copy remains usable.
- Git must be available for folder import because the local repository backs
  worktree execution.

## Validation

- Local gate (2026-11-16): `make check` passed (fmt-check, clippy, tests,
  audit, deny).
- Domain tests cover absolute-path validation and the folder source kind.
- `tests/db_projects.rs` covers schema v12, folder round-tripping, and
  migration preservation.
- Workspace tests cover copy fidelity, `.git` exclusion, initial commit,
  empty folders, symlink rejection, overlap rejection, depth bounds, and
  destination safety.
- `tests/cli_projects.rs` covers folder creation without GitHub settings,
  missing/relative paths, mutual exclusion, and the copied workspace.
- The dashboard test creates a folder project and checks the local repository.
- Hosted CI has not been run in this session; the plan stays `In progress`
  until it is green.

## Task checklist

- [x] **F00-01** Write ADR-0054; update the registry and `AGENTS.md`.
- [x] **F01-01** Add `FolderSource` and `ProjectSource` domain types with tests.
- [x] **F02-01** Add schema v12 and project read/write support for
  `folder_source`.
- [x] **F03-01** Add the workspace folder-import port and adapter with copy,
  bounds, git initialization, and atomicity.
- [x] **F04-01** Add the app create path for `ProjectSource` and update project
  overviews and memories.
- [x] **F05-01** Add `--folder-source`, mutual exclusion, and CLI rendering.
- [x] **F06-01** Add the dashboard source choice and display.
- [x] **F07-01** Update the specification, CLI reference, README, playbook,
  data model, retention, and security docs.
- [x] **F08-01** Run `make check` and fix every failure.
- [ ] **F08-02** Push the branch, confirm hosted CI is green, and set the
  status to `Done`.
