# Feature 00024 — Autonomous clarification resolution

| Field | Value |
|---|---|
| Identifier | `00024` |
| Branch | `feat/autonomous-clarification` |
| Status | `In progress` |
| Date | 2026-11-17 |
| Related ADRs | ADR-0066 (autonomous clarification), ADR-0026 (planning), ADR-0003 (bounded provider) |
| Scope | Resolve Tech Lead questions without a human and keep the board moving |

## Goal

A request must never park in `NEEDS_CLARIFICATION` waiting for an operator. The
Tech Lead and its bounded follow-up calls resolve open questions themselves,
then create cards and start the existing workflow.

## Acceptance criteria

1. The planning prompt states that no human is available and that the Tech Lead
   must choose reasonable assumptions.
2. A model response with questions triggers up to two bounded clarification
   calls; each call is one provider attempt, capped at 90 seconds, cancellable,
   and uses the existing model/output limits.
3. A clarification response with no questions continues through the normal
   story validation, persistence, and workflow.
4. Questions that remain after two calls produce `FAILED` with
   `clarification_unresolved`; no human prompt and no partial stories.
5. New planning never writes `NEEDS_CLARIFICATION`; legacy rows remain readable.
6. The clarification prompt reuses the bounded request, roster, and memory
   context; questions and responses remain untrusted, validated data.
7. Tests cover autonomous success, unresolved failure, cancellation, prompt
   content, and the updated planning docs; `make check` passes and hosted CI is
   green.

## Non-goals

- Asking the operator for answers.
- Unbounded model repair or agent conversation loops.
- Creating a story for each open question.
- Changing the schema or removing clarification fields.

## Risks

- Extra provider calls increase latency and cost; the per-call and per-request
  bounds keep this finite.
- A model may keep asking questions; the stable failed outcome makes that
  visible without blocking the queue.
- The clarification prompt contains project context; it follows the same
  scoped-memory and secret rules as planning.

## Validation

- Local gate: `make check` passed (fmt-check, clippy, tests, audit, deny).
- `tests/planning_flow.rs` covers resolved and unresolved clarification.
- `tests/security_planning.rs` covers prompt injection and secret exclusion on
  the clarification path.
- Hosted CI has not run; the plan stays `In progress` until it is green.

## Task checklist

- [x] **F00-01** Write ADR-0066 and update the registry/AGENTS links.
- [x] **F01-01** Add the autonomous clarification prompt and up to two bounded
  follow-up calls.
- [x] **F02-01** Stop writing `NEEDS_CLARIFICATION` for new requests and fail
  unresolved questions with `clarification_unresolved`.
- [x] **F03-01** Update tests and documentation.
- [x] **F04-01** Run `make check`.
- [ ] **F04-02** Push, confirm hosted CI is green, and set the status to `Done`.
