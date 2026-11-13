# Security

## Status and Scope

**Proposed security baseline — controls described here must not be assumed
implemented or verified.**

This document assumes a Rust application that uses one AI agent to complete
bounded tasks through explicitly authorized tools.

The product domain, deployment environment, and data classification remain
undefined. Revisit this policy before handling sensitive data, enabling
side-effecting tools, or exposing the application to untrusted users.

## Reporting a Vulnerability

Do not disclose credentials, personal data, exploit details, or other sensitive
information in public issues.

Use the repository's private vulnerability-reporting feature if enabled.
Otherwise, request a private reporting channel from the maintainers without
publishing vulnerability details.

A monitored private reporting channel must be established before public release.

A useful report includes:

- Affected version or commit.
- Preconditions and potential impact.
- Minimal reproduction steps using synthetic data.
- Relevant logs with sensitive information removed.
- Suggested mitigation, if known.

Do not test against systems or data without authorization.

## Supported Versions

No supported release range has been established in this proposed baseline.

Before publishing a release, maintainers must document supported versions and
the process for delivering security fixes. Do not assume that older releases
receive security updates.

## Security Principles

- Deny access unless explicitly permitted.
- Grant each run and tool only the capabilities it requires.
- Keep authorization outside the model.
- Treat model responses and external content as untrusted input.
- Require approval for destructive, irreversible, or high-impact actions.
- Bound execution time, resource usage, and output size.
- Fail closed when authorization cannot be established.
- Report uncertainty rather than claiming unverified success.

Prompts are behavioral guidance, not security boundaries.

## Assets to Protect

- User data and task inputs, including laboratory, project, and agent memory content.
- Credentials and provider API keys stored in the private `settings.yaml`.
- Local files and repository contents.
- External systems accessible through tools.
- Approval records and policy configuration.
- Logs and retained execution state.
- Compute resources, model budgets, and service availability.

## Trust Boundaries

| Boundary | Required protection |
|---|---|
| Caller → application | Validate inputs and establish caller identity and scope where applicable. |
| Application → model provider | Minimize transmitted data and apply provider-specific privacy requirements. |
| Application → GitHub | Validate the project source; clone only the validated repository; pass the global credential through a short-lived askpass helper and child environment; bound time; discard output; no token in arguments or logs. |
| Model → orchestrator | Validate responses; never interpret proposals as authorization. |
| Workflow output → story state | Accept only strict work/review JSON; the application owns `IN_REVIEW`/`DONE` and outcome persistence; the model has no tools and cannot choose a transition. |
| Tool output → orchestrator | Treat content as data, not instructions or policy. |
| Memory content → orchestrator | Validate scope, encoding, and size; include only within the memory budget; delimit as data; never let it change policy, permissions, or assignment rules. |
| Orchestrator → tool | Enforce permissions, approval, and execution limits. |
| Tool → filesystem or network | Restrict resources and credentials independently of model behavior. |
| Application → logs or storage | Minimize, redact, and restrict access to retained information. |

For a local CLI, the operating-system user may define the caller boundary.
A network service requires explicit authentication and per-caller authorization
before exposure.

## Primary Threats

| Threat | Required mitigation |
|---|---|
| Prompt injection | External content, including memory content, cannot modify policy, permissions, or approval requirements. |
| Memory poisoning | Memory text cannot grant capabilities, alter rules, or be treated as an instruction; it is bounded and delimited. |
| Memory data leakage | Only the scoped memories selected by the specification are sent to the provider; content is never logged or rendered unsanitized. |
| Simulated outcome mistaken for real execution | `DONE` is documented and displayed as a no-tool simulation; no shell, filesystem, network, or repository capability exists in the workflow. |
| Unauthorized tool use | Use an explicit tool registry and validate every invocation. |
| Data exfiltration | Restrict outbound destinations, transmitted data, and credential access. |
| Filesystem escape | Restrict operations to approved resources and address traversal and symlink attacks. |
| Command injection | Avoid shell interpretation; use fixed executables and validated arguments if needed. |
| Approval substitution | Bind approval to the exact run, action, arguments, and affected resources. |
| Resource exhaustion | Enforce finite budgets, timeouts, and input/output limits. |
| Duplicate side effects | Retry only when idempotency is established or the previous outcome is reconciled. |
| Supply-chain compromise | Review dependencies and protect build and release credentials. |

