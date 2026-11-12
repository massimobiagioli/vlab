---
name: vlab-rust-tdd
description: "Implement or fix VLab behavior in Rust using test-driven development and the repository quality gates. Use for any Rust source change, new command behavior, domain rule, validation, persistence call, or bug fix."
---

# VLab Rust TDD workflow

Follow this skill for every behavior change. It enforces the project's
red-green-refactor loop, layer boundaries, and merge gates.

## Read first

- `AGENTS.md`
- `.docs/CONSTITUTION.md` and `.docs/PRINCIPLES.md`
- `.docs/STACK.md`
- The relevant ADRs in `.docs/adr/registry.md`

## Procedure

1. **State the problem.** Write the observable behavior, the acceptance
   criteria, and the failure paths. Identify assumptions that affect
   correctness or security.
2. **Check the architecture.** Place the change in the correct layer:
   - pure rules and validation in `domain`;
   - use cases, authorization, budgets, and transactions in `app`;
   - SQLite, filesystem, and provider details in `infra`;
   - parsing, rendering, and exit-code mapping in `cli`.
3. **Write a failing test first.** Use the smallest test that proves the
   behavior: unit test for rules, file-backed SQLite test for persistence,
   `assert_cmd` for CLI behavior, or a mock provider for model calls. Confirm it
   fails for the intended reason.
4. **Implement the smallest change that passes.** Do not add speculative
   abstraction, configuration, or dependencies.
5. **Cover failure paths.** Add tests for invalid input, missing context,
   authorization denial, provider failure, timeout, cancellation, and partial
   persistence where applicable.
6. **Refactor with tests green.** Remove duplication, tighten types, and keep
   public interfaces narrow.
7. **Run all gates** before declaring completion:

   ```sh
   cargo fmt --all -- --check
   cargo clippy --locked --all-targets -- -D warnings
   cargo test --locked
   cargo audit
   cargo deny check
   ```

8. **Record technical decisions.** If the change selects a dependency, schema,
   boundary, protocol, or policy, use the `vlab-adr` skill before finishing.
9. **Report evidence.** State exactly which commands ran, what passed, and what
   remains unverified.

## Rust rules

- Prefer safe Rust; do not add `unsafe` without an approved ADR.
- Use typed errors (`thiserror` in domain and modules, `anyhow` at the
  application boundary). Do not use `unwrap` or `expect` for expected failures.
- Model invalid states with enums and newtypes rather than unchecked strings.
- Keep domain code free of `sqlx`, `reqwest`, `clap`, filesystem, and network
  access.
- Use parameterized queries and short transactions.
- Never log request content, provider bodies, CVs, or credentials.
- Do not silence warnings or suppress security findings to pass a gate.

## Definition of done

- Acceptance criteria are met and covered by automated tests.
- Failure and security boundaries are tested, not only the happy path.
- Formatting, linting, tests, dependency audit, and license policy pass.
- Documentation and ADR registry are updated where required.
- The completion report is accurate about what was and was not verified.
