---
name: vlab-security-review
description: "Review a VLab change against the project security baseline before merge. Use for changes touching authorization, approvals, untrusted input, model output, persistence, secrets, filesystem paths, network calls, logging, or destructive operations."
---

# VLab security review

Security is a design constraint, not a final checklist. Use this skill for any
change that touches a trust boundary or a high-impact action.

## Read first

- `AGENTS.md`
- `.docs/SECURITY.md`
- `.docs/CONSTITUTION.md` (secure-by-design rules)
- `.docs/adr/registry.md`

## Review procedure

1. **Name the assets.** User data, request text, profiles, credentials, local
   files, execution metadata, logs, and availability.
2. **Map the trust boundaries.** Caller to CLI; application to provider; model
   to orchestrator; tool output to orchestrator; orchestrator to storage or
   filesystem.
3. **Check authorization and approval.**
   - Deny by default; grant the minimum capability.
   - Enforce authorization in application code, not in prompts.
   - Bind confirmation to the exact entity, action, arguments, and scope.
   - Invalidate approval if the proposal changes.
   - Fail closed when authorization cannot be established.
4. **Check untrusted input handling.**
   - Validate identifiers, sizes, encodings, enums, and dates locally.
   - Reject unknown tools, fields, and identifiers before use.
   - Keep external content clearly separate from instructions.
   - Never execute model output or construct shell commands from it.
5. **Check secrets and data flow.**
   - No credential in source, args, prompts, logs, errors, database, or tests.
   - Minimize outbound data; confirm retention implications.
   - Redact diagnostics; never log payloads by default.
6. **Check resource bounds.** Timeouts, deadlines, retry limits, input/output
   caps, story limits, and single-connection database behavior.
7. **Check failure behavior.** Denied, failed, cancelled, and unknown outcomes
   are distinct. No retry after cancellation. No unsafe retry of side effects.
   Partial work is reported honestly.
8. **Check filesystem and network.** Private permissions, traversal and symlink
   handling where applicable, TLS validation, redirect policy, and destination
   restriction.
9. **Verify with tests, not claims.** Security tests must run without a live
   model and must assert the boundary, not only the final message.

## Required evidence

- The tests that prove each boundary, named explicitly.
- The commands executed and their results.
- Remaining risks, assumptions, and unverified areas.
- An ADR for any new security control, exception, or accepted risk.

## Merge rule

Do not merge while a permission-boundary, execution-limit, secret-leak, or
untrusted-execution defect is known and unresolved. Do not average security
failures into a quality score. Do not broaden permissions as error recovery.
