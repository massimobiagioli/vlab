# Data Retention and Sensitive Data

This policy covers what VLab stores, where, for how long, and what it never
stores. It is required by `SECURITY.md` (Secrets and Data Privacy) and
implemented by schema v1 ([ADR-0016](adr/adr-0016.md)). WP-4 and WP-5 extend
the schema; they must update this document before adding durable data.

## Summary

- Data is local to the operating-system user. VLab runs no server and has no
  account or synchronization model.
- Provider credentials are stored only in the private `settings.yaml`. They are
  never stored in the database, logs, errors, events, prompts, or command
  output ([ADR-0030](adr/adr-0030.md)).
- Prompts, provider response bodies, full chat transcripts, and CVs of people
  outside the laboratory are not stored.
- SQLite is **not encrypted at rest**. Use full-disk encryption (FileVault on
  macOS, BitLocker on Windows, LUKS on Linux) for projects whose data is
  sensitive.
- There is no automatic purge and no built-in backup command. Deleting data is
  explicit (laboratory, project, or agent deletion) or manual.

## Stored data

### `settings.yaml`

Stored in the per-user configuration directory with mode `0600`.

| Field | Why it is stored |
|---|---|
| `provider` | Selects the only supported provider; needed on every provider-backed command. |
| `model` | The provider model identifier the user has chosen. It is not a credential. |
| `source_control.provider` | Selects the source-control provider; currently GitHub only. |
| `credentials.deepseek_api_key` | Credential for planning and questions; sent only in the provider Authorization header. |
| `credentials.github_api_key` | Optional credential used only to authorize GitHub-backed project creation. |

The file is the only place where credentials are stored. It is retained until
the user edits or deletes it; it is not copied into the database, events, or
logs. Its directory is mode `0700` and the file mode `0600`. The tracked
`.data/settings.example.yaml` contains placeholders only, never a real secret.
Errors from the strict parser repeat only the offending field name, never its
value.

### Project checkouts (`.workspace/`)

A project created with `--github-source` is cloned to
`.workspace/<project-id>`. A project created with `--folder-source` is copied
to the same location and receives a local git repository inside the copy. The
checkout contains project files and, for folder sources, the imported copy of
the original folder. It is outside version control and is not part of the
database or the settings file. It is retained until the user removes it;
project deletion does not remove it automatically in the initial release, so
use `make clean-workspace` for an explicit reset. VLab does not back it up or
encrypt it; use disk encryption for sensitive sources.

### Testable sandboxes (`.sandbox/`)

Each project of the active laboratory with a source gets a disposable checkout
of its `develop` branch at `.sandbox/<project-id>` (or
`<data-dir>/sandbox/<project-id>` for non-repository layouts). The sandbox is
private, ignored by version control, and refreshed at startup, after each DONE
card, and by `--sync-sandbox`. A refresh resets the checkout to `develop` and
removes untracked files, so local edits are discarded by design. VLab does not
back it up or encrypt it; use `make clean-sandbox` for an explicit reset.

The GitHub token used for the clone, fetch, and push is passed through a
short-lived private askpass helper and the child process environment. It is
never part of the checkout, its git config, or the sandbox. Folder import reads
no credential and performs no network call.

### SQLite database (`vlab.db`)

Stored in the per-user application-data directory with mode `0600`; the parent
directory is mode `0700`. Schema v1 contains two application tables.

#### `app_state`

| Column | Contents | Why it is stored |
|---|---|---|
| `key` | A state name from a fixed set; currently only `active_lab_id` | Lets separate invocations agree on the active laboratory. |
| `value` | The selected laboratory identifier, or another typed state value | The selection itself; an identifier, not user content. |
| `updated_at` | RFC 3339 UTC timestamp | Makes stale or unexpected state diagnosable. |

`key` is the documented exception to the credential-column policy: it holds a
state name, not a credential. Values are validated by the consumer; an invalid
stored value is treated as no selection and never executed.

#### `events`

Audit events are typed: every field is an enum, an identifier, a number, or a
short validated token.

