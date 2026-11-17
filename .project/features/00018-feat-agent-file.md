# Feature 00018 — Agent from markdown definition

| Field | Value |
|---|---|
| Identifier | `00018` |
| Branch | `feat/agent-file` |
| Status | `Done` |
| Date | 2026-11-17 |
| Related ADRs | ADR-0060 (agent definitions), ADR-0020 (validation), ADR-0059 (file store) |
| Scope | Read an agent from a markdown frontmatter file and create the profile |

## Goal

Create an agent from a tracked or local markdown definition with
`make agent-create-from-file SRC=<file>`. The frontmatter defines the profile;
the body is the CV. A usable active laboratory is joined automatically.

## Acceptance criteria

1. A definition is UTF-8 markdown with strict YAML frontmatter fields `name`,
   `role`, `description`, and `skills`; the body is the optional CV.
2. Name, role, description, skills, CV, and file size follow the existing
   domain limits; malformed or oversized files are rejected before storage.
3. The command creates a new active profile with provider/model from settings;
   it never overwrites an existing agent.
4. If the active laboratory exists, the agent joins it; a second lead is
   rejected. Without a usable active laboratory the profile is created without
   membership.
5. The dispatch records `agent_created` and the automatic structural agent
   memory; no provider call is made.
6. `make agent-create-from-file SRC=<file>` wraps the hidden CLI action.
7. Tests cover parser and validation, membership, lead conflict, catalog-only
   creation, and the CLI path.
8. `make check` passes; hosted CI is green before the plan is `Done`.

## Non-goals

- Editing an existing agent from a file.
- Importing memberships for multiple laboratories from the file.
- Storing the definition file as a document.
- Executing or sending markdown instructions to the provider.

## Validation

- Local gate (2026-11-17): `make check` passed (fmt-check, clippy, tests,
  audit, deny).
- `src/infra/agent_definition.rs` tests cover frontmatter, CV, strict fields,
  invalid roles/skills, malformed YAML, BOM, and non-UTF-8 input.
- `tests/agent_definition.rs` covers profile creation, active-laboratory
  membership, the second-lead rejection, catalog-only creation, file
  validation, and the CLI path including the structural memory.
- Hosted CI is green on commit `6fad608`.

## Task checklist

- [x] **F00-01** Write ADR-0060; update the registry and `AGENTS.md`.
- [x] **F01-01** Add the markdown frontmatter parser with tests.
- [x] **F02-01** Add the create-from-definition use case.
- [x] **F03-01** Wire the hidden CLI action and Makefile target.
- [x] **F04-01** Update the specification, CLI reference, README, and playbook.
- [x] **F05-01** Add parser, application, and CLI tests.
- [x] **F06-01** Run `make check` and fix every failure.
- [x] **F06-02** Push the branch, confirm hosted CI is green, and set the
  status to `Done`.
