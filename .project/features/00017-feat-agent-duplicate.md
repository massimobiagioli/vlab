# Feature 00017 — Agent duplication

| Field | Value |
|---|---|
| Identifier | `00017` |
| Branch | `feat/agent-duplicate` |
| Status | `Done` |
| Date | 2026-11-17 |
| Related ADRs | ADR-0058 (duplication), ADR-0020 (agent validation), ADR-0022 (memories) |
| Scope | Duplicate an agent profile with optional agent-memory preservation |

## Goal

Let an operator duplicate an agent from the Agents dashboard. The copy gets a
new identifier and name, keeps the role, description, skills, CV, provider, and
model, copies compatible laboratory memberships, and can optionally preserve
the source agent's memories.

## Acceptance criteria

1. The Agents dashboard offers `c` **Duplicate** for the selected active agent.
2. The duplicate form asks for the new name (default `<source> copy`) and a
   `Preserve memory` choice (`No` default, `Yes` optional).
3. A duplicate receives a new identifier; role, description, skills, provider,
   model, and CV are copied; it is always active.
4. Memberships are copied only where the one-Tech-Lead rule allows; a duplicated
   lead gets no memberships and the result reports the skipped labs.
5. With `Preserve memory = Yes`, every agent-scope memory is copied with a new
   identifier into the duplicate's scope, respecting the 20-memory and
   2,000-character limits and rejecting the current provider credential.
6. Profile, memberships, and copied memories are inserted in one transaction;
   a failure writes nothing.
7. The result reports the new identifier, copied/skipped memberships, and copied
   memory count; an `agent_created` event records the new profile.
8. Tests cover the application copy, lead filtering, memory copy and cap,
   credential rejection, database atomicity, and the dashboard form.
9. `make check` passes; hosted CI is green before the plan is `Done`.

## Non-goals

- A CLI duplication flag.
- Editing the duplicate in the same operation.
- Copying laboratory/project memories or project assignments.
- Copying stories, outcomes, or interaction history.
- Preserving memory timestamps or identifiers.

## Risks

- Duplicating a lead creates a catalog profile without memberships; the operator
  must use Populate to assign it where no lead exists.
- Preserved memories can contain sensitive project context; the choice is
  explicit and the credential rule still applies.
- A full source scope fills the duplicate's memory scope; the structural
  creation memory is best-effort and will not exceed the cap.

## Validation

- Local gate (2026-11-17): `make check` passed (fmt-check, clippy, tests,
  audit, deny).
- `tests/agent_duplicate.rs` covers profile copying, membership filtering for a
  duplicated lead, optional memory copying, credential rejection, name
  validation, unknown sources, and no-write-on-failure.
- The dashboard test opens Duplicate from Agents, selects `Preserve memory`,
  submits, and verifies the new profile and copied memory.
- Hosted CI is green on commit `6fad608`.

## Task checklist

- [x] **F00-01** Write ADR-0058; update the registry and `AGENTS.md`.
- [x] **F01-01** Add the duplicate use case and `AgentCopyStore` port.
- [x] **F02-01** Add the atomic SQLite implementation for profile, memberships,
  and memories.
- [x] **F03-01** Add the dashboard form, action, help, and result reporting.
- [x] **F04-01** Update the specification, CLI reference, README, playbook,
  data model, retention, and security docs.
- [x] **F05-01** Add application, database, and dashboard tests.
- [x] **F06-01** Run `make check` and fix every failure.
- [x] **F06-02** Push the branch, confirm hosted CI is green, and set the
  status to `Done`.
