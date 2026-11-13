# VLab — Technical Specification

## 1. Purpose

VLab is a command-line application for managing virtual software laboratories staffed by AI agents.

Users create laboratories, configure specialized agents, organize projects, and submit natural-language requests. A Tech Lead analyzes each request, converts it into user stories, assigns work according to agent skills, and proposes due dates.

The application shall prioritize:

- Simple and intuitive terminal interactions.
- Clear responsibilities and work ownership.
- Reliable persistence and recoverable workflows.
- Traceable AI decisions.
- Human oversight of consequential actions.

The executable shall be named `vlab`.

## 2. Scope and Assumptions

This specification is based on the supplied CLI requirements. Product intent is summarized in [`ELEVATOR-PITCH.md`](ELEVATOR-PITCH.md); where the pitch and this specification differ, this specification governs.

The initial release shall include:

- Provider and model settings, plus optional source-control (GitHub) settings.
- Laboratory management and validation.
- Agent creation, inspection, and deletion.
- Project management.
- Request intake and AI-assisted planning.
- A read-only project Kanban overview.
- Questions addressed to an agent or the Tech Lead.

The initial release shall not include:

- Autonomous source-code execution or modification.
- Automatic deployment.
- Distributed or multi-user operation.
- A graphical interface.
- Providers other than DeepSeek.
- Autonomous execution of the entire software-development lifecycle.
- Manual memory-management commands and memory editing. Memories are application-managed, immutable, and explicitly scoped.

Submitting a request triggers a bounded planning workflow, not an unrestricted agent execution loop.

## 3. Core Domain Rules

### 3.1 Laboratories

A laboratory is a workspace containing projects and agent memberships.

A laboratory:

- Has a unique identifier and a non-empty name.
- May have an optional single-line description of at most 2,000 characters.
- May initially be empty.
- May share agents with other laboratories.
- Must contain at least one agent and exactly one designated Tech Lead to pass validation.

For the initial release, a laboratory shall have at most one member with the `lead` role. That member is its designated Tech Lead.

### 3.2 Projects

A project belongs to exactly one laboratory and may reference one GitHub
repository through an optional **GitHub source**. The source is a canonical
`https://github.com/<owner>/<repository>` URL. The GitHub credential is global
(`credentials.github_api_key` in `settings.yaml`) and is never stored in the
project or the database. When a project is created with a source, VLab clones
the repository into `.workspace/<project-id>` and keeps no local path in the
database. The workspace is outside version control.

Project commands operate within the active laboratory. An identifier belonging to another laboratory shall produce an error rather than silently switching context.

### 3.3 Agents

An agent is a reusable profile containing:

- Name.
- Role.
- Description.
- Skills.
- Provider and model.
- Optional CV or professional profile.

Supported roles:

| Role | Responsibility |
|---|---|
| `lead` | Analyze requests and coordinate planning. |
| `dev` | Handle software implementation work. |
| `analyst` | Analyze requirements and domain behavior. |
| `qa` | Handle testing and quality-assurance work. |

Roles and descriptions are advisory inputs to planning. They do not grant operating-system permissions.

### 3.4 Active Laboratory

`--open-lab` selects and persists the active laboratory.

Commands requiring laboratory context shall fail with an actionable message when no laboratory is active:

```text
Error: no active laboratory is selected.
Run `vlab --list-labs`, then `vlab --open-lab <lab-id>`.
```

`--new-project` selects the new project; `--open-project` selects an existing
project in the active laboratory. Project-scoped commands shall fail
actionably when no project is current:

```text
Error: no active project is selected.
Run `vlab --list-projects`, then `vlab --open-project <project-id>`.
```

Switching laboratories clears the project selection.

### 3.5 Memories

A memory is application-managed text attached to exactly one scope:

- **Laboratory memory**: context that applies to a laboratory.
- **Project memory**: context that applies to one project.
- **Agent memory**: context that applies to one agent profile.