## Required Controls

### Model and Agent Execution

- Validate response structure and semantic constraints before use.
- Reject unknown tools and invalid arguments.
- Keep tool capabilities and policy immutable from model output.
- Limit model calls, tool calls, total duration, and context growth, including a fixed memory-context budget.
- Enforce token or cost limits where supported; otherwise enforce conservative
  request and size limits without claiming exact cost control.
- Stop safely when required validation or authorization fails.
- Bound the simulated workflow to two provider calls per story, one attempt per
  call, a 60-second call timeout, 512 output tokens, and a 240-second total
  deadline. Validate both outputs before persisting the outcome and `DONE`.
  Treat the outcome text as untrusted project data and never execute it.

Retrieved documents, repository files, tool outputs, and memory content may
contain malicious instructions. Their apparent source or formatting does not
grant authority. The application decides which automatic memories to store;
the model cannot choose, edit, or delete them, and memory content is never
executed.

### Authorization and Approval

Permissions must be scoped to the current caller and run.

Approval must:

- Explain the concrete action and foreseeable effects.
- Identify the exact arguments and affected resources.
- Be recorded through a trusted application path.
- Apply only to the approved action.
- Become invalid if the proposal changes.

Approval does not override an explicit policy denial. Untrusted content cannot
approve actions or request broader privileges on the caller's behalf.

Recheck authorization at dispatch time. Where resource identity may change
between validation and use, use resource handles or equivalent platform controls
rather than relying solely on path strings.

### Tool Isolation

Expose only tools required by the first supported workflow.

Each tool must define:

- Input schema and validation rules.
- Accessible resources and required credentials.
- Side effects and approval requirements.
- Timeout and output limits.
- Error semantics and retry safety.

Do not expose arbitrary shell execution by default.

If executing untrusted code becomes a requirement, use an appropriately reviewed
isolation mechanism with restricted filesystem, network, process, and resource
access. Application checks and ordinary containers alone must not be assumed
sufficient for hostile workloads.

### Filesystem Access

- Restrict reads and writes to explicitly approved resources.
- Reject path traversal and unauthorized absolute paths.
- Address symlink and time-of-check/time-of-use risks.
- Avoid following untrusted links during sensitive operations.
- Use restrictive permissions for sensitive files.
- Do not overwrite or delete existing data without the required authorization.

Path normalization alone is not a complete filesystem security boundary.

### Network Access

- Permit only required destinations and protocols.
- Validate TLS certificates.
- Apply connection, request, and response-size limits.
- Revalidate redirected destinations.
- Block unintended access to loopback, private networks, link-local addresses,
  and cloud metadata endpoints.
- Account for DNS resolution changes when enforcing destination restrictions.

Private destinations may be enabled only by explicit policy for a defined use
case. Model-generated URLs are never implicitly trusted.

### Secrets and Data Privacy

- Never commit secrets to source control.
- Store credentials only in the private `settings.yaml` (`0600`, outside version control); never commit it.
- Prefer short-lived, narrowly scoped credentials.
- Keep credentials out of prompts, tool results, errors, logs, database rows, and every file other than `settings.yaml`. The GitHub key comes only from `credentials.github_api_key`.
- Validate the project GitHub source as a canonical HTTPS GitHub URL; it is metadata, not a credential.
- For the clone, disable credential helpers, set `GIT_TERMINAL_PROMPT=0`, use a private short-lived `GIT_ASKPASS`, and never put the token in a URL or process argument.
- Do not assume that the model can reliably redact secrets.
- Send only necessary data to model providers.
- Confirm provider retention and processing requirements before transmitting
  sensitive information.
