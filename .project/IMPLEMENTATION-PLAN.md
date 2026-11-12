# VLab — Implementation Plan

| Field | Value |
|---|---|
| Status | **Proposed plan — no implementation exists and no test result has been verified.** |
| Version | 1.0 |
| Date | 2026-11-12 |
| Language | English (required by `CONSTITUTION.md` §9) |
| Basis | `.docs/CONSTITUTION.md`, `.docs/PRINCIPLES.md`, `.docs/ARCHITECTURE.md`, `.docs/VALIDATION.md`, `.docs/SECURITY.md`, `.docs/STACK.md`, `.docs/TECHNICAL-SPECIFICATIONS.md`, `.docs/ELEVATOR-PITCH.md` |
| Repository state (verified 2026-11-12) | Documentation only: `AGENTS.md`, `.docs/`, `.agents/`, `.project/IMPLEMENTATION-PLAN.md`. No source code, no `Cargo.toml`, no version-control metadata. |

This plan converts the VLab technical specification into six sequential working packages. Each package is independently reviewable and ends at a gate that must pass before the next package starts.

> **Later decisions override the command details below.** ADR-0033 makes
> `--open-project` select a current project for `--show-project-kanban`,
> `--delete-project`, `--new-request`, and `--assign-agent`. ADR-0034 adds
> project agent rosters. ADR-0035 removes the memory commands. ADR-0036 makes
> memories automatic. Historical task text that still shows project
> identifiers or memory commands is retained for traceability; the ADRs and
> the current specification are authoritative.

Normative language follows `CONSTITUTION.md` §2: **MUST**, **SHOULD**, **MAY**.

---

## 1. Purpose and Execution Rules

### 1.1 What this plan guarantees

The plan is designed so that:

1. Executing every task from WP-1-T1 through WP-6-T9, then the Go-Live tasks GL-T1 through GL-T6, in order delivers the complete initial release described by `TECHNICAL-SPECIFICATIONS.md`.
2. No task consumes an artifact, type, table, or test helper that is produced later in the plan. §14 audits this property.
3. Every product requirement is mapped to implementing tasks and validating tests in §13.
4. Work proceeds in small changes: one task is one reviewable commit (or a short series) with tests written first where behavior is testable.
5. Missing information is stated as an assumption or open question in §5 instead of being silently invented.

### 1.2 Execution rules

- **Sequential packages.** WP-n starts only after the WP-(n-1) exit gate passes. Within a package, tasks are ordered; a task marked `Depends on` must wait for that specific task.
- **Test-first.** For each behavior change, write the failing test first, confirm it fails for the expected reason, then implement the smallest change that passes, then refactor. When TDD is impractical, the task records why and still ships automated coverage.
- **No hosted or live activity before Go-Live.** WP-1 to WP-6 need no remote repository, no hosted CI, no provider key, no live provider call, and no real-user session. Every such activity is in the Go-Live section, after WP-6, so implementation proceeds without external blockers. Quality gates run locally (ADR-0013).
- **No release before Go-Live.** Intermediate packages may expose commands that return a typed "not implemented" error; no artifact is published until the Go-Live release gate (GL-T5).
- **Assumption discipline.** Assumptions marked **blocking** (see §5) must be confirmed by the product owner before the package that depends on them. If an assumption is rejected, the affected tasks are revised before work continues; the plan is not silently reinterpreted.
- **Honest reporting.** A task is complete only when its stated verification command has actually been run and its output recorded in the package report.
- **Scope discipline.** Anything listed in §2.4 as out of scope MUST NOT be implemented opportunistically.

### 1.3 Package status

| Package | Outcome | Status | Gate |
|---|---|---|---|
| WP-1 | Buildable, tested CLI skeleton with a stable contract | Done (local gates); hosted CI deferred to Go-Live | 7.6 |
| WP-2 | Settings, SQLite core, migrations, active-laboratory state | Done (local gates) | 8.6 |
| WP-3 | Laboratories and agent profiles with validation | Done (local gates) | 9.6 |
| WP-4 | Projects, read-only Kanban, safe deletion, scoped memories, schema v3 | Done (local gates) | 10.6 |
| WP-5 | End-to-end Tech Lead planning via DeepSeek with bounded execution, memory context, and GitHub source configuration | Done (local gates) | 11.6 |
| WP-6 | Direct questions with memory context, hardening, deterministic acceptance suite | Done (local gates) | 12.6 |
| Go-Live | Hosted CI, live-provider verification and evaluation, release gate, tag | In progress (CI activated; manual protection pending) | GL.6 |

---

## 2. Requirement Sources, Precedence, and Scope

### 2.1 Sources and authority

| Rank | Source | Role in this plan |
|---|---|---|
| 1 | `.docs/TECHNICAL-SPECIFICATIONS.md` | Primary product requirements (domain, commands, workflows, acceptance criteria §17). |
| 2 | `.docs/ELEVATOR-PITCH.md` | Product intent. Binding only where it does not contradict the specification. |
| 3 | `.docs/CONSTITUTION.md` | Engineering, security, and working-agreement rules. Overrides convenience in all other sources. |
| 4 | `.docs/SECURITY.md` | Security baseline, trust boundaries, required controls, release gate. |
| 5 | `.docs/ARCHITECTURE.md` | Architectural baseline: single process, deterministic orchestration, bounded execution. |
| 6 | `.docs/STACK.md` | Mandatory technology and dependency constraints. |
| 7 | `.docs/PRINCIPLES.md` | Lean/Agile/XP/TDD/BDD/DDD/security-first/Twelve-Factor practices. |
| 8 | `.docs/VALIDATION.md` | Provisional validation slice and test patterns. Its product slice is explicitly replaceable. |

### 2.2 Conflicts and resolutions

| ID | Conflict | Resolution | Affected |
|---|---|---|---|
| D-1 | `VALIDATION.md` defines a "summarize one authorized file" slice, while the VLab specification defines planning workflows. | Adopt the VLab planning slice as the product workflow. `VALIDATION.md` authorizes this ("Replace it if it does not deliver value…"). Retain its security-boundary, execution-limit, cancellation, and evidence-integrity test patterns, adapted in §13.4. | WP-5, WP-6 |
| D-2 | The elevator pitch describes agents collaborating autonomously; `ARCHITECTURE.md` lists multi-agent orchestration as a non-goal and the specification excludes autonomous lifecycle execution. | Agent profiles are persisted data used as planning context. Model-backed operations are Tech Lead planning, direct questions, and a bounded automatic planning-summary memory. No agent-to-agent delegation, no background execution, no automatic story transitions. | WP-3, WP-5, WP-6 |
| D-3 | `TECHNICAL-SPECIFICATIONS.md` §12 allows two transient retries; `VALIDATION.md` uses zero retries. | Retry policy is a typed, configurable component. Product default: at most two retries for retryable transport, 429, and 5xx failures. Security tests may set zero. No retry after cancellation, after an unknown external outcome, or for validation errors. | WP-5 |
| D-4 | `VALIDATION.md` budgets (30 s run, 10 s request) differ from §12 (60 s per attempt). | Limits are injected constants. Production defaults follow §12 and Appendix D; deterministic tests use small values. | WP-5 |
| D-5 | `STACK.md` asks for structured JSON output "where useful"; the product command surface does not define a JSON mode. | Deferred. Text output is the only required interface in the initial release. Recorded as open question Q-03. | WP-6 |
| D-6 | `STACK.md` requires a SQLite-aware backup mechanism; no command is specified. | Backups are out of scope for the initial release and documented as an operational limitation (A-18). | WP-2, WP-6 |
| D-7 | `STACK.md` permits WAL only when justified; the specification is silent. | WAL disabled; single connection. Revisit only with measured concurrency needs. | WP-2 |
| D-8 | The example model name `flash` is not a verified DeepSeek identifier. | No default model identifier is hardcoded. `settings.yaml` must provide one; WP-5 verifies supported identifiers against current provider documentation. | WP-2, WP-5 |

### 2.3 Product goal

A Rust CLI named `vlab` that lets a user create laboratories staffed by AI agent profiles, organize projects, assign agents to a project, submit natural-language requests, and have the Tech Lead produce validated user stories with assignments and due dates, plus a read-only Kanban view and direct questions to agents. Memories are application-managed: deterministic structural facts and a bounded planning summary. Memory content is never an instruction and never logged. All model output is treated as untrusted data; no model output is executed.

### 2.4 Out of scope for the initial release

- Autonomous source-code execution or modification, deployment, or full lifecycle execution.
- Story state transitions and execution commands; the Kanban is read-only and only `TODO` stories are created.
- Manual memory management commands and memory editing; memories are application-managed, immutable, and explicitly scoped.
- Agent-to-agent delegation, background jobs, distributed or multi-user operation.
- Providers other than DeepSeek; a web UI; an HTTP server; PostgreSQL; a TUI framework; an agent framework.
- Database encryption at rest; automatic backup/restore; data export.
- JSON output mode, packaging/distribution channels, and Windows-specific hardening (see §5).

---

## 3. Architectural Baseline

### 3.1 Runtime shape

```text
User (terminal)
     |
     v
 CLI parsing / rendering (clap, stdout/stderr, exit codes)
     |
     v
 Application use cases (settings, labs, agents, projects, requests, ask)
     |
     v
 Domain rules and validation (pure, no I/O)
     |
     +--> SQLite repositories (sqlx, migrations)
     |
     +--> Provider port --> DeepSeek adapter (reqwest + rustls)
     |                  \-> Scripted provider (tests only)
     |
     +--> Observability (tracing to stderr, audit events in SQLite)
```

Control flow, permissions, budgets, persistence, and validation belong to the application. The model only returns structured proposals or text.

### 3.2 Layers and dependency direction

| Layer | Module | May depend on | Must not depend on |
|---|---|---|---|
| Presentation | `cli` | `app`, `domain` | `infra` internals, `reqwest`, `sqlx` |
| Application | `app` | `domain`, ports | `cli` |
| Domain | `domain` | standard library, `thiserror`, time/ID value types | I/O, `sqlx`, `reqwest`, `clap` |
| Infrastructure | `infra` | `domain` | `cli`, `app` use-case logic |
| Observability | `obs` | tracing APIs | domain policy |

Ports (traits) are declared where the application consumes them and implemented in `infra`. Proposed layout is in Appendix A.

### 3.3 Key architecture decisions

| ID | Decision | Rationale |
|---|---|---|
| AD-1 | Modular monolith, one binary, no agent framework | `ARCHITECTURE.md`, `STACK.md`; smallest adequate structure. |
| AD-2 | One model-backed orchestration for planning; one for direct questions | Meets the specification without autonomous multi-agent complexity (D-2). |
| AD-3 | `ModelProvider` port with a DeepSeek adapter and a scripted test double | Deterministic tests; provider replaceable; no live model in CI. |
| AD-4 | SQLite via sqlx, one connection, embedded versioned migrations, runtime queries | Avoids build-time database coupling; migrations are testable. |
| AD-5 | Short 8-character hexadecimal identifiers from UUIDv4, unique per table | "Short UUID-style identifier suitable for terminal use" (§4.2); collision-checked. |
| AD-6 | `settings.yaml` for provider/model; active laboratory in SQLite `app_state` | The settings schema defines only two fields; runtime selection is application state. |
| AD-7 | The model receives no tools and no filesystem/network capabilities | Removes the largest exfiltration and injection surface. |
| AD-8 | Budget, clock, and cancellation are explicit injectable components | Deterministic boundary tests; finite execution (§12, `VALIDATION.md`). |
| AD-9 | Rendering is width-injectable, emits no ANSI color, sanitizes control characters | Meets §10 and `NO_COLOR` with minimal complexity. |
| AD-10 | All persistent timestamps are RFC 3339 UTC; story dates are `YYYY-MM-DD` | §9.5, §11.2. |
| AD-11 | Settings and model-output parsing are strict (unknown fields rejected) | Catches typos and malformed model output early (§5.1, §9.3). |
| AD-12 | Memories are application-managed, scoped, bounded, and delimited untrusted context; the application controls writes | Preserves the authority boundary while allowing lab/project/agent context (§3.5, ADR-0022). |

### 3.4 Model interaction contract

- **Planning:** one logical invocation per accepted request, at most two transient retries. Input: system instructions, agent roster, request text, request creation date, and the scoped memories selected by §3.5 of the specification. Output: strict JSON (`schema_version`, `stories`, `clarification_questions`).
- **Questions:** one logical invocation, same retry policy. Input: agent profile digest, the question, and the selected laboratory/agent memories. Output: plain text answer.
- **Never transmitted:** API key, full database contents, unrelated projects, chat transcripts, CV content (A-13), memories outside the selected scopes, or secrets.
- **Never executed:** model text, generated commands, URLs, or code.
- **Local authority:** validation, assignment checks, due-date calculation, persistence, and permissions are application code.

### 3.5 Agentic workflow contracts (Constitution §6)

**Planning workflow**

| Element | Definition |
|---|---|
| Purpose | Convert one accepted request into a locally validated set of user stories, or a classified clarification/failure. |
| Scope | One logical model invocation; no tool use, no delegation, no code execution, no story transitions. |
| Inputs | Active laboratory, project, request text, agent roster (ID, name, role, skills, workload), request creation date, laboratory/project/Tech Lead memories within 8 KiB. |
| Outputs | Strict JSON (`schema_version`, stories, clarification questions); persisted stories or a terminal status. |
| Tools | None. The provider connection is application infrastructure, not an agent capability. |
| State | Request text, stories, scoped memories, event records, interaction metadata; retention defined in WP-2-T8 and WP-4-T10. |
| Success | All stories valid, assignments restricted to members, due dates computed locally, status `PLANNED`. |
| Limits | §4.8: one invocation, two retries, 60 s per attempt, 240 s total, size/story/token caps, 8 KiB memory context. |
| Recovery | `NEEDS_CLARIFICATION` or `FAILED` with a stable code; request preserved; stale `PLANNING` leased; no automatic repair. |
| Oversight | Human confirmation for destructive actions; no automatic execution of any model output. |

**Question workflow**

| Element | Definition |
|---|---|
| Purpose | Answer one question in the voice of a selected agent or the Tech Lead. |
| Scope | Read-only; no story creation, assignment change, or project mutation. |
| Inputs | Active laboratory, agent profile digest (name, role, description, skills), question, active-laboratory memories, responding agent memories within 8 KiB. |
| Outputs | Plain-text answer displayed with agent and model identification. |
| Tools | None. |
| State | Interaction metadata and an event; no answer or memory content is stored. |
| Success | Answer displayed, no domain table mutated. |
| Limits | One invocation, two retries, 180 s deadline, response cap, 8 KiB memory context. |
| Recovery | Classified provider failure or cancellation; no partial domain effect exists to roll back. |
| Oversight | None required; the answer is advisory text and is never executed. |

---

## 4. Cross-Cutting Conventions

These conventions are implemented in WP-1 and WP-2, then reused by every later package.

### 4.1 Identifiers

- Entity IDs are 8 lowercase hexadecimal characters derived from a UUIDv4, e.g. `a3f91c07`.
- Uniqueness is enforced by the primary key; generation retries on collision (bounded attempts, then a typed internal error).
- IDs are validated against `^[0-9a-f]{8}$` before any query. They are never used as filesystem paths.
- Typed newtypes (`LabId`, `AgentId`, `ProjectId`, `RequestId`, `StoryId`, `MemoryId`) prevent cross-entity misuse.
- Creation commands print the short identifier; all other commands accept exactly that value.

### 4.2 Exit codes

| Code | Meaning | Typical causes |
|---|---|---|
| 0 | Success | Completed command |
| 1 | Operation or provider failure | I/O, database, transport, 5xx after retries, internal error |
| 2 | Invalid arguments, input, or configuration | Unknown flag, conflicting input source, malformed YAML |
| 3 | Missing entity or active context | Unknown ID, no active laboratory |
| 4 | Validation or precondition failure | Invalid laboratory, missing editor, confirmation required in non-interactive mode, invalid planning output |
| 5 | User cancellation | Declined confirmation, aborted prompt |
| 130 | Interrupted execution | SIGINT during a run |

### 4.3 Error contract

- Every user-facing error is rendered to stderr as:

  ```text
  Error: <problem, without sensitive payloads>
  Hint: <next useful action>
  ```

- Errors never include request text, provider bodies, credentials, or file contents.
- Domain modules return `thiserror` enums; the application boundary adds context; `main` maps a single `AppError` to an exit code.
- Production paths do not use `unwrap`/`expect` for expected failures. Panics are reserved for violated internal invariants and are caught and reported as exit 1.

### 4.4 Output streams and rendering

- Results to stdout; diagnostics, progress, warnings, and errors to stderr.
- No ANSI escape sequences are emitted in the initial release; `NO_COLOR` is therefore always honored.
- Rendering receives terminal width as a parameter; production reads terminal width or `COLUMNS`, defaulting to 80 when not a terminal.
- Untrusted strings (names, titles, model output) are sanitized: C0/C1 control characters except newline are removed or replaced with spaces; escape sequences are stripped.
- Long values are truncated with an explicit ellipsis; truncation is never silent.

### 4.5 Observability and audit

- Diagnostics use `tracing` to stderr, disabled unless `RUST_LOG` is set.
- Audit events are persisted in the `events` table for domain-relevant actions: laboratory/agent/project mutations, memory creation/deletion, request intake, planning start/completion/failure, confirmation outcomes, budget exhaustion, cancellation, migration application.
- Event fields: `id`, `run_id`, `action_id`, `kind`, `entity_type`, `entity_id`, `outcome`, `error_code`, `duration_ms`, `attempt`, `provider`, `model`, `created_at`. No prompts, responses, request text, CV content, memory content, or credentials.
- `run_id` is generated once per process invocation; `action_id` identifies a discrete operation.

### 4.6 Paths and permissions

| Purpose | Location | Override | Permissions |
|---|---|---|---|
| Settings | OS per-user config dir + `vlab/settings.yaml` | `VLAB_CONFIG_DIR` | directory `0700`, file `0600` |
| Database | OS per-user app-data dir + `vlab/vlab.db` | `VLAB_DATA_DIR`, `VLAB_DB_PATH` | directory `0700`, file `0600` |
| Editor temp copy | Settings directory | — | file `0600`, removed on exit |

No application state is written to the current working directory by default. Overrides exist for tests and controlled deployments and are documented as operational facilities, not product commands.

### 4.7 Database conventions

- One connection (`max_connections = 1`), `foreign_keys = ON` verified per connection, `busy_timeout = 5 s`, WAL disabled.
- Parameterized queries only; short transactions; multi-entity writes atomic.
- Embedded migrations named `NNNN_description.sql`; applied in order; checksum/version recorded by sqlx.
- A database with a newer schema version than the binary supports is refused with an actionable error (exit 1), not opened read-write.
- Lock contention produces a retry hint, never a silent wait or a generic failure.
- No credentials are stored in the database.

### 4.8 Execution limits (production defaults, provisional)

| Limit | Value |
|---|---|
| Planning invocations per request | 1 |
| HTTP attempts per invocation | 1 + up to 2 retries |
| Per-attempt timeout | 60 s |
| Connect timeout | 10 s |
| Total planning deadline | 240 s |
| Request input | 64 KiB UTF-8 |
| Provider response body | 256 KiB |
| Stories per request | 20 |
| Output tokens per call | 4096 |
| Retry backoff | 500 ms base, ×2, jitter, capped at 8 s |
| Question invocation deadline | 180 s |
| Memory content | 2,000 characters per memory |
| Memories per scope | 20 |
| Memory context per invocation | 8 KiB, whole memories only |

These are code constants with a typed configuration object, not user-editable settings. Tests inject smaller values. Request counts do not imply exact cost control.

### 4.9 Time

- Wall-clock timestamps are RFC 3339 with UTC offset `Z`.
- Story due dates are calendar dates (`YYYY-MM-DD`) derived from the request creation date in UTC.
- Deadline arithmetic and backoff use an injectable clock so tests never sleep on wall time.

### 4.10 Testing conventions

- Unit tests for domain logic and parsing.
- Integration tests with `tempfile` for filesystem and file-backed SQLite.
- CLI tests with `assert_cmd` and `predicates`, isolating `VLAB_CONFIG_DIR`/`VLAB_DATA_DIR` per test.
- Provider contract tests with `wiremock`.
- Memory fixtures for scope, boundary, cascade, and prompt-injection tests; a synthetic secret marker inside a memory proves it never reaches logs or events.
- End-to-end tests use a scripted provider or a local mock server, never a live provider.
- Live tests are opt-in, `#[ignore]` by default, environment-gated, budget-limited, never committed with credentials, and written and run only in Go-Live (GL-T2, GL-T3).
- No test asserts on incidental prose except where the specification fixes an exact message (e.g. §3.4).

---

## 5. Assumptions and Open Questions

Assumptions are decisions this plan must make to remain executable. **Blocking** assumptions need confirmation before the named package; non-blocking assumptions may be revised later without invalidating prior work.

