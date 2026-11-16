# Feature 00013 — Preset agent catalog and laboratory populate

| Field | Value |
|---|---|
| Identifier | `00013` |
| Branch | `feat/preset-agents` |
| Status | `In progress` |
| Date | 2026-11-16 |
| Related ADRs | ADR-0052 (preset catalog), ADR-0053 (populate screen), ADR-0020 (agent validation), ADR-0045 (read-only floor) |
| Scope | Tracked ten-agent catalog, hidden loader flag, Makefile target, and a two-pane laboratory populate screen |

## Goal

Ship thirty fictional preset agents inspired by German, Dutch, English,
Japanese, Greek, and Slavic football names from the 1960s–1980s, including
female profiles, stored in `assets/agents.yaml`, and make them available to any
laboratory through an intuitive two-pane **Populate** screen reachable from the
Laboratories dashboard and the Lab floor. A developer can load the catalog with
`make load-preset`.

## Acceptance criteria

1. `assets/agents.yaml` is tracked, versioned, and contains thirty fictional
   agents with mixed roles and skills: one Tech Lead; backend, frontend,
   platform, mobile, data, and full-stack developers; designers and researchers;
   business, data, and security analysts; and test, accessibility, and security
   testers. The roster mixes German, Dutch, English, Japanese, Greek, and Slavic
   names from the 1960s–1980s, including female profiles. Every non-lead has at
   least one skill.
2. `vlab --load-preset <file>` validates the whole file before opening the
   database, creates only agent profiles, copies provider/model from settings,
   skips existing names case-insensitively, and is atomic.
3. `make load-preset` loads the tracked catalog into `.data/` and is listed in
   `make help` and the README.
4. The Laboratories dashboard shows `[o] Populate` for the selected
   laboratory, and the Lab floor shows the same key for the active laboratory;
   neither requires changing the active selection just to edit a roster.
5. The screen shows **Available agents** and **In this laboratory** side by
   side, with names, roles, counts, selection, and scrolling.
6. `Tab`/`←→` switches pane, `↑↓` moves, `Enter`/`Space` moves an agent, `v`
   opens the selected agent's full profile, `s` saves, and `Esc` cancels with a
   discard confirmation when dirty.
7. Saving requires at least one member and exactly one active Tech Lead; an
   invalid roster writes nothing and shows an actionable message.
8. Membership changes are applied in one transaction, unchanged `joined_at`
   values are preserved, and archived memberships are not removed.
9. A successful save records one `lab_populated` audit event with no content.
10. Tests cover the YAML parser, the catalog loader, the populate use case, the
    database diff, and the dashboard screen; `make check` passes.

## Non-goals

- Editing agent profiles or deleting agents from the populate screen.
- Assigning agents to projects from the populate screen.
- Changing the read-only floor.
- Creating laboratories from the preset file.
- Live synchronization with another process editing the same database.

## Risks

- A long catalog needs scrolling; the screen uses a simple offset and a visible
  count.
- A save with unsaved edits in another dashboard session could remove a member
  added elsewhere. The local CLI is single-user; reloading before editing is the
  documented behavior.
- Requiring exactly one lead prevents saving a deliberately lead-less
  laboratory from this screen; the laboratory check still reports such states
  created elsewhere.

## Validation

- Local gate (2026-11-16): `make check` passed (fmt-check, clippy, tests,
  audit, deny).
- `tests/cli_presets.rs` covers the thirty-profile tracked catalog,
  idempotency, missing and malformed files, and the no-database-before-
  validation rule.
- `tests/db_populate.rs` covers the membership diff, preserved `joined_at`, and
  archived membership preservation.
- `tests/cli_tui.rs` opens Populate from the Laboratories dashboard, moves two
  agents, saves, and verifies both panes and the database membership.
- The dashboard test opens the `v` profile dialog for a selected agent and
  checks the full description and skills.
- Hosted CI has not been run in this session; the plan stays `In progress`
  until it is green.

## Task checklist

- [x] **F00-01** Write ADR-0052 and ADR-0053; update the registry and
  `AGENTS.md`.
- [x] **F01-01** Add the preset domain types, YAML parser, and catalog loader
  with tests.
- [x] **F01-02** Add `--load-preset` and the `make load-preset` target.
- [x] **F02-01** Add `all_agents` and `update_lab_memberships` ports and the
  SQLite implementation with tests.
- [x] **F02-02** Add the populate use case: partition, diff, and lead
  validation with tests.
- [x] **F03-01** Add the two-pane TUI screen, entry key, save, cancel, help,
  and dashboard tests.
- [x] **F03-02** Add the `v` profile-preview action with a dashboard test.
- [x] **F04-01** Add the thirty-agent `assets/agents.yaml` and validate it in a
  test.
- [x] **F05-01** Update the CLI reference, specification, README, playbook, and
  observability docs.
- [x] **F06-01** Run `make check` and fix every failure.
- [ ] **F06-02** Push the branch, confirm hosted CI is green, and set the status
  to `Done`.