| Column | Contents | Why it is stored |
|---|---|---|
| `id` | Random 128-bit hex identifier | Uniquely identifies the event. |
| `run_id` | Random 128-bit hex identifier | Groups events from one process invocation. |
| `action_id` | Random 128-bit hex identifier | Groups events from one discrete operation. |
| `kind` | Event kind such as `migration_applied` | Answers what happened. |
| `entity_type`, `entity_id` | Entity class and 8-character identifier | Answers which domain object was affected. |
| `outcome` | `success`, `failure`, `cancelled`, or `denied` | Answers whether the operation succeeded. |
| `error_code` | Stable code such as `storage` | Classifies failures without storing messages. |
| `duration_ms`, `attempt` | Integers | Shows latency and retry behavior. |
| `provider`, `model` | Provider name and model token | Shows which integration ran. |
| `schema_from`, `schema_to` | Integers | Records which migration ran and from where. |
| `created_at` | RFC 3339 UTC timestamp | Orders events. |

There is deliberately no free-text column: request text, prompts, provider
responses, file contents, and credentials cannot be recorded. Diagnostics
follow the same rule ([`OBSERVABILITY.md`](OBSERVABILITY.md)).

### Schema v2: laboratories, agents, and memberships

WP-3 adds three tables ([ADR-0019](adr/adr-0019.md)).

#### `labs`

| Column | Contents | Why it is stored |
|---|---|---|
| `id` | 8-character identifier | Primary key and command operand. |
| `name` | Trimmed user-provided name | Displayed in lists and summaries. |
| `description` | Optional trimmed user-provided text (max 2,000 chars) | Laboratory context shown in the dashboard. |
| `created_at`, `updated_at` | RFC 3339 UTC timestamps | Ordering and diagnosis. |

#### `agents`

| Column | Contents | Why it is stored |
|---|---|---|
| `id` | 8-character identifier | Primary key and command operand. |
| `name` | Trimmed user-provided name | Displayed in lists and details. |
| `role` | `lead`, `dev`, `analyst`, or `qa` | Planning context and validation. |
| `description` | Trimmed user-provided text | Planning context and display. |
| `skills_json` | Bounded JSON array of slug tags | Assignment context and validation. |
| `provider`, `model` | Copies of the settings used at creation | The profile must not change when settings later change (§5.4). They are not credentials. |
| `cv` | Optional user-provided profile text | Describes the agent for the user. It is **never** sent to the provider (A-13). |
| `archived_at` | RFC 3339 UTC timestamp or `NULL` | Preserves historical identity when the agent is archived. |
| `created_at`, `updated_at` | RFC 3339 UTC timestamps | Ordering and diagnosis. |

#### `lab_memberships`

| Column | Contents | Why it is stored |
|---|---|---|
| `lab_id`, `agent_id` | Foreign keys | Establish which agents belong to which laboratories. |
| `joined_at` | RFC 3339 UTC timestamp | Ordering and diagnosis. |

Deleting a laboratory cascades to its memberships. Agent profiles are
preserved because they are reusable across laboratories.

### Schema v3: projects, requests, stories, and memories

WP-4 adds these tables ([ADR-0023](adr/adr-0023.md), [ADR-0022](adr/adr-0022.md)).

#### `projects`

| Column | Contents | Why it is stored |
|---|---|---|
| `id` | 8-character identifier | Primary key and command operand. |
| `lab_id` | Owning laboratory | Enforces §3.2 ownership and scopes every project command. |
| `name` | Trimmed user-provided name | Displayed in lists and the board header. |
| `github_source` | Optional canonical GitHub repository URL (added by schema v4, WP-5) | Project metadata for future GitHub operations; not a credential. |
| `folder_source` | Optional canonical absolute folder path (added by schema v12, ADR-0054) | Records the imported source folder; the copied files stay in `.workspace/`. A project stores at most one of `github_source` and `folder_source`. |
| `created_at`, `updated_at` | RFC 3339 UTC timestamps | Ordering and diagnosis. |

#### `requests`

| Column | Contents | Why it is stored |
|---|---|---|
| `id` | 8-character identifier | Primary key and story link. |
| `project_id` | Owning project | Scopes the request and enables cascade deletion. |
| `content` | Original request text | Traceable input that must survive failed planning (§9.6). |
| `status` | `TODO`, `PLANNING`, `PLANNED`, `NEEDS_CLARIFICATION`, or `FAILED` | Drives the terminal outcome and the pending-intake count. |
| `questions_json` | Bounded clarification questions | Shown when planning needs clarification. |
| `error_code`, `error_message` | Stable code and a sanitized explanation | Explains a failed planning attempt without storing prompts or provider bodies. |
| `planning_started_at` | RFC 3339 UTC timestamp or `NULL` | Detects an active or stale planning lease. |
| `planned_by_agent_id` | Agent identifier or `NULL` (added by schema v13, ADR-0056) | Attributes the planning interaction to the actual Tech Lead even if the lead changes later. |
| `created_at`, `updated_at` | RFC 3339 UTC timestamps | Ordering and diagnosis. |