### 5.1 Assumptions

| ID | Assumption | Impact | Blocking |
|---|---|---|---|
| A-01 | `TECHNICAL-SPECIFICATIONS.md` is the authoritative product definition; the `VALIDATION.md` slice is replaced as permitted by its own text (D-1). | WP-5, WP-6 tests | Yes, before WP-5 |
| A-02 | The DeepSeek model identifier is unknown until verified; no default is hardcoded. `settings.yaml` must specify one; WP-5 records the verified identifier and contract. | Settings, planning | Yes, before WP-5 |
| A-03 | IDs are 8 lowercase hex characters; no prefix resolution (full display value is the key). | All entities | No |
| A-04 | The active laboratory is stored in SQLite `app_state`, not in `settings.yaml`. | Settings, labs | No |
| A-05 | Story priority is `low`, `medium`, or `high`. | Domain, planning schema | Yes, before WP-5 |
| A-06 | Skills are lowercase slug tags (1–32 chars, `[a-z0-9-]`), deduplicated, at most 20 per agent. | Agent wizard, validation | No |
| A-07 | `--check-lab` requires at least one skill for members with role `dev`, `analyst`, or `qa`; the `lead` may have none. | Laboratory validation | Yes, before WP-3 |
| A-08 | Destructive commands prompt `y/N` on a terminal; a `--yes` modifier permits non-interactive execution. Without `--yes` and without a terminal the command fails with exit 4. | Deletion UX, tests | Yes, before WP-4 |
| A-09 | `--new-agent` requires interactive stdin. Non-interactive creation is not supported in the initial release; the wizard logic is unit-tested through a prompt port, with an optional PTY smoke test. | Agent creation | Yes, before WP-3 |
| A-10 | "Unfinished assigned story" means any story whose state is not `DONE`. Initially only `TODO` exists. | Agent deletion | No |
| A-11 | An active planning operation is `requests.status = 'PLANNING'` within a lease equal to the planning deadline. A stale lease is reported as failed on the next command. Single-process assumption: no concurrent CLI invocations are supported. | Deletion guards, recovery | No |
| A-12 | Input/response/story/token limits are the values in §4.8 and are provisional until measured. | Planning | No |
| A-13 | CV/profile prose is not sent to the provider; planning uses role, description, and skills only. | Planning context, privacy | Yes, before WP-5 |
| A-14 | The Tech Lead is a member and may be assigned stories like any other member. | Assignments | No |
| A-15 | Agent deletion archives the profile (`archived_at`), removes memberships, and preserves story references with an "archived" label. | Schema, deletion | No |
| A-16 | The initial release emits no color; `NO_COLOR` is trivially satisfied. | Rendering | No |
| A-17 | JSON output is deferred (Q-03). | CLI | No |
| A-18 | Backup/restore and encryption at rest are out of scope; both are documented limitations with operational recommendations. | Docs, release | No |
| A-19 | CI targets are macOS and Linux (GitHub Actions); Windows is best-effort and Unix permission assertions are gated. | CI, tests | No |
| A-20 | Distribution, packaging, and licensing are out of scope; the binary is built with `cargo build --locked`. | Release | No |
| A-21 | No vulnerability disclosure channel exists yet; the first release is internal until the channel is established. | Release gate | Yes, before Go-Live |
| A-22 | An agent may belong to multiple laboratories; `--new-agent` pre-fills the active laboratory when one is selected and accepts additional existing laboratory IDs. If no laboratory is active, at least one existing membership must be entered. At most one `lead` per laboratory. | Agent creation | No |
| A-23 | Laboratory and project names are trimmed, 1–100 characters, and need not be globally unique. | Domain | No |
| A-24 | `--ask` sends only the agent profile digest and the question; no project or story data. | Privacy, ask | No |
| A-25 | Retry policy default is two transient retries (D-3); tests may use zero. | Provider | No |
| A-26 | WAL is disabled; SQLite over network filesystems is unsupported and documented. | Persistence | No |
| A-27 | Planning output is rejected as a whole if any story fails validation; no partial story set is committed. | Planning | No |
| A-28 | Model output uses `schema_version: 1`; unknown fields are rejected. Provider transport envelopes tolerate unknown fields. | Provider, validation | No |
| A-29 | Unknown fields in `settings.yaml` are rejected to catch typos. | Settings | No |
| A-30 | A planned request is excluded from the pending-intake count; `TODO`, `PLANNING`, `NEEDS_CLARIFICATION`, and `FAILED` requests are counted. | Kanban | No |
| A-31 | Memories are application-managed, immutable text attached to exactly one laboratory, project, or agent profile. The application writes structural facts and a bounded planning summary; there are no memory commands. | Memory storage | No |
| A-32 | Planning includes project-laboratory, project, and Tech Lead agent memories; questions include active-laboratory and responding-agent memories. Ordering is lab, project, agent, newest first, with an 8 KiB total cap and whole-memory omission. | Prompt context, privacy | No |
| A-33 | A memory is 1–2,000 characters and a scope holds at most 20 memories. Memories are global to an agent profile, not per membership. | Storage, prompt bounds | No |
| A-34 | Superseded: memories have no CLI; project and agent selectors were removed with the manual commands. | Memory CLI | No |

### 5.2 Open questions

| ID | Question | Default if unanswered |
|---|---|---|
| Q-01 | Which DeepSeek model identifier is supported and stable? | No default; WP-5 verifies and records one. |
| Q-02 | Is `low/medium/high` the desired priority scale? | Adopt A-05. |
| Q-03 | Is a machine-readable JSON mode required for scripting? | Not implemented. |
| Q-04 | Is a `--yes` bypass acceptable for destructive commands? | Adopt A-08 with confirmation binding. |
| Q-05 | Must non-interactive agent creation be supported? | Not in the initial release (A-09). |
| Q-06 | May CV content be sent to the provider? | No (A-13). |
| Q-07 | What are the distribution channel and license? | None; internal build only. |
| Q-08 | Who owns vulnerability response and where is the private channel? | Internal release only (A-21). |
| Q-09 | Should laboratory names be unique per user? | No (A-23). |
| Q-10 | Should memories be editable? | No in the initial release; delete and recreate (A-31). |
| Q-11 | Should agent memory be per laboratory membership? | No; global to the agent profile (A-33). |

### 5.3 Documentation gaps and how this plan closes them

| Gap in the source material | Resolution in this plan |
|---|---|
| `STACK.md` lists no YAML parser although settings are YAML | WP-1-T2 selects and verifies a maintained parser; recorded in `docs/dependencies.md`. |
| The example model `flash` is not a verified provider identifier | A-02 and Q-01: no default; WP-5 verifies and records a supported identifier. |
| Story priority values are not defined | A-05 and Q-02: `low`, `medium`, `high`. |
| "Short UUID-style identifier" is ambiguous | A-03: 8 lowercase hex characters, collision-checked, stored as the key. |
| Confirmation and non-interactive deletion behavior are unspecified | A-08 and Q-04: interactive `y/N`, explicit `--yes`, non-TTY fails closed. |
| Where the active laboratory is persisted is unspecified | A-04: SQLite `app_state`; settings remain provider/model only. |
| "Active provider operation" cannot be observed across processes | A-11: single-process assumption plus a conservative `PLANNING` lease. |
| No backup command, encryption, distribution, license, or disclosure channel is defined | A-18, A-20, A-21 and §13.5 record them as explicit limitations or release-gate blockers. |
| `STACK.md` mentions JSON output only as optional | A-17 and D-5 defer it; the text interface remains the required contract. |

---

## 6. Working Package Overview

| Package | Primary outcome | Depends on | Specification coverage |
|---|---|---|---|
| WP-1 | Repository, toolchain, CLI contract, errors, observability, local gates | — | §4.1, §14, §15 (partial), `STACK.md` |
| WP-2 | Settings lifecycle and SQLite foundation | WP-1 | §5, §11.1, §14 |
| WP-3 | Laboratories and agent profiles | WP-2 | §3, §6, §7 (except delete/ask) |
| WP-4 | Projects, Kanban, deletions, scoped memories, schema v3 | WP-3 | §3.5, §4.1, §6.4, §7.3, §8, §10, §11.2 |
| WP-5 | Request intake and Tech Lead planning with DeepSeek and scoped memory context | WP-4 | §3.5, §9, §12, §16 (provider), §13 |
| WP-6 | Direct questions with memory context, hardening, deterministic acceptance suite | WP-5 | §3.5, §7.5, §7.6, §13, §16, §17 |
| Go-Live | Hosted CI, live verification and evaluation, release gate, tag | WP-6 | §16 (live tests), §17 (release evidence), `SECURITY.md` release gate |

Dependency chain: **WP-1 → WP-2 → WP-3 → WP-4 → WP-5 → WP-6 → Go-Live**. There are no back edges and no package may start early. Tasks inside a package are sequential except where explicitly marked as parallel-safe; this plan does not require parallelism.

---

## 7. WP-1 — Foundation and Delivery Guardrails

### 7.1 Objective

Produce a buildable Rust binary with the complete CLI contract, a typed error/exit-code model, observability and redaction, isolated test helpers, and enforced local quality and supply-chain gates, with hosted CI prepared for Go-Live. No domain behavior is delivered in this package.

### 7.2 Scope

**In scope:** repository setup, pinned toolchain, verified dependencies, crate/module skeleton, full `clap` argument surface from §4.1, one-primary-action enforcement, exit codes, path resolution, tracing, redaction, test harness, CI, ADRs.

**Out of scope:** settings semantics, database, domain entities, provider calls.

### 7.3 Prerequisites and dependencies

None. This package needs only the documentation in §2 and approval of no blocking assumption.

**Execution checklist**

- [x] T1 — Initialize the repository and baseline hygiene
- [x] T2 — Verify and pin the toolchain and dependencies (after T1)
- [x] T3 — Create the crate skeleton and enforce module boundaries (after T2)
- [x] T4 — Implement the complete CLI contract (after T3)
- [x] T5 — Define the error taxonomy and exit-code mapping (after T4)
- [x] T6 — Resolve paths and create private directories (after T5)
- [x] T7 — Establish observability and redaction (after T6)
- [x] T8 — Build the test harness (after T7)
- [x] T9 — Add CI and supply-chain gates (after T2, T8) — workflows committed disabled; activation in GL-T1
- [x] T10 — Record architecture decisions and contributor guidance (after T3, T5, T6, T9)

### 7.4 Tasks

#### WP-1-T1 · Initialize the repository and baseline hygiene

**Spec:** `CONSTITUTION.md` §7, §9; `STACK.md` dependency policy.
**Depends on:** none.

**Work**
- Initialize Git in `/Users/massimo/sviluppo/personale/vlab`.
- Add `.gitignore`: `target/`, `*.db`, `*.db-wal`, `*.db-shm`, `.env`, OS artifacts, editor backups. Track `AGENTS.md`, `.docs/`, `.agents/`, and `.project/IMPLEMENTATION-PLAN.md`; ignore `.tmp/` and all other content under `.project/` (ADR-0007).
- Add a minimal `README.md` stating the project purpose, build command, and documentation index.
- Make the initial commit.

**Deliverables**
- `.gitignore`, `README.md`, initial commit containing the existing documentation.

**Done when**
- `git status` is clean after `cargo build` (once the crate exists) and ignored artifacts are absent from `git status`.

#### WP-1-T2 · Verify and pin the toolchain and dependencies

**Spec:** `STACK.md` version and maintenance policy; `CONSTITUTION.md` §4.
**Depends on:** T1.

**Work**
- Select a currently supported stable Rust toolchain from upstream release information; create `rust-toolchain.toml` with that channel and `clippy`/`rustfmt` components.
- Create `Cargo.toml` (edition matching the toolchain) with only the dependencies required by the plan: `clap` (derive), `tokio`, `sqlx` (sqlite, runtime-tokio, migrate), `serde`, `serde_json`, `reqwest` (rustls-tls, json, no default features), `thiserror`, `anyhow`, `tracing`, `tracing-subscriber`, `directories`, `uuid` (v4), a maintained YAML parser, a date/time crate, `terminal_size`, `unicode-width`, a maintained shell-word parser for editor arguments.
- Verify each candidate against upstream release status, RustSec advisories, license, and maintenance activity. Record the result in `docs/dependencies.md` with version, purpose, license, and verification date.
- Commit `Cargo.lock`; enable only needed features.

**Deliverables**
- `rust-toolchain.toml`, `Cargo.toml`, `Cargo.lock`, `docs/dependencies.md`, `src/main.rs` placeholder.

**Done when**
- `cargo build --locked` succeeds on the pinned toolchain.
- The dependency register lists every direct dependency with a justification and no unmaintained or known-vulnerable selection.
- The YAML parser choice is explicit and justified (the ecosystem has unmaintained options; this is a deliberate decision, not a default).

#### WP-1-T3 · Create the crate skeleton and enforce module boundaries

**Spec:** §15; `ARCHITECTURE.md` components; AD-1, AD-2.
**Depends on:** T2.

**Work**
- Create `src/lib.rs` exposing `cli`, `app`, `domain`, `infra`, `obs`; `src/main.rs` remains a thin entry point that parses arguments, calls the dispatcher, and exits with the mapped code.
- Add `#![forbid(unsafe_code)]` at the crate root.
- Create empty modules with doc comments describing allowed dependencies (Appendix A).
- Add a compile-time test that asserts the crate builds without `unsafe` (the attribute itself is the enforcement).

**Deliverables**
- Module tree per Appendix A; `src/main.rs` with no business logic beyond dispatch and exit mapping.

**Done when**
- `cargo clippy --locked --all-targets -- -D warnings` passes.
- `src/main.rs` contains no `unwrap`, no direct database/provider code, and no output formatting beyond error rendering.

#### WP-1-T4 · Implement the complete CLI contract

**Spec:** §4.1, §4.2; `STACK.md` textual UI conventions; A-08.
**Depends on:** T3.

**Work**
- Implement the full flag surface with `clap` derive and `help_heading` groups exactly as §4.1 lists them at this stage: Settings, Laboratories, Projects, Agents. WP-4-T9 adds the Memories group.
- Add the `--yes` modifier for destructive commands (A-08) and `--to-agent` restricted to `--ask`.
- Enforce exactly one primary action per invocation; no arguments prints concise help and exits 0 without touching state; unknown flags and invalid combinations exit 2 with usage on stderr.
- Dispatch each action to a handler module; handlers not yet implemented return a typed `NotImplemented` error (exit 1) with a message that the command is unavailable in this build.
- Ensure `--to-agent` without `--ask` and `--ask` without a question are usage errors.

**Deliverables**
- `src/cli/args.rs`, `src/cli/dispatch.rs`, `src/cli/help.rs` (if needed), snapshot tests in `tests/cli_help.rs` and `tests/cli_args.rs`.

**Done when**
- `vlab --help` exits 0, writes to stdout, and lists all commands under the four groups delivered at this stage.
- `vlab` with no arguments prints the same concise help and performs no I/O beyond help output.
- Representative invalid combinations (two actions, unknown flag, modifier misuse, missing value) each exit 2 with a diagnostic on stderr and empty stdout.
- `vlab --version` exits 0 and prints the crate version.

#### WP-1-T5 · Define the error taxonomy and exit-code mapping

**Spec:** §14; §4.3.
**Depends on:** T4.

**Work**
- Define `AppError` with variants for usage, configuration, missing entity, no active laboratory, validation, precondition, confirmation required, cancellation, provider, storage, I/O, interrupted, and internal errors.
- Map each variant to an exit code per §4.2 and render the `Error:`/`Hint:` contract.
- Add a narrow `NotImplemented` variant used only by scaffold handlers; remove it by WP-6.
- Ensure `main` catches panics from the runtime boundary and reports exit 1 without a backtrace unless `RUST_BACKTRACE` is set.

**Deliverables**
- `src/domain/errors.rs` (or `src/app/errors.rs` depending on final module split), `src/cli/exit.rs`, unit tests covering every variant-to-code mapping, `tests/cli_errors.rs`.

**Done when**
- A table-driven test asserts the exit code for every variant.
- An unknown entity produces exit 3 and an actionable hint.
- No error rendering includes request content, provider bodies, or credentials.
- `cargo clippy -D warnings` passes; no `unwrap` on fallible production paths.

#### WP-1-T6 · Resolve paths and create private directories

**Spec:** `STACK.md` storage; §5.1; A-04.
**Depends on:** T5.

**Work**
- Implement resolution for settings directory, data directory, database path, and editor temporary path with `directories`, honoring `VLAB_CONFIG_DIR`, `VLAB_DATA_DIR`, and `VLAB_DB_PATH`.
- Create directories with `0700` on Unix and set files to `0600`; fail closed with an actionable error if creation fails.
- Guarantee that `--help`, `--version`, and argument errors create no files or directories.
- Provide a way for tests to isolate all paths without touching the real user profile.

**Deliverables**
- `src/infra/paths.rs`, `tests/paths.rs`, README section documenting locations and overrides.

**Done when**
- Tests on Unix assert directory mode `0700` and that a file created through the private-file helper is `0600`; database file permissions are asserted in WP-2-T1 when the database exists.
- Overrides are honored; no default write occurs in the current working directory.
- Help/version leave an empty temporary home untouched.

#### WP-1-T7 · Establish observability and redaction

**Spec:** `SECURITY.md` Logging and Audit; `CONSTITUTION.md` §6; §4.5.
**Depends on:** T6.

**Work**
- Initialize `tracing-subscriber` to stderr only, off by default, configurable through `RUST_LOG`, with no ANSI when not a terminal.
- Define audit event kinds and fields from §4.5; create a small event-emission API that cannot accept arbitrary payload strings.
- Implement redaction for diagnostics: never accept request text, provider bodies, CVs, or credentials as fields.
- Add a synthetic secret marker to tests and assert it never appears on stdout, stderr, or in an error rendered through the app boundary.

**Deliverables**
- `src/obs/mod.rs`, `src/obs/events.rs`, `tests/redaction.rs`, `docs/observability.md` describing event kinds.

**Done when**
- `RUST_LOG` unset produces no diagnostics beyond user-facing output.
- A test that deliberately formats a context containing `sk-vlab-SECRET-MARKER` proves the marker is absent from captured output.
- The event API has no `detail: String` escape hatch.

#### WP-1-T8 · Build the test harness

**Spec:** `STACK.md` testing; `VALIDATION.md` test strategy.
**Depends on:** T7.

**Work**
- Create `tests/support/mod.rs` with:
  - an isolated command builder that sets `VLAB_CONFIG_DIR`, `VLAB_DATA_DIR`, and clears `RUST_LOG`/provider variables;
  - helpers asserting exit code, stdout, and stderr independently;
  - temporary-directory and fixture builders;
  - documented extension points for the scripted provider added in WP-5 (no provider types are introduced here).
- Add `assert_cmd`, `predicates`, `tempfile`, and `wiremock` as development dependencies after verification.

**Deliverables**
- `tests/support/mod.rs`; at least one test in each existing CLI test file using the helper.

**Done when**
- `cargo test --locked` runs deterministically with no network access and no writes outside temporary directories.
- The helper can assert "stdout empty, stderr contains X, exit code N" in one call.

#### WP-1-T9 · Add CI and supply-chain gates

**Spec:** `STACK.md` Quality and Security Tooling; `SECURITY.md` CI permissions.
**Depends on:** T2, T8.

**Work**
- Add a CI workflow (GitHub Actions assumed, A-19) running on push and pull request:
  - `cargo fmt --all -- --check`
  - `cargo clippy --locked --all-targets --all-features -- -D warnings`
  - `cargo test --locked`
  - `cargo audit`
  - `cargo deny check`
- Use a macOS + Linux matrix, pinned action versions, minimal `permissions: contents: read`, and cached Cargo artifacts keyed by lockfile.
- Add `deny.toml` with advisory, license, bans, and source policies that reject unmaintained crates and unknown registries.
- Add Dependabot for Cargo and GitHub Actions (choose one updater; do not add Renovate too).
- Add a daily scheduled security scan.
- Document required branch protection as a manual repository setting.

**Deliverables**
- `.github/workflows/ci.yml`, `deny.toml`, `.github/dependabot.yml`, `docs/ci.md`.

**Done when**
- The workflow and Dependabot files are reviewed and committed, commented out until Go-Live (ADR-0013).
- A deliberately introduced formatting or clippy violation fails the local gates (demonstrated once and reverted).
- `cargo deny check` and `cargo audit` run locally with documented commands.
- The first hosted run happens in GL-T1, not in this package.

#### WP-1-T10 · Record architecture decisions and contributor guidance

**Spec:** `CONSTITUTION.md` §9; §15; AD-1…AD-11.
**Depends on:** T3, T5, T6, T9.

