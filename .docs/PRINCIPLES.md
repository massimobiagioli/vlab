# Development Principles

## Purpose

This document defines the development principles adopted by the project:

- Lean
- Agile
- Extreme Programming (XP)
- Test-Driven Development (TDD)
- Behavior-Driven Development (BDD)
- Domain-Driven Design (DDD)
- Security-First
- The Twelve-Factor App methodology

These principles guide decisions, not ceremony. Apply them proportionally to the
problem, favor working software, and validate assumptions through short feedback
loops.

## 1. Lean: Maximize Value, Minimize Waste

Deliver the smallest useful improvement that addresses a demonstrated need.

- Prioritize observable user outcomes over output volume.
- Keep changes small, testable, and independently reviewable.
- Limit work in progress and finish work before starting more.
- Avoid speculative features, abstractions, dependencies, and infrastructure.
- Automate repetitive work when the benefit justifies the cost.
- Measure bottlenecks before optimizing.
- Treat defects, unnecessary handoffs, and slow feedback as waste.

**Decision rule:** If a change does not deliver current value or reduce a
demonstrated risk, defer it.

## 2. Agile: Learn Through Working Software

Plan incrementally and adapt based on evidence.

- Organize work into small vertical slices that deliver usable behavior.
- Define clear acceptance criteria before implementation.
- Seek early feedback from users and maintainers.
- Reassess priorities as requirements and constraints become clearer.
- Make assumptions, dependencies, and trade-offs explicit.
- Maintain a sustainable pace.
- Use retrospectives to select concrete, measurable process improvements.

Plans are hypotheses, not commitments to ignore new information.

## 3. Extreme Programming: Keep Change Safe

Use disciplined engineering practices to sustain frequent delivery.

- Integrate changes frequently and keep the main branch releasable.
- Require automated checks and appropriate review before merging.
- Prefer simple designs that satisfy current requirements.
- Refactor continuously while preserving observable behavior.
- Maintain shared coding conventions and collective responsibility for quality.
- Use pairing or focused collaborative review for complex or high-risk changes.
- Keep builds reproducible and feedback fast.
- Address failing tests promptly; do not normalize a broken build.

Avoid large, long-lived branches and unrelated changes in a single review.

## 4. TDD: Specify Behavior Before Implementation

Use the red–green–refactor cycle for new behavior and defect fixes.

1. **Red:** Write a focused test and confirm it fails for the expected reason.
2. **Green:** Implement the smallest change that makes it pass.
3. **Refactor:** Improve the design while keeping the tests green.

### Testing Guidelines

- Test observable behavior rather than incidental implementation details.
- Prefer fast, deterministic unit tests for local logic.
- Add integration tests for system boundaries and real component interactions.
- Use a small number of end-to-end tests for critical user journeys.
- Cover invalid input, failure paths, authorization, and recovery behavior.
- Add a regression test for each reproducible defect.
- Use property-based testing or fuzzing where input complexity warrants it.
- Treat coverage as diagnostic information, not proof of correctness.

When test-first development is impractical, document the reason and establish
appropriate automated coverage before considering the work complete.

## 5. BDD: Establish Shared Behavioral Expectations

Describe intended behavior using language understood by users and developers.

- Start with the user need and expected outcome.
- Use concrete examples to resolve ambiguous requirements.
- Include important error, misuse, and boundary scenarios.
- Automate examples when doing so provides durable regression protection.
- Keep acceptance criteria independent of internal implementation choices.

Example:

> **Given** a user without permission to modify a resource  
> **When** the user attempts to update it  
> **Then** the operation is rejected and the resource remains unchanged

BDD does not require a specific framework or Gherkin files. Use the lightest
format that creates shared understanding.

## 6. DDD: Let the Domain Shape the Design

Model meaningful business concepts explicitly without introducing unnecessary
architectural layers.

- Establish a consistent vocabulary in discussions, code, tests, and documentation.
- Identify domain invariants and enforce them at appropriate boundaries.
- Use types to represent meaningful distinctions and prevent invalid states.
- Keep core domain rules independent of transport, persistence, and AI providers.
- Define bounded contexts where terminology, rules, or ownership genuinely differ.
- Introduce entities, value objects, aggregates, and repositories only when useful.
- Translate external representations at system boundaries.

DDD does not imply microservices. Prefer a modular application until operational
or domain requirements justify distribution.

## 7. Security-First: Build Trust Deliberately

Security is part of design, implementation, testing, and operations.

### Secure Defaults

- Deny access by default and grant the minimum required permissions.
- Validate untrusted inputs at trust boundaries.
- Enforce authorization for every protected operation.
- Keep secrets out of source control, logs, test fixtures, and generated artifacts.
- Use maintained cryptographic libraries and established protocols.
- Protect sensitive data in transit and at rest according to its classification.
- Minimize data collection, retention, and exposure.

