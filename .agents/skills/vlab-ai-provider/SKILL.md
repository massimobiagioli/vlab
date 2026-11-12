---
name: vlab-ai-provider
description: "Work on the VLab model provider, prompts, planning output, or direct-question flow. Use when changing the DeepSeek adapter, execution budgets, retries, prompt context, structured output validation, or any model-backed workflow."
---

# VLab AI provider workflow

Model output is untrusted data. The application, not the model, owns
validation, permissions, time limits, and persistence.

## Read first

- `AGENTS.md`
- `.docs/ARCHITECTURE.md`
- `.docs/SECURITY.md`
- `.docs/STACK.md`
- `.docs/adr/adr-0001.md` and `adr-0003.md`

## Invariants

1. **No tools.** The model has no filesystem, shell, network, or code-execution
   capability. Never add tool calling without a new accepted ADR and a security
   review.
2. **The model proposes; the application decides.** Never treat model output as
   authorization, proof of correctness, or evidence that an external effect
   happened.
3. **Secrets stay out.** The API key comes only from `DEEPSEEK_API_KEY` and must
   not appear in prompts, logs, errors, database rows, tests, or command output.
4. **Minimal context.** Send only what the task needs. Do not send CVs,
   unrelated projects, transcripts, or internal identifiers that are not
   required.
5. **Bounded execution.** Enforce per-attempt timeouts, a total deadline,
   request and response size caps, story/token caps, and a retry bound. Stop
   dispatching after cancellation or budget exhaustion.
6. **Strict output.** Parse planning output with a versioned schema, reject
   unknown fields and identifiers, validate every enum and length, and compute
   all dates locally. Do not add an automatic repair loop.
7. **Classified failures.** Distinguish authentication, rate limit, timeout,
   transport, server, invalid response, oversized response, cancellation, and
   validation failures. Report the class without echoing payloads.
8. **No unsafe retries.** Retry only transient, pre-dispatch-safe failures.
   Never retry after cancellation or when the remote outcome is unknown in a
   way that could duplicate effects.

## Testing

- Use the scripted provider for deterministic orchestration tests.
- Use `wiremock` for HTTP contract tests: success, 401, 429, 5xx, malformed
  JSON, truncated body, oversized body, and timeout.
- Test that the outbound request contains no API key, no CV, and no unrelated
  data.
- Test prompt-injection fixtures: instructions in source text or model output
  cannot grant capabilities, alter policy, or cause execution.
- Test cancellation before and during dispatch; assert no retry.
- Use an injected clock and no wall-clock sleeps.
- Live-provider tests are opt-in, environment-gated, budget-limited, and never
  required in CI.

## Completion

- Provider contract and limits are documented.
- Errors and logs contain no prompt, response body, or credential.
- Counters never exceed the configured limits.
- Any change to provider contract, budget policy, or prompt trust model has an
  ADR.