**Work**
- Write ADR-0001 (layered architecture and module boundaries), ADR-0002 (settings vs application state and file locations), ADR-0003 (single orchestration, no agent framework), ADR-0004 (identifier format and collision policy), ADR-0005 (SQLite connection/migration policy), ADR-0006 (rendering and no-color policy).
- Write `CONTRIBUTING.md`: TDD loop, required commands, assumption/ADR process, security review checklist.
- Link documentation from `README.md`.

**Deliverables**
- `docs/adr/0001…0006`, `CONTRIBUTING.md`, updated `README.md`.

**Done when**
- Every decision in §3.3 has an ADR or is covered by one.
- A reviewer can follow `CONTRIBUTING.md` to run all gates without additional instructions.

### 7.5 Tests and acceptance criteria

| ID | Criterion | Verification |
|---|---|---|
| WP1-AC1 | Full help documents all §4.1 commands in the groups delivered at this stage; WP-4 adds Memories | `cargo test --test cli_help` snapshot |
| WP1-AC2 | No-argument invocation shows help, exits 0, writes nothing | CLI test with empty isolated home |
| WP1-AC3 | Exactly one action is enforced; invalid combinations exit 2 | Table-driven `tests/cli_args.rs` |
| WP1-AC4 | Every error variant maps to the documented exit code | Table-driven unit test |
| WP1-AC5 | Paths are per-user, private, and overridable; help creates nothing | `tests/paths.rs` (Unix modes) |
| WP1-AC6 | Secrets never reach stdout, stderr, or rendered errors | `tests/redaction.rs` |
| WP1-AC7 | Locked build and all quality gates pass | Local `cargo fmt`, `clippy`, `test`, `audit`, `deny` (hosted CI in GL-T1) |
| WP1-AC8 | Module boundaries are enforceable and documented | Review of `src/lib.rs`, ADR-0001, clippy gate |

### 7.6 Exit gate

- All WP1-AC pass with the local gates.
- `cargo build --locked` from a clean checkout reproduces the binary.
- `docs/dependencies.md` and ADRs are merged.
- The complete CLI surface is stable before domain work begins; later packages implement handlers without renaming flags.

### 7.7 Package report (2026-11-12)

Executed locally on macOS (aarch64), Rust 1.99.1: `cargo fmt --all -- --check`, `cargo clippy --locked --all-targets [--all-features] -- -D warnings`, `cargo test --locked` (45 passed, 0 failed), `cargo audit` (no findings), `cargo deny check` (advisories, bans, licenses, sources ok). A deliberate fmt and clippy violation was shown to fail both gates and reverted. `cargo build --locked` succeeded from a clean clone.

Deviations, each recorded in an ADR or `AGENTS.md`:

- Documents planned under `docs/` live in `.docs/` (`DEPENDENCIES.md`, `OBSERVABILITY.md`, `CI.md`); ADRs continue the existing numbering (ADR-0008 to ADR-0012) because ADR-0001 to ADR-0006 already existed.
- Only crates used by WP-1 were added; the rest are deferred to their first-use package (ADR-0008). `predicates` and `wiremock` are deferred too.
- Modifier rules (`--to-agent`, `--yes`) are enforced in code, not with clap `requires` (ADR-0009).
- `CONTRIBUTING.md` points to `AGENTS.md` instead of repeating its rules (ADR-0007, no duplication).

Hosted CI is deferred to Go-Live (ADR-0013, superseding ADR-0012): the workflow files are committed commented out, and GL-T1 activates them. Not verified: Linux was not tested locally; A-08 (`--yes`) still needs product-owner confirmation before WP-4.

---

## 8. WP-2 — Configuration and Persistence Core

### 8.1 Objective

Deliver the settings lifecycle (`settings.yaml` read, show, atomic edit) and a private, migration-managed SQLite database, including application state for the active laboratory. No laboratories, agents, or projects exist yet.

### 8.2 Scope

**In scope:** database connection and pragmas, migration runner, schema v1 (`app_state`, `events`), settings model/loader, `--show-settings`, `--edit-settings`, active-laboratory storage primitives, retention policy, dispatcher wiring.

**Out of scope:** domain entities, provider calls, laboratory commands.

### 8.3 Prerequisites and dependencies

- WP-1 gate passed: paths, errors, CLI contract, test harness.
- Non-blocking assumptions A-04, A-26, A-29 accepted.

**Execution checklist**

- [x] T1 — Implement the SQLite connection and pragmas
- [x] T2 — Add the migration runner and schema v1 (after T1)
- [x] T3 — Implement the settings model and loader (after T2)
- [x] T4 — Implement `--show-settings` (after T3)
- [x] T5 — Implement `--edit-settings` with atomic replacement (after T3)
- [x] T6 — Implement the application-state repository (after T2)
- [x] T7 — Implement the no-active-context contract (after T6)
- [x] T8 — Define and enforce the retention and sensitive-data policy (after T2)
- [x] T9 — Wire settings commands and close the package (after T4, T5, T7)

### 8.4 Tasks

#### WP-2-T1 · Implement the SQLite connection and pragmas

**Spec:** §11.1; `STACK.md` SQLite Persistence; AD-4, AD-5.
**Depends on:** WP-1 gate.

**Work**
- Open or create the database at the resolved path with `create_if_missing`; set file permissions `0600` on Unix.
- Apply and verify `PRAGMA foreign_keys = ON` on every connection; set `busy_timeout` to 5 s; keep one connection; WAL off.
- Map SQLite errors to `StorageError`: busy/locked with a retry hint, permission denied, corrupt database, disk full.
- Refuse to open a database whose `PRAGMA user_version`/migration state is newer than this binary supports.

**Deliverables**
- `src/infra/db/mod.rs`, `tests/db_connect.rs`.

**Done when**
- A test opens two connections, holds a write transaction, and asserts the second write fails with the documented lock message and exit code 1.
- A test asserts `foreign_keys` is `1` on each connection.
- Unix test asserts database file mode `0600` and parent directory `0700`.
- No connection is created by `--help`.

#### WP-2-T2 · Add the migration runner and schema v1

**Spec:** §11.1; `STACK.md`; AD-4.
**Depends on:** T1.

**Work**
- Embed migrations with `sqlx::migrate!`; create `migrations/0001_init.sql` containing:
  - `app_state(key TEXT PRIMARY KEY, value TEXT NOT NULL, updated_at TEXT NOT NULL)`;
  - `events(id TEXT PRIMARY KEY, run_id TEXT NOT NULL, action_id TEXT, kind TEXT NOT NULL, entity_type TEXT, entity_id TEXT, outcome TEXT, error_code TEXT, duration_ms INTEGER, attempt INTEGER, provider TEXT, model TEXT, created_at TEXT NOT NULL)`;
  - indexes on `events(run_id)`, `events(entity_type, entity_id)`, and `events(created_at)`.
- Apply migrations at startup; record an audit event `db_migrated` with the from/to version only when a migration actually runs.
- Ensure failed migrations roll back and report an actionable error.

**Deliverables**
- `migrations/0001_init.sql`, migration code in `src/infra/db/migrate.rs`, `tests/db_migrations.rs`.

**Done when**
- A fresh temporary database reaches the expected version; re-running applies nothing.
- A deliberately invalid migration in a test fails without leaving partial schema.
- A database created by a newer binary version is rejected with exit 1.

#### WP-2-T3 · Implement the settings model and loader

**Spec:** §5.1; §5.4; A-29.
**Depends on:** T2.

**Work**
- Define typed `Settings { provider: ProviderId, model: ModelId }` with `ProviderId` restricted to `deepseek` for now and `ModelId` non-empty, trimmed, length-bounded.
- Parse YAML strictly (reject unknown fields), returning exit 2 with the file path and a hint for missing file, malformed YAML, wrong types, unknown provider, and empty model.
- Never store credentials outside `settings.yaml`; expose only `set`/`unset` for `credentials.deepseek_api_key` without revealing the value.
- Do not contact the provider.

**Deliverables**
- `src/infra/config.rs`, `src/domain/settings.rs`, unit tests for every failure mode, `tests/fixtures/settings-*.yaml`.

**Done when**
- Valid fixture loads; each invalid fixture produces exit 2 and a path-specific message.
- A file containing a plausible API key produces no output containing that value.
- Settings parsing is a pure function except for file reading, and is unit-testable with in-memory strings.

#### WP-2-T4 · Implement `--show-settings`

**Spec:** §5.2; §14.
**Depends on:** T3.

**Work**
- Display the resolved settings path, provider, model, validation status, and whether `credentials.deepseek_api_key` is set (`set`/`unset` only).
- Missing or invalid settings exit 2 with a hint to run `--edit-settings`.
- Make no remote calls and create no files.

**Deliverables**
- Handler in `src/app/settings.rs`, CLI test `tests/cli_settings_show.rs`.

**Done when**
- stdout contains path/provider/model and never the key value.
- Invalid settings produce exit 2 with stderr-only diagnostics.
- A test with the synthetic key in `settings.yaml` asserts the marker is absent from all output.

#### WP-2-T5 · Implement `--edit-settings` with atomic replacement

**Spec:** §5.3; `SECURITY.md` filesystem; `CONSTITUTION.md` shell standards.
**Depends on:** T3.

**Work**
- Resolve the editor from `VISUAL`, then `EDITOR`; parse the value into executable plus arguments with a documented quoting parser; never invoke a shell and never `eval`.
- Create the settings directory if needed; if the file is missing, write a commented template containing the two required fields with a placeholder model value.
- Copy to a `0600` temporary file in the same directory, launch the editor as a child process, wait for exit.
- On editor exit 0, parse and validate the edited file; on success, fsync, rename over the original, and fsync the directory. On any failure, keep the previous file and remove the temporary file.
- Missing editor, editor non-zero exit, and invalid content produce actionable errors with the documented exit codes.

**Deliverables**
- `src/app/settings_edit.rs`, `src/cli/prompt.rs` (editor launch), test scripts under `tests/fixtures/editors/`, `tests/cli_settings_edit.rs`.

**Done when**
- Valid edit replaces the file; invalid edit preserves the previous bytes (asserted by hash).
- Editor argument quoting is documented and tested with a path containing a space.
- A test editor that writes a plausible secret into an invalid file leaves no temporary file and no secret on disk after cleanup.
- No shell interpreter is involved (test editor asserts it receives arguments directly).

#### WP-2-T6 · Implement the application-state repository

**Spec:** §3.4; §6.3; A-04.
**Depends on:** T2.

**Work**
- Implement get/set/clear for `active_lab_id` in `app_state` with a typed wrapper and parameterized queries.
- Corrupt or invalid stored values are treated as "no active laboratory" with a warning, not a panic.
- Provide a transaction-scoped setter used later by `--open-lab` and deletion.

**Deliverables**
- `src/infra/db/app_state.rs`, `src/app/context.rs`, tests with a file-backed temporary database.

**Done when**
- A value persists across two separate connections/invocations.
- Clearing restores the no-context state.
- Corrupt values produce the documented no-context behavior.

#### WP-2-T7 · Implement the no-active-context contract

**Spec:** §3.4.
**Depends on:** T6.

**Work**
- Add a shared helper that returns the exact §3.4 message and exit 3:

  ```text
  Error: no active laboratory is selected.
  Run `vlab --list-labs`, then `vlab --open-lab <lab-id>`.
  ```

- Expose it to commands that require laboratory context (wired in WP-3/WP-4).

**Deliverables**
- `src/app/context.rs` helper, unit test asserting exact text and code.

**Done when**
- The test compares the full message and exit code, not a substring.
- The helper is exported for the first context-requiring commands in WP-3 and reads no other state.

#### WP-2-T8 · Define and enforce the retention and sensitive-data policy

**Spec:** `SECURITY.md` Secrets and Data Privacy; §11.2; D-6; A-18.
**Depends on:** T2.

**Work**
- Write `docs/data-retention.md`: what is stored (request text, agent profiles, skills, optional CV), what is not (provider credentials, chat transcripts, provider payloads, prompts), why request text is retained (traceability), deletion behavior on laboratory/project deletion, and that SQLite is unencrypted at rest (recommend OS disk encryption).
- Add a schema test that rejects credential-bearing columns (for example `api_key`, `secret`, `password`, `credential`, `access_token`, `auth_token`) in application tables; `app_state.key` and usage counters such as `tokens_in` are documented exceptions because they hold no credentials.
- Document that no automatic purge exists and that backups are out of scope.

**Deliverables**
- `docs/data-retention.md`, schema policy test.

**Done when**
- The document is reviewed and linked from `README.md`.
- The schema test fails if a forbidden column is added.
- Every stored field in schema v1 is justified in the document.

#### WP-2-T9 · Wire settings commands and close the package

**Spec:** §5; §14.
**Depends on:** T4, T5, T7.

**Work**
- Connect `--show-settings` and `--edit-settings` to the dispatcher, removing their scaffold stubs.
- Ensure dispatch initializes paths lazily: settings commands never open the database unless needed.
- Add CLI end-to-end tests for both commands through the real binary.

**Deliverables**
- Updated `src/cli/dispatch.rs`, `tests/e2e_settings.rs`.

**Done when**
- Both commands work through `assert_cmd` with isolated directories.
- Exit codes and stream separation match §14.
- No database file is created by settings-only commands.

### 8.5 Tests and acceptance criteria

| ID | Criterion | Verification |
|---|---|---|
| WP2-AC1 | Database opens with FK enforcement, busy timeout, and private permissions | `tests/db_connect.rs` |
| WP2-AC2 | Migrations are versioned, idempotent, and refuse newer schemas | `tests/db_migrations.rs` |
| WP2-AC3 | Settings load/validate with actionable errors and no secrets | Unit tests + `tests/cli_settings_show.rs` |
| WP2-AC4 | Invalid edits never replace a valid settings file | `tests/cli_settings_edit.rs` (hash comparison) |
| WP2-AC5 | Active laboratory state persists and clears correctly | App-state tests |
| WP2-AC6 | No-active-context message matches §3.4 exactly | Exact-string test |
| WP2-AC7 | Retention policy is documented and schema-enforced | `docs/data-retention.md` + schema test |
| WP2-AC8 | Settings commands perform no network or database work | Test asserting no DB file and no HTTP |

### 8.6 Exit gate

- All WP2-AC pass.
- A user can create/edit valid settings, see them, and recover from an invalid edit without data loss.
- The database initializes and migrates on first use with correct permissions.
- The active-laboratory primitives are ready for WP-3.

### 8.7 Package report (2026-11-12)

Executed locally on macOS (aarch64), Rust 1.99.1: `make check`, which runs
`cargo fmt --all -- --check`, `cargo clippy --locked --all-targets -- -D warnings`,
`cargo test --locked` (110 passed, 0 failed), `cargo audit`
(no findings, 1471 advisories loaded), and `cargo deny check` (advisories,
bans, licenses, and sources ok).

Acceptance evidence:

| Criterion | Evidence |
|---|---|
| WP2-AC1 | `tests/db_connect.rs`: FK pragma on every acquired connection, busy timeout 5000 ms, journal mode `delete`, file `0600`, parent `0700`, lock failure classified and exit 1, corrupt file, newer schema, `--help` creates nothing. |
| WP2-AC2 | `tests/db_migrations.rs`: fresh migration reaches v1 and records `migration_applied` 0→1; a second run applies nothing; a broken migration rolls back (`partial` and `events` absent, `user_version` 0); modified history and older targets are refused. |
| WP2-AC3 | `src/infra/config.rs` unit tests over every fixture; `tests/cli_settings_show.rs` shows path/provider/model and never the key value. |
| WP2-AC4 | `tests/cli_settings_edit.rs` compares previous bytes after an invalid edit, checks private modes, argument quoting, stale-draft removal, and no secret left on disk. |
| WP2-AC5 | `tests/db_app_state.rs`: selection survives a second connection, clears, corrupt values become `Invalid`, transaction rolls back. |
| WP2-AC6 | `tests/context.rs` compares the exact §3.4 message and exit code 3. |
| WP2-AC7 | [`.docs/data-retention.md`](../.docs/data-retention.md); `tests/db_schema_policy.rs` reads the migrated schema and rejects credential columns. |
| WP2-AC8 | `tests/cli_settings_show.rs`, `tests/cli_settings_edit.rs`, `tests/e2e_settings.rs`: no `data/` directory is created. Settings code has no HTTP client or provider dependency, so no remote call is possible. |

Deviations and decisions, each recorded in an ADR:

- ADR-0015 records the settings adapter and atomic-edit rules that the earlier
  WP-2 code already referenced.
- ADR-0016 records schema v1, connection and migration policy, and the
  synchronous runtime-owning facade. It adds typed `schema_from`/`schema_to`
  columns to `events`, beyond the column list in Appendix B, because WP-2-T2
  requires recording the from/to version and no other column may carry them
  without abusing an unrelated field.
- ADR-0017 vendors `sqlx-sqlite` 0.9.0 with a single widened dependency bound
  after RUSTSEC-2026-0459: released SQLx caps `libsqlite3-sys` below the fixed
  0.39.0. The build bundles SQLite 3.54.1 and the exit condition is recorded.
- ADR-0018 explicitly allows the reviewed BSD-3-Clause and Zlib transitive
  licenses rather than ignoring them.
- WP-2 timestamps are generated by SQLite (`strftime('%Y-%m-%dT%H:%M:%fZ','now')`),
  so no date/time crate is added until WP-5 needs date arithmetic.
- The migration audit event uses the kind already established in WP-1 and
  `OBSERVABILITY.md` (`migration_applied`) rather than the plan's provisional
  name `db_migrated`.
- The existing WP-2 settings tests were formatted and two incorrect
  expectations were corrected: a space-containing editor path must be quoted
  in `EDITOR`/`VISUAL`, and sanitization removes the whole CSI sequence.

Not verified in this environment: Linux; an actual process kill during a
multi-row transaction (WP-5 scope); disk-full behavior (classified but not
simulated). No command opens the database yet; WP-3 wires the first DB-backed
command, so T1, T2, and T6 are exercised by integration tests only.

---

## 9. WP-3 — Laboratories and Agent Profiles

### 9.1 Objective

Deliver the domain model and commands to create, list, open, and validate laboratories, and to create, list, and inspect agent profiles, including the guided agent wizard and the structural validation rules from §6.5.

### 9.2 Scope

**In scope:** typed IDs, roles, skills, laboratory and agent domain rules, schema v2, `--new-lab`, `--list-labs`, `--open-lab`, `--check-lab`, `--new-agent` (interactive), `--list-agents`, `--show-agent-detail`, shared rendering primitives.

**Out of scope:** deletion (WP-4), project counts and assigned-story metrics (WP-4/WP-5), provider calls, questions.

### 9.3 Prerequisites and dependencies

- WP-2 gate passed: settings, database, active-state primitives, rendering error contract.
- Blocking assumptions confirmed: A-07 (skill eligibility), A-09 (interactive wizard).

**Execution checklist**

- [x] T1 — Model the laboratory and agent domain
- [x] T2 — Implement identifier generation and validation (after T1)
- [x] T3 — Add schema v2 for laboratories and agents (after T1, T2)
- [x] T4 — Implement laboratory creation and listing (after T3)
- [x] T5 — Implement `--open-lab` (after T4)
- [x] T6 — Implement `--check-lab` (after T4)
- [x] T7 — Implement the agent repository and creation wizard (after T3, T4, T6)
- [x] T8 — Implement agent listing and detail (after T7)
- [x] T9 — Add shared rendering primitives (after T4, T8)
- [x] T10 — Document laboratory and agent workflows and close the package (after T1–T9)

### 9.4 Tasks

#### WP-3-T1 · Model the laboratory and agent domain

**Spec:** §3.1, §3.3, §7.2; `PRINCIPLES.md` DDD.
**Depends on:** WP-2 gate.

**Work**
- Define `Role` (`lead`, `dev`, `analyst`, `qa`), `SkillTag`, `Lab`, `AgentProfile`, and membership invariants.
- Validate: names trimmed and 1–100 chars; roles and providers parsed from canonical lowercase input; skills normalized (A-06); a laboratory has at most one `lead`; memberships reference existing laboratories.
- Timestamps are UTC; domain structs contain no I/O and no database annotations.

**Deliverables**
- `src/domain/lab.rs`, `src/domain/agent.rs`, `src/domain/skills.rs`, unit tests for every invariant.

**Done when**
- Empty/whitespace names, unknown roles, invalid skill tags, and duplicate lead insertion are rejected with typed errors.
- Case-insensitive role input normalizes to the canonical variant.
- A review confirms the domain module imports no `sqlx`, `reqwest`, or `clap`.

#### WP-3-T2 · Implement identifier generation and validation

**Spec:** §4.2; A-03.
**Depends on:** T1.

**Work**
- Generate 8-hex-character IDs from UUIDv4; validate with `^[0-9a-f]{8}$`.
- Provide typed constructors (`LabId::parse`, etc.) that reject malformed input before any query (exit 2 for malformed, exit 3 for well-formed but absent).
- Provide a bounded collision-retry helper used by repositories before insert.

**Deliverables**
- `src/domain/ids.rs`, unit tests for parsing, formatting, and rejection.