`TODO` is the durable queue; the request content is retained until the request
is planned or fails and then remains for traceability (ADR-0061). Queued
requests are not duplicated into a second table or into events.

#### `schedules` (schema v14, ADR-0057)

| Column | Contents | Why it is stored |
|---|---|---|
| `id` | 8-character identifier | Primary key and page operand. |
| `project_id` | Owning project | Scopes the schedule and cascades on project deletion. |
| `agent_id` | Owner agent | Names the intended task owner; the Tech Lead still assigns resulting stories. |
| `title` | Short task title | Displayed in the page and included in the generated request. |
| `request_text` | Operator-provided single-line task request, 1–2,000 characters | Becomes the normal bounded request when due. |
| `interval_seconds` | 86400, 604800, or 2592000 | Daily, weekly, or monthly recurrence. |
| `enabled` | Boolean | Pause/resume without deleting the schedule. |
| `next_run_at`, `last_run_at` | RFC 3339 UTC timestamps or `NULL` | Due selection and last-run display. |
| `last_request_id` | Request identifier or `NULL` | Links the last generated request; set to `NULL` if that request is deleted. |
| `last_error_code` | Stable code or `NULL` | Shows the last failed execution class without free text. |
| `created_at`, `updated_at` | RFC 3339 UTC timestamps | Ordering and diagnosis. |

#### `stories`

| Column | Contents | Why it is stored |
|---|---|---|
| `id` | 8-character identifier | Primary key and card operand. |
| `project_id`, `request_id` | Owning project and request | Links every story to its origin and enables cascade deletion. |
| `title`, `description` | Validated model proposal, locally checked | Project work item content. |
| `acceptance_criteria_json`, `required_skills_json` | Bounded, validated arrays | Planning output used by the board and later work. |
| `priority`, `complexity` | Constrained enums | Planning estimate and sorting. |
| `assigned_agent_id` | Project roster member or `NULL` | Assignment; references are validated locally. |
| `assignment_rationale` | Bounded explanation or `NULL` | Explains the assignment or why it is unassigned. |
| `state` | Kanban state | Board column written only by the application workflow. Assigned stories start in `IN PROGRESS`; unassigned stories start in `TODO` (ADR-0047). |
| `execution_attempts` | Integer counter | Bounds paid retries; exhausted attempts start remediation or rework (ADR-0050, ADR-0071). |
| `remediation_attempts` | Integer counter (schema v16) | Bounds Tech Lead remediation cycles before the closing decision (ADR-0069). |
| `rework_attempts` | Integer counter (schema v20) | Bounds failed attempts: direct rework, escalation, then close (ADR-0071). |
| `tech_lead_context` | Bounded decision, findings, or context text or `NULL` (schema v16) | Gives the next attempt the Tech Lead decision or the reviewer findings, and stores the final closing compromise; untrusted prompt data, displayed sanitized. |
| `debt_of_story_id` | Parent story identifier or `NULL` (schema v17) | Links one technical-debt follow-up card to the story it was opened for (ADR-0069). |
| `due_date` | `YYYY-MM-DD` | Locally computed estimate, not a commitment. |
| `created_at`, `updated_at` | RFC 3339 UTC timestamps | Ordering and diagnosis. |

#### `story_outcomes`

ADR-0050 adds one optional row per completed story:

| Column | Contents | Why it is stored |
|---|---|---|
| `story_id` | Story identifier | Primary key and cascade owner. |
| `work_outcome` | Bounded work summary | What the agent changed and ran. |
| `review_outcome` | Bounded review text | Reviewer assessment. |
| `review_accepted` | `0` or `1` | Final review decision; `0` keeps the story in remediation. |
| `reviewer_agent_id` | Reviewer agent | Who reviewed the work. |
| `branch` | Story branch, for example `vlab/5a617283` | Locates the committed work. |
| `worktree_path` | Path relative to `.workspace`, for example `.worktrees/5a617283` | Locates the isolated checkout; a retry reuses it (ADR-0071). |
| `commit_sha` | Git commit hash | Identifies the exact committed state. |
| `review_findings_json` | Bounded string array or `NULL` (schema v20) | Reviewer findings used by the rework and debt card; untrusted project data. |
| `review_retest` | Bounded text or `NULL` (schema v20) | Scope the next review must retest; untrusted project data. |
| `completed_at` | RFC 3339 UTC timestamp | Completion ordering and diagnosis. |
| `integrated_at` | RFC 3339 UTC timestamp or `NULL` (schema v18) | When the commit entered local `develop` (ADR-0070). |
| `push_ok` | `0` or `1` (schema v18) | No pending push: pushed, no remote, or a legacy outcome intentionally not published (schema v19). |
| `integration_error` | Content-free code or `NULL` (schema v18) | Last integration/push failure for retry; never git output. |