### Risk Management

- Identify assets, trust boundaries, abuse cases, and failure consequences.
- Review dependencies for necessity, maintenance, licensing, and security risk.
- Test relevant security properties, not only successful workflows.
- Produce actionable errors without exposing sensitive internals.
- Maintain a process for vulnerability reporting, triage, and remediation.
- Require explicit approval for destructive, irreversible, or high-impact actions.

### AI-Specific Controls

- Treat model output, retrieved documents, and tool responses as untrusted data.
- Do not allow external content to redefine permissions or trusted instructions.
- Validate generated commands and structured outputs before consequential use.
- Enforce permissions in application code and tool boundaries, not only prompts.
- Bound execution time, retries, tool calls, resource use, and cost.
- Preserve human oversight for sensitive actions.

## 8. Twelve-Factor: Keep Deployments Predictable

Apply the Twelve-Factor methodology to deployable services. For libraries, CLI
tools, and local agents, apply the relevant intent rather than forcing a service
architecture.

| Factor | Project practice |
| --- | --- |
| **I. Codebase** | Maintain one version-controlled codebase per independently deployable application, with multiple deployments. |
| **II. Dependencies** | Declare dependencies explicitly and isolate them. Commit lockfiles where appropriate. |
| **III. Config** | Keep deployment-specific configuration outside code, normally using environment variables. Inject secrets securely and never bake them into artifacts. |
| **IV. Backing services** | Treat databases, queues, and external APIs as attached resources with explicit configuration. |
| **V. Build, release, run** | Separate artifact creation, release configuration, and execution. Promote immutable artifacts. |
| **VI. Processes** | Keep service processes stateless and share-nothing; store durable state in appropriate backing services. |
| **VII. Port binding** | Make network services self-contained and expose them through explicit port binding. |
| **VIII. Concurrency** | Scale through process instances and appropriate worker types. Make concurrent state changes safe. |
| **IX. Disposability** | Support fast startup, graceful shutdown, and safe interruption or retry of work. |
| **X. Dev/prod parity** | Minimize meaningful differences between development, test, staging, and production. |
| **XI. Logs** | Emit logs as event streams and delegate collection and retention to the runtime platform. Redact sensitive data. |
| **XII. Admin processes** | Run migrations and maintenance as controlled one-off processes using the same release and configuration mechanisms. |

Configuration must be validated at startup. Invalid configuration should produce
clear errors without leaking secrets.

## 9. Implementation Standards

### Rust

- Prefer safe, idiomatic Rust with explicit ownership and clear interfaces.
- Represent expected failures with deliberate error types and propagation.
- Avoid panics in normal request handling and recoverable failure paths.
- Justify, isolate, document, and test any necessary `unsafe` code.
- Keep dependencies intentional and public APIs minimal.
- Run formatting, linting, and relevant tests in CI.

### Shell

- State the required shell and portability assumptions.
- Quote expansions and handle filenames and untrusted input safely.
- Check failures explicitly; do not rely solely on shell error modes.
- Avoid `eval` and dynamically constructed shell code.
- Guard destructive operations and clean up temporary resources reliably.
- Never print secrets or disable security controls for convenience.

### AI Agents

- Prefer one agent or a deterministic workflow unless multiple agents show value.
- Define a clear purpose, bounded responsibilities, and explicit success criteria.
- Specify contracts for tools, inputs, outputs, state, and handoffs.
- Provide only the context and permissions required for the task.
- Build repeatable evaluations using representative success and failure scenarios.
- Make execution observable, cancellable, and recoverable.
- Optimize for reliable completion, not apparent autonomy.

## 10. Definition of Done

A change is complete when:

- [ ] Its intended behavior and acceptance criteria are clear.
- [ ] The implementation is the simplest adequate solution.
- [ ] Relevant automated tests pass.
- [ ] Security and privacy implications have been assessed.
- [ ] Error handling and important failure paths are covered.
- [ ] Formatting, linting, and applicable CI checks pass.
- [ ] Appropriate review is complete.
- [ ] Documentation and configuration guidance are updated where needed.
- [ ] Operational changes include proportionate observability and recovery plans.
- [ ] Any remaining limitations or unverified assumptions are explicit.

Never claim that commands were executed, files inspected, or behavior validated
without evidence.

## 11. Exceptions and Continuous Improvement

When principles conflict:

1. Protect correctness, security, and user data.
2. Choose the simplest solution that satisfies the current requirement.
3. Explain the trade-off and how it will be validated.
4. Prefer reversible decisions and small experiments.
5. Revisit the decision when evidence changes.

Document material exceptions with their rationale, risks, mitigation, and review
trigger. Use a short architecture decision record when long-term traceability is
valuable.

These principles should evolve through demonstrated project needs, not through
the accumulation of process.