**Done when**
- Malformed IDs never reach the database (asserted with a spy repository).
- Collision retry is covered by a test that forces one collision.
- Display and parsing round-trip for every ID type.

#### WP-3-T3 · Add schema v2 for laboratories and agents

**Spec:** §11.2; §3.1, §3.3; A-15, A-22.
**Depends on:** T1, T2.

**Work**
- Create `migrations/0002_labs_agents.sql`:
  - `labs(id TEXT PRIMARY KEY, name TEXT NOT NULL CHECK(length(trim(name)) BETWEEN 1 AND 100), created_at TEXT NOT NULL, updated_at TEXT NOT NULL)`;
  - `agents(id TEXT PRIMARY KEY, name TEXT NOT NULL, role TEXT NOT NULL CHECK(role IN ('lead','dev','analyst','qa')), description TEXT NOT NULL, skills_json TEXT NOT NULL, provider TEXT NOT NULL, model TEXT NOT NULL, cv TEXT, archived_at TEXT, created_at TEXT NOT NULL, updated_at TEXT NOT NULL)`;
  - `lab_memberships(lab_id TEXT NOT NULL REFERENCES labs(id) ON DELETE CASCADE, agent_id TEXT NOT NULL REFERENCES agents(id) ON DELETE CASCADE, joined_at TEXT NOT NULL, PRIMARY KEY(lab_id, agent_id))`;
  - indexes for membership lookups and active agents.
- Add repository-level checks for one lead per laboratory (application-enforced, tested; SQLite partial unique index may also be added if it remains simple).

**Deliverables**
- `migrations/0002_labs_agents.sql`, migration test.

**Done when**
- Foreign keys cascade memberships when a laboratory is deleted in a test.
- Role `CHECK` and name length constraints reject invalid inserts.
- Migration from schema v1 preserves `app_state` and `events`.

#### WP-3-T4 · Implement laboratory creation and listing

**Spec:** §6.1, §6.2.
**Depends on:** T3.

**Work**
- `--new-lab <name>`: validate name, generate ID, insert in a transaction, print the short ID and a note that a Tech Lead is required before request processing. Creating does not change the active laboratory.
- `--list-labs`: display ID, name, agent count, structural validation result, and active marker; empty state explains how to create the first laboratory.
- Project count is intentionally added in WP-4-T8 when the projects table exists.

**Deliverables**
- `src/app/labs.rs`, `src/infra/db/labs.rs`, rendering updates, `tests/cli_labs.rs`.

**Done when**
- Creating a laboratory persists exactly one row and prints its ID.
- Empty names exit 2; no row is written.
- Listing shows counts and active marker; empty state is actionable.
- Creating a laboratory does not modify `app_state`.

#### WP-3-T5 · Implement `--open-lab`

**Spec:** §6.3; §3.4.
**Depends on:** T4.

**Work**
- Verify existence (exit 3 if absent), persist the selection transactionally, print a concise summary and validation warnings.
- Opening an incomplete laboratory is allowed.
- A failed persistence leaves the previous selection unchanged.

**Deliverables**
- Handler in `src/app/labs.rs`, `tests/cli_open_lab.rs`.

**Done when**
- Selection persists across separate invocations.
- Unknown ID exits 3 with a hint to list laboratories.
- Warnings appear on stderr, summary on stdout.

#### WP-3-T6 · Implement `--check-lab`

**Spec:** §6.5; A-07.
**Depends on:** T4.

**Work**
- Validate: laboratory exists; at least one membership; exactly one `lead`; every role valid; memberships and settings resolve; every member with role `dev`/`analyst`/`qa` has at least one skill; provider/model structurally valid.
- Produce a line-per-rule report; exit 0 only if all pass, exit 4 otherwise; no provider call.
- Include an actionable hint for each failure.

**Deliverables**
- `src/app/lab_validation.rs`, table-driven tests for each rule, `tests/cli_check_lab.rs`.

**Done when**
- Each rule has an isolated failing fixture and a passing fixture.
- A laboratory with zero agents fails; with two leads fails; with a non-lead member lacking skills fails.
- The command performs no network I/O (test asserts no HTTP client is constructed).

#### WP-3-T7 · Implement the agent repository and creation wizard

**Spec:** §7.2; §5.4; A-09, A-22.
**Depends on:** T3, T4, T6.

**Work**
- Repository: insert profile and memberships atomically; enforce one lead per laboratory; copy provider/model defaults from settings at creation time.
- Wizard steps: name, role, description, skills, provider (default settings), model (default settings), optional CV, memberships (active laboratory pre-filled when one is selected; otherwise one or more existing laboratory IDs must be entered), summary, confirmation.
- Validate each answer at entry and again at commit; reject empty required fields, unknown roles/providers, unknown laboratories, and a second lead.
- Define a `Prompter` port so the flow is unit-testable; non-TTY stdin exits 4 with a hint (A-09).
- Cancellation at confirmation exits 5 and writes nothing.

**Deliverables**
- `src/app/agents/wizard.rs`, `src/cli/prompt.rs` implementation, `src/infra/db/agents.rs`, unit tests with a scripted prompter, `tests/cli_agent_noninteractive.rs`.

**Done when**
- A full scripted happy path persists profile and memberships in one transaction.
- Invalid answers re-prompt or abort with the documented code and no partial write.
- A second lead attempt is rejected before persistence.
- A later change to `settings.yaml` does not alter provider/model values copied into an existing agent.
- Non-interactive invocation exits 4 without hanging and without creating data.

#### WP-3-T8 · Implement agent listing and detail

**Spec:** §7.1, §7.4.
**Depends on:** T7.

**Work**
- `--list-agents`: members of the active laboratory with ID, name, role, provider, model; project and assigned-story counts render as `N/A` until WP-4/WP-5 wire the data.
- `--show-agent-detail`: profile, skills, optional CV, memberships, archived status (always active in this package), and metrics. Metrics that cannot be derived are printed as `N/A`, never invented.
- Archived agents are excluded from lists but resolvable by ID in detail (the filter is implemented now; archive creation arrives in WP-4).

**Deliverables**
- Handlers in `src/app/agents.rs`, `tests/cli_agents.rs`.

**Done when**
- Listing requires an active laboratory and exits 3 without one.
- Detail shows all stored profile fields; each unavailable metric is literally `N/A`.
- No score, ranking, or productivity percentage is computed.

#### WP-3-T9 · Add shared rendering primitives

**Spec:** §10 UX requirements; `STACK.md` textual UI; AD-9.
**Depends on:** T4, T8.

**Work**
- Implement a renderer that takes terminal width and a list of rows, producing aligned columns; falls back to stacked `label: value` blocks below a configured threshold.
- Truncate long cells with an explicit ellipsis; sanitize control characters; emit no ANSI.
- Provide empty-state rendering helpers.
- Use the renderer for laboratory, agent, and settings output where tables are appropriate.

**Deliverables**
- `src/cli/render.rs`, unit tests with widths 40, 80, 120, and control-character fixtures.

**Done when**
- Output at width 40 is readable and contains no truncated identifier.
- Escape sequences in names are neutralized.
- No ANSI bytes appear when output is captured (assertion on raw bytes).
- `NO_COLOR` behavior is documented as trivially satisfied (no color emitted).

#### WP-3-T10 · Document laboratory and agent workflows and close the package

**Spec:** §6, §7; `CONSTITUTION.md` §9.
**Depends on:** T1–T9.

**Work**
- Update `README.md` with a local walkthrough: create laboratory, create agents, open, check.
- Document validation rules and exit codes for this package.
- Record the verification evidence for WP3-AC.

**Deliverables**
- Documentation updates, `docs/validation/wp-3.md` with executed commands and results.

**Done when**
- A new contributor can reproduce the walkthrough from a clean state.
- The validation report lists what was executed and any remaining limitation.

### 9.5 Tests and acceptance criteria

| ID | Criterion | Verification |
|---|---|---|
| WP3-AC1 | Laboratory and agent invariants are enforced in the domain | Domain unit tests |
| WP3-AC2 | IDs are valid, unique, and malformed IDs never reach storage | ID tests with spy repository |
| WP3-AC3 | Schema v2 enforces constraints and cascades | Migration and constraint tests |
| WP3-AC4 | Laboratory create/list/open behave per §6.1–§6.3 | `tests/cli_labs.rs`, `tests/cli_open_lab.rs` |
| WP3-AC5 | `--check-lab` reports every rule and exits 0/4 correctly | Table-driven validation tests |
| WP3-AC6 | Agent wizard validates, confirms, and persists atomically | Scripted-prompter tests |
| WP3-AC7 | Agent list/detail show `N/A` instead of invented metrics | `tests/cli_agents.rs` |
| WP3-AC8 | Rendering is width-safe, sanitized, and color-free | Render unit tests |
| WP3-AC9 | No command performs provider I/O | Test asserting no HTTP construction |

### 9.6 Exit gate

- All WP3-AC pass.
- A user can create a laboratory, create a Tech Lead and other agents through the wizard, open the laboratory, and see a validation report.
- Every validation failure is actionable and returns the documented exit code.
- No destructive operation exists yet; deletion arrives in WP-4 with confirmation tests.

### 9.7 Package report (2026-11-12)

Blocking assumptions A-07 and A-09 were confirmed by the product owner before
this package: operational roles need at least one skill, and `--new-agent`
requires an interactive terminal. Both are recorded in
[ADR-0020](../.docs/adr/adr-0020.md).

Executed locally on macOS (aarch64), Rust 1.99.1: `make check`, which runs
`cargo fmt --all -- --check`, `cargo clippy --locked --all-targets -- -D warnings`,
`cargo test --locked` (185 passed, 0 failed), `cargo audit` (no findings), and
`cargo deny check` (advisories, bans, licenses, and sources ok).

Acceptance evidence:

| Criterion | Evidence |
|---|---|
| WP3-AC1 | Domain unit tests for names, roles, descriptions, skills, CV bounds, identifier parsing, and the single-lead rule (`src/domain/*`). |
| WP3-AC2 | `src/domain/ids.rs` tests every ID type, collision retry, and exhaustion; `app::labs` tests prove malformed IDs never call the repository. |
| WP3-AC3 | `tests/db_labs_agents.rs`: schema v2 role/name checks, membership cascade, v1→v2 state preservation, atomic profile+membership insert with FK rollback. |
| WP3-AC4 | `tests/cli_labs.rs`: create/list/open, active marker, malformed versus missing IDs, warnings on stderr, diagnostics events. |
| WP3-AC5 | `src/app/lab_validation.rs` unit tests for every rule plus `tests/cli_labs.rs` for the exit 0/4 contract and the full report. |
| WP3-AC6 | `app::agents::wizard` scripted-prompter tests (happy path, re-prompting, second lead, unknown laboratory, cancellation, non-interactive) and `tests/cli_agent_noninteractive.rs`. |
| WP3-AC7 | `tests/cli_agents.rs`: list and detail show every stored field and literal `N/A` for unavailable metrics; archived agents resolve by ID and are excluded from lists. |
| WP3-AC8 | `src/cli/render.rs` tests at widths 40, 80, and 120, truncation with `…`, identifier preservation, sanitization, and raw-byte no-ANSI assertions. |
| WP3-AC9 | `tests/provider_scope.rs` asserts no HTTP client dependency exists in this package; `--check-lab` succeeds with no credential or settings file. |

Decisions and deviations:

- ADR-0019 records schema v2, the application-enforced one-lead rule, and the
  small skills JSON grammar (no `serde_json` until WP-5).
- ADR-0020 records the wizard contract, the confirmed A-07/A-09 assumptions,
  field bounds, and the interactive-only limitation.
- ADR-0021 records width-aware tables, the stacked fallback, identifier
  preservation, and the new `terminal_size`/`unicode-width` dependencies.
- `--check-lab` interprets "settings resolve" as the provider/model copies in
  each agent profile, because agents own copies (§5.4) and the command must
  work without reading the settings file. Project references are reserved for
  WP-4, when projects exist.
- The CV prompt is single-line in this release; a multiline editor flow is a
  possible later improvement and is documented as a limitation.
- A defect in the WP-1 entry point was found and fixed: `main_entry` held the
  process stdout lock for the whole run, which deadlocked when `RUST_LOG`
  enabled SQLx diagnostics from the SQLite worker thread. The entry point now
  uses short-lived default stream handles; the redaction test that exposed the
  hang completes in milliseconds.

Not verified in this environment: Linux; a real PTY walkthrough of the wizard
(the scripted port and the non-TTY refusal are covered); rendering on a live
narrow terminal (width is injected in tests and defaults to 100 when redirected).

---

## 10. WP-4 — Projects, Kanban, and Safe Deletion

### 10.1 Objective

Deliver project lifecycle within the active laboratory, the read-only Kanban board, the confirmation subsystem, scoped memories, and all destructive commands (laboratory, project, agent) with preconditions and audit events. This package also introduces schema v3 for projects, requests, stories, and memories so later planning can persist and consume context from a tested schema.

### 10.2 Scope

**In scope:** schema v3 (projects, requests, stories, memories), project create/list/delete, Kanban rendering and counts, confirmations, laboratory/agent deletion, active-planning guard, memory create/list/delete commands, count extensions for laboratory/agent listings.

**Out of scope:** request intake, planning, model calls, story transitions.

### 10.3 Prerequisites and dependencies

- WP-3 gate passed.
- Blocking assumption A-08 confirmed (confirmation and `--yes` behavior).
- Schema v3 is created here, before any planning code, so WP-5 never introduces tables it needs for basic persistence.
- Scoped memories require laboratories and agents (WP-3) and projects (WP-4), so they are added to schema v3 and implemented here.

**Execution checklist**

- [x] T1 — Add schema v3 for projects, requests, and stories
- [x] T2 — Model projects and implement the project repository (after T1)
- [x] T3 — Implement project commands (after T2)
- [x] T4 — Implement the Kanban query and renderer (after T1, T3)
- [x] T5 — Implement the confirmation subsystem (after T3)
- [x] T6 — Implement the active-planning guard (after T1)
- [x] T7 — Implement deletion commands (after T5, T6)
- [x] T8 — Extend list counts with project and story data (after T1–T4)
- [x] T9 — Implement scoped memory storage and commands (after T1, T2, T7)
- [x] T10 — Document project, Kanban, deletion, and memory behavior and close the package (after T1–T9)

### 10.4 Tasks

#### WP-4-T1 · Add schema v3 for projects, requests, and stories

**Spec:** §11.2; §9.3; §10; A-30.
**Depends on:** WP-3 gate.

**Work**
- Create `migrations/0003_projects_requests_stories.sql`:
  - `projects(id TEXT PRIMARY KEY, lab_id TEXT NOT NULL REFERENCES labs(id) ON DELETE CASCADE, name TEXT NOT NULL CHECK(length(trim(name)) BETWEEN 1 AND 100), created_at TEXT NOT NULL, updated_at TEXT NOT NULL)`;
  - `requests(id TEXT PRIMARY KEY, project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE, content TEXT NOT NULL, status TEXT NOT NULL CHECK(status IN ('TODO','PLANNING','PLANNED','NEEDS_CLARIFICATION','FAILED')), questions_json TEXT, error_code TEXT, error_message TEXT, planning_started_at TEXT, created_at TEXT NOT NULL, updated_at TEXT NOT NULL)`;
  - `stories(id TEXT PRIMARY KEY, project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE, request_id TEXT NOT NULL REFERENCES requests(id) ON DELETE CASCADE, title TEXT NOT NULL, description TEXT NOT NULL, acceptance_criteria_json TEXT NOT NULL, priority TEXT NOT NULL CHECK(priority IN ('low','medium','high')), complexity TEXT NOT NULL CHECK(complexity IN ('small','medium','large')), required_skills_json TEXT NOT NULL, assigned_agent_id TEXT REFERENCES agents(id) ON DELETE SET NULL, assignment_rationale TEXT, state TEXT NOT NULL CHECK(state IN ('TODO','IN_PROGRESS','IN_REVIEW','BLOCKED','DONE')), due_date TEXT NOT NULL, created_at TEXT NOT NULL, updated_at TEXT NOT NULL)`;
  - indexes on `projects(lab_id)`, `requests(project_id)`, `requests(project_id, status)`, `stories(project_id, state)`, `stories(request_id)`, `stories(assigned_agent_id)`.
- Ensure deletion cascades from laboratory to project to request/story; agents set story assignment to `NULL` on hard delete (profiles are archived, not deleted).

**Deliverables**
- Migration file, migration test, constraint tests for every enum and length check.

**Done when**
- Cascades behave as specified in a test that deletes a laboratory with a project, request, story, and memories.
- A story with an unknown priority/state and a request with an unknown status are rejected.
- A memory with zero or multiple owners, or a mismatched `scope_type`, is rejected.
- Migration v2 → v3 preserves all earlier data.

#### WP-4-T2 · Model projects and implement the project repository

**Spec:** §3.2, §8.1–§8.2; A-23.
**Depends on:** T1.

**Work**
- Define `Project` with typed `ProjectId` and `LabId`; validate names per A-23.
- Repository: create, list by active laboratory, find by ID **and** laboratory, delete inside a transaction.
- A project ID owned by another laboratory produces exit 3 with a hint, never a silent context switch.

**Deliverables**
- `src/domain/project.rs`, `src/infra/db/projects.rs`, unit and integration tests.

**Done when**
- Cross-laboratory access is rejected and asserted before any mutation.
- Listing is scoped to the active laboratory.
- Deleting a project removes its requests and stories but preserves agent profiles.

#### WP-4-T3 · Implement project commands

**Spec:** §8.1, §8.2.
**Depends on:** T2, WP-3-T9.

**Work**
- `--new-project <name>`: require active laboratory; reject empty names; persist; print ID.
- `--list-projects`: show ID, name, request count, story count, and story counts by state; actionable empty state.
- Project creation is allowed in a structurally invalid laboratory; the message notes that requests require a valid laboratory.

**Deliverables**
- `src/app/projects.rs`, `tests/cli_projects.rs`.

**Done when**
- Project commands fail with exit 3 when no laboratory is active.
- Counts are accurate against fixtures.
- Empty state suggests creating a project.

#### WP-4-T4 · Implement the Kanban query and renderer

**Spec:** §10; A-30; AD-9.
**Depends on:** T1, T3, WP-3-T9.

**Work**
- Query the project with laboratory name, Tech Lead name (or explicit missing-lead warning), story counts by state, completed count, and pending intake count (requests in `TODO`, `PLANNING`, `NEEDS_CLARIFICATION`, `FAILED`; `PLANNED` excluded).
- Render five columns in order: `TODO`, `IN PROGRESS`, `IN REVIEW`, `BLOCKED`, `DONE`.
- Each card: story ID, concise title, assignee or `Unassigned`, priority, due date, warning indicator for `BLOCKED` or overdue-without-DONE.
- Wide layout: aligned columns. Narrow layout: vertically stacked sections. Long titles truncated with `…`.
- Sanitize control characters; emit no ANSI; provide useful empty states for no stories and no requests.
- Clearly state that only planning creates `TODO` stories and that execution transitions are not available.

**Deliverables**
- `src/app/kanban.rs`, `src/infra/db/kanban.rs`, `tests/cli_kanban.rs` with fixtures spanning all states and widths.

**Done when**
- Golden tests at widths 50 and 120 pass.
- Redirected output contains zero escape bytes.
- A story title containing `\x1b[31m` is rendered inert.
- Pending intake counts match the fixture and exclude `PLANNED`.
- An empty project shows guidance, not an empty box.

#### WP-4-T5 · Implement the confirmation subsystem

**Spec:** `CONSTITUTION.md` §4, §5; `SECURITY.md` approval; §13; A-08.
**Depends on:** T3, WP-3-T7 (prompt port).

**Work**
- Define `ConfirmationRequest` with a human-readable summary, the exact entity IDs, and affected counts.
- On a terminal: show the summary and prompt `y/N`, defaulting to no.
- With `--yes`: skip the prompt only for commands that declare support.
- Without a terminal and without `--yes`: exit 4 with a message explaining the required flag.
- Declined confirmation or EOF exits 5 with no mutation.
- Bind confirmation to the displayed snapshot: re-read the entity inside the deletion transaction and abort if it no longer matches (counts, archived state, ownership).

**Deliverables**
- `src/cli/confirm.rs`, unit tests with a scripted prompter, `tests/cli_confirmation.rs`.

**Done when**
- Yes executes; no, EOF, and `q` cancel with no changes.
- `--yes` works; non-TTY without it exits 4.
- A test that changes the entity between display and confirm causes an abort with no partial deletion.
- Every confirmation and outcome emits an audit event.

#### WP-4-T6 · Implement the active-planning guard

**Spec:** §6.4, §8.3; A-11.
**Depends on:** T1.