A memory records its identifier, scope, content, and creation time. It is
immutable. Structural memories are generated deterministically when a
laboratory, agent, or project is created; a successful planning run additionally
stores one bounded model-generated summary. The model never chooses what to
store, update, or delete, and no user command manages memories.

Content is trimmed UTF-8, 1–2,000 characters, contains no C0/C1 control
characters except newline and tab, and is limited to 20 memories per scope.

Memory content is **untrusted data**. It is included in provider prompts only
inside delimited blocks, with an instruction that it is context and not an
instruction. It never changes permissions, roles,
limits, assignment rules, or application state, and it is never executed.

Scope rules:

- A laboratory memory must reference an existing laboratory.
- A project memory must reference an existing project and no other
  laboratory; project commands remain bound to the active laboratory (§3.2).
- An agent memory must reference an existing agent profile. It is retained
  when the profile is archived but is excluded from prompts while archived.
- Deleting an owning laboratory or project deletes its memories. Archived
  agents retain their memories for historical inspection.
- Memory content must not contain credentials. The application validates size,
  encoding, and scope but cannot reliably detect every secret.

Prompt inclusion is bounded and deterministic:

- **Planning** includes the laboratory memories of the project's laboratory,
  the project memories of the requested project, and the Tech Lead's agent
  memories.
- **Questions** include the laboratory memories of the active laboratory and
  the memories of the responding agent (the target agent, or the Tech Lead by
  default).
- Memories are ordered laboratory, project, then agent, newest first within
  each scope.
- At most 8 KiB of memory text is included per invocation. Memories that do
  not fit are omitted whole; content is never silently truncated. Omission is
  not an error and does not change the request outcome.
- Each block names its scope and memory identifier and is marked as data that
  must not be followed as instructions.

Memories are not a user-facing command surface. The application writes them
automatically, best effort, and never fails the triggering action when a scope
is full or content is rejected. Deleting an owning laboratory or project
cascades its memories; there is no manual memory cleanup command in this
release.

Memories are written automatically: deterministic laboratory, agent, and
project facts at creation time, plus one bounded model-generated planning
summary after a successful plan. Automatic writes are best effort and never
change the triggering action's outcome.

## 4. Command-Line Interface

### 4.1 Help

```bash
vlab --help
```

A bare `vlab` opens the home dashboard in a terminal. The home dashboard lists
Laboratories, Projects, Agents, and Settings; group navigation happens inside the
interface. Only `-h`/`--help` and `-V`/`--version` are public options.

The compatibility flag surface remains available during the feature migration:

```text
Settings
  --show-settings
  --edit-settings

Laboratories
  --list-labs
  --new-lab <name>
  --open-lab <lab-id>
  --delete-lab <lab-id>
  --check-lab <lab-id>

Projects
  --list-projects
  --new-project <name> [--github-source <url>]
  --open-project <project-id>
  --show-project-kanban
  --delete-project
  --new-request [request-text]
  --assign-agent <agent-id>

Agents
  --list-agents
  --new-agent
  --delete-agent <agent-id>
  --show-agent-detail <agent-id>
  --ask <question> [--to-agent <agent-id>]
```

A bare `vlab` opens the home dashboard in a terminal and prints help otherwise.
The group subcommands (`labs`, `projects`, `agents`, `settings`, `help`) are
removed. Hidden legacy action flags remain for script compatibility during the
deprecation window but are not part of the public help surface.

`--to-agent` is a modifier and is valid only with `--ask`.

`--github-source` is a modifier valid only with `--new-project`.

`--yes` is valid only with `--delete-lab`, `--delete-project`, and
`--delete-agent`.

Running `vlab` without arguments shall display concise help without changing application state.

### 4.2 Identifiers

All persistent entities, including memories, shall have unique identifiers.

Laboratory creation shall return a short UUID-style identifier suitable for terminal use.

Identifiers shall:

- Be checked for collisions before persistence.
- Resolve to exactly one entity.
- Never be used directly as unchecked filesystem paths.
- Be displayed consistently across commands.

