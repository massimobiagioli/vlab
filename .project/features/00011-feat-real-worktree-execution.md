# Feature 00011 — Real worktree execution

| Field | Value |
|---|---|
| Identifier | `00011` |
| Branch | `feat/real-worktree-execution` |
| Status | `Done` |
| Date | 2026-11-13 |
| Related ADRs | ADR-0050 (real execution), ADR-0049 (startup resume), ADR-0032 (workspace), ADR-0043 (Kanban detail) |
| Scope | Replace the simulated workflow with real bounded work in git worktrees |

## Goal

An assigned story is executed for real in a dedicated git worktree of the
project checkout. The agent reads and writes files, runs commands, and commits
on `vlab/<story-id>`. The work summary, review, branch, worktree, and commit are
stored in SQLite and shown on the `DONE` card. No simulated outcomes.

## Acceptance criteria

1. A pending story creates `.workspace/.worktrees/<story-id>` on branch
   `vlab/<story-id>`; stale worktrees and branches are replaced.
2. The agent loop executes validated `list_files`, `read_file`, `write_file`,
   and `run_command` actions inside the worktree only; absolute paths, `..`,
   and symlink escapes are rejected.
3. The loop is bounded: 24 rounds, 120-second call timeout, 8,192 output
   tokens, four actions per round, 15-minute work deadline, 60-second command
   timeout, 64 KiB command output, and one empty-output retry per round.
4. A successful attempt stages and commits all changes with a fixed author and
   records the commit hash; a bound that stops the loop also commits partial
   work with an explicit summary. An unchanged worktree fails the attempt
   without an outcome.
5. Three failed attempts set the story `BLOCKED` and record a content-free
   `workflow_failed` event.
6. The QA or Tech Lead review receives the summary, changed files, and a
   bounded diff and returns `review_outcome` plus `accepted`. An accepted
   review stores the outcome and sets `DONE`; a rejected review stores it with
   `review_accepted = 0` and sets `BLOCKED`.
7. The `DONE` dialog shows branch, worktree, commit, work, review, reviewer,
   and completion time. No simulation note remains.
8. Schema v11 adds `review_accepted`; v10 adds `stories.execution_attempts` and
   `story_outcomes.branch`, `worktree_path`, `commit_sha`.
9. Tests cover action validation, confinement, command bounds, worktree
   creation/commit, the loop, attempt exhaustion, the migration, and detail
   rendering; `make check` and hosted CI pass.

## Non-goals

- OS-level sandboxing, containers, or network isolation.
- Automatic merge to the project default branch; the human merges the story
  branch after review.
- Parallel story execution; stories remain sequential.
- Provider tool-calling protocol changes; actions use the existing JSON-mode
  provider contract.

## Risks

- Arbitrary commands run with the operator's privileges. Documented as a
  residual security risk; VLab must run on a trusted machine/account.
- Provider usage can grow with rounds; the 12-round and 15-minute bounds cap one
  attempt, and three attempts cap a story.
- A repository whose tests require network or secrets will fail; the agent sees
  the error and may still commit partial work.
- Worktrees and branches accumulate until cleaned.

## Validation

- Local gate (2026-11-13): `make check` passed (fmt, clippy, tests, audit,
  deny).
- Hosted CI runs `43455969285` and `43456825125` passed on Ubuntu, macOS, and
  audit/deny.
- Unit tests for the action schema, path validation, observation bounds, and
  command output.
- Git worktree test against a temporary repository: create, write, commit,
  assert branch and commit.
- Scripted-provider loop tests: actions then `done`, provider failure, attempt
  exhaustion, reviewer fallback.
- Workflow test: file exists, story `DONE`, outcome fields stored.
- Migration test for v10.
- Hosted CI is required before the feature is marked `Done`.

## Task checklist

- [x] **F00-01** Write ADR-0050 and update the registry, `AGENTS.md`, and the
  security/architecture/specification documents.
- [x] **F01-01** Add the action schema, validation, and observation bounds with
  tests.
- [x] **F01-02** Add the confined action runner and the git worktree adapter
  with tests.
- [x] **F02-01** Implement the bounded work loop, commit, attempts, review, and
  outcome persistence.
- [x] **F02-02** Add schema v10 and update the store, migration, and policy
  tests.
- [x] **F03-01** Show branch, worktree, and commit in the `DONE` detail.
- [x] **F03-02** Remove the simulation wording and update the CLI reference,
  README, playbook, and docs.
- [x] **F04-01** Reset the two local `DONE` cards to `TODO` without outcomes.
- [x] **F05-01** Run `make check` and fix every failure.
- [x] **F05-02** Push, confirm hosted CI is green, and set the status to `Done`.