**Work**
- Introduce the planning-lease constant (240 s, §4.8) in shared domain configuration so WP-5 reuses the same value rather than redefining it.
- Add a repository query for requests in `PLANNING` with a `planning_started_at` lease equal to that constant.
- Expose `ensure_no_active_planning(lab_id/project_id)` used by deletions.
- Treat leases older than the lease as failed from the command's perspective (no automatic status rewrite in this package; WP-5 owns recovery).

**Deliverables**
- Guard module and tests with inserted `PLANNING` fixtures (fresh and stale).

**Done when**
- Fresh `PLANNING` blocks deletion with exit 4 and a hint.
- Stale `PLANNING` does not block.
- No provider call is required to evaluate the guard.

#### WP-4-T7 · Implement deletion commands

**Spec:** §6.4, §7.3, §8.3; §13; A-10, A-15.
**Depends on:** T5, T6.

**Work**
- `--delete-lab <lab-id>`: show name and affected project count; confirm; refuse if active planning; delete projects/requests/stories/memories/memberships in one transaction; preserve agent profiles; clear the active selection if it points to the deleted laboratory.
- `--delete-project <project-id>`: verify ownership by the active laboratory; confirm; refuse if active planning; delete the project, requests, stories, and project memories atomically; preserve agent profiles.
- `--delete-agent <agent-id>`: global scope; show all affected laboratories; confirm; block when any assigned story is not `DONE`; block while any active `PLANNING` request exists in a laboratory where the agent is a member (conservative single-process guard); warn when the agent is a `lead`; archive the profile (`archived_at`), remove memberships, retain its memories for history but exclude them from prompts, and leave stories referencing the archived agent.
- Archived agents are excluded from lists and wizard selection but resolvable by detail.

**Deliverables**
- Deletion handlers, repository methods, `tests/cli_delete_lab.rs`, `tests/cli_delete_project.rs`, `tests/cli_delete_agent.rs`.

**Done when**
- Each command requires confirmation and emits audit events.
- Cancellation leaves all data unchanged (byte-level database comparison or row counts).
- Laboratory deletion clears a matching active selection.
- Agent deletion with an unfinished story exits 4 and preserves everything.
- Agent deletion with only `DONE` stories archives the profile and removes memberships; its memories remain but are excluded from prompts.
- Deleting a lead warns explicitly before confirmation.
- Laboratory and project deletion remove their memories; agent memories remain for history.

#### WP-4-T8 · Extend list counts with project and story data

**Spec:** §6.1, §7.1, §8.1.
**Depends on:** T1–T4.

**Work**
- Add project count to `--list-labs` and project/assigned-story counts to `--list-agents`.
- Extend `--show-agent-detail` with current story assignments, assigned-story counts, and a memory count with a pointer to `--list-memories --for-agent`; keep completed, review, and timing metrics `N/A` while no lifecycle transitions exist.
- Extend `--check-lab` to verify that project references resolve within the laboratory once schema v3 exists; no provider call is added.
- Add memory counts to laboratory and project listings only if they fit the existing table without inventing a metric; the canonical view remains `--list-memories`.
- Replace the `N/A` placeholders introduced in WP-3 only where data is now derivable; keep `N/A` for metrics that still lack lifecycle events.
- Update existing golden tests and add count fixtures.

**Deliverables**
- Updated queries and tests.

**Done when**
- Laboratory list shows accurate project counts.
- Agent list shows accurate project and assigned-story counts.
- `--check-lab` resolves project references and reports a broken reference as a validation failure.
- Unsupported metrics remain `N/A`, not zero or invented values.

#### WP-4-T9 · Implement scoped memory storage and commands

**Spec:** §3.5, §4.1, §11.2; ADR-0022; `SECURITY.md` memory trust boundary.
**Depends on:** T1, T2, T7.

**Work**
- Define `Memory`, `MemoryId`, and `MemoryScope` (`Lab(LabId)`, `Project(ProjectId)`, `Agent(AgentId)`).
- Validate content: trimmed, 1–2,000 characters, no C0/C1 controls except newline and tab, at most 20 memories per scope; memories are immutable (no update command).
- Repository: create, list by scope (newest first), find, delete, count; enforce scope/owner consistency. A project memory must belong to a project in the active laboratory; a laboratory memory uses its explicit ID; an agent memory is global to the profile and is retained when the agent is archived. Creating a memory for an archived agent exits 4; listing existing memories remains allowed.
- Commands: `--new-memory <text>` and `--list-memories` require exactly one of `--for-lab`, `--for-project`, or `--for-agent`; `--delete-memory <memory-id>` uses the confirmation subsystem and supports `--yes`. Its confirmation summary shows the memory ID, scope, owner ID, and a sanitized content preview. Invalid scope combinations exit 2; unknown owners exit 3; the 21st memory in a scope exits 4.
- Extend `--help` with the **Memories** group and the three scope modifiers; update the reviewed help snapshot. The modifiers are valid only with `--new-memory` and `--list-memories`; `--yes` is also valid with `--delete-memory`.
- `--list-memories` prints ID, scope, a sanitized content preview, and creation time; empty states are actionable. Creating prints the ID and scope without echoing content.
- Record `memory_created` and `memory_deleted` events with the memory entity ID only; never log content. No command performs provider I/O.
- Cascade laboratory and project deletion to their memories; project memories are removed with the project; agent memories survive archival and are excluded from prompt selection while archived.

**Deliverables**
- `src/domain/memory.rs`, `src/app/memories.rs`, `src/infra/db/memories.rs`, `tests/db_memories.rs`, `tests/cli_memories.rs`.

**Done when**
- The scope matrix passes: every valid owner/scope pair is accepted, while zero or multiple selectors, a mismatched selector, and a cross-laboratory project are rejected with the documented exit code.
- Content at 2,000 characters is accepted, 2,001 is rejected; control characters other than newline/tab are rejected; the 20th memory is accepted and the 21st is rejected.
- Deletion requires confirmation; `--yes` works; non-TTY without `--yes` exits 4; cancellation and EOF leave the row unchanged.
- Laboratory/project deletion cascades memories; archived agent memories remain queryable but are excluded by the prompt selector (WP-5/WP-6), and new memories for an archived agent are rejected.
- No memory content appears in events, errors, or diagnostics; a synthetic secret marker inside a memory is absent from those surfaces.
- `--list-memories` and `--new-memory` succeed with no credential and no settings file.

#### WP-4-T10 · Document project, Kanban, deletion, and memory behavior and close the package

**Spec:** §3.5, §6, §7.3, §8, §10.
**Depends on:** T1–T9.

**Work**
- Document command examples, confirmation semantics, active-planning behavior, Kanban layout rules, memory scopes, the memory-context budget, and retention.
- Record WP4 acceptance evidence in `docs/validation/wp-4.md`.

**Deliverables**
- Documentation and validation report.

**Done when**
- A user can create a project, view an empty and a populated board, create and delete scoped memories, and safely delete entities using documented commands.
- The report states what was executed and any limitation.

### 10.5 Tests and acceptance criteria

| ID | Criterion | Verification |
|---|---|---|
| WP4-AC1 | Schema v3 enforces enums, cascades, and indexes | Migration/constraint tests |
| WP4-AC2 | Project commands are laboratory-scoped | `tests/cli_projects.rs` |
| WP4-AC3 | Kanban renders five states at wide and narrow widths without color | Golden tests + byte assertions |
| WP4-AC4 | Pending intake excludes `PLANNED` and totals are correct | Fixture tests |
| WP4-AC5 | Confirmations are bound, cancellable, and non-interactive-safe | `tests/cli_confirmation.rs` |
| WP4-AC6 | Deletions are atomic, cascading, and audited | Deletion tests with row-count assertions |
| WP4-AC7 | Active planning blocks destructive operations | Guard tests |
| WP4-AC8 | Agent archiving preserves history and blocks unfinished work | Agent deletion tests |
| WP4-AC9 | Lists show accurate counts and honest `N/A` | Count fixtures |
| WP4-AC10 | Scoped memories enforce owner, size, and cascade rules and never leak content into audit surfaces | `tests/db_memories.rs`, `tests/cli_memories.rs` |

### 10.6 Exit gate

- All WP4-AC pass.
- The complete local product (settings, labs, agents, projects, Kanban, deletion, scoped memories) works without any provider access.
- No destructive path can execute without confirmation or an explicit `--yes`.
- Schema v3 and its constraints are ready for planning persistence and memory-aware prompts in WP-5.

### 10.7 Package report (2026-11-12)

Executed locally on macOS (aarch64), Rust 1.99.1: `make check`, which runs
`cargo fmt --all -- --check`, `cargo clippy --locked --all-targets -- -D warnings`,
`cargo test --locked` (249 passed, 0 failed), `cargo audit` (no findings), and
`cargo deny check` (advisories, bans, licenses, and sources ok).

Acceptance evidence:

| Criterion | Evidence |
|---|---|
| WP4-AC1 | `tests/db_projects.rs`: project/request/story/memory constraints, cascades, v2→v3 preservation. |
| WP4-AC2 | `tests/cli_projects.rs`: active-laboratory requirement, create/list, empty state, cross-laboratory rejection. |
| WP4-AC3 | `tests/cli_kanban.rs` + `src/cli/render.rs`: five states, wide columns and narrow sections, sanitization, no ANSI. |
| WP4-AC4 | `tests/cli_kanban.rs`: pending intake excludes `PLANNED`; counts and completed total. |
| WP4-AC5 | `src/cli/confirm.rs` unit tests plus non-TTY deletion tests; `--yes`, decline, EOF, and snapshot binding. |
| WP4-AC6 | `tests/cli_deletions.rs`: atomic cascades, active-selection clearing, agent preservation, memory retention. |
| WP4-AC7 | Fresh `PLANNING` blocks lab/project/agent deletion; stale lease does not. |
| WP4-AC8 | Agent deletion blocks unfinished stories, archives, removes memberships, warns for leads. |
| WP4-AC9 | `--list-labs` shows project counts; `--list-agents` shows project/open-story counts; unavailable metrics remain `N/A`. |
| WP4-AC10 | `tests/db_memories.rs`, `tests/cli_memories.rs`: scopes, caps, cascade, archived-agent behavior, no content in events. |

Decisions and deviations:

- ADR-0023 records schema v3 (projects, requests, stories, memories).
- ADR-0024 records bound confirmation, snapshot re-check, cascades, and the
  240-second active-planning guard.
- The Kanban wide layout uses five side-by-side columns with one title line and
  two metadata lines per card; below 100 columns it uses stacked sections. The
  empty board names the expected planning flow without pretending that
  `--new-request` is available in this package.
- `--list-labs` now shows project count. `--list-agents` shows distinct
  projects with assigned stories and open assigned stories. `--check-lab`
  gains the "project references resolve" rule.
- Deletion snapshot binding compares the confirmed plan (name and affected
  counts) with a re-read inside the transaction; a changed plan exits 4. The
  test suite uses row counts rather than byte-level database comparison.
- Agent deletion writes the Tech Lead warning to stderr even when `--yes` is
  used, so the effect is not hidden by the non-interactive bypass.
- Memories are implemented exactly as specified in ADR-0022: immutable,
  scoped, bounded, cascading, and content-free in audit events.

Not verified in this environment: Linux; an interactive terminal confirmation
(the decline path is unit-tested through the prompt port, and non-TTY behavior
is exercised end to end); a real provider-backed active planning operation
(arrives with WP-5, so the guard is tested with inserted `PLANNING` rows).

---

## 11. WP-5 — Request Intake and Tech Lead Planning

### 11.1 Objective

Deliver `--new-request` end to end: validate and persist the request, include the correctly scoped and bounded memories, invoke DeepSeek through a bounded provider adapter, validate the structured proposal locally, assign real laboratory members, compute due dates in application code, and persist stories atomically with an explicit terminal outcome. This package also carries the project GitHub-source configuration and global GitHub credential required for future repository operations.

### 11.2 Scope

**In scope:** execution limits, provider port, DeepSeek adapter, retry policy, prompt/context builder (including scoped memories), output schema and validation, assignment rules, due dates, planning state machine, input handling, security tests, cancellation, recovery from stale `PLANNING`, optional project GitHub source, separate source-control provider configuration, global `credentials.github_api_key` validation, and the clone-only GitHub operation that populates `.workspace/` (ADR-0032). Fetch, push, branch, and other GitHub operations remain out of scope.

**Out of scope:** direct questions (WP-6), any live-provider call or evaluation (Go-Live), story transitions, autonomous execution, CV transmission.

### 11.3 Prerequisites and dependencies

- WP-4 gate passed: schema v3, projects, scoped memories, confirmations, active-planning guard, renderer.
- Blocking assumptions confirmed: A-01, A-02, A-05, A-07, A-13.
- Memory semantics and the memory-context budget are fixed by §3.5 and ADR-0022; no additional confirmation is needed.
- GitHub source semantics and credential policy are fixed by ADR-0025; only configuration and local validation are in scope.
- No DeepSeek key is needed: all tests use `wiremock` or the scripted provider. Live confirmation of the contract is GL-T2.

**Execution checklist**

- [x] T1 — Implement execution budgets, clocks, and cancellation
- [x] T2 — Define the provider port and message types (after T1)
- [x] T3 — Implement the DeepSeek adapter (after T2)
- [x] T4 — Implement the retry policy (after T3)
- [x] T5 — Build the planning prompt and context (after T2)
- [x] T6 — Define and validate the structured planning output (after T5)
- [x] T7 — Apply assignment and due-date rules (after T6)
- [x] T8 — Implement the planning state machine and atomic persistence (after T1–T7)
- [x] T9 — Implement `--new-request` input handling and CLI output (after T8)
- [x] T10 — Add planning security tests (after T9)
- [x] T11 — Handle cancellation and unknown external outcomes (after T8, T9)
- [x] T12 — Document planning behavior and close the package (after T1–T11)
- [x] T13 — Add GitHub source configuration and global credential (delivered ahead of T1–T12)

### 11.4 Tasks

#### WP-5-T1 · Implement execution budgets, clocks, and cancellation

**Spec:** §12; `VALIDATION.md` execution budget; AD-8; D-3, D-4.
**Depends on:** WP-4 gate.

**Work**
- Define `ExecutionLimits` with the values in §4.8, injectable for tests; reuse the planning-lease constant introduced in WP-4-T6 for stale-run detection.
- Define `Clock` (`SystemClock`, `TestClock`) and `Deadline` arithmetic returning remaining time or `Expired`.
- Define counters for logical invocations and HTTP attempts; a dispatch increments before the call, including failed calls.
- Provide a cancellation token propagated from the application signal handler to the provider call.
- Stop issuing new work after cancellation or budget exhaustion; never retry after cancellation.

**Deliverables**
- `src/domain/budget.rs`, `src/domain/clock.rs`, unit tests for boundaries: exact deadline, one millisecond past, zero remaining, counter at limit and limit+1.

**Done when**
- Table-driven tests prove no dispatch occurs with zero remaining budget.
- Test clock removes all wall-clock sleeps from retry/cancellation tests.
- A cancelled token prevents any subsequent dispatch.

#### WP-5-T2 · Define the provider port and message types

**Spec:** §12; AD-3; `SECURITY.md` model boundary.
**Depends on:** T1.

**Work**
- Define `ModelProvider` with one asynchronous `complete` operation taking a typed request, deadline, and cancellation token.
- Request: system text, user text, model identifier, max output tokens, JSON-response flag. Response: content, model, usage, finish reason, provider request ID.
- Define `ProviderError`: authentication, rate limited (with retry-after), timeout, transport, server (status), invalid response, response too large, cancelled, configuration.
- Implement `Debug` manually for anything carrying request content so diagnostics cannot leak payloads.
- Provide a scripted provider for tests that returns queued results and records received requests.

**Deliverables**
- `src/domain/provider.rs`, `src/infra/provider/scripted.rs`, `tests/provider_port.rs`.

**Done when**
- A test asserts the scripted provider records exact arguments and supports success, error, timeout, and cancellation outcomes.
- `Debug` output of a request containing a secret marker does not include the marker.
- No `reqwest` type appears in the domain signature.

#### WP-5-T3 · Implement the DeepSeek adapter

**Spec:** §12; `STACK.md` AI Integration; `SECURITY.md` network.
**Depends on:** T2.

**Work**
- Build a `reqwest` client with rustls, certificate validation enabled, connect timeout, and no automatic redirects to untrusted hosts (or redirects disabled).
- Send `POST` to the verified chat-completions endpoint with `Authorization: Bearer` from `credentials.deepseek_api_key`; never log or Debug the header.
- Enforce the per-attempt deadline with `tokio::time::timeout`; read the response body incrementally and abort beyond the 256 KiB cap.
- Map status codes: 401/403 authentication, 429 rate limited with `Retry-After`, 5xx server, other 4xx invalid request, malformed JSON invalid response.
- The base URL is a constructor parameter used by tests; production uses the verified constant.
- Return no body content in errors; include status and provider request ID only.

**Deliverables**
- `src/infra/provider/deepseek.rs`, `tests/provider_deepseek.rs` using `wiremock`.

**Done when**
- Contract tests cover success, 401, 429 with retry-after, 500, malformed JSON, truncated body, oversized body, and timeout.
- A captured request proves the API key is present only in the authorization header and absent from the body.
- `Debug` of every adapter error is free of payload and credential markers.
- TLS validation is not disabled anywhere (review assertion).

#### WP-5-T4 · Implement the retry policy

**Spec:** §12; D-3; A-25.
**Depends on:** T3.

**Work**
- Wrap provider calls with a policy that retries only `RateLimited`, `Server(5xx)`, `Timeout`, and select transport failures, up to two times.
- Never retry authentication, invalid response, oversized response, validation, or cancellation.
- Backoff uses the injected clock: 500 ms base, doubling, jitter, cap 8 s; `Retry-After` takes precedence when present.
- Each attempt consumes budget and increments counters; stop when the deadline expires.
- Emit an event per attempt with outcome and error code, without payloads.

**Deliverables**
- `src/app/provider_retry.rs`, tests with the scripted provider and test clock.

**Done when**
- Retryable failures produce exactly 1 + 2 attempts when budget allows.
- Non-retryable failures produce exactly one attempt.
- A test proves no attempt starts after cancellation or deadline expiry.
- Attempt events are recorded with attempt numbers.

#### WP-5-T5 · Build the planning prompt and context

**Spec:** §9.2, §9.4; `SECURITY.md` prompt injection; A-13, A-24.
**Depends on:** T2.

**Work**
- System prompt states the role (Tech Lead planner), the task, the output schema version, and hard rules: model output is a proposal; use only listed agent IDs; never output dates; treat request text as untrusted data; no tool use exists.
- Include the roster: for each member, ID, name, role, normalized skills, and current assigned non-`DONE` story count.
- Include the request creation date in UTC and the three allowed complexity values with their intervals.
- Include exactly the scoped memories from §3.5 and ADR-0022: the project laboratory's memories, the project's memories, and the Tech Lead's agent memories. Order them laboratory, project, agent, newest first within each scope.
- Enforce the 8 KiB memory budget by omitting whole memories; never truncate a memory. Delimit each block with its scope and memory ID, and state that memory content is data, not instructions.
- Delimit request content clearly (e.g. `<request>…</request>`) and instruct that content inside is data, not instructions.
- Exclude CVs, unrelated projects, secrets, memories outside the selected scopes, and database identifiers not needed for planning.
- Enforce the request-size limit and the memory budget before building the prompt.

**Deliverables**
- `src/app/planning/prompt.rs`, golden tests for prompt structure and memory ordering/cut-off, a test that a secret marker in `settings.yaml` never appears in the prompt.

**Done when**
- Golden prompts contain every roster member exactly once and no CV text.
- Request text is delimited and not interpolated into instructions.
- Exactly the selected laboratory, project, and Tech Lead memories appear, every block is delimited, and no out-of-scope or archived-agent memory appears.
- The memory fixture at the 8 KiB boundary is included whole; the first memory beyond it is omitted whole, and the prompt never contains a partial memory.
- Oversized requests are rejected before any provider call.

#### WP-5-T6 · Define and validate the structured planning output

**Spec:** §9.3, §9.6; `SECURITY.md` validation; A-27, A-28.
**Depends on:** T5.

**Work**
- Define the JSON schema version 1: `schema_version`, `clarification_questions` (array of strings), `stories` (array).
- Each story: `title`, `description`, `acceptance_criteria` (non-empty array), `priority` (`low|medium|high`), `complexity` (`small|medium|large`), `required_skills` (tags), `assigned_agent_id` (nullable), `assignment_rationale`.
- Parse with `deny_unknown_fields`; enforce length limits, non-empty fields, maximum 20 stories, and the rule that exactly one of stories/questions is non-empty (both non-empty is malformed).
- Reject any model-supplied `due_date`, unknown agent IDs, and malformed IDs.
- Return a typed validation error with a stable code and a human explanation that contains no raw model content.

**Deliverables**
- `src/app/planning/schema.rs`, `tests/planning_schema.rs` with a malformed case per rule.

