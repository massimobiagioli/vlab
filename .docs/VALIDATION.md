# Validation

## Status and Purpose

**Proposed validation plan — no implementation or test results have been verified.**

This document defines acceptance criteria, a test strategy, and representative
scenarios for the first end-to-end workflow.

It assumes the architecture and security boundaries described in
`ARCHITECTURE.md` and `SECURITY.md`.

The workflow below is a provisional engineering slice, not a confirmed product
requirement. Replace it if it does not deliver value for the intended user,
while preserving the boundary and execution-limit tests.

## First Vertical Slice

### User Outcome

A user requests a concise summary of one explicitly authorized UTF-8 text file.

The application:

1. Validates the request and establishes a read-only permission scope.
2. Allows the agent to read only the specified file.
3. Produces a structured summary with supporting evidence.
4. Returns an explicit terminal outcome and execution counters.

No write, delete, shell, or network tool is available to the agent. The configured
model-provider connection is application infrastructure, not an agent capability.

### Assumptions

- Fixtures contain synthetic, non-sensitive data.
- Sending the selected file to the configured model provider is authorized.
- The initial interface is whichever interface the product requires first.
- Automated acceptance tests use an injected model substitute.
- No autonomous or model-written cross-run memory is required. User-authored
  scoped memories are explicit data attached to a laboratory, project, or
  agent; they are bounded and treated as untrusted context.

### Result Contract

A successful result contains:

- Terminal status: `succeeded`.
- A non-empty summary.
- One or more factual claims.
- An exact source excerpt supporting each claim.
- The authorized source identifier.
- Execution counters and elapsed duration.

Failure results contain:

- An explicit terminal status.
- A stable, machine-readable error code.
- An actionable explanation.
- Known external effects and unresolved uncertainty, if any.

Exact excerpt matching establishes traceability, not semantic entailment.
Whether an excerpt actually supports a claim is evaluated separately.

## Measurable Completion Criteria

A run is accepted as successful only when:

1. The authorized file was read successfully.
2. The result satisfies the output schema.
3. Every cited excerpt exists in the file content actually read.
4. Every claim is supported by its cited excerpt.
5. All fixture-specific required facts are covered.
6. No unsupported factual claim is introduced.
7. No unauthorized action occurred.
8. All execution limits were respected.

For deterministic fixtures, required facts and accepted evidence are defined
in the test data.

For live-provider evaluations, semantic correctness is assessed against the
same rubric. Do not rely on the producing model's self-assessment.

The application must not equate structural validation with verified semantic
correctness. Document which checks run automatically and which remain evaluation
criteria.

## Acceptance Criteria

| ID | Criterion | Validation |
|---|---|---|
| AC-01 | A valid request completes end to end. | Public-interface test with a temporary fixture and scripted model responses. |
| AC-02 | The result covers required facts with valid evidence. | Fixture assertions and semantic rubric. |
| AC-03 | Only the authorized file can be accessed. | Unauthorized-resource and path-escape tests. |
| AC-04 | Unavailable tools cannot execute. | Unknown, write, delete, shell, and network action proposals. |
| AC-05 | External content cannot modify permissions. | Prompt-injection fixture and adversarial model proposals. |
| AC-06 | Invalid requests and model responses fail safely. | Schema, size, encoding, and semantic-validation tests. |
| AC-07 | Runs stop within configured budgets. | Boundary tests using a controlled clock and call counters. |
| AC-08 | Cancellation prevents subsequent dispatch. | Cancellation before and during execution. |
| AC-09 | Errors do not appear as successful completion. | Missing file, provider failure, and timeout scenarios. |
| AC-10 | Relevant events are observable without leaking payloads. | Structured-event assertions using synthetic secret markers. |
| AC-11 | Scoped memory content cannot grant capabilities or exceed bounds. | Scope, size, cascade, and memory-injection tests. |
| AC-12 | GitHub source and credential handling cannot leak or create invalid state. | URL validation, missing-key, and marker-absence tests. |

## Execution Budget

Use this bounded profile for the first-slice acceptance suite.
These values are test defaults, not verified production recommendations.