The outcome text and findings are project data, not audit metadata. They are
untrusted and rendered sanitized. File content, command output, and diffs are
never stored in this table. The integration columns store no remote URL, branch
list, or token.

#### `memories`

WP-4 adds the `memories` table ([ADR-0022](adr/adr-0022.md)).

| Column | Contents | Why it is stored |
|---|---|---|
| `id` | 8-character identifier | Primary key and command operand. |
| `scope_type` | `lab`, `project`, or `agent` | Selects the owner kind. |
| `lab_id`, `project_id`, `agent_id` | Exactly one non-null foreign key | Identifies the owner and enables cascade deletion. |
| `path` | Relative markdown path under `data/memories/` (schema v15, ADR-0059) | The database stores only the reference; content is in the file. |
| `content` | Legacy column, always empty after materialization | Kept for migration compatibility; a future schema can drop it. |
| `created_at` | RFC 3339 UTC timestamp | Ordering and diagnosis. |

Agent duplication may copy agent-scope memories into a new agent scope. The
copied files get new identifiers and creation times and follow the same
retention as any other memory (ADR-0058).

#### Markdown file store (ADR-0059)

`data/memories/` and `data/documents/` contain `agents/`, `projects/`, and
`lab/` subfolders. Files are UTF-8 markdown, bounded to 8 KiB for memories and
1 MiB for documents. Automatically generated project memories are planning
recaps with metadata and the story list; automatically generated agent memories
are card-completion recaps with branch, commit, work, review, and Tech Lead
context. Both are bounded to the 2,000-character memory limit (ADR-0068). The
data directory is private. The repository tracks only
`.data/memories/.gitkeep` and `.data/documents/.gitkeep`; all generated content
is ignored by git. VLab does not back up or encrypt these files.

#### `documents` (schema v15)

| Column | Contents | Why it is stored |
|---|---|---|
| `id` | 8-character identifier | Primary key and event entity. |
| `scope_type` | `agent`, `project`, or `lab` | Selects the owner kind. |
| `agent_id`, `project_id`, `lab_id` | Exactly one non-null foreign key | Identifies the owner and enables cascade deletion. |
| `path` | Relative markdown path under `data/documents/` | The reference; content remains in the file. |
| `title` | Sanitized source title | Display metadata. |
| `created_at`, `updated_at` | RFC 3339 UTC timestamps | Ordering and diagnosis. |

#### `entity_folders` (schema v15)

| Column | Contents | Why it is stored |
|---|---|---|
| `kind`, `entity_id` | Owner kind and identifier | Stable folder mapping per entity. |
| `folder` | Sanitized unique folder name | Readable path segment; collision-safe with a unique constraint. |

Memories are application-managed. Laboratory and project content can be
created, viewed, edited, and deleted from the Lab floor (ADR-0046); agent
memories have no manual surface. Deleting a laboratory or project removes its
memories by cascade. Archiving an agent retains its memories for historical
inspection but excludes them from prompts. Structural memories are deterministic. A successful plan makes one
additional bounded provider call and stores its summary as a project memory;
the summary is model output and therefore untrusted. Memory content is sent to
the provider only when selected for a prompt, within 8 KiB, and is never
logged. Credentials are rejected from memory content and are never stored.

### Schema v5: interaction metadata

WP-6 adds `interactions` with run and agent identifiers, kind (`planning`,
`question`, or `workflow`), provider, model, outcome, error code, duration,
input/output token counts, and a timestamp. It stores **no content**: no question, answer, prompt,
request text, memory, CV, or provider body. This is operational metadata used
for diagnostics and evaluation, not project data.

### Schema v6: project agent assignments

ADR-0034 adds `project_agents`:

| Column | Contents | Why it is stored |
|---|---|---|
| `project_id`, `agent_id` | Foreign keys | Many-to-many project roster; cascades with either owner. |
| `created_at` | RFC 3339 UTC timestamp | Ordering and diagnosis. |

The table contains identifiers only, no content or credentials.

Schema changes must justify each stored field and update this document before
the migration lands. The current schema version is v11 (ADR-0050).