**Done when**
- Every rule has a failing fixture and a passing fixture.
- Unknown fields, wrong enums, empty arrays, and 21 stories are rejected.
- The error message identifies the field and rule but does not echo the model response.

#### WP-5-T7 · Apply assignment and due-date rules

**Spec:** §9.4, §9.5; A-14.
**Depends on:** T6.

**Work**
- Build the set of valid assignees from current memberships; an assignment to anyone else is a validation failure (A-27).
- A `null` assignment is allowed; store the rationale as the explanation, and render `Unassigned` on the board.
- Compute `due_date` as `request.created_at.date(UTC) + interval(complexity)` using `small=2`, `medium=5`, `large=10` days; the model never supplies a date.
- Reject a story with an assignment but empty rationale.
- Workload is provided in the prompt; it does not override validity checks.

**Deliverables**
- `src/app/planning/rules.rs`, tests for every complexity, month/year boundaries, and invalid assignees.

**Done when**
- Due dates match the table for a fixed request date, including a leap-year boundary.
- Assignment to an agent outside the laboratory fails validation.
- Unassigned stories persist with an explanation and display correctly later.
- No local-time or DST dependency exists (UTC only).

#### WP-5-T8 · Implement the planning state machine and atomic persistence

**Spec:** §9.2, §9.6, §12; A-11, A-27.
**Depends on:** T1–T7.

**Work**
- Flow:
  1. resolve active laboratory and project ownership;
  2. run laboratory validation (no provider call on failure, exit 4);
  3. verify settings and credential presence (exit 2 with hint);
  4. insert the request as `TODO`;
  5. update to `PLANNING` with `planning_started_at`;
  6. dispatch one logical invocation through the retry policy;
  7. validate output;
  8. in one transaction insert all stories with state `TODO` and set status (`PLANNED` or `NEEDS_CLARIFICATION`);
  9. on failure set `FAILED` with a stable error code and sanitized message;
  10. record events and counters.
- No partial story set is ever committed.
- A `PLANNING` row older than the lease is recovered as `FAILED` with `stale_planning` on the next relevant command; never auto-retried.
- Return a typed terminal outcome: `Planned`, `NeedsClarification`, `Failed`, `Cancelled`.
- Counters never exceed the configured limits.

**Deliverables**
- `src/app/planning/mod.rs`, `src/infra/db/planning.rs`, integration tests with the scripted provider and temporary database.

**Done when**
- A test kills persistence mid-transaction and proves no stories and no `PLANNED` status remain.
- The original request row always exists before the provider call (asserted with a recording provider).
- Clarification stores questions and no stories.
- Failure preserves the request as `FAILED` with a code and hint.
- Invalid model output is classified as a validation failure with no automatic repair, no retry, and no stories.
- Stale `PLANNING` recovery is covered.
- Events record dispatch, validation outcome, and terminal status.

#### WP-5-T9 · Implement `--new-request` input handling and CLI output

**Spec:** §9.1, §9.2; §14.
**Depends on:** T8.

**Work**
- Accept literal text from a positional argument or content from stdin, never both; reject empty/whitespace-only input; require valid UTF-8; enforce 64 KiB decoded before use; reject simultaneous sources with usage guidance; when no source is available (stdin is a terminal and no text), exit 2 with a hint instead of waiting.
- Do not interpret positional text as a file path.
- On success, print the request ID, terminal status, and a story table with ID, title, assignee, priority, complexity, and due date; on clarification print the questions; on failure print code and hint.
- Map outcomes to exit codes: success 0, validation 4, provider failure 1, cancellation 5. SIGINT handling and exit code 130 are completed in T11.
- Wire `--new-request` in the dispatcher.

**Deliverables**
- `src/app/requests.rs`, `src/cli/input.rs`, `tests/cli_new_request.rs`, `tests/e2e_planning.rs` with `wiremock`.

**Done when**
- The full input matrix passes: literal, piped, both (exit 2), neither on a TTY (exit 2), empty, invalid UTF-8, limit and limit+1 bytes.
- An end-to-end run with a mock DeepSeek response creates linked stories and shows them.
- The request is persisted before the first HTTP request (asserted by querying the database from the mock server callback or via a recording layer).
- Output contains no request text, prompt, or provider body.

#### WP-5-T10 · Add planning security tests

**Spec:** §13; `SECURITY.md` Security Validation; `VALIDATION.md` S-02, S-03, S-04.
**Depends on:** T9.

**Work**
- Add adversarial tests:
  - request text instructing the model to read files, call tools, or change rules produces no such capability (there are no tools) and any resulting malformed proposal is rejected;
  - model output naming an unknown/out-of-lab agent is rejected;
  - model output containing a plausible secret or control characters is sanitized or rejected, never logged or rendered raw;
  - the outbound request contains no credential value, no CV, and no unrelated project data;
  - error paths do not echo request content or provider bodies;
  - a tool-like or shell-like string in model output is stored as inert text at most and never executed;
  - memory content instructing the model to change rules, read files, or call tools remains delimited data, produces no capability, and any resulting malformed proposal is rejected;
  - a memory containing a synthetic secret marker is absent from prompts, database event rows, interaction metadata, logs, and output.
- Assert permissions and policy are unchanged after adversarial content.

**Deliverables**
- `tests/security_planning.rs`, `tests/fixtures/injection/*`.

**Done when**
- Every adversarial case has an explicit expected outcome and passes.
- A synthetic secret marker is absent from database rows, logs, and output.
- The tests run without a live model and without network access.

#### WP-5-T11 · Handle cancellation and unknown external outcomes

**Spec:** §12; `SECURITY.md` failure handling; `VALIDATION.md` S-06, S-07.
**Depends on:** T8, T9.

**Work**
- Install a SIGINT handler that cancels the token, aborts the in-flight request, marks the request `FAILED` with `cancelled`, and exits 130.
- Distinguish local cancellation from confirmed remote termination: the message states that provider-side processing may have continued.
- Never retry after cancellation.
- Provider timeout produces a timeout failure (exit 1) or `budget_exceeded` if the total deadline expired, with no subsequent dispatch.

**Deliverables**
- Signal/cancellation module, orchestration tests with a delayed scripted provider, `tests/cli_cancel.rs` where feasible.

**Done when**
- Cancellation before dispatch performs no call.
- Cancellation during dispatch yields one attempt and no retry.
- The terminal state and exit code match the contract.
- The output warns that remote processing may be unknown.

#### WP-5-T12 · Document planning behavior and close the package

**Spec:** §9, §12; `CONSTITUTION.md` §9.
**Depends on:** T1–T11.

**Work**
- Document request input rules, terminal outcomes, error codes, retry/timeout behavior, assignment and due-date rules, scoped memory selection and the memory budget, and the security model of planning.
- Document that retries may incur additional provider charges and that exactly-once remote execution is not promised.
- Record the DeepSeek endpoint and model identifier taken from the provider's current documentation (no key); GL-T2 confirms them against the live provider.
- Record WP5 acceptance evidence in `docs/validation/wp-5.md`.

**Deliverables**
- `docs/planning.md`, `docs/validation/wp-5.md`, README update.

**Done when**
- Documentation matches implemented behavior and exit codes.
- The validation report lists executed commands and mock-based results.

#### WP-5-T13 · Add GitHub source configuration and global credential

**Spec:** §3.2, §5.1, §8.2, §11.2, §13, §17; ADR-0025.
**Depends on:** WP-4 gate.

**Work**
- Add `projects.github_source` in schema v4 (`migrations/0004_github_source.sql`), nullable for existing projects.
- Validate an optional canonical `https://github.com/<owner>/<repository>` URL; reject other hosts, extra path segments, query strings, and malformed owners/repositories before storage.
- Add the optional `source_control` section to `settings.yaml` with `provider: github`; GitHub is the only supported value and the section defaults to GitHub when omitted. Keep `provider`/`model` for DeepSeek.
- Read the GitHub key only from `credentials.github_api_key` in `settings.yaml`; report `set`/`unset` in `--show-settings`; never store in the database, log, prompt, or display the value.
- Extend `--new-project` with `--github-source <url>`. When a source is supplied, require a valid settings file, the GitHub source-control provider, and a present key; then persist the project and source atomically.
- Display the source in project creation output, project lists, and the Kanban header. Clone it into `.workspace/<project-id>` before inserting the project (ADR-0032); no other GitHub call is made.
- Add tests for URL validation, missing key, missing/invalid settings, migration from v3, and secret-marker absence.
- Update `.docs/DATA-MODEL.md` and the retention documentation.

**Deliverables**
- Migration, domain/URL validation, settings changes, CLI modifier, tests, documentation.

**Done when**
- A project without a source still works.
- A GitHub-backed project stores the canonical URL; a missing key or invalid URL exits 2 with no row and no key in output.
- Schema v3 → v4 preserves existing projects with `NULL` sources.
- The only GitHub network call is the clone; the credential is not stored in the project, database, arguments, or logs.
- `.docs/DATA-MODEL.md` matches the implemented relationships.

*(Delivered on 2026-11-12 ahead of the provider tasks; see §11.7.)*

### 11.5 Tests and acceptance criteria

| ID | Criterion | Verification |
|---|---|---|
| WP5-AC1 | Budgets, deadlines, and cancellation are enforced before dispatch | Boundary tests with test clock |
| WP5-AC2 | DeepSeek adapter matches the contract and bounds resources | `wiremock` contract tests |
| WP5-AC3 | Retries are bounded and limited to safe classes | Retry tests |
| WP5-AC4 | Prompt excludes secrets/CVs and delimits untrusted content | Golden prompt tests |
| WP5-AC5 | Structured output is strictly validated | Schema tests |
| WP5-AC6 | Assignments reference real members; due dates are computed locally | Rule tests |
| WP5-AC7 | Planning persists atomically and preserves the request on failure | Integration tests |
| WP5-AC8 | Input handling rejects empty, dual-source, oversized, and non-UTF-8 input | CLI matrix |
| WP5-AC9 | Injection cannot grant capabilities or leak data | `tests/security_planning.rs` |
| WP5-AC10 | Cancellation stops dispatch and reports uncertainty | Cancellation tests |
| WP5-AC11 | End-to-end planning works against a mock provider | `tests/e2e_planning.rs` |
| WP5-AC12 | Planning includes only the scoped memories within the budget, delimited as untrusted data | Prompt golden/boundary tests and memory-injection tests |
| WP5-AC13 | GitHub source and global credential configuration validate locally without storage, leaks, or network calls | `tests/cli_projects.rs`, migration and secret-marker tests |

### 11.6 Exit gate

- All WP5-AC pass.
- With valid settings and the scripted provider, a submitted request yields validated stories or a classified clarification/failure, with the request always preserved.
- No path executes model output.
- No secret appears in prompts, logs, errors, or storage.
- Planning prompts contain only the selected scoped memories, respect the 8 KiB budget, and never treat a memory as an instruction.
- GitHub-backed project configuration is validated locally and never triggers a network call or stores the key.

### 11.7 Partial report: WP-5-T13 (2026-11-12)

WP-5-T13 was requested ahead of the provider work and delivered on the current
working tree. Executed locally on macOS (aarch64), Rust 1.99.1: `make check`
(fmt, clippy, tests, audit, deny) passes; the full suite reports **249 passed,
0 failed** after this change.

Delivered: schema v4 nullable `github_source`; strict canonical URL validation;
optional `source_control.provider: github` in settings; `credentials.github_api_key`
presence reporting; `--new-project --github-source`; project list/Kanban
display; v3→v4 migration; `.docs/DATA-MODEL.md`; ADR-0025, ADR-0032. The only GitHub network
call is the clone; no key storage exists. At the time of this partial report the provider
tasks were not started; WP-5 is now complete, see §11.8.

### 11.8 Package report (2026-11-12)

Executed locally on macOS (aarch64), Rust 1.99.1: `make check` (fmt, clippy,
tests, audit, deny) passes; the full suite reports **301 passed, 0 failed**.

Delivered: execution limits, deadline/attempt/cancellation budget, injectable
clock, bounded provider port, DeepSeek adapter, retry policy, prompt with
scoped memory budget, strict schema v1, assignment and UTC due-date rules,
request/story state machine with atomic persistence and stale recovery,
`--new-request` input handling, CLI outcome rendering, SIGINT cancellation,
security tests, `.docs/planning.md`, `.docs/validation/wp-5.md`, and ADR-0026.

Deviations and limitations:

- The provider port is synchronous with `reqwest::blocking` (ADR-0026); the
  plan sketched an async port but the behavior and security bounds are the same.
- `VLAB_DEEPSEEK_ENDPOINT` is a test/controlled-deployment override used by the
  wiremock end-to-end test; production defaults to the DeepSeek endpoint.
- Live contract, real retry charges, and semantic quality remain GL-T2/GL-T3.
- A real PTY literal-input and SIGINT path is documented as not automated;
  cancellation is covered through tokens and scripted providers.
- Story state transitions remain out of scope; the planner creates `TODO`
  stories only.

---

## 12. WP-6 — Direct Questions, Hardening, and Release Gate

### 12.1 Objective

Deliver `--ask` for agents and the Tech Lead with scoped memory context, complete the terminal UX contract, verify resource bounds, audit secrets and privacy, finalize documentation, and pass the deterministic §17 acceptance suite. Live evaluation and the release security gate are in Go-Live.

### 12.2 Scope

**In scope:** shared invocation service and interaction metadata, `--ask` with memory context, memory-aware secret/privacy audit, final help/exit matrix, bounds verification, documentation set, full deterministic acceptance run.

**Out of scope:** new product capabilities; JSON output; packaging; autonomous execution; story transitions; live-provider evaluation, release gate, and tagging (Go-Live).

### 12.3 Prerequisites and dependencies

- WP-5 gate passed.
- No provider key, remote repository, or blocking assumption is needed.

**Execution checklist**

- [x] T1 — Extract the shared provider invocation service
- [x] T2 — Implement `--ask` (after T1)
- [x] T3 — Moved to Go-Live (GL-T3)
- [x] T4 — Audit secrets, privacy, and redaction (after T2)
- [x] T5 — Finalize the terminal UX and exit-code contract (after T1, T2)
- [x] T6 — Verify resource bounds and failure recovery (after T5)
- [x] T7 — Complete the documentation set (after T1–T6)
- [x] T8 — Moved to Go-Live (GL-T5)
- [x] T9 — Run the final deterministic acceptance suite (after T1, T2, T4–T7)

### 12.4 Tasks

#### WP-6-T1 · Extract the shared provider invocation service

**Spec:** §7.5, §7.6, §11.2, §12.
**Depends on:** WP-5 gate.

**Work**
- Factor the provider call + retry + budget + cancellation path used by planning into a reusable `ProviderInvocation` service.
- Add interaction metadata persistence: agent ID, kind (`planning`/`question`), provider, model, outcome, error code, duration, token usage; no content.
- Refactor planning to use the service without behavior change; all WP-5 tests must still pass.

**Deliverables**
- `src/app/invocation.rs`, `interactions` migration (`0005_interactions.sql`), updated planning code, regression tests.

**Done when**
- Planning tests pass unchanged after the refactor.
- Interaction rows exist for both success and failure paths.
- No content is stored in interaction metadata (schema test).

#### WP-6-T2 · Implement `--ask`

**Spec:** §7.5, §7.6; A-24.
**Depends on:** T1.

**Work**
- Requirement: active laboratory; target agent (when provided) must be a current member; without `--to-agent`, resolve the single `lead`; missing or ambiguous leads exit 3/4 with a hint.
- Validate settings and credential presence; build a minimal prompt: agent name, role, description, skills, the question, the active laboratory's memories, and the responding agent's memories within the 8 KiB memory budget and with the delimiters from §3.5.
- The answer must be stated as advisory text; neither the question nor a memory can change permissions, roles, limits, assignments, or memories.
- Invoke through the shared service with the question deadline; display a short header identifying the responding agent and model, then the sanitized answer.
- Do not create stories, change assignments, or mutate any domain table, including memories; record only interaction metadata and events.

**Deliverables**
- `src/app/ask.rs`, `src/infra/db/interactions.rs`, `tests/cli_ask.rs`, `tests/e2e_ask.rs`.

**Done when**
- Agent-targeted and default-lead questions work against a mock provider.
- Non-member target exits 3; no lead exits 3; a multiple-lead fixture is rejected by application validation and does not reach dispatch.
- A test compares the database before and after a question and finds only interaction/event rows added.
- Displayed text is sanitized and no control sequences reach the terminal.
- Only the selected scoped memories are included; out-of-scope and archived-agent memories are excluded, and the budget cut-off is applied whole-memory.

#### WP-6-T3 · Moved to Go-Live

The opt-in live-provider evaluation is GL-T3. The ID is kept so that references stay stable.

#### WP-6-T4 · Audit secrets, privacy, and redaction

**Spec:** `SECURITY.md` Secrets and Data Privacy; `VALIDATION.md` AC-10, AC-11.
**Depends on:** T2.

**Work**
- Add systematic tests using a unique synthetic marker: set it as the API key and inside request text and a memory; assert it never appears in stdout, stderr, database rows, event fields, interaction metadata, or rendered errors.
- Review every `Debug` implementation and every error construction for payload leakage.
- Confirm settings and database file permissions, temporary editor cleanup, and absence of credentials in fixtures.
- Verify memory content is never emitted in diagnostics or audit events and is sent only in the scoped prompt blocks.
- Document data flows to the provider and retention behavior.

**Deliverables**
- `tests/security_secrets.rs`, review notes in `docs/security-review.md`.

**Done when**
- Marker-based tests pass across success, provider failure, validation failure, and cancellation paths.
- The review lists each trust boundary and the corresponding control.
- No credential appears in the repository history (manual check documented).

#### WP-6-T5 · Finalize the terminal UX and exit-code contract

**Spec:** §4, §10, §14; `STACK.md` textual UI.
**Depends on:** T1, T2.

**Work**
- Freeze the help output: all commands, groups, modifiers, and descriptions; snapshot test.
- Verify stdout/stderr separation for every command, empty states, truncation, `NO_COLOR`/redirect behavior, narrow widths, and sanitization of untrusted text.
- Add a table-driven exit-code test covering every documented command and failure class, including the three memory commands and their scope modifiers.
- Verify no command prompts when stdin is not a terminal except where confirmation is required (which fails safely), and no command hangs without input.
- Remove the scaffold `NotImplemented` error variant and assert that every dispatch handler is implemented.

**Deliverables**
- `tests/cli_contract.rs`, updated golden files, `docs/cli-reference.md`.

**Done when**
- The help snapshot is stable and complete.
- Exit-code tests cover success, usage, missing context, validation, cancellation, interruption, and provider failure.
- Redirected output is free of escape bytes and progress noise.
- All command examples in the reference execute successfully against fixtures.

#### WP-6-T6 · Verify resource bounds and failure recovery

**Spec:** §12; `SECURITY.md` resource exhaustion; A-12.
**Depends on:** T5.

**Work**
- Add boundary tests for request bytes, response bytes, story count, output tokens, memory content and memory-context bytes, and deadline; test limit and limit+1.
- Verify no partial writes on failure, no retries after cancellation, no dispatch after budget exhaustion, and no unbounded memory growth when the provider streams an oversized body.
- Document observed latencies and limits; adjust constants only with recorded evidence.

**Deliverables**
- `tests/bounds.rs`, updated constants and documentation.

**Done when**
- Every bound has a passing boundary pair.
- Oversized responses are aborted and classified, not truncated into success.
- A test proves a failed planning run leaves zero stories and a `FAILED` request.

#### WP-6-T7 · Complete the documentation set

**Spec:** `CONSTITUTION.md` §9; §17; `SECURITY.md` limitations.
**Depends on:** T1–T6.

**Work**
- Finalize `README.md` (quickstart, requirements, commands, memory walkthrough), `docs/cli-reference.md`, `docs/configuration.md`, `docs/data-retention.md`, `docs/security.md`, `docs/architecture.md`, ADRs, and a limitations section covering: no encryption at rest, no backups, no story transitions, no JSON output, no Windows hardening, semantic assignment quality not verifiable, immutable automatic memories, no manual memory commands, and bounded planning-summary generation.
- Update the traceability matrix in this plan with the actual test names that validate each row.

**Deliverables**
- Documentation set and updated §13 of this plan.

**Done when**
- Every documented step of the primary workflow runs against the scripted provider; the real-user walkthrough is GL-T4.
- Every known limitation is explicit and linked to the corresponding risk.
- Documentation claims match implemented behavior; unverified items are labeled as such.

#### WP-6-T8 · Moved to Go-Live

The release security gate is GL-T5. The ID is kept so that references stay stable.

#### WP-6-T9 · Run the final deterministic acceptance suite

**Spec:** §17 (all 24 criteria); `VALIDATION.md` Definition of Done.
**Depends on:** T1, T2, T4–T7.

