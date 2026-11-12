# Data Model

This document is the conceptual data model for VLab. It describes entities,
relationships, and invariants without SQL or column-level implementation. Keep
it aligned with every migration and with
[`TECHNICAL-SPECIFICATIONS.md`](TECHNICAL-SPECIFICATIONS.md) §11.2. The physical
schema is defined by `migrations/` and recorded in the ADRs.

## Overview

```text
Laboratory 1 ─── * Project 1 ─── * Request 1 ─── * Story
     │                │
     │                └── * Memory (project scope)
     ├── * Membership * ─── Agent 1 ─── * Memory (agent scope)
     └── * Memory (laboratory scope)

Event, Interaction: execution metadata, not domain ownership edges.
```

## Entities

### Laboratory

A named workspace. It owns projects and has agent memberships. It has at most
one member with the `lead` role (the Tech Lead). A laboratory may initially be
empty; structural validation runs separately and never blocks creation.

### Agent

A reusable profile: name, role (`lead`, `dev`, `analyst`, `qa`), description,
skills, a provider/model pair copied from settings at creation time, an
optional CV, and an archive timestamp. An agent can belong to several
laboratories through memberships. Archiving removes memberships and preserves
the profile and historical story references.

### Membership

A many-to-many relationship between a laboratory and an agent, with a join
timestamp. Membership cascades when either owner is removed.

### Project

A unit of work inside exactly one laboratory. It has a name and an **optional
GitHub source**: a canonical URL `https://github.com/<owner>/<repository>`.
The source is descriptive metadata until a concrete GitHub operation is
implemented; the application performs no GitHub network call in the initial
release.

### Request

An accepted natural-language request for one project. It records the original
content, a status (`TODO`, `PLANNING`, `PLANNED`, `NEEDS_CLARIFICATION`,
`FAILED`), optional bounded clarification questions, a stable failure code and
sanitized explanation, and the planning-lease start. The request is preserved
even when planning fails.

### Story

A planned work item linked to one project and one originating request. It
records title, description, acceptance criteria, required skills, priority,
complexity, optional assignment and rationale, a Kanban state, and a locally
computed due date. The initial release creates `TODO` stories only.

### Memory

User-authored context attached to exactly one scope:

- a laboratory;
- a project;
- an agent profile.

A memory has bounded text and a creation timestamp. It is immutable in the
initial release (delete and recreate to change it). Memories are untrusted
data: they are selected and delimitated for prompts, never executed and never
treated as permission. Agent memories survive archival but are excluded from
prompts while the agent is archived.

### Event

A typed audit record: run/action identifiers, kind, optional entity reference,
outcome, stable error code, duration, attempt, provider/model, optional schema
versions, and timestamp. It has no free-text field and never stores request,
prompt, response, CV, memory, or credential content.

### Interaction

Execution metadata for a model-backed operation: agent, kind, provider, model,
outcome, error code, duration, token usage, and timestamp. It stores no
content. It is added by WP-6.

## Relationships and cardinalities

| Relationship | Cardinality | Behavior |
|---|---|---|
| Laboratory → Project | one to many | Projects belong to exactly one laboratory; deleting the laboratory deletes its projects. |
| Laboratory ↔ Agent | many to many | Memberships; deleting either side removes the membership. An archived agent has no active memberships. |
| Project → Request | one to many | Requests are scoped to one project; deleting the project deletes them. |
| Request → Story | one to many | Stories retain the originating request; deleting the request cascades. |
| Project → Story | one to many | A story belongs to the project of its request. |
| Agent → Story assignment | one to many, optional | Stories may be assigned to an active member; archiving preserves the reference. |
| Laboratory/Project/Agent → Memory | one to many | Exactly one owner scope per memory; laboratory and project deletion cascade, agent archival retains. |
| Any entity → Event | optional reference | Events are append-only metadata, not ownership edges. |
| Agent/Operation → Interaction | one to many | Added in WP-6; metadata only. |

## Invariants

- A project belongs to exactly one laboratory and never switches context
  silently.
- A request belongs to exactly one project; a story belongs to exactly one
  project and one request.
- A story assignment is `NULL` or a member of the project's laboratory.
- A laboratory has at most one active `lead`; check-lab reports stored
  violations.
- A memory has exactly one owner scope and a bounded, control-free content.
- Archives are additive: archived agents remain resolvable; deletion of
  laboratories/projects is a cascade.
- Timestamps are UTC; due dates are calendar dates computed locally.
- Credentials are **not entities**. They live only in the private
  `settings.yaml` (`credentials.deepseek_api_key`, optional
  `credentials.github_api_key`) and are never stored in the database.

## Identifier and lifecycle conventions

All persistent entities use a short 8-character lowercase hexadecimal
identifier. Requests and stories have explicit status/state machines;
laboratories, projects, agents, and memories do not imply execution state.
Only the initial planning path creates stories, and it creates them as `TODO`.

## Sensitive data and retention

The model stores user-provided project content and bounded memories. It never
stores provider credentials, prompts, response bodies, or interaction content.
Retention, cascade behavior, and the lack of encryption at rest are documented
in [`data-retention.md`](data-retention.md). The GitHub source URL is not a
secret; the GitHub key remains a private settings credential.

## Keeping this document aligned

A migration that adds, removes, or changes an entity, relationship, ownership
edge, cascade, or sensitive field must update this document in the same change.
If the change is material, record it in an ADR and update the registry.
