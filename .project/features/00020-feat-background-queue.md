# Feature 00020 — Background request queue

| Field | Value |
|---|---|
| Identifier | `00020` |
| Branch | `feat/background-queue` |
| Status | `Done` |
| Date | 2026-11-17 |
| Related ADRs | ADR-0061 (queue), ADR-0049 (startup resume), ADR-0050 (bounded execution) |
| Scope | Immediate TODO intake and a serial in-process Tech Lead worker |

## Goal

Make request intake immediate. A submitted request is persisted as `TODO` and
returns at once; a bounded background Tech Lead drains the queue oldest-first,
running the existing planning and worktree workflow one request at a time. Two
consecutive requests are accepted without waiting, and the second visibly waits
in `TODO`.

## Acceptance criteria

1. Dashboard submission and `--new-request` persist a `TODO` request and return
   without a provider call or workflow; the request identifier is reported.
2. The `TODO` column shows queued requests as `REQUEST` cards before stories,
   with `TODO`/`PLANNING` status and a read-only detail on `Enter`.
3. One in-process worker drains the active laboratory's queue oldest-first,
   one request at a time, using a separate SQLite connection and no terminal
   access.
4. A second queued request remains `TODO` while the first is planned; when the
   first completes, the worker picks the second without operator action.
5. Hidden `--run-queue` and `make run-queue` drain at most eight requests and
   exit; `--new-request` itself never waits.
6. Failures are isolated per request; a missing provider configuration leaves
   the request in `TODO`; stale `PLANNING` recovery is unchanged.
7. The dashboard refreshes on worker events and remains responsive during
   planning and workflow execution; quitting cancels the worker token.
8. No schema, dependency, provider-contract, or trust-boundary change; new
   events are content-free.
9. Tests cover enqueue-without-provider, queue ordering, serial processing,
   Kanban request cards, worker event handling, CLI enqueue/drain, and the
   no-secret rules; `make check` passes.

## Non-goals

- A daemon, service, or work that survives process exit.
- Parallel planning, multiple workers, or model-chosen priority.
- Retrying a failed request automatically.
- Changing planning prompts, story schema, or workflow budgets.
- Editing a queued request before it is planned.

## Risks

- The UI and worker use two SQLite connections; short lock waits are possible
  and must remain bounded by the existing busy timeout.
- A long provider call keeps the worker busy; later requests visibly wait in
  `TODO`, which is the intended behavior.
- Quitting during a provider call leaves in-flight work to the process exit and
  stale-lease recovery on the next start.
- Malformed queued content is revalidated before planning and fails safely.
- Request cards expose user-authored text; they are sanitized and read-only.

## Validation

- Local gate (2026-11-17): `make check` passed (fmt-check, clippy, tests,
  audit, deny).
- `tests/queue.rs` proves enqueue without a provider call, deterministic
  oldest-first ordering, the second request remaining `TODO`, serial planning,
  and secret rejection before persistence.
- `tests/cli_new_request.rs` proves `--new-request` returns a `TODO` row with
  no stories and `--run-queue` creates the linked stories.
- `src/cli/tui/mod.rs` tests prove the `REQUEST` card, read-only detail, and
  content-free worker status events; `tests/cli_tui.rs` exercises the
  background worker end to end through the prompt.
- Hosted CI is green on commit `7c19563`.

## Task checklist

- [x] **F00-01** Write ADR-0061; update the registry and `AGENTS.md` links.
- [x] **F01-01** Split enqueue from planning and add the queue read model.
- [x] **F02-01** Add the SQLite queue queries and `request_queued` event.
- [x] **F03-01** Make the TUI enqueue immediately and run the serial worker.
- [x] **F04-01** Show queued requests as `REQUEST` cards in the Kanban TODO.
- [x] **F05-01** Add `--run-queue` and `make run-queue`; change `--new-request`.
- [x] **F06-01** Update the specification, planning, CLI reference, README,
  observability, security, and data-retention notes.
- [x] **F07-01** Add application, database, TUI, and CLI tests.
- [x] **F08-01** Run `make check` and fix every failure.
- [x] **F08-02** Push the branch, confirm hosted CI is green, and set the
  status to `Done`.