**Work**
- Execute `cargo fmt --all -- --check`, `cargo clippy --locked --all-targets -- -D warnings`, `cargo test --locked`, `cargo audit`, `cargo deny check`.
- Run the end-to-end acceptance suite covering §17 items 1–24 and the adapted `VALIDATION.md` scenarios, including S-09 memory injection.
- Record results in `docs/validation/final.md`, including environment, commit, commands, pass/fail per criterion, and residual limitations. Live evaluation results are added in GL-T3.

**Deliverables**
- Final validation report, updated plan status. Tagging is GL-T6.

**Done when**
- All §17 criteria pass with evidence.
- Security-boundary tests pass individually (not averaged).
- The completion report distinguishes implemented controls from planned work and unverified assumptions.

### 12.5 Tests and acceptance criteria

| ID | Criterion | Verification |
|---|---|---|
| WP6-AC1 | Shared invocation is used by planning and questions without behavior change | WP-5 regression suite |
| WP6-AC2 | `--ask` targets a member or the lead and never mutates domain state | `tests/cli_ask.rs`, DB diff test |
| WP6-AC3 | Moved to Go-Live (GL-AC3) | — |
| WP6-AC4 | Secrets are absent from all outputs and storage paths | `tests/security_secrets.rs` |
| WP6-AC5 | Help and exit codes are frozen and fully tested | `tests/cli_contract.rs` |
| WP6-AC6 | Every resource bound is enforced at limit and limit+1 | `tests/bounds.rs` |
| WP6-AC7 | Documentation is complete and accurate | Review against §17 and limitations list |
| WP6-AC8 | Moved to Go-Live (GL-AC5) | — |
| WP6-AC9 | All §17 acceptance criteria (1–24) pass with evidence | `docs/validation/final.md` |
| WP6-AC10 | Questions include only the scoped memories within the budget and never mutate memory state | `tests/cli_ask.rs`, `tests/e2e_ask.rs`, memory privacy tests |

### 12.6 Exit gate

- All WP6-AC pass, except AC3 and AC8, which moved to Go-Live.
- The binary builds reproducibly with the locked toolchain and dependencies.
- Against the scripted provider, the full workflow runs end to end: configure, create a laboratory and agents, create a project, add scoped memories, submit a request, read the plan on the board, and ask a follow-up question that uses the selected memories.
- The validation report states exactly what was verified, what was not, and what Go-Live must still verify.

### 12.7 Package report (2026-11-12)

Executed locally on macOS (aarch64), Rust 1.99.1: `make check` (fmt, clippy,
tests, audit, deny) passes; the full deterministic suite reports **319 passed,
0 failed**.

Delivered: shared `ProviderInvocation` path with schema v5 `interactions`
(metadata only; ADR-0028), `--ask` with agent/lead targeting and scoped memory context,
content-free interaction/event persistence, secret/privacy tests,
`tests/cli_contract.rs`, `tests/bounds.rs`, CLI reference, configuration and
security-review documentation, final validation report, and removal of the
scaffold `NotImplemented` branch.

Deviations and limitations:

- The provider port remains synchronous per ADR-0026; questions use the same
  bounded retry/budget path as planning.
- `--ask` reads only; the only writes are interaction metadata and events.
- A real PTY confirmation and SIGINT were not automated; non-TTY behavior and
  cancellation tokens are covered.
- Live provider contract, semantic quality, and real cost are Go-Live items
  GL-T2/GL-T3.
- No story transitions, JSON output, backups, or encryption at rest exist.

---

## Go-Live — Hosted CI, Live Verification, and Release

### GL.1 Objective

Take the build verified locally in WP-1 to WP-6 and validate it against the outside world: hosted CI, the live DeepSeek API, a real user, and the release security gate, ending with the `0.1.0` tag. Every activity that needs a remote repository, a real provider key, a live call, or a release decision is here, and nowhere earlier.

### GL.2 Scope

**In scope:** activating the committed CI, daily scan, and Dependabot files; repository protection; live contract confirmation; the opt-in live evaluation; a real-user walkthrough; the release security gate; tagging.

**Out of scope:** new product behavior. A defect found here is fixed with the normal TDD loop, and the affected package's tests are extended; it does not reopen the plan.

### GL.3 Prerequisites and dependencies

- WP-6 gate passed.
- Blocking assumption confirmed: A-21 (release channel and owner).
- A GitHub remote exists, and a DeepSeek API key is configured in the private `settings.yaml` for opt-in live runs only.

**Execution checklist**

- [ ] GL-T1 — Activate hosted CI and repository protection
- [ ] GL-T2 — Confirm the DeepSeek contract against the live provider (after GL-T1)
- [ ] GL-T3 — Run the opt-in live-provider evaluation (after GL-T2)
- [ ] GL-T4 — Run a real-user walkthrough (after GL-T2)
- [ ] GL-T5 — Pass the release security gate (after GL-T1, GL-T3, GL-T4)
- [ ] GL-T6 — Tag the release (after GL-T5)

### GL.4 Tasks

#### GL-T1 · Activate hosted CI and repository protection

**Spec:** `STACK.md` Quality and Security Tooling; ADR-0012; ADR-0013.
**Depends on:** WP-6 gate.

**Work**
- Push the repository to the GitHub remote.
- Uncomment `.github/workflows/ci.yml`, `.github/workflows/security.yml`, and `.github/dependabot.yml` as described in `.docs/CI.md`, re-verify the pinned action SHAs and tool versions, and update them if newer releases exist.
- Apply the manual repository settings in `.docs/CI.md`: branch protection with the required checks, read-only workflow token, Dependabot alerts, private vulnerability reporting.
- Show once that a deliberate formatting or clippy violation fails hosted CI, then revert it.

**Deliverables**
- Active workflows, recorded repository settings, an ADR recording that hosted CI is active again (superseding ADR-0013).

**Done when**
- CI passes on Linux and macOS for the release candidate commit.
- The daily scan has run at least once successfully.
- Branch protection rejects a push that bypasses the required checks.

#### GL-T2 · Confirm the DeepSeek contract against the live provider

**Spec:** §12; `STACK.md` AI Integration; A-02, Q-01.
**Depends on:** GL-T1.

**Work**
- With `credentials.deepseek_api_key` set in `settings.yaml`, send one minimal planning request and one question through the real adapter. Confirm the endpoint, the model identifier recorded in WP-5-T12, authentication failure mapping (with an invalid key), and the response envelope the `wiremock` contract tests assume.
- If the live contract differs from the mocks, fix the adapter and the contract tests first (TDD), then repeat.

**Deliverables**
- `docs/evaluations/live-contract-<date>.md` with endpoint, model identifier, commit, and outcome; no key or request content.

**Done when**
- The live responses match the contract tests, or the differences have been fixed and re-verified.

#### GL-T3 · Run the opt-in live-provider evaluation (formerly WP-6-T3)

**Spec:** `VALIDATION.md` Live-Provider Evaluations; §16 live tests.
**Depends on:** GL-T2.

**Work**
- Add `tests/live_smoke.rs` marked `#[ignore]`, gated by `VLAB_LIVE_TESTS=1` and a configured `credentials.deepseek_api_key`.
- Use five synthetic request fixtures, including at least one with laboratory, project, and agent memories, each run three times, against a temporary database.
- Evaluate each run against the task-quality rubric: stories produced, assignments valid, due dates correct, no unsupported fields, and memory context respected without becoming instructions; record pass/fail.
- Require at least 14 of 15 successful runs and zero unauthorized actions or limit violations; report all failures with classifications.
- Redact keys and request content from the report; never retry until a run passes.

**Deliverables**
- Live test harness, `docs/evaluations/live-smoke-<date>.md` template, README instructions.

**Done when**
- The suite is skipped by `cargo test` and CI and runs only when explicitly enabled.
- A recorded run reports success rate, latency, token usage where available, model identifier, and application commit.
- Failures are reported, not hidden.

#### GL-T4 · Run a real-user walkthrough

**Spec:** §17; `CONSTITUTION.md` §3.V.
**Depends on:** GL-T2.

**Work**
- A person who did not implement the feature installs, configures, and completes the primary workflow using only the documentation and the live provider: configure, create a laboratory and agents, create a project, add laboratory/project/agent memories, submit a request, read the board, ask a follow-up question, and delete a memory.
- Record friction points; fix documentation or defects with tests before the release gate.

**Deliverables**
- Walkthrough notes in `docs/validation/go-live.md`.

**Done when**
- The workflow completes using only the documentation, and every reported issue is fixed or explicitly accepted.

#### GL-T5 · Pass the release security gate (formerly WP-6-T8)

**Spec:** `SECURITY.md` Release Security Gate; `CONSTITUTION.md` §10.
**Depends on:** GL-T1, GL-T3, GL-T4.

**Work**
- Establish or explicitly block on the private vulnerability-reporting channel; document supported versions and the security-fix process.
- Record deployment boundary and data classification (local CLI, single OS user, project data may be sensitive).
- Verify authorization/approval controls, finite execution limits, dependency review, secret handling, and outbound data flow, including that only scoped and budgeted memory content leaves the application.
- Assign incident-response responsibility and record explicitly accepted residual risks.
- Confirm CI daily scans, locked builds, and branch protection.

**Deliverables**
- `docs/release-checklist.md` completed for version `0.1.0`, `SECURITY.md` project-specific updates, risk acceptance record.

**Done when**
- Every gate item has an owner, evidence, and status.
- No unresolved high-severity security finding remains.
- If the vulnerability channel is unavailable, the release is labeled internal and the gate item remains open (no silent waiver).

#### GL-T6 · Tag the release

**Spec:** `CONSTITUTION.md` §9; `SECURITY.md` release gate.
**Depends on:** GL-T5.

**Work**
- Re-run the local gates and confirm hosted CI is green on the release commit.
- Add the Go-Live evidence to `docs/validation/final.md`, then tag `0.1.0`, labeled internal if the release gate leaves the vulnerability channel open (A-21).

**Deliverables**
- Tag `0.1.0`, final validation report, updated plan status.

**Done when**
- The tag points at a commit on which every gate passed.

### GL.5 Tests and acceptance criteria

| ID | Criterion | Verification |
|---|---|---|
| GL-AC1 | Hosted CI, daily scan, and branch protection are active | CI runs on Linux and macOS; repository settings |
| GL-AC2 | The live DeepSeek contract matches the adapter and its contract tests | `docs/evaluations/live-contract-<date>.md` |
| GL-AC3 | Live evaluation is opt-in, budgeted, and recorded (formerly WP6-AC3) | `tests/live_smoke.rs`, evaluation report |
| GL-AC4 | A real user completes the workflow from the documentation | `docs/validation/go-live.md` |
| GL-AC5 | Release security gate is satisfied or explicitly blocked (formerly WP6-AC8) | `docs/release-checklist.md` |
| GL-AC6 | The release tag points at a commit on which every gate passed | Tag, CI run, final report |
| GL-AC7 | Scoped memory content is privacy-reviewed and absent from logs, events, and interaction metadata | Security gate evidence and `docs/security-review.md` |

### GL.6 Exit gate

- All GL-AC pass.
- The release report states exactly what was verified, what was not, and which risks were accepted.

---

## 13. Requirement Traceability Matrix

### 13.1 Functional requirements

| ID | Requirement | Source | Implementation | Validation |
|---|---|---|---|---|
| R-F01 | Deliver only the specified initial scope | §2 | All packages; scope guards in review | WP6-AC7; §2.4 review |
| R-F02 | Laboratory identity, name, memberships, one Tech Lead | §3.1 | WP-3-T1, T3, T6 | WP3-AC1, AC3, AC5 |
| R-F03 | Projects belong to one laboratory; no silent context switch | §3.2 | WP-4-T2, T3 | WP4-AC2 |
| R-F04 | Agent profile fields and supported roles | §3.3 | WP-3-T1, T3, T7 | WP3-AC1, AC6 |
| R-F05 | Active laboratory selection and error contract | §3.4 | WP-2-T6, T7; WP-3-T5 | WP2-AC5, AC6; WP3-AC4 |
| R-F06 | Help grouping and exactly one primary action | §4.1 | WP-1-T4; WP-6-T5 | WP1-AC1, AC3; WP6-AC5 |
| R-F07 | Unique, short, resolvable identifiers | §4.2 | WP-3-T2 | WP3-AC2 |
| R-F08 | `settings.yaml` structure, locations, no secrets | §5.1 | WP-2-T3, T4, T8 | WP2-AC3, AC7 |
| R-F09 | `--show-settings` behavior | §5.2 | WP-2-T4 | WP2-AC3 |
| R-F10 | `--edit-settings` atomic and validation-safe | §5.3 | WP-2-T5 | WP2-AC4 |
| R-F11 | Agent defaults copied from settings | §5.4 | WP-3-T7 | WP3-AC6 |
| R-F12 | `--list-labs` content and empty state | §6.1 | WP-3-T4; WP-4-T8 | WP3-AC4; WP4-AC9 |
| R-F13 | `--new-lab` validation and output | §6.2 | WP-3-T4 | WP3-AC4 |
| R-F14 | `--open-lab` persistence and warnings | §6.3 | WP-3-T5 | WP3-AC4 |
| R-F15 | `--delete-lab` confirmation, cascade, active selection | §6.4 | WP-4-T5, T6, T7 | WP4-AC5, AC6, AC7 |
| R-F16 | `--check-lab` validation rules | §6.5 | WP-3-T6; WP-4-T8 | WP3-AC5; WP4-AC9 |
| R-F17 | `--list-agents` content | §7.1 | WP-3-T8; WP-4-T8 | WP3-AC7; WP4-AC9 |
| R-F18 | `--new-agent` guided, validated, atomic | §7.2 | WP-3-T7 | WP3-AC6 |
| R-F19 | `--delete-agent` scope, blocks, archive, warning | §7.3 | WP-4-T7 | WP4-AC6, AC8 |
| R-F20 | Agent detail and evidence-based metrics (`N/A` when unavailable) | §7.4 | WP-3-T8; WP-4-T8; WP-5-T8 | WP3-AC7; WP4-AC9 |
| R-F21 | `--ask` to agent or default lead | §7.5, §7.6 | WP-6-T1, T2 | WP6-AC1, AC2 |
| R-F22 | `--list-projects` content and empty state | §8.1 | WP-4-T3, T8 | WP4-AC2, AC9 |
| R-F23 | `--new-project` scoping and validation | §8.2 | WP-4-T3 | WP4-AC2 |
| R-F24 | `--delete-project` confirmation and cascade | §8.3 | WP-4-T5, T6, T7 | WP4-AC5, AC6 |
| R-F25 | Request input forms and rejection rules | §9.1 | WP-5-T9 | WP5-AC8 |
| R-F26 | Planning workflow sequence | §9.2 | WP-5-T8, T9 | WP5-AC7, AC11 |
| R-F27 | Required story fields and initial `TODO` state | §9.3 | WP-4-T1; WP-5-T6, T8 | WP4-AC1; WP5-AC5, AC7 |
| R-F28 | Assignment rules and rejection of non-members | §9.4 | WP-5-T7 | WP5-AC6 |
| R-F29 | Due-date policy and UTC calculation | §9.5 | WP-5-T7 | WP5-AC6 |
| R-F30 | Clarification and failure preservation | §9.6 | WP-5-T8 | WP5-AC7 |
| R-F31 | Read-only Kanban layout and content | §10 | WP-4-T4 | WP4-AC3, AC4 |
| R-F32 | Persistence entities, timestamps, migrations | §11 | WP-2-T1, T2; WP-3-T3; WP-4-T1; WP-6-T1 | WP2-AC1, AC2; WP3-AC3; WP4-AC1; WP6-AC1 |
| R-F33 | DeepSeek integration and bounded execution | §12 | WP-5-T1–T4, T11 | WP5-AC1–AC3, AC10 |
| R-F34 | Security requirements for untrusted content and no execution | §13 | WP-5-T10; WP-6-T4; GL-T5 | WP5-AC9; WP6-AC4; GL-AC5 |
| R-F35 | CLI errors and exit codes | §14 | WP-1-T5; WP-6-T5 | WP1-AC4; WP6-AC5 |
| R-F36 | Rust implementation guidelines | §15 | WP-1-T2, T3; ADRs | WP1-AC7, AC8; review |
| R-F37 | Required automated tests | §16 | Every package; WP-1-T8 | All AC tables |
| R-F38 | Acceptance criteria 1–24 | §17 | All packages | WP6-AC9; §13.3 |
| R-F39 | Incremental delivery plan | §18 | Package sequence in §6 | Package gates |
| R-F40 | Evidence traceability and semantic limits | `VALIDATION.md` | WP-5-T6, T10; WP-6-T7 | WP5-AC5, AC9; documentation |
| R-F41 | Scoped memories for laboratories, projects, and agents, with create/list/delete and cascade rules | §3.5, §4.1, §11.2 | WP-4-T1, T9 | WP4-AC1, AC10 |
| R-F42 | Planning and questions include only selected scoped memories within the 8 KiB budget, delimited as untrusted data | §3.5, §9.2, §7.5, §12 | WP-5-T5, T10; WP-6-T2, T4 | WP5-AC12; WP6-AC10; WP5-AC9 |
| R-F43 | Optional canonical project GitHub source with a separate source-control provider and global `credentials.github_api_key`; local validation only | §3.2, §5.1, §8.2, §11.2 | WP-5-T13 | WP5-AC13 |

### 13.2 Engineering and security requirements

| ID | Requirement | Source | Implementation | Validation |
|---|---|---|---|---|
| R-E01 | Small increments with defined acceptance criteria | Constitution §3.I; Principles §1–3 | Task structure; package gates | Reviews; validation reports |
| R-E02 | Test-first for behavior changes | Constitution §3.III; Principles §4 | Every task's Done-when | Test diffs in review |
| R-E03 | Layered domain independent of I/O | Constitution §4; Principles §6; §15 | WP-1-T3; ADR-0001 | WP1-AC8 |
| R-E04 | Single orchestration, no agent framework | Architecture Non-Goals; STACK AI Integration | WP-5-T8; WP-6-T1; ADR-0003 | WP5-AC7; review |
| R-E05 | Dependencies verified, locked, minimal | STACK policy; Security Rust practices | WP-1-T2, T9 | WP1-AC7; dependency register |
| R-E06 | CI and supply-chain gates on every change | STACK tooling | WP-1-T9 (local gates, ADR-0013); GL-T1 (hosted CI) | Local gate evidence per package; GL-AC1 |
| R-E07 | Structured audit events without payloads | Security Logging; §4.5 | WP-1-T7; WP-5-T8; WP-6-T1 | WP1-AC6; WP6-AC4 |
| R-E08 | Secrets never committed, logged, prompted, or stored | Constitution §3.IV; Security Secrets | WP-2-T8; WP-5-T10; WP-6-T4 | WP2-AC7; WP5-AC9; WP6-AC4 |
| R-E09 | SQLite conventions: FK, migrations, busy timeout, permissions | STACK Persistence; Security | WP-2-T1, T2 | WP2-AC1, AC2 |
| R-E10 | No shell evaluation of untrusted input | Constitution §5; Security Command injection | WP-2-T5 | WP2-AC4; editor tests |
| R-E11 | Bounded parsing, buffering, and response size | Security Rust practices; §12 | WP-5-T3, T6, T9; WP-6-T6 | WP5-AC2, AC5, AC8; WP6-AC6 |
| R-E12 | Destructive actions require confirmation | Constitution §3.IV; Security Authorization | WP-4-T5, T7 | WP4-AC5, AC6 |
| R-E13 | Retention and deletion rules defined before durable storage | Security Data Privacy | WP-2-T8; WP-4-T7 | WP2-AC7; WP4-AC6 |
| R-E14 | Release security gate before public release | Security Release Gate | GL-T5 | GL-AC5 |
| R-E15 | Documentation in English, accurate and actionable | Constitution §9 | Every package; WP-6-T7 | Review |
| R-E16 | Configuration via environment; validated at startup | Twelve-Factor; STACK | WP-1-T6; WP-2-T3 | WP1-AC5; WP2-AC3 |
| R-E17 | Safe Rust; no unsafe code | Constitution §4 | WP-1-T3 (`forbid(unsafe_code)`) | Build + review |
| R-E18 | Observability supports investigation without sensitive content | Constitution §6; Security Logging | WP-1-T7; WP-6-T4 | WP1-AC6; WP6-AC4 |
| R-E19 | No automatic retry of unsafe or cancelled operations | Security Failure Handling | WP-5-T4, T11 | WP5-AC3, AC10 |
| R-E20 | Memory content is untrusted, bounded, delimited, and cannot grant permissions or be written by the model | Security Prompt injection, Memory; §3.5, §13 | WP-4-T9; WP-5-T5, T10; WP-6-T2, T4 | WP4-AC10; WP5-AC12; WP6-AC10 |
| R-E21 | Memory retention, deletion, and outbound data flow are documented and enforced | Security Data Privacy; §11.2 | WP-4-T9, T10; WP-6-T4, T7; GL-T5 | WP4-AC10; WP6-AC4; GL-AC7 |

