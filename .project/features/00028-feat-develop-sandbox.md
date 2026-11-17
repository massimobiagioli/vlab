# Feature 00028 — Develop integration and testable sandbox

| Field | Value |
|---|---|
| Identifier | `00028` |
| Branch | `feat/develop-sandbox` |
| Status | `In progress` |
| Date | 2026-11-17 |
| Related ADRs | ADR-0070 (integration), ADR-0032 (clone), ADR-0050 (workflow), ADR-0069 (DONE closure) |
| Scope | DONE → `develop` merge and push, `.sandbox/<project-id>`, startup/manual sync |

## Goal

Every DONE card is integrated into the project's `develop` branch and pushed to
its GitHub remote, while `.sandbox/<project-id>` always contains a testable
checkout of `develop` for every project in the active laboratory.

## Acceptance criteria

1. A DONE story with a branch and commit is merged into `develop` with
   `--no-ff`; `develop` is created from `origin/develop`, the remote default
   branch, or the base `HEAD`.
2. When `origin` exists, `develop` is pushed without force; a push failure
   leaves the card DONE, records a content-free event, and is retried on the next
   startup or `--sync-sandbox`.
3. Integration is idempotent: a commit already in `develop` is not merged twice,
   and only a pending push is retried. Conflicts are aborted and recorded.
4. Schema v18 stores `integrated_at`, `push_ok`, and `integration_error` on
   `story_outcomes`; a new execution resets them. Schema v19 marks outcomes that
   predate this feature as `legacy_not_integrated` so historical partial work is
   never published retroactively.
5. `.sandbox/<project-id>` is a disposable clone of `develop` for every project
   in the active laboratory with a GitHub or folder source. Projects without a
   source are skipped.
6. The sandbox is refreshed at startup, after each DONE, and by
   `--sync-sandbox`; the refresh fetches `origin/develop` when possible, resets
   to `develop`, and removes untracked files.
7. `.sandbox/` is private (`0700`), ignored by git, and created at runtime
   because a tracked checkout cannot carry a private directory mode.
8. Push credentials use the askpass child environment only; no force push, no
   URL/token in arguments or logs, bounded timeouts, and confined paths.
9. Tests cover develop creation, merge, push, idempotence, conflict, sandbox
   clone/refresh, startup reconciliation, workflow integration, and the CLI
   action; `make check` passes and hosted CI is green.

## Non-goals

- Merging into `main` or deploying.
- Automatic conflict resolution or force-push.
- GitHub API calls, submodules, LFS, or pull requests.
- Tracking sandbox contents in git.
- Sandboxing project processes (the sandbox is a checkout, not a container).

## Risks

- A failed push is visible but leaves the remote `develop` behind; the next
  startup retries.
- A conflict needs a human; VLab aborts rather than guessing.
- The sandbox reset discards local test edits by design.
- The token is available to the git child process, as with cloning.
- Startup integration/sandbox work adds bounded git latency.

## Validation

- Infrastructure tests use local `file://` bare remotes; no network is needed.
- Application tests assert integration state, events, and sandbox calls with a
  fake repository.
- The workflow test asserts develop and sandbox after an accepted card.
- Migration tests cover schema v18 and the v19 legacy marker.
- `make check` passed; hosted CI pending.

## Task checklist

- [x] **F00-01** Write ADR-0070, amend ADR-0032/ADR-0050, update the registry
  and AGENTS.
- [x] **F01-01** Add schema v18 integration columns, schema v19 legacy marker,
  and repository methods.
- [x] **F02-01** Add the develop integration and sandbox ports and the git
  adapter (merge, create, push, clone, refresh, conflict abort).
- [x] **F03-01** Wire integration into every DONE path and record events.
- [x] **F04-01** Add startup reconciliation and sandbox refresh for all active
  laboratory projects.
- [x] **F05-01** Add the hidden `--sync-sandbox` action and output.
- [x] **F06-01** Add infrastructure, application, workflow, migration, and CLI
  tests.
- [x] **F07-01** Update documentation, retention notes, `.gitignore`, Makefile,
  and the Tech Lead skill.
- [x] **F08-01** Run `make check`.
- [ ] **F08-02** Push, confirm hosted CI is green, and set the status to `Done`.
