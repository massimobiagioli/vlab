# Planning

This document describes request intake (dashboard prompt and `--new-request`): input rules, the planning workflow,
terminal outcomes, provider bounds, assignment and due-date rules, scoped memory
context, and the security model. The conceptual entities are in
[`DATA-MODEL.md`](DATA-MODEL.md); the provider port is recorded in
[ADR-0026](adr/adr-0026.md).

## Input

From the dashboards (ADR-0044): press `r` on a project in **Projects**, or `n`
on its Kanban board. A prompt dialog collects the text (`Alt+Enter` line
break, bracketed paste keeps line breaks and drops other control characters,
`Enter` submits, `Esc` cancels; empty text is not submitted). The project
becomes current, a `Planning…` dialog is shown while the single provider
invocation runs on the dashboard's database connection, and `Esc` or `Ctrl-C`
cancels with the same rules as SIGINT below. An outcome dialog reports the
result; the exit-code column below applies only to the flag path.

The hidden legacy flag remains for scripts:

```bash
vlab --open-project <project-id>
vlab --new-request "Add password reset"
cat request.md | vlab --new-request
```

- Literal positional text is **always text**, never a file path.
- Piped/redirected stdin is accepted only when no positional text is present.
- Providing both sources exits 2 with usage guidance.
- Empty or whitespace-only input, invalid UTF-8, and input over 64 KiB are
  rejected before any database or provider work.
- When no positional text is given and stdin is a terminal, VLab exits 2
  instead of waiting indefinitely.

## Workflow

1. Validate the active laboratory and project ownership.
2. Run structural laboratory validation. A failed laboratory exits 4 before any
   request or provider call.
3. Persist the request as `TODO`, then mark it `PLANNING`.
4. Build a bounded prompt with the roster, request text, request date, and the
   selected scoped memories (laboratory, project, Tech Lead agent).
5. One logical provider invocation through the bounded retry policy.
6. Parse and validate JSON schema version 1 locally.
7. Apply project-roster and due-date rules locally.
8. In one transaction, insert every story as `IN PROGRESS` when it has an
   assignee and as `TODO` otherwise, then set the terminal status (ADR-0047).
9. Record `planning_started`, `provider_attempt`, and `planning_completed` or
   `planning_failed` events with no content.

No partial story set is committed. The request row is written before the
provider call and is preserved on clarification, validation failure, provider
failure, timeout, budget exhaustion, and cancellation.

## Terminal outcomes

| Outcome | Request status | Exit code |
|---|---|---|
| Stories created | `PLANNED` | 0 |
| Questions returned | `NEEDS_CLARIFICATION` | 0 |
| Local validation/assignment/date failure | `FAILED` (`validation`) | 4 |
| Provider authentication/rate/transport/server/timeout | `FAILED` (provider code) | 1 |
| Budget exhausted | `FAILED` (`budget_exceeded`) | 1 |
| Cancelled (SIGINT or token) | `FAILED` (`cancelled`) | 130 |
| Storage/configuration error | unchanged or preserved request | 1 or 2 |

A stale `PLANNING` request older than the 240-second lease is recovered as
`FAILED` with `stale_planning` by the next relevant command; it is never
retried automatically. There is no dedicated retry command in this release;
submit a revised request and keep the previous record for traceability.

## Provider contract

- DeepSeek chat-completions endpoint:
  `https://api.deepseek.com/chat/completions`.
- Model identifier comes from `settings.yaml`; no default model is hardcoded.
- Credential: `credentials.deepseek_api_key` from `settings.yaml`, sent only in
  the `Authorization` header.
- Attempt timeout 60 s, connect timeout 10 s, total planning deadline 240 s,
  response body cap 256 KiB, max output tokens 4096.
- At most one logical invocation and two retries. Only rate-limit, timeout,
  transport, and 5xx failures are retried; authentication, malformed output,
  oversized output, validation failures, and cancellation are not.
- Backoff is 500 ms doubling to 8 s; `Retry-After` takes precedence.
- Retries may incur additional provider charges. Exactly-once remote execution
  is not promised.

For deterministic tests, `VLAB_DEEPSEEK_ENDPOINT` can point the CLI at a
loopback mock server (`http://127.0.0.1:...` or `http://localhost:...`). It is
a test-only override; any other value is ignored and the production endpoint is
used, so the credential cannot be redirected to an arbitrary host.

## Structured output

Schema version 1 contains `schema_version`, `clarification_questions`, and
`stories`. Exactly one of the arrays is non-empty. Each story has title,
description, acceptance criteria, priority (`low|medium|high`), complexity
(`small|medium|large`), required skills, an optional assigned agent ID, and an
assignment rationale. Unknown fields (including model-supplied due dates),
wrong enums, empty arrays, more than 20 stories, and malformed IDs are rejected.
Validation errors name a field and rule but never echo the model response.

Assignments must reference active agents in the project roster. If the project
has no assignments, the active laboratory roster is used. Unassigned stories are
allowed and retain an explanation. Due dates are never taken from the model:
`small=2`, `medium=5`, `large=10` calendar days from the request creation date
in UTC, formatted `YYYY-MM-DD`.

## Memory context

The prompt includes laboratory memories of the project's laboratory, project
memories, and the Tech Lead's agent memories, in that order, newest first within
each scope. At most 8 KiB of memory text is included; memories that do not fit
are omitted whole. Every block is delimited with scope and memory ID and marked
as data. Archived-agent memories are excluded. A memory or request containing
the configured `credentials.deepseek_api_key` value is rejected before the
provider call.

After a successful plan, VLab makes one additional bounded provider call for a
short project-memory summary. Its failure or cancellation does not change the
`PLANNED` outcome. Memories are generated automatically (ADR-0036). Laboratory
and project memories can also be viewed, created, edited, and deleted from the
Lab floor (ADR-0046); agent memories remain automatic. Manual content uses the
same validation and is untrusted prompt context.

## Security

- Model output is untrusted data and is never executed.
- There are no tools, filesystem, shell, URL, or network capabilities in the
  model path; the provider connection is application infrastructure.
- Request text, memory content, CVs, unrelated projects, and credentials are
  never logged, stored in events, or echoed in errors.
- Validation, membership checks, due dates, persistence, and permissions are
  application code.
- The response body is bounded and errors carry status/request identifiers only.

## Limits (production defaults)

| Limit | Value |
|---|---|
| Logical invocation per request | 1 |
| HTTP attempts | 1 + up to 2 retries |
| Attempt timeout | 60 s |
| Connect timeout | 10 s |
| Total planning deadline | 240 s |
| Request input | 64 KiB UTF-8 |
| Provider response body | 256 KiB |
| Stories per request | 20 |
| Output tokens | 4096 |
| Memory content / scope / context | 2,000 chars / 20 / 8 KiB |