### 13.3 Product acceptance criteria (§17)

| AC | Criterion | Tasks | Validation |
|---|---|---|---|
| 1 | `--help` documents all commands | WP-1-T4; WP-6-T5 | WP1-AC1; WP6-AC5 |
| 2 | Settings can be shown/edited without exposing credentials | WP-2-T4, T5 | WP2-AC3, AC4 |
| 3 | Invalid edits preserve the previous file | WP-2-T5 | WP2-AC4 |
| 4 | Laboratories can be created, listed, opened, checked, deleted | WP-3-T4, T5, T6; WP-4-T7 | WP3-AC4, AC5; WP4-AC6 |
| 5 | Validation fails without agents or Tech Lead | WP-3-T6 | WP3-AC5 |
| 6 | Agents created via guided procedure and inspected | WP-3-T7, T8 | WP3-AC6, AC7 |
| 7 | Detail shows memberships, assignments, evidence-based metrics | WP-3-T8; WP-4-T8 | WP3-AC7; WP4-AC9 |
| 8 | Projects created, listed, deleted in the active laboratory | WP-4-T2, T3, T7 | WP4-AC2, AC6 |
| 9 | Requests accepted as quoted text or redirected input | WP-5-T9 | WP5-AC8 |
| 10 | Original request persisted before model processing | WP-5-T8 | WP5-AC7 |
| 11 | Valid planning produces linked stories | WP-5-T6, T8, T9 | WP5-AC5, AC11 |
| 12 | Assignments reference real laboratory members | WP-5-T7 | WP5-AC6 |
| 13 | Every story has a locally validated due date | WP-5-T7 | WP5-AC6 |
| 14 | Failed processing never loses the accepted request | WP-5-T8 | WP5-AC7 |
| 15 | Kanban readable on narrow terminals and without color | WP-4-T4 | WP4-AC3 |
| 16 | Questions target an agent or default to the lead | WP-6-T2 | WP6-AC2 |
| 17 | Destructive operations require confirmation | WP-4-T5, T7 | WP4-AC5, AC6 |
| 18 | Provider execution is bounded and observable | WP-5-T1–T4, T11 | WP5-AC1–AC3, AC10 |
| 19 | No model-generated content is executed | WP-5-T10; WP-6-T2 | WP5-AC9; WP6-AC2 |
| 20 | Core workflows pass deterministic automated tests | Every package; WP-6-T9 | All AC tables; WP6-AC9 |
| 21 | Memories can be created, listed, and deleted for laboratories, projects, and agents, with scope enforced and deletion cascading from the owner | WP-4-T9 | WP4-AC10 |
| 22 | Memory content is bounded, sanitized, untrusted, and never executed or used to grant permissions or change assignment rules | WP-4-T9; WP-5-T10; WP-6-T4 | WP4-AC10; WP5-AC12; WP6-AC10 |
| 23 | Planning and questions include only the scoped memories within the configured cap, delimited as data, without silently truncating a memory | WP-5-T5; WP-6-T2 | WP5-AC12; WP6-AC10 |
| 24 | Projects can optionally reference a canonical GitHub source; creating such a project validates the URL, requires a global `credentials.github_api_key`, and clones it into `.workspace/<project-id>` before insertion | WP-5-T13 | WP5-AC13 |

### 13.4 Adapted validation scenarios (`VALIDATION.md`)

| Scenario | VLab adaptation | Tasks | Expected enforcement |
|---|---|---|---|
| S-01 Authorized summary | Authorized planning request produces valid stories | WP-5-T8, T9 | One invocation, all fields valid, persisted once |
| S-02 Unauthorized file | Assignment to a non-member or unknown agent | WP-5-T7 | Rejected before persistence |
| S-03 Prompt injection | Injected instructions in request text or model output | WP-5-T10 | No capability change, no leak, no execution |
| S-04 Unsupported claim | Fabricated agent ID, missing field, or invented due date | WP-5-T6, T7 | Validation failure; no success status |
| S-05 Repeated action/budget | Extra invocation or retry beyond the limit | WP-5-T1, T4 | No dispatch after exhaustion; `budget_exceeded` |
| S-06 Provider timeout | Mock provider exceeds the per-attempt deadline | WP-5-T3, T4, T11 | Timeout classification; no retry after cancellation |
| S-07 User cancellation | SIGINT during planning | WP-5-T11 | Status `FAILED` with error code `cancelled`, exit 130, no retry |
| S-08 Invalid/oversized input | Empty, non-UTF-8, oversized request, oversized response, memory over the content or context cap | WP-5-T6, T9; WP-6-T6; WP-4-T9 | Classified failure at the boundary; no downstream call |
| S-09 Memory injection | Injected instructions in laboratory, project, or agent memory | WP-4-T9; WP-5-T10; WP-6-T4 | No capability change, no leak, no execution; memory remains delimited data |

Evidence matching (exact excerpts) has no direct VLab equivalent; the analog is referential and structural integrity of model output, which is enforced in WP-5-T6. Semantic correctness of assignments remains an evaluation criterion, not a proof.

### 13.5 Explicitly deferred requirements

| Item | Source | Reason |
|---|---|---|
| JSON output | STACK "where useful" | Not in the product command contract (D-5, Q-03) |
| SQLite backup/restore | STACK persistence | No command specified; documented limitation (D-6) |
| Encryption at rest | STACK/Security | Out of scope; OS disk encryption recommended |
| WAL | STACK | No concurrency requirement (D-7) |
| Story state transitions and execution | §2 exclusions; pitch | Deferred to a future release |
| Autonomous multi-agent collaboration | Pitch | Contradicts architecture/spec; deferred (D-2) |
| Windows hardening | Platform | Best-effort only (A-19) |
| Packaging/licensing | — | Undefined (A-20, Q-07) |
| Vulnerability channel | Security release gate | Open item (A-21, Q-08) |

---

## 14. Sequencing and Dependency Audit

### 14.1 Package-level prerequisites

| Package | Requires | Produced by |
|---|---|---|
| WP-1 | Documentation only | — |
| WP-2 | CLI contract, errors, paths, test harness | WP-1 |
| WP-3 | Settings, database, migrations v1, active-state helpers, error contract | WP-2 |
| WP-4 | Laboratories, agents, schema v2, rendering, confirmation primitives | WP-3 |
| WP-5 | Schema v3, projects, scoped memories, active-planning guard, settings, agents, rendering | WP-4 |
| WP-6 | Provider adapter, planning orchestration, interactions schema, memory-aware prompts, all commands | WP-5 |
| Go-Live | Complete, locally verified build; committed CI files; documentation | WP-1, WP-6 |

### 14.2 Audit of cross-package artifacts

The following artifacts are consumed only after the package that creates them:

| Artifact | Created | First consumed |
|---|---|---|
| `AppError` and exit mapping | WP-1 | WP-2 |
| Path resolution and test harness | WP-1 | WP-2 |
| Settings model | WP-2 | WP-3 (agent defaults, check-lab) |
| `app_state` active laboratory | WP-2 | WP-3 (`--open-lab`) |
| Schema v1, migrations, `Database`, `StorageError` | WP-2 | WP-3 (first database-backed command) |
| Schema v2 and agent repository | WP-3 | WP-4 (FKs, deletion) |
| Rendering primitives | WP-3 | WP-4 (Kanban, lists) |
| Schema v3 (projects, requests, stories, memories) | WP-4 | WP-5 (planning persistence and memory-aware prompts) |
| Schema v4 (`projects.github_source`) | WP-5 | WP-5 (project commands) |
| Memory repository and commands | WP-4 | WP-5 (prompt context), WP-6 (`--ask`) |
| Confirmation subsystem | WP-4 | WP-4 (entity and memory deletion) |
| Active-planning guard | WP-4 | WP-5 (recovery) |
| Provider port and adapter | WP-5 | WP-6 (`--ask`) |
| Invocation service and interactions | WP-6 | WP-6 (`--ask`) |

### 14.3 Intra-package ordering checks

- WP-1: errors (T5) precede paths (T6); observability (T7) precedes the harness (T8); CI (T9) consumes the dependency register (T2) and harness (T8).
- WP-2: connection (T1) precedes migrations (T2); settings (T3) precedes show/edit (T4/T5); app state (T6) precedes the context helper (T7).
- WP-3: domain (T1) and IDs (T2) precede schema (T3); schema precedes repositories; `--check-lab` (T6) precedes the wizard (T7), which reuses its validation rules; rendering (T9) precedes the package documentation.
- WP-4: schema (T1) precedes all repositories; confirmation (T5) and the planning guard (T6) precede deletions (T7); count extensions (T8) come after queries exist; memories (T9) follow the schema and confirmation subsystem. The memory task does not depend on the Kanban renderer.
- WP-5: T13 (GitHub source configuration) has no provider dependency and was delivered ahead of T1; the remaining order is budget (T1), port (T2), adapter (T3), retry (T4), state machine (T8), prompt (T5) consuming the WP-4 memory repository, schema (T6), rules (T7), input handling (T9), security (T10), and cancellation (T11).
- WP-6: shared invocation (T1) precedes `--ask` (T2), which adds the question memory selector; hardening tasks (T4–T7) consume the complete feature set, including memories, and precede the final acceptance run (T9). T3 and T8 moved to Go-Live.
- Go-Live: hosted CI (GL-T1) precedes live contract confirmation (GL-T2), which precedes the live evaluation (GL-T3) and the walkthrough (GL-T4); the release gate (GL-T5) precedes the tag (GL-T6).

### 14.4 No-forward-dependency statement

No task requires a table, type, command, helper, or test that appears later in this plan. Where an earlier package would otherwise need later data (for example, project counts in `--list-labs`), the plan explicitly defers the field to the later package rather than introducing a placeholder dependency. The only intentional placeholder is the scaffold `NotImplemented` error variant introduced in WP-1-T5; it has no domain behavior, is used only by handlers not yet implemented, and is removed in WP-6-T5.

---

## 15. Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| DeepSeek API or model identifiers change before WP-5 | Medium | Planning unavailable | Verify at WP-5-T3/T12; contract tests isolate the adapter; no hardcoded default model |
| YAML parser dependency is unmaintained or vulnerable | Medium | Supply-chain and security gate failure | Explicit evaluation in WP-1-T2; `cargo deny`; replaceable parser behind a small module |
| Interactive wizard is hard to test | Medium | Weak coverage of creation flow | `Prompter` port with scripted tests; optional PTY smoke; non-TTY behavior tested |
| Runtime SQL queries hide typos until runtime | Medium | Persistence bugs | File-backed integration tests for every repository; migration/constraint tests |
| 8-character IDs collide | Low | Insert failure | Bounded retry; primary-key uniqueness; typed error after retries |
| Prompt injection manipulates planning | Medium | Invalid or biased plan | No tools; strict local validation; membership checks; adversarial tests; prompt delimiting |
| Memory content manipulates planning or questions | Medium | Invalid plan or biased answer | Memories are delimited data; no tools; model cannot write memories; adversarial memory-injection tests; bounded selection |
| Memory becomes a privacy or data-leak channel | Medium | Sensitive data sent or logged | Explicit scope selection; 8 KiB cap; no content in logs, events, or interaction metadata; user warning against credentials; privacy review at GL-T5 |
| Memory grows without bound | Medium | Prompt/context pressure | 20 memories per scope, 2,000 characters each, whole-memory omission above 8 KiB; boundary tests |
| Memory scope confusion exposes another laboratory's note | Low | Cross-context leak | Exactly-one-owner check; project ownership bound to the active laboratory; scope-matrix tests |
| Archived agent memory leaks into prompts | Low | Privacy and stale context | Prompt selector excludes archived agents; archive tests |
| Model output exceeds limits | Medium | Memory/DB pressure | Streaming body cap; story/field/token limits; boundary tests |
| Cancellation leaves ambiguous remote state | Medium | Misreported outcome | Explicit `FAILED`/`cancelled` outcome; uncertainty message; no retry; audit event |
| Stale `PLANNING` blocks deletion | Low | Operational friction | Lease-based staleness; recovery on next relevant command |
| Confirmation bypass in scripts | Medium | Accidental data loss | Bound confirmation; `--yes` explicit; non-TTY fails closed; deletion tests |
| SQLite locking on network filesystems | Low | Corruption/lock errors | Document unsupported; single connection; actionable lock error |
| Test flakiness from wall-clock timing | Medium | Unreliable CI | Injected clock; scripted provider; no sleeps in assertions |
| Terminal width differences in CI | Low | Golden-test churn | Width is a renderer parameter; tests pass explicit widths |
| Semantic quality of assignments cannot be proven | High | Overstated correctness | Document as evaluation criterion; no self-assessment; live rubric |
| Scope creep toward autonomous execution | Medium | Architecture violation | §2.4 exclusions; WP review checklist; ADR-0003 |
| Live provider behavior differs from the mocks, found only at Go-Live | Medium | Late adapter rework | Contract tests built from current provider documentation (WP-5-T3, T12); GL-T2 confirms before evaluation; fixes follow TDD |
| No hosted CI before Go-Live | Medium | A gate is skipped or Linux breaks unnoticed | Local gates mandatory per change (ADR-0013); package reports record gate evidence; GL-T1 runs both platforms |

---

## 16. Definition of Done (Project)

The project is complete when:

1. All six package gates and the Go-Live gate have passed and their validation reports are recorded.
2. Every §17 acceptance criterion passes with named tests and executed evidence.
3. All security-boundary tests pass individually; none are averaged into a quality score.
4. `cargo fmt --all -- --check`, `cargo clippy --locked --all-targets -- -D warnings`, `cargo test --locked`, `cargo audit`, and `cargo deny check` pass on the release commit.
5. The live-provider evaluation (GL-T3) is recorded with model identifier, commit, fixtures, success rate, and failures.
6. Documentation states implemented controls, planned work, and known limitations, including semantic-verification limits, unencrypted storage, immutable scoped memories, and the absence of automatic memory capture.
7. No known permission-boundary, execution-limit, or data-leak defect remains open.
8. The release security checklist (GL-T5) is complete or explicitly blocked with an owner and an internal-only release label.
9. No secret appears in the repository, logs, errors, database, or test fixtures. No memory content appears in logs, events, or interaction metadata.
10. The completion report accurately states what was verified and what was not.

---

## Appendix A — Proposed Repository Layout

```text
.
├── Cargo.toml
├── Cargo.lock
├── rust-toolchain.toml
├── deny.toml
├── README.md
├── CONTRIBUTING.md
├── docs/
│   ├── adr/0001..0006
│   ├── architecture.md
│   ├── cli-reference.md
│   ├── configuration.md
│   ├── data-retention.md
│   ├── dependencies.md
│   ├── planning.md
│   ├── security-review.md
│   ├── release-checklist.md
│   ├── evaluations/
│   └── validation/
├── migrations/
│   ├── 0001_init.sql
│   ├── 0002_labs_agents.sql
│   ├── 0003_projects_requests_stories.sql (includes memories)
│   ├── 0004_github_source.sql
│   └── 0005_interactions.sql
├── src/
│   ├── main.rs
│   ├── lib.rs
│   ├── cli/{mod,args,dispatch,exit,render,prompt,confirm}.rs
│   ├── app/
│   │   ├── mod.rs
│   │   ├── settings.rs
│   │   ├── settings_edit.rs
│   │   ├── context.rs
│   │   ├── labs.rs
│   │   ├── lab_validation.rs
│   │   ├── agents.rs
│   │   ├── agents/wizard.rs
│   │   ├── projects.rs
│   │   ├── memories.rs
│   │   ├── kanban.rs
│   │   ├── requests.rs
│   │   ├── planning/{mod,prompt,schema,rules}.rs
│   │   ├── provider_retry.rs
│   │   ├── invocation.rs
│   │   └── ask.rs
│   ├── domain/
│   │   ├── mod.rs
│   │   ├── ids.rs
│   │   ├── errors.rs
│   │   ├── settings.rs
│   │   ├── lab.rs
│   │   ├── agent.rs
│   │   ├── skills.rs
│   │   ├── project.rs
│   │   ├── memory.rs
│   │   ├── request.rs
│   │   ├── story.rs
│   │   ├── budget.rs
│   │   ├── clock.rs
│   │   └── provider.rs
│   ├── infra/
│   │   ├── mod.rs
│   │   ├── paths.rs
│   │   ├── config.rs
│   │   ├── db/{mod,migrate,app_state,labs,agents,projects,memories,planning,kanban,interactions}.rs
│   │   └── provider/{mod,deepseek,scripted}.rs
│   └── obs/{mod,events}.rs
└── tests/
    ├── support/mod.rs
    ├── cli_*.rs
    ├── db_*.rs
    ├── provider_*.rs
    ├── security_*.rs
    ├── e2e_*.rs
    └── live_smoke.rs
```

---

## Appendix B — Database Schema by Migration

**v1 (WP-2):** `app_state`, `events` (including typed `schema_from`/`schema_to`, ADR-0016), indexes on events.

**v2 (WP-3):** `labs`, `agents`, `lab_memberships`, indexes; role/name constraints; cascade memberships.

**v3 (WP-4):** `projects`, `requests`, `stories`, `memories`, status/priority/complexity/state checks, scope checks for memories, cascades from laboratory and project, indexes for board, assignment, and memory-scope queries.

**v4 (WP-5):** `projects.github_source` (nullable canonical GitHub URL; ADR-0025).

**v5 (WP-6):** `interactions` (agent, kind, provider, model, outcome, error_code, duration, token counts, created_at) with no content columns.

Timestamps are RFC 3339 UTC text; story due dates are `YYYY-MM-DD`. No credentials are stored. JSON columns hold validated, bounded arrays (skills, acceptance criteria, questions).

---

## Appendix C — Command Behavior Matrix

| Command | Requires active lab | Writes | Confirmation | Exit codes (typical) | Package |
|---|---|---|---|---|---|
| `--help`, `--version` | No | No | No | 0, 2 | WP-1 |
| `--show-settings` | No | No | No | 0, 2 | WP-2 (source-control/GitHub reporting in WP-5) |
| `--edit-settings` | No | Settings file | No | 0, 1, 2, 4, 5 | WP-2 |
| `--list-labs` | No | No | No | 0, 1 | WP-3 |
| `--new-lab` | No | Yes | No | 0, 1, 2 | WP-3 |
| `--open-lab` | No | Yes | No | 0, 1, 3, 4 | WP-3 |
| `--check-lab` | No | No | No | 0, 3, 4 | WP-3 |
| `--list-agents` | Yes | No | No | 0, 1, 3 | WP-3 |
| `--new-agent` | No (active pre-filled) | Yes | Yes (summary) | 0, 1, 2, 4, 5 | WP-3 |
| `--show-agent-detail` | No | No | No | 0, 1, 3 | WP-3 |
| `--list-projects` | Yes | No | No | 0, 1, 3 | WP-4 |
| `--new-project [--github-source]` | Yes | Yes | No | 0, 1, 2, 3, 4 | WP-4; WP-5 for `--github-source` |
| `--show-project-kanban` | Yes | No | No | 0, 1, 3 | WP-4 |
| `--delete-lab` | No | Yes | Yes | 0, 1, 3, 4, 5 | WP-4 |
| `--delete-project` | Yes | Yes | Yes | 0, 1, 3, 4, 5 | WP-4 |
| `--delete-agent` | No | Yes | Yes | 0, 1, 3, 4, 5 | WP-4 |
| `--list-memories` | For project scope | No | No | 0, 1, 2, 3 | WP-4 |
| `--new-memory` | For project scope | Yes | No | 0, 1, 2, 3, 4 | WP-4 |
| `--delete-memory` | No | Yes | Yes | 0, 1, 3, 4, 5 | WP-4 |
| `--new-request` | Yes | Yes | No | 0, 1, 2, 3, 4, 5, 130 | WP-5 |
| `--ask` | Yes | Metadata only | No | 0, 1, 2, 3, 4, 5, 130 | WP-6 |

---

## Appendix D — Execution Limits and Constants

| Constant | Default | Injected in tests |
|---|---|---|
| Logical planning invocations | 1 | Yes |
| Retries per invocation | 2 (0 allowed) | Yes |
| Per-attempt timeout | 60 s | Yes (milliseconds) |
| Connect timeout | 10 s | Yes |
| Total planning deadline | 240 s | Yes |
| Question deadline | 180 s | Yes |
| Request input cap | 64 KiB | Yes |
| Provider body cap | 256 KiB | Yes |
| Max stories per request | 20 | Yes |
| Max output tokens | 4096 | Yes |
| Max memory content | 2,000 characters | Yes |
| Max memories per scope | 20 | Yes |
| Memory context per invocation | 8 KiB | Yes |
| Backoff base / factor / cap | 500 ms / 2 / 8 s | Yes |
| Kanban wide threshold | 100 columns | Yes |
| Table truncation ellipsis | `…` | Fixed |

Limits are code constants combined into a typed `ExecutionLimits` structure. Values may be revised only with recorded evidence in `docs/validation/`.
