# Feature 00016 — Recurring project schedules

| Field | Value |
|---|---|
| Identifier | `00016` |
| Branch | `feat/schedules` |
| Status | `In progress` |
| Date | 2026-11-16 |
| Related ADRs | ADR-0057 (schedules), ADR-0050 (bounded execution), ADR-0044 (request intake) |
| Scope | Project schedules storage, dashboard page, bounded due-run, and CLI/Makefile runner |

## Goal

Let an operator create recurring project tasks owned by an agent, view and
manage them on a separate Schedules page, and run due schedules through the
existing bounded request and workflow path. Examples: a weekly security review
or a monthly dependency/framework update.

## Acceptance criteria

1. Schema v14 adds the `schedules` table; schedules cascade with their project
   and reference an agent, a last request, and a stable last error code.
2. A schedule has a title, a single-line task request (1–2,000 characters), a
   daily/weekly/monthly interval, an enabled flag, next/last run timestamps, and
   an owner who is an active laboratory member.
3. The Projects dashboard `s` opens a Schedules page with the selected project's
   schedules: title, owner, interval, next run, status, and last result.
4. The page creates a schedule with a form, pauses/resumes the selected
   schedule, deletes it after confirmation, and opens a scrollable detail with
   the full task request.
5. Due schedules create a normal bounded request and run the existing workflow;
   at most three run per pass, the owner is named in the request, and the Tech
   Lead still assigns stories.
6. An inactive owner, planning failure, workflow failure, or cancellation
   records a stable code, advances the next run, and does not retry in a loop.
7. Due schedules run on dashboard startup and with hidden `--run-schedules`
   (also `make run-schedules`). The page itself never calls the provider.
8. Creating, pausing/resuming, and deleting a schedule records typed audit
   events with the schedule identifier only.
9. Tests cover schema, creation, due ordering, run advancement, inactive
   owners, pause/resume, deletion, dashboard rendering, form creation, and CLI
   argument parsing.
10. `make check` passes; hosted CI is green before the plan is `Done`.

## Non-goals

- A background daemon or in-dashboard timer.
- Arbitrary cron expressions or intervals shorter than one day.
- Model-chosen schedule creation or schedule execution.
- Storing prompts, provider responses, command output, or file content.
- Editing the task text/owner after creation (delete and recreate instead).
- Cross-project or laboratory-wide schedules.

## Risks

- Startup execution can incur provider cost; the batch is capped at three and
  the first run is one full interval after creation.
- Missed intervals run once on the next pass, not once per missed interval.
- The owner is advisory; the Tech Lead may assign resulting work to another
  agent.
- A schedule can create a request that fails planning or workflow; the page
  shows the last request status/error for follow-up.

## Validation

- Local gate (2026-11-16): `make check` passed (fmt-check, clippy, tests,
  audit, deny).
- `tests/schedules.rs` covers due-run creation and advancement, inactive
  owners, active-member ownership, pause/resume, and deletion.
- `src/cli/tui/schedules.rs` tests cover the list, navigation, and full detail;
  the dashboard test creates, pauses, and deletes a schedule.
- `src/cli/args.rs` covers `--run-schedules`.
- Hosted CI has not been run in this session; the plan stays `In progress`
  until it is green.

## Task checklist

- [x] **F00-01** Write ADR-0057; update the registry and `AGENTS.md`.
- [x] **F01-01** Add `ScheduleId`, the schedule domain types, and schema v14.
- [x] **F02-01** Add the schedule store, due query, and run advancement.
- [x] **F03-01** Add the Schedules page, form, pause/resume, delete, and detail.
- [x] **F04-01** Wire startup and `--run-schedules`/`make run-schedules`.
- [x] **F05-01** Update specification, CLI reference, README, playbook, data
  model, retention, observability, and security docs.
- [x] **F06-01** Add database, run, dashboard, and argument tests.
- [x] **F07-01** Run `make check` and fix every failure.
- [ ] **F07-02** Push the branch, confirm hosted CI is green, and set the
  status to `Done`.