- Treat scoped memories as data sent to the model provider for planning and
  questions; automatic and manual laboratory/project content is checked against
  the configured credential. Manual edits are local and never call the model.
- Do not log memory content; audit only identifiers, scope, and outcomes.
- Define retention and deletion rules before adding durable task storage;
  memories follow their owning laboratory or project and are retained for
  archived agents as documented.

Treat outbound model requests as data leaving the application's trust boundary.

### Failure Handling

- Distinguish denied, failed, cancelled, and unknown outcomes.
- Do not report model claims as verified external effects.
- Do not automatically retry side-effecting operations unless safe.
- Stop issuing new actions after cancellation or budget exhaustion.
- Report that remote or completed actions may not be reversible.
- Never broaden permissions as an error-recovery mechanism.

## Secure Rust and Dependency Practices

- Prefer safe Rust.
- Justify, isolate, and review any necessary unsafe code.
- Use validated types at trust boundaries.
- Handle errors explicitly without exposing sensitive internals.
- Bound parsing, buffering, concurrency, and allocation driven by untrusted input.
- Keep dependencies minimal and review their maintenance and security posture.
- Commit `Cargo.lock` for application builds.
- Review dependency and lockfile changes.
- Include vulnerability and license-policy checks in CI.

Safe Rust reduces memory-safety risks but does not prevent authorization errors,
injection, data leakage, or denial of service.

CI jobs must use minimum required permissions. Untrusted contributions must not
receive release credentials or other privileged secrets.

## Logging and Audit

Record structured security-relevant events:

- Authorization decisions.
- Approval requests and outcomes.
- Tool dispatch and completion.
- Validation failures.
- Budget exhaustion and cancellation.
- Administrative policy changes, if supported.

Use run and action identifiers for correlation. Do not log full prompts,
credentials, tool payloads, or memory content by default. Memory audit events
carry identifiers, scope, and outcome only.

Treat log fields as untrusted data and render them safely. Restrict log access
and define retention before production use.

Record observable events and concise summaries, not private model reasoning.

## Security Validation

Use deterministic model and tool substitutes for repeatable tests.

Required scenarios include:

- Prompt injection in tool output or memory content cannot grant permissions.
- A missing or invalid GitHub credential blocks GitHub-backed project creation without creating a project row or checkout; the key never appears in arguments, output, events, or the database.
- An invalid or non-GitHub source URL is rejected before persistence.
- Unknown tools and malformed arguments are rejected before execution.
- Denied actions never reach the tool implementation.
- Approval cannot be reused for a modified action or another run.
- Filesystem traversal and symlink escape attempts are blocked.
- Network restrictions remain effective across redirects.
- Secrets are excluded from logs and outbound model requests.
- Memory content is scoped, bounded, and delimited; an oversized or out-of-scope
  memory is rejected before dispatch.
- Timeouts and exhausted budgets prevent subsequent actions.
- Cancellation does not trigger unsafe retries.
- Uncertain external effects remain explicitly marked as unknown.

Test security boundaries independently of live model behavior. Passing a small
set of prompt-injection examples is not proof of resistance.

## Incident Response

If compromise is suspected:

1. Stop affected runs and disable exposed capabilities.
2. Revoke or rotate affected credentials.
3. Preserve necessary evidence with restricted access.
4. Determine affected versions, resources, users, and external effects.
5. Contain and fix the issue.
6. Add regression tests and verify the remediation.
7. Notify affected parties as required.
8. Document lessons and update controls.

Do not delete evidence or automatically reverse external actions without
assessing the consequences and obtaining appropriate authorization.

## Release Security Gate

Before the first public or production release:

- Establish a monitored private vulnerability-reporting channel.
- Document supported versions.
- Confirm deployment boundaries and data classification.
- Implement and test authorization and approval controls.
- Verify finite execution limits and safe failure behavior.
- Review dependencies, secret handling, and outbound data flows.
- Assign responsibility for incident response and security fixes.
- Document known limitations and explicitly accept remaining risks.