## 5. Settings

### 5.1 Configuration File

Application configuration shall be stored in `settings.yaml`.

Example structure:

```yaml
provider: deepseek
model: "<supported-model-id>"
source_control:
  provider: github
credentials:
  deepseek_api_key: "sk-..."
  github_api_key: "ghp-..."
```

| Field | Required | Description |
|---|---|---|
| `provider` | Yes | Provider identifier; initially only `deepseek`. |
| `model` | Yes | Model identifier accepted by the configured provider. |
| `source_control.provider` | No | Source-control provider; `github` only. |
| `credentials.deepseek_api_key` | Planning, questions, and automatic summaries | Provider credential; a non-empty token of at most 512 bytes. |
| `credentials.github_api_key` | For `--github-source` | GitHub credential; a non-empty token of at most 512 bytes. |

The example model name `flash` from the requirements shall not be assumed to be a valid DeepSeek model identifier. Supported identifiers must be verified against the provider integration.

`settings.yaml` is the only credential store. Credentials shall not be read
from environment variables, command-line arguments, the database, or any other
file. The file contains secrets, so it shall be excluded from version control,
created with mode `0600`, and kept in a directory with mode `0700`. A tracked
`settings.example.yaml` with placeholders may document the shape but shall
never contain a real credential.

`source_control` is optional and defaults to `github`; GitHub is the only
supported source-control provider. Omitting the section is equivalent to
declaring it.

Configuration shall use the operating system's per-user configuration directory. Runtime data shall use its per-user application-data directory.

The resolved configuration path shall be visible in settings output.

### 5.2 Show Settings

```bash
vlab --show-settings
```

The command shall:

- Display the configuration path, provider, model, and source-control provider.
- Report whether `credentials.deepseek_api_key` and
  `credentials.github_api_key` are set, using only `set`/`unset`.
- Report missing or invalid configuration.
- Never display API keys or other secrets.
- Make no remote provider or GitHub calls.

### 5.3 Edit Settings

```bash
vlab --edit-settings
```

The command shall:

1. Resolve the editor from `VISUAL`, then `EDITOR`.
2. Report an actionable error if neither is available.
3. Open a temporary copy or initial template.
4. Wait for the editor to exit.
5. Validate the edited YAML.
6. Atomically replace the configuration only if validation succeeds.

Invalid edits shall not overwrite the previous valid configuration.

The editor shall be launched without evaluating an arbitrary shell command string. Supported argument parsing and quoting shall be documented.

### 5.4 Agent Defaults

New agents inherit provider and model values from the current settings.

These values shall be copied into the agent profile at creation time. Later settings changes shall not silently alter existing agents.

## 6. Laboratory Management

### 6.1 List Laboratories

```bash
vlab --list-labs
```

Display:

- Identifier and name.
- Agent count.
- Project count.
- Current structural validation result.
- Active-laboratory indicator.

An empty collection shall produce a helpful empty-state message.

### 6.2 Create a Laboratory

```bash
vlab --new-lab "Payments Platform"
```

The command shall:

- Reject empty or whitespace-only names.
- Create and persist the laboratory atomically.
- Return its short identifier.
- Explain that a Tech Lead is required before request processing.

Creating a laboratory shall not implicitly change the active laboratory.

### 6.3 Open a Laboratory

```bash
vlab --open-lab <lab-id>
```

The command shall:

- Verify existence.
- Persist the active selection.
- Display a concise summary and validation warnings.

Opening an incomplete laboratory is allowed so that the user can configure it.

### 6.4 Delete a Laboratory

```bash
vlab --delete-lab <lab-id>
```

The command shall:

- Display the laboratory name and affected project count.
- Require explicit confirmation.
- Delete its projects, requests, stories, and memberships in one transaction.
- Preserve reusable agent profiles.
- Clear the active selection if necessary.
- Refuse deletion while a planning operation is active.

Cancellation shall leave all data unchanged.

