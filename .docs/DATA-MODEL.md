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

A named workspace. It owns projects and has agent memberships. It has an
optional single-line description and at most one member with the `lead` role
(the Tech Lead). A laboratory may initially be
empty; structural validation runs separately and never blocks creation.

### Agent

A reusable profile: name, role (`lead`, `dev`, `analyst`, `qa`), description,
skills, a provider/model pair copied from settings at creation time, an
optional CV, and an archive timestamp. An agent can belong to several
laboratories through memberships. A profile may exist without memberships; the
preset catalog uses this to keep profiles available for later laboratory
population. Duplicating a profile creates an independent active profile with a
new identifier and optional copies of the source agent-scope memories; the
source is never modified (ADR-0058). Archiving removes memberships and preserves
the profile and historical story references.

### Membership

A many-to-many relationship between a laboratory and an agent, with a join
timestamp. Membership cascades when either owner is removed.

### Project

A unit of work inside exactly one laboratory. It has a name and at most one
**source**:

- `github`: a canonical URL `https://github.com/<owner>/<repository>`.
  Creating the project clones it into `.workspace/<project-id>` (ADR-0032).
  No other GitHub network call is performed.
- `folder`: an absolute local folder path without version control. Creating the
  project copies the folder into `.workspace/<project-id>` and initializes a
  local git repository only inside that copy (ADR-0054). The source folder is
  never modified.
- `none`: a legacy project with no source and no checkout.

The source fields are metadata; the copied files stay in the disposable
workspace and are not stored in the database.

### Request

An accepted natural-language request for one project. It records the original
content, a status (`TODO`, `PLANNING`, `PLANNED`, `NEEDS_CLARIFICATION`,
`FAILED`), optional bounded clarification questions, a stable failure code and
sanitized explanation, the planning-lease start, and the agent that planned it
(`planned_by_agent_id`, schema v13). The request is preserved even when planning
fails. `TODO` is the durable queue: intake writes it and returns, and one
background Tech Lead selects the oldest `TODO` for the active laboratory,
oldest first, before marking it `PLANNING` (ADR-0061). New planning resolves
questions autonomously in bounded follow-up calls; `NEEDS_CLARIFICATION`
remains a readable status for legacy rows (ADR-0066).

### Project interaction history

A read model over requests, stories, story outcomes, and agents. It produces
`planning`, `work`, and `review` entries with participants, reference, outcome,
time, and a bounded recap. It stores no duplicate content and is not a
free-form agent chat (ADR-0056).

### Schedule

A recurring project task owned by one active laboratory member. It records a
title, a single-line task request, a daily/weekly/monthly interval, an enabled
flag, `next_run_at`, `last_run_at`, an optional last request, and an optional
stable error code. A due schedule creates a normal bounded request; it never
executes model-chosen commands directly (ADR-0057).

### Story

A planned work item linked to one project and one originating request. It
records title, description, acceptance criteria, required skills, priority,
complexity, optional assignment and rationale, a Kanban state, and a locally
computed due date. Planning creates assigned stories in `IN PROGRESS` and
unassigned stories in `TODO` (ADR-0047). The bounded real workflow may execute a
story in a worktree, advance it to `IN_REVIEW`, and then `DONE`. A story that
fails execution or review stores a bounded `tech_lead_context` and a
`remediation_attempts` counter; the Tech Lead may reassign it, add context, and
reset the execution attempts for a bounded number of cycles. After the last
cycle the story closes `DONE` with a recorded compromise, and one linked
`TODO` story records the residual technical debt through `debt_of_story_id`
(ADR-0069). The automatic workflow never sets `BLOCKED`.

### Story outcome

One optional row per completed story, linked by `story_id`. It stores the work
summary, review text, review decision, reviewer agent, branch, worktree path,
commit hash, and completion timestamp (ADR-0050). It also records develop
integration state: when the commit entered the local `develop`, whether the
remote push is complete (or unnecessary because there is no remote), and a
content-free error code for retry (ADR-0070). The row is project data, not audit
metadata; the text is untrusted and displayed sanitized. Deleting the story
cascades the outcome.

### Memory

Application-managed context attached to exactly one scope:

- a laboratory;
- a project;
- an agent profile.

A memory has a bounded markdown file, a relative path in the data directory,
and a creation timestamp. The database stores only the path and scope. Its
scope and creation time are immutable; laboratory and project content may be
edited from the floor (ADR-0046). Structural memories are deterministic facts
written when a laboratory, agent, or project is created; a successful plan adds
a bounded markdown planning recap and an accepted card adds a completion recap
(ADR-0068). Memories are untrusted data: they are
selected and delimitated for prompts, never executed and never treated as
permission. Agent memories survive archival but are excluded from prompts while
the agent is archived.

### Document

A markdown file under `data/documents/<scope>/<folder>/<document-id>.md` with a
database reference containing its scope, title, and relative path. Documents are
imported with `make import`; they are project data and are never executed.
Deleting the owning lab or project cascades the reference.

### Entity folder

A stable, unique folder name for one lab, project, or agent under both the
memories and documents trees. The name is a sanitized entity name, suffixed with
the entity identifier on collision. The mapping is stored in `entity_folders`.

### Event

A typed audit record: run/action identifiers, kind, optional entity reference,
outcome, stable error code, duration, attempt, provider/model, optional schema
versions, and timestamp. It has no free-text field and never stores request,
prompt, response, CV, memory, or credential content.

### Floor read model

Derived, never persisted: the active laboratory and project, active agents,
their open tasks, the fixed Project Lab/War Room/Lounge assignment, and a
presentation tick. The tick and agent positions are view state only; they are
not stored, audited, or sent to the provider (ADR-0062).

### Interaction

Execution metadata for a model-backed operation: agent, kind, provider, model,
outcome, error code, duration, token usage, and timestamp. It stores no
content. It is added by WP-6.

## Relationships and cardinalities

| Relationship | Cardinality | Behavior |
|---|---|---|
| Laboratory → Project | one to many | Projects belong to exactly one laboratory; deleting the laboratory deletes its projects. |
| Laboratory ↔ Agent | many to many | Memberships; deleting either side removes the membership. An archived agent has no active memberships. |
| Project ↔ Agent | many to many | Project roster; deleting either side removes the assignment. Planning uses the roster when non-empty. |
| Project → Request | one to many | Requests are scoped to one project; deleting the project deletes them. |
| Request → Story | one to many | Stories retain the originating request; deleting the request cascades. |
| Project → Story | one to many | A story belongs to the project of its request. |
| Agent → Story assignment | one to many, optional | Stories may be assigned to an active member; archiving preserves the reference. |
| Story → Story outcome | one to zero or one | A completed story stores one work/review outcome; deleting the story cascades it. |
| Laboratory/Project/Agent → Memory | one to many | Exactly one owner scope per memory; laboratory and project deletion cascade, agent archival retains. |
| Any entity → Event | optional reference | Events are append-only metadata, not ownership edges. |
| Agent/Operation → Interaction | one to many | Added in WP-6; metadata only. |

## Invariants

- A project belongs to exactly one laboratory and never switches context
  silently.
- A request belongs to exactly one project; a story belongs to exactly one
  project and one request.
- A story assignment is `NULL` or an agent in the project roster; without a roster, the laboratory roster applies.
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