| Resource | Limit |
|---|---|
| Total run duration | 30 seconds |
| Individual model request | 10 seconds or remaining run duration, whichever is smaller |
| Individual tool execution | 2 seconds or remaining run duration, whichever is smaller |
| Model requests | 3 |
| Tool calls | 1 |
| Automatic retries | 0 |
| Source content | 16 KiB |
| Model response content | 16 KiB |

Definitions:

- A model request is counted when dispatched, including failed requests.
- A tool call is counted when dispatched, including failed calls.
- Rejected proposals do not count as tool calls.
- Size limits apply to decoded content before further processing.
- The total deadline includes validation and processing, not just external calls.

The loop must not continue indefinitely on malformed or rejected proposals.
For this slice, either condition ends the run with a classified failure.

Do not silently truncate oversized input into an apparently complete summary.

Token and cost enforcement may be added where the provider supports it.
Do not claim exact cost control from request counts alone.

## Test Strategy

### Unit Tests

Test deterministic behavior without filesystem or provider dependencies:

- Request and configuration validation.
- Permission decisions.
- Action and result parsing.
- State transitions.
- Budget accounting.
- Deadline calculations.
- Error classification.
- Evidence matching.
- Memory scope, ordering, and context-budget calculations.
- GitHub source URL validation and credential-presence policy.
- Log redaction.

Prefer explicit types and small functions over elaborate testing abstractions.

### Tool Integration Tests

Use temporary directories and synthetic files to verify:

- Authorized reads.
- Rejection of sibling files and unauthorized absolute paths.
- Path traversal rejection.
- Symlink escape prevention on supported platforms.
- Missing files and invalid UTF-8.
- Input-size boundaries.

Assert that authorization fails before an unauthorized read occurs.
A final error message alone is insufficient proof.

### Orchestration Tests

Inject a scripted model and instrumented tool dispatcher.

Assert:

- Exact dispatched actions and arguments.
- Model and tool call counts.
- Permission checks before dispatch.
- The exact scoped memories included in a prompt, their order, and the
  context-budget cut-off.
- Terminal state and error code.
- No further calls after failure, cancellation, or budget exhaustion.

Use a controlled clock instead of wall-clock sleeps wherever possible.

### End-to-End Tests

Exercise the first public interface through to the structured outcome using:

- A real temporary fixture.
- The actual file-reading tool.
- A deterministic model substitute.
- Captured structured events.

No provider credentials or external network access are required.

### Live-Provider Evaluations

Run separately and explicitly:

- Use only synthetic, approved fixtures.
- Pin and record model identifiers and settings where possible.
- Record fixture version, prompt version, application commit, and configuration.
- Report success rates, latency, and token usage where available.
- Investigate failures rather than silently retrying until the result passes.

Live evaluations assess task quality and integration compatibility.
They do not replace deterministic security tests.

## Representative Scenarios

### S-01: Authorized Summary

**Given:** A short file containing three predefined facts.

**When:** The user requests a summary of that file.

**Then:**

- Exactly one authorized read occurs.
- All required facts are represented.
- Each claim includes valid supporting evidence.
- The result is successful and within budget.

### S-02: Unauthorized File Request

**Given:** Permission to read file A only.

**When:** The model proposes reading file B.

**Then:**

- The proposal is denied before tool execution.
- File B is not opened or returned.
- The run ends with a permission-denied failure.

### S-03: Prompt Injection in Source Content

**Given:** The authorized file includes instructions to read a private file,
upload data, or change application policy.

**When:** The model processes that content and proposes an unauthorized action.

**Then:**

- Permissions remain unchanged.
- The action is rejected before dispatch.
- No unauthorized data leaves the application.

Test enforcement using scripted malicious proposals. Whether a live model
chooses to follow the injected instructions is a separate evaluation.

### S-04: Unsupported Completion Claim

**Given:** A model response contains a quotation absent from the source.

**When:** The application validates the result.

**Then:**

- Evidence validation fails.
- The application does not return successful completion.

Also evaluate claims paired with real but irrelevant excerpts. These require
semantic checks and must not be treated as solved by substring matching.