### 6.5 Check a Laboratory

```bash
vlab --check-lab <lab-id>
```

Validation shall check:

- Laboratory existence and structural integrity.
- At least one agent membership.
- Exactly one Tech Lead.
- Valid agent roles.
- Resolvable project and agent references.
- Non-empty skills for agents eligible for assignment.
- Structurally valid provider and model configuration.

The command shall not contact the provider or incur model costs.

Remote availability and credential validity are checked when a provider-backed operation runs.

Validation failures shall produce a non-zero exit code and an actionable report.

## 7. Agent Management

### 7.1 List Agents

```bash
vlab --list-agents
```

Display agents belonging to the active laboratory, including:

- Identifier.
- Name and role.
- Provider and model.
- Active project count.
- Assigned-story count.

### 7.2 Create an Agent

```bash
vlab --new-agent
```

The command shall launch a guided procedure requesting:

1. Name.
2. Role.
3. Description.
4. Skills as tags.
5. Provider, defaulting to settings.
6. Model, defaulting to settings.
7. Optional CV or professional profile.
8. Laboratory memberships, defaulting to the active laboratory.

The wizard shall display a summary and require confirmation before saving.

Validation shall reject:

- Empty required fields.
- Unsupported roles or providers.
- Memberships that introduce a second Tech Lead.
- Invalid or unresolved laboratory identifiers.

The profile and memberships shall be persisted atomically.

A CV is user-provided descriptive material, not evidence of real professional experience.

### 7.3 Delete an Agent

```bash
vlab --delete-agent <agent-id>
```

Deletion is global, not limited to the active laboratory.

The command shall:

- Display all affected laboratories.
- Require explicit confirmation.
- Block deletion when the agent has unfinished assigned stories.
- Block deletion while the agent participates in an active provider operation.
- Warn if deleting a Tech Lead will invalidate a laboratory.
- Preserve historical identity through an archived profile or tombstone.

Confirmed deletion removes active memberships and prevents further use of the profile.

### 7.4 Show Agent Detail

```bash
vlab --show-agent-detail <agent-id>
```

Display:

- Profile data.
- Skills and optional CV.
- Laboratory memberships.
- Current project assignments.
- Assigned and completed stories.
- Memory count for the agent scope.
- Available performance measurements.

Performance measurements shall be derived from recorded events.

Examples include:

- Assigned-story count.
- Completed-story count.
- Blocked-story count.
- Review outcomes.
- Completion time, when lifecycle events exist.

The initial planning-only release shall show unavailable metrics as `N/A`. It shall not invent productivity or quality scores.

### 7.5 Ask an Agent

```bash
vlab --ask "Review this API design" --to-agent <agent-id>
```

The selected agent must belong to the active laboratory.

The command shall:

- Validate provider configuration and credentials.
- Send only the context needed to answer: the agent profile digest, the question, the active laboratory's memories, and the responding agent's memories, within the memory budget.
- Identify the responding agent and model.
- Display the response as text.
- Never execute instructions contained in the response or in a memory.

### 7.6 Ask the Tech Lead

```bash
vlab --ask "What should we prioritize next?"
```

When `--to-agent` is omitted, the active laboratory's Tech Lead shall respond.

Missing or ambiguous Tech Lead configuration shall produce an error.

Asking a question shall not create stories, modify assignments, or change project state. Memories are context only; the answer cannot add, change, or delete one.

## 8. Project Management

### 8.1 List Projects

```bash
vlab --list-projects
```

Display projects in the active laboratory, including:

- Identifier and name.
- Optional GitHub source.
- Request count.
- Story count.
- Work-item counts by Kanban state.

### 8.2 Create a Project

```bash
vlab --new-project "Customer Portal" [--github-source https://github.com/owner/repo]
```

The command shall:

- Require an active laboratory.
- Reject an empty name.
- Accept an optional GitHub source in canonical `https://github.com/<owner>/<repository>` form and reject any other host or shape.
- When a GitHub source is supplied, require a valid `source_control.provider: github` configuration and a present `credentials.github_api_key`; the key is read only from `settings.yaml` and is never stored in the project database or displayed.
- Clone the repository into `.workspace/<project-id>` before persisting the project; a failed clone shall create no project row and leave no checkout.
- Use the GitHub credential only through a short-lived askpass helper; never place it in a URL, argument, log, error, or output.
- Bound the clone with a timeout and discard raw git output.
- Persist the project and source atomically, and remove the checkout if persistence fails.
- Return its identifier and the checkout path.
- Select the new project as the current project.

A project may be created in an incomplete laboratory, but requests cannot be processed until laboratory validation succeeds.

### 8.3 Select a Project

```bash
vlab --open-project <project-id>
```

The command shall verify that the project belongs to the active laboratory and
store it as the current project. `--show-project-kanban`, `--delete-project`,
`--new-request`, and `--assign-agent` operate on the current project. Switching
laboratories clears the project selection.

### 8.4 Assign Agents

```bash
vlab --assign-agent <agent-id>
```

The command shall require a current project and an active agent that is a
member of the project's laboratory. Repeated assignment is idempotent.
Archived agents and agents from another laboratory shall be rejected. Planning
uses the project roster when non-empty; otherwise it uses the active laboratory
roster.

### 8.5 Delete a Project

```bash
vlab --delete-project
```

The command shall:

- Use the current project and verify ownership by the active laboratory.
- Require confirmation.
- Delete the project and associated data atomically, including its agent assignments.
- Preserve agent profiles.
- Refuse deletion while request processing is active.
- Clear the current project after deletion.

### 8.6 Edit a Project

The Projects dashboard `Edit` action shall:

- Use the selected project and verify ownership by the active laboratory.
- Change only the project name, validated with the laboratory name rules.
- Leave the GitHub source unchanged; it names the existing checkout
  (ADR-0043). A different repository requires a new project.
- Record a `project_updated` audit event with the project identifier only.

## 9. Request Intake

### 9.1 Input Syntax

In the interactive dashboards, the Projects `Request` action (`r`) and the
Kanban `n` key shall open a prompt dialog for the project (ADR-0044). The
dialog accepts typed or pasted multi-line text, rejects empty text without
submitting, and applies every rule below except those specific to arguments
and standard input. The flag forms below remain for scripts.

Requests shall support literal text:

```bash
vlab --new-request "Add password reset functionality"
```

Requests shall also support standard input:

```bash
vlab --new-request < request.md
```

Or:

```bash
cat request.md | vlab --new-request
```

The target is the current project selected with `--open-project`.

The `<` character is shell input redirection, not a CLI argument.

A positional request argument is always literal text. VLab shall not guess whether it is a file path.

Rules:

- Reject empty or whitespace-only content.
- Require UTF-8 input.
- Enforce a documented input-size limit.
- Reject simultaneous literal text and piped or redirected input.
- Fail with usage guidance rather than waiting indefinitely when neither input form is available.

### 9.2 Planning Workflow

A valid request shall trigger this sequence:

1. Validate the project and laboratory.
2. Verify provider prerequisites.
3. Persist the original request as a `TODO` intake item.
4. Mark processing as `PLANNING`.
5. Give the Tech Lead the request and relevant team context, including the project agent roster (or the laboratory roster when none is assigned) and the bounded memories defined in §3.5.
6. Obtain structured user-story proposals.
7. Validate the proposals locally.
8. Persist valid stories, assignments, due dates, and rationale atomically.
9. Mark the request as `PLANNED`.
10. Best effort: request and store a bounded model-generated project memory summary.
11. Display the result.

The original request shall remain linked to every generated story.

Once planned, the intake item shall no longer appear as a separate pending Kanban card, preventing duplicate work counts.

### 9.3 Required Story Fields

Each story shall contain:

- Identifier.
- Project and originating request.
- Title.
- Description.
- Acceptance criteria.
- Priority.
- Required skills.
- Assigned agent, when a suitable agent exists.
- Proposed due date.
- Assignment rationale.
- Kanban state.

New stories shall begin in `TODO`.

### 9.4 Assignment Rules

The Tech Lead shall select among the project's assigned agents. If the project
has no assignments, it shall use all active laboratory members. Selection uses:

- Role.
- Declared skills.
- Relevant profile information.
- Current workload.

Provider output shall reference existing identifiers only.

VLab shall reject assignments to unknown agents or agents outside the laboratory.

When no suitable agent exists, the story shall remain unassigned in `TODO`, with a visible explanation.

### 9.5 Due Dates

Every generated story shall receive a proposed due date.

The initial policy shall derive dates from validated estimates:

| Complexity | Default interval |
|---|---|
| Small | 2 calendar days |
| Medium | 5 calendar days |
| Large | 10 calendar days |

The date shall be calculated from the request creation date using UTC and persisted as an ISO 8601 calendar date.

The Tech Lead proposes complexity; application code calculates the date.

These dates are planning estimates, not delivery commitments.

### 9.6 Incomplete or Failed Planning

If clarification is necessary, preserve the request as `NEEDS_CLARIFICATION` and show the questions.

If processing fails, preserve it as `FAILED` with an actionable reason.

In either case, the request remains visible in the `TODO` intake view.

No partially validated story set shall be committed.

The initial release does not include a dedicated retry command. Users may submit a revised request; the previous record remains available for traceability.

## 10. Kanban Overview

```bash
vlab --show-project-kanban
```

The command shall render a read-only board with:

- `TODO`
- `IN PROGRESS`
- `IN REVIEW`
- `BLOCKED`
- `DONE`

Each card shall show:

- Item identifier and concise title.
- Assigned agent or `Unassigned`.
- Priority.
- Proposed due date.
- Relevant warning or blocked indicator.

The board shall include:

- Project and laboratory names.
- Optional project GitHub source.
- Tech Lead name.
- Story counts by state.
- Completed-story count.
- Pending intake count, separately from story totals.

UX requirements:

- Use aligned columns on wide terminals and interactive dashboards in a TTY.
- Fall back to vertically stacked sections on narrow terminals and plain output.
- Truncate long titles predictably.
- Use semantic color, box drawing, and a persistent action bar in dashboards.
- Remain understandable without color; never rely on color alone.
- Respect `NO_COLOR`, `--no-color`, and `TERM=dumb`.
- Avoid ANSI decoration and cursor control when output is redirected or not a terminal.
- Sanitize untrusted terminal-control sequences.
- Provide useful empty-state guidance.

In the interactive dashboards the board is a dedicated full-screen view, not a
dialog (ADR-0043). The five states are side-by-side lists of bordered cards; a
card shows only the story's short description (title). Each list scrolls
independently and shows how many cards are above and below. `Enter` on a card
opens a detail dialog with every field listed above plus complexity,
description, acceptance criteria, required skills, assignment rationale, and
the originating request; long details scroll. Terminals too narrow for five
readable lists show a window of adjacent lists around the focused one. The
plain `--show-project-kanban` output keeps the full card fields.

The initial release creates `TODO` stories only. Later execution workflows may introduce state-transition commands; the Kanban display shall not imply that such execution already exists.

## 11. Persistence and Data Model

### 11.1 Recommended Storage

Use:

- YAML for user-editable settings.
- SQLite for domain data and event history.

SQLite shall enforce foreign keys and transactional updates.

Schema changes shall use versioned migrations.

### 11.2 Entities

| Entity | Key information |
|---|---|
| Laboratory | ID, name, timestamps |
| Agent | ID, profile, role, skills, provider, model, archive status |
| Laboratory membership | Laboratory ID, agent ID |
| Project | ID, laboratory ID, name, optional GitHub source, timestamps |
| Request | ID, project ID, original content, processing status, error |
| User story | ID, request ID, title, criteria, state, assignment, due date |
| Planning event | Request ID, timestamp, validated decision, provider metadata |
| Interaction metadata | Agent ID, timestamp, provider, model, outcome |
| Memory | ID, scope (`lab`, `project`, `agent`), scope owner ID, content, created timestamp |