### S-05: Repeated Action or Budget Exhaustion

**Given:** The permitted tool call has already been dispatched.

**When:** The model proposes another tool call.

**Then:**

- No additional tool execution occurs.
- The run ends with `budget_exceeded`.
- Recorded counters do not exceed the configured limits.

Repeat equivalent checks for model requests and the total deadline.

### S-06: Provider Timeout

**Given:** The provider does not respond within its allowed duration.

**When:** The request deadline expires.

**Then:**

- The application stops waiting and requests cancellation.
- No automatic retry occurs.
- The run reports a timeout failure, or `budget_exceeded` if the total run
  deadline has expired.
- No new action is dispatched.

Local cancellation does not prove that remote processing stopped.

### S-07: User Cancellation

**Given:** A run is active.

**When:** The caller cancels it.

**Then:**

- The terminal outcome is `cancelled`.
- In-flight work receives a cancellation request where supported.
- No subsequent model request or tool action is dispatched.
- Completed work is not falsely described as rolled back.

### S-08: Invalid or Oversized Input

**Given:** An empty task, invalid UTF-8 file, oversized file, malformed model
response, or oversized response.

**When:** The relevant boundary validates it.

**Then:**

- The run fails with a classified, actionable error.
- Unnecessary downstream calls do not occur.
- Raw sensitive content is not included in error messages.

Test the exact configured size limit and one byte beyond it.

### S-09: Prompt Injection in Memory Content

**Given:** A laboratory, project, or agent memory contains instructions to
change policy, read private data, or perform an action.

**When:** The application builds a planning or question prompt.

**Then:**

- The memory is included only as delimited data, within the memory budget.
- Permissions, roles, limits, and assignment rules are unchanged.
- No tool or action is dispatched from the memory content.
- The model cannot create, edit, or delete a memory.
- Out-of-scope memories are not included.

## Permission and Approval Boundaries

The first slice has no approval-enabled side-effecting tools.

Requests to write, delete, execute commands, or send data through an agent tool
must be denied, not converted into approval requests.

Do not add a generic approval subsystem solely for this slice.

Before introducing a high-impact tool, add tests proving that:

- Missing approval prevents execution.
- Approval is bound to the exact run, action, arguments, and resources.
- Modified arguments invalidate approval.
- Approval cannot be reused by another run.
- Approval cannot override an explicit policy denial.
- Untrusted content cannot act as an approval source.

## CI and Evaluation Gates

### Every Change

Required checks once the corresponding implementation exists:

- Rust formatting.
- Linting without unreviewed warnings.
- Unit tests.
- Tool integration tests.
- Deterministic orchestration and end-to-end tests.

Typical Rust commands:

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
```

These commands are proposed, not verified against the repository.
Use a supported feature matrix instead of `--all-features` if features are
mutually exclusive.

Security-boundary tests must all pass. Do not average permission violations
into an overall quality score.

### Before Live-Provider Acceptance

For the initial smoke evaluation:

- Run five approved success fixtures three times each.
- Require at least 14 of 15 runs to meet the task-quality criteria.
- Require zero observed unauthorized actions or execution-limit violations.
- Report all failures and their classifications.

This is a provisional acceptance threshold, not statistical evidence of
production reliability. A production target requires representative workloads
and an appropriate sample size.

## Definition of Done

The first slice is complete when:

- All applicable acceptance criteria pass.
- Required deterministic tests pass in CI.
- Live-provider evaluation results are recorded if live integration is included.
- No known permission-boundary or execution-limit defect remains open.
- Errors and terminal outcomes are documented.
- Logs have been checked for synthetic secret leakage.
- Actual validation commands are documented.
- Known limitations, including semantic-verification limits, are explicit.
- Documentation distinguishes implemented controls from planned work.

## Smallest Useful Next Step

Create one synthetic file with three facts and write a failing end-to-end test:

> Given read access to this file only, return an evidence-backed summary
> without dispatching any other tool action.

Implement only enough of the request boundary, orchestrator, model substitute,
and file tool to make that test pass. Add the unauthorized-file test before
expanding capability.