Timestamps shall use UTC.

Original requests, memories, and validated planning results shall be retained as project data. Project GitHub sources are retained as project metadata; GitHub credentials are never stored. Cloned repositories under `.workspace/` are disposable local files, outside version control and outside the database; no local path is persisted.

Full chat transcripts shall not be retained by default. Diagnostic logs shall omit request and response bodies.

## 12. Provider Integration and Bounded Execution

The initial implementation shall support only DeepSeek.

The provider integration shall:

- Use encrypted transport.
- Read the provider credential from `credentials.deepseek_api_key` in `settings.yaml`.
- Enforce timeouts.
- Limit response size.
- Validate structured output against a local schema.
- Distinguish authentication, rate-limit, transport, and validation failures.

Recommended initial limits:

- One logical planning invocation per request.
- At most two retries for retryable transient failures.
- A 60-second timeout per attempt.
- Exponential backoff.
- No recursive agent delegation.
- No automatic repair loop for invalid model output.
- At most 8 KiB of scoped memory text per invocation, included as delimited untrusted data (§3.5).

Retry attempts may incur additional provider charges. The application shall not promise exactly-once remote execution.

Cancellation shall stop further attempts and preserve an accepted request in a recoverable state.

## 13. Security Requirements

The application shall treat requests, profiles, CVs, and provider responses as untrusted content.

It shall:

- Keep instructions separate from external content.
- Validate identifiers, dates, roles, sizes, and state changes locally.
- Prevent model output from granting permissions.
- Never execute generated shell commands or source code.
- Treat memory content as untrusted data: validate its scope, encoding, and size, delimit it in prompts, and never let it change permissions, roles, limits, or assignment rules.
- Never log or render memory content without sanitization, and never store credentials in it.
- Treat the GitHub source as a validated URL and `credentials.github_api_key` as a credential: never log, persist in the database, or display the key.
- For the project clone, use the validated canonical URL, the global credential through a short-lived askpass helper, a bounded timeout, no shell, and no token in arguments or output. No other GitHub operation is permitted.
- Include only the scoped memories selected by §3.5, within the configured memory budget.
- Avoid sending unrelated project data to providers.
- Redact credentials from logs and errors.
- Restrict local data permissions where supported.
- Require explicit confirmation for deletion.
- Fail safely in non-interactive environments when confirmation is required.

Local data is not encrypted at rest by this specification. Documentation shall recommend operating-system disk encryption for sensitive projects.

## 14. CLI Errors and Exit Codes

| Code | Meaning |
|---|---|
| `0` | Success |
| `1` | Operation or provider failure |
| `2` | Invalid arguments, input, or configuration |
| `3` | Missing entity or active context |
| `4` | Validation or precondition failure |
| `5` | User cancellation |
| `130` | Interrupted execution |

Normal results shall go to standard output.

Errors, warnings, and progress indicators shall go to standard error.

Errors shall explain both the problem and the next useful action.

## 15. Rust Implementation Guidelines

Use a small modular Rust application with these boundaries:

```text
CLI parsing and rendering
          |
Application use cases
          |
Domain rules and validation
          |
SQLite storage and DeepSeek adapter
```

Implementation requirements:

- Prefer safe Rust.
- Use enums for roles, states, memory scopes, and processing outcomes.
- Use typed identifiers where they prevent accidental cross-entity use.
- Represent dates with date types rather than unchecked strings.
- Propagate typed errors to the CLI boundary.
- Keep domain validation independent of provider calls.
- Perform multi-entity writes in transactions.
- Keep dependencies minimal and intentional.

Do not introduce a general-purpose agent framework, plugin system, or distributed task queue for the initial scope.

## 16. Testing and Validation

Automated tests shall cover:

### Domain Tests

- Laboratory validity and Tech Lead uniqueness.
- Agent membership rules.
- Assignment target validation.
- Due-date calculation.
- Request and story state invariants.
- Deletion preconditions.
- Memory scope rules, content bounds, and ordering.

### CLI Tests

- Help grouping and argument conflicts.
- Literal text versus standard-input handling.
- Empty and oversized requests.
- Missing active context.
- Exit codes and error messages.
- Confirmation and cancellation.
- Narrow-terminal and no-color rendering.

### Persistence Tests

- Atomic creation and deletion.
- Foreign-key enforcement.
- Rollback after failed story persistence.
- Original-request preservation.
- Agent-history preservation.
- Memory creation, deletion, and cascade behavior.
- Project GitHub source persistence and migration from projects without a source.
- Schema migrations.

### Provider Tests

Use a mock provider by default to test:

- Timeouts and bounded retries.
- Authentication and rate-limit errors.
- Malformed or incomplete output.
- Unknown agent identifiers.
- Oversized responses.
- Prompt-injection attempts in request text and memory content.
- Oversized or out-of-scope memory content.
- Cancellation during planning.

Live-provider tests shall be opt-in and shall never require committed credentials.

## 17. Acceptance Criteria

The initial release is acceptable when:

1. `vlab --help` documents all specified commands.
2. Settings can be shown and edited without exposing credentials.
3. Invalid configuration edits preserve the previous valid file.
4. Laboratories can be created, listed, opened, checked, and deleted.
5. Validation fails for a laboratory without agents or a Tech Lead.
6. Agents can be created through a guided procedure and inspected.
7. Agent details show memberships, assignments, and evidence-based metrics.
8. Projects can be created, listed, and deleted within the active laboratory.
9. Requests can be submitted as quoted text or redirected file content.
10. The original request is persisted before model processing.
11. Valid planning output produces linked user stories.
12. Assignments reference real members of the laboratory.
13. Every generated story has a locally validated proposed due date.
14. Failed processing never loses the accepted request.
15. The Kanban board is readable with narrow terminals and without color.
16. Questions can target a selected agent or default to the Tech Lead.
17. Destructive operations require confirmation.
18. Provider execution is bounded and observable.
19. No model-generated content is executed automatically.
20. Core workflows pass deterministic automated tests.
21. Memories are generated automatically for laboratories, agents, projects, and successful plans, with scope enforced and deletion cascading from the owner.
22. Memory content is bounded, sanitized, treated as untrusted data, and never executed or used to grant permissions or change assignment rules; automatic writes never fail the triggering action.
23. Planning and questions include only the scoped memories within the configured cap, delimited as data; omission beyond the cap is explicit in the design and never truncates a memory silently.

## 18. Incremental Delivery Plan

### Increment 1 — Local Foundation

Implement settings, persistence, laboratory commands, and CLI errors.

Validate with tests for configuration editing, laboratory creation, and active-context selection.

### Increment 2 — Agents and Projects

Implement the agent wizard, memberships, laboratory checks, project commands with optional GitHub source configuration, scoped memory storage and commands, and empty Kanban rendering.

Validate a complete local setup without provider access, including memory scope and deletion cascades.

### Increment 3 — Request Planning

Implement request intake, durable storage, memory-aware prompts, a mock provider, structured validation, assignments, and due dates.

Validate success, clarification, invalid output, failure recovery, and bounded memory context.

### Increment 4 — DeepSeek and Direct Questions

Integrate DeepSeek, credential handling, bounded retries, and `--ask` with scoped memory context.

Validate with mocked contract tests, memory-injection tests, and an opt-in live smoke test.

### Increment 5 — UX and Security Hardening

Refine terminal layouts, confirmations, redaction, cancellation, and audit metadata.

Validate the full workflow and document current limitations.

The smallest useful first deliverable is a tested CLI that can create a laboratory, select it, and report why it is not yet valid.
