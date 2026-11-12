# AGENTS.md

This is the entry point for every human or AI contributor working on this
repository. It defines which documents are authoritative, how technical
decisions are recorded, which skills are available, and which checks must pass
before work is considered complete.

All tracked documentation, decisions, skills, and instructions are written in
English.

---

## 1. Project

VLab is a local, text-only Rust command-line application. It uses SQLite for
local persistence and a single outbound AI provider integration. It does not
run an HTTP server, expose a network API, or execute model-generated code.

The tracked documentation defines engineering rules, architecture, security,
technology constraints, and validation expectations. Product behavior must be
specified in tracked documentation before it is implemented; untracked notes are
never a source of truth.

---

## 2. Authoritative documents

Read the relevant documents before changing code. When two documents conflict,
apply the precedence below; `CONSTITUTION.md` and `SECURITY.md` take priority
over convenience or delivery speed.

| Document | Purpose |
|---|---|
| [`.docs/CONSTITUTION.md`](.docs/CONSTITUTION.md) | Engineering principles, security rules, working agreement, definition of done. |
| [`.docs/PRINCIPLES.md`](.docs/PRINCIPLES.md) | Lean, Agile, XP, TDD, BDD, DDD, security-first, and Twelve-Factor practices. |
| [`.docs/ARCHITECTURE.md`](.docs/ARCHITECTURE.md) | Runtime shape, component boundaries, non-goals, and control-flow ownership. |
| [`.docs/VALIDATION.md`](.docs/VALIDATION.md) | Test strategy, acceptance criteria, execution budgets, and validation scenarios. |
| [`.docs/SECURITY.md`](.docs/SECURITY.md) | Trust boundaries, threats, required controls, logging, incident response, release gate. |
| [`.docs/STACK.md`](.docs/STACK.md) | Mandatory technology choices, persistence rules, provider constraints, tooling, and CI checks. |
| [`.docs/TECHNICAL-SPECIFICATIONS.md`](.docs/TECHNICAL-SPECIFICATIONS.md) | Product requirements: domain rules, commands, workflows, and acceptance criteria. |
| [`.docs/ELEVATOR-PITCH.md`](.docs/ELEVATOR-PITCH.md) | Product intent; binding only where it does not contradict the specification. |
| [`.docs/DEPENDENCIES.md`](.docs/DEPENDENCIES.md) | Approved toolchain and dependencies with verification evidence; deferred selections. |
| [`.docs/OBSERVABILITY.md`](.docs/OBSERVABILITY.md) | Diagnostics, audit event kinds and fields, redaction. |
| [`.docs/data-retention.md`](.docs/data-retention.md) | What is stored, where, for how long, and what is never stored. |
| [`.docs/DATA-MODEL.md`](.docs/DATA-MODEL.md) | Conceptual entities, relationships, invariants, and retention alignment. |
| [`.docs/cli-reference.md`](.docs/cli-reference.md) | Commands, modifiers, output, and exit codes. |
| [`.docs/configuration.md`](.docs/configuration.md) | Settings, credentials, paths, and the editor. |
| [`.docs/planning.md`](.docs/planning.md) | Request planning, provider bounds, outcomes, and memory context. |
| [`.docs/security-review.md`](.docs/security-review.md) | Trust boundaries, secret handling, and residual risks. |
| [`.docs/PLAYBOOK.md`](.docs/PLAYBOOK.md) | Step-by-step setup and first planning walkthrough. |
| [`.docs/CI.md`](.docs/CI.md) | Local gate commands, active CI workflows, and manual repository settings. |
| [`.docs/adr/registry.md`](.docs/adr/registry.md) | Authoritative index of all architecture decisions. |
| [`.docs/adr/adr-0001.md`](.docs/adr/adr-0001.md) | Rust modular monolith with explicit layer boundaries. |
| [`.docs/adr/adr-0002.md`](.docs/adr/adr-0002.md) | Local SQLite persistence and per-user configuration storage. |
| [`.docs/adr/adr-0003.md`](.docs/adr/adr-0003.md) | Bounded DeepSeek integration with no agent tools. |
| [`.docs/adr/adr-0004.md`](.docs/adr/adr-0004.md) | Short hexadecimal entity identifiers. |
| [`.docs/adr/adr-0005.md`](.docs/adr/adr-0005.md) | Deterministic terminal rendering without color. |
| [`.docs/adr/adr-0006.md`](.docs/adr/adr-0006.md) | Agent guidance, portable skills, and ADR governance (superseded by ADR-0007). |
| [`.docs/adr/adr-0007.md`](.docs/adr/adr-0007.md) | Tracked implementation plan in agent guidance. |
| [`.docs/adr/adr-0008.md`](.docs/adr/adr-0008.md) | Pinned toolchain and incremental dependency baseline. |
| [`.docs/adr/adr-0009.md`](.docs/adr/adr-0009.md) | Flag-based command surface with one primary action. |
| [`.docs/adr/adr-0010.md`](.docs/adr/adr-0010.md) | Absolute path overrides and fail-closed private directories. |
| [`.docs/adr/adr-0011.md`](.docs/adr/adr-0011.md) | Default-off diagnostics and typed audit events. |
| [`.docs/adr/adr-0012.md`](.docs/adr/adr-0012.md) | GitHub Actions CI and supply-chain policy (superseded by ADR-0013). |
| [`.docs/adr/adr-0013.md`](.docs/adr/adr-0013.md) | Local quality gates until Go-Live; hosted CI deferred. |
| [`.docs/adr/adr-0014.md`](.docs/adr/adr-0014.md) | Makefile task runner and project-local development data. |
| [`.docs/adr/adr-0015.md`](.docs/adr/adr-0015.md) | Strict settings file and atomic in-place editing. |
| [`.docs/adr/adr-0016.md`](.docs/adr/adr-0016.md) | SQLite schema v1 and the synchronous database adapter. |
| [`.docs/adr/adr-0017.md`](.docs/adr/adr-0017.md) | Temporary vendored sqlx-sqlite patch for RUSTSEC-2026-0459. |
| [`.docs/adr/adr-0018.md`](.docs/adr/adr-0018.md) | Allow the BSD-3-Clause and Zlib transitive licenses. |
| [`.docs/adr/adr-0019.md`](.docs/adr/adr-0019.md) | Schema v2 for laboratories, agents, and memberships. |
| [`.docs/adr/adr-0020.md`](.docs/adr/adr-0020.md) | Interactive agent wizard and laboratory skill policy. |
| [`.docs/adr/adr-0021.md`](.docs/adr/adr-0021.md) | Width-aware aligned tables with stacked fallback. |
| [`.docs/adr/adr-0022.md`](.docs/adr/adr-0022.md) | Scoped memories as bounded, untrusted prompt context. |
| [`.docs/adr/adr-0023.md`](.docs/adr/adr-0023.md) | Schema v3 for projects, requests, stories, and planning states. |
| [`.docs/adr/adr-0024.md`](.docs/adr/adr-0024.md) | Bound confirmations and the active-planning deletion guard. |
| [`.docs/adr/adr-0025.md`](.docs/adr/adr-0025.md) | Optional project GitHub source with a global GitHub credential. |
| [`.docs/adr/adr-0026.md`](.docs/adr/adr-0026.md) | Bounded synchronous provider port and planning contract. |
| [`.docs/adr/adr-0027.md`](.docs/adr/adr-0027.md) | Allow ISC and CDLA-Permissive-2.0 for the TLS dependency graph. |
| [`.docs/adr/adr-0028.md`](.docs/adr/adr-0028.md) | Content-free interaction metadata (schema v5). |
| [`.docs/adr/adr-0029.md`](.docs/adr/adr-0029.md) | Activate hosted CI before the release tag. |
| [`.project/IMPLEMENTATION-PLAN.md`](.project/IMPLEMENTATION-PLAN.md) | Ordered work packages, tasks, and gates for the initial release. |

### Precedence

1. `.docs/CONSTITUTION.md` and `.docs/SECURITY.md`
2. `.docs/adr/registry.md` and the accepted ADRs
3. `.docs/ARCHITECTURE.md` and `.docs/STACK.md`
4. `.docs/TECHNICAL-SPECIFICATIONS.md`
5. `.docs/VALIDATION.md`
6. `.docs/PRINCIPLES.md`
7. `.docs/ELEVATOR-PITCH.md`
8. `.project/IMPLEMENTATION-PLAN.md`
9. This file

If a task appears to require violating a higher-ranked document, stop and
record the conflict. Do not silently reinterpret the rules.

---

## 3. Architecture decisions (ADRs)

Every material technical decision must be recorded as an ADR **before or in the
same change** that implements it. This is mandatory, not optional
documentation.

A material technical decision includes:

- Selecting or replacing a dependency, crate, toolchain, or external service.
- Changing module boundaries, layering, or dependency direction.
- Adding or changing a database table, column, index, constraint, or migration.
- Changing a command, flag, output contract, exit code, or confirmation flow.
- Changing security controls, authentication, authorization, trust boundaries,
  retention, or logging.
- Changing the provider contract, prompt trust model, execution limits, retry
  policy, or structured output schema.
- Changing repository layout, tracked documentation structure, or agent
  guidance.

Routine bug fixes that restore documented behavior and have no alternatives do
not need an ADR. If a future contributor could reasonably ask "why was this
chosen?", write the ADR.

### Procedure

1. Read [`.docs/adr/registry.md`](.docs/adr/registry.md) and check whether an
   accepted decision already covers the topic.
2. Create `.docs/adr/adr-NNNN.md` using the next unused four-digit number.
3. State context, decision, consequences, alternatives, and references.
4. Add one row to the registry in the same change.
5. Never rewrite an accepted ADR. Supersede it with a new ADR and update the old
   one's status and the registry's supersession map.
6. Use the `vlab-adr` skill for the full procedure and template.

The registry is the authoritative index. A decision that is not in the registry
is not binding.

---

## 4. Skills

Skills are reusable procedures stored in `.agents/skills/<name>/SKILL.md`. They
use portable frontmatter (`name` and `description`) understood by both Claude
Code and OpenCode. Use the matching skill before starting the corresponding
work.

| Skill | Path | Use when |
|---|---|---|
| `vlab-adr` | [`.agents/skills/vlab-adr/SKILL.md`](.agents/skills/vlab-adr/SKILL.md) | Making, revising, or superseding a technical decision; maintaining the registry. |
| `vlab-rust-tdd` | [`.agents/skills/vlab-rust-tdd/SKILL.md`](.agents/skills/vlab-rust-tdd/SKILL.md) | Implementing or fixing any Rust behavior, domain rule, or test. |
| `vlab-cli-contract` | [`.agents/skills/vlab-cli-contract/SKILL.md`](.agents/skills/vlab-cli-contract/SKILL.md) | Adding or changing commands, flags, help, output, exit codes, or confirmations. |
| `vlab-sqlite-migration` | [`.agents/skills/vlab-sqlite-migration/SKILL.md`](.agents/skills/vlab-sqlite-migration/SKILL.md) | Changing the schema, migrations, repositories, constraints, or transactions. |
| `vlab-ai-provider` | [`.agents/skills/vlab-ai-provider/SKILL.md`](.agents/skills/vlab-ai-provider/SKILL.md) | Changing provider calls, prompts, planning output, budgets, retries, or questions. |
| `vlab-security-review` | [`.agents/skills/vlab-security-review/SKILL.md`](.agents/skills/vlab-security-review/SKILL.md) | Reviewing a change that touches trust boundaries, secrets, model output, or destructive actions. |

Do not duplicate these procedures in tool-specific files. If a tool cannot
discover `.agents/skills` natively, configure it to load that directory.

---

## 5. Required workflow

1. **Read.** Load `AGENTS.md`, the relevant `.docs/` documents, and the ADR
   registry.
2. **Frame.** State the problem, observable acceptance criteria, material
   assumptions, and affected trust boundaries.
3. **Decide.** If the work requires a material technical decision, write the ADR
   and update the registry first.
4. **Test first.** Write a failing test for the intended behavior, confirm it
   fails for the expected reason, then implement the smallest passing change.
5. **Refactor.** Improve structure while tests remain green.
6. **Review security.** Use `vlab-security-review` for any security-relevant
   change.
7. **Run gates.** Execute the full quality gate set below.
8. **Document.** Update affected documentation, retention notes, and the ADR
   registry.
9. **Report.** State exactly what was executed, what passed, and what remains
   unverified or accepted as risk.

Never claim to have inspected a file, executed a command, or verified behavior
unless it actually happened.

---

## 6. Engineering guardrails

- **Language:** Rust, pinned stable toolchain, safe Rust first.
- **Layering:** `cli -> app -> domain`; `infra` implements ports. Domain code
  performs no I/O.
- **No agent framework:** do not add multi-agent orchestration, autonomous
  execution, or a plugin system.
- **No tools for the model:** model output is data, never authority. No shell,
  filesystem, network, or code execution through the model.
- **Persistence:** SQLite through SQLx, one connection initially, foreign keys
  enabled, parameterized queries, versioned migrations.
- **Provider:** DeepSeek only, credentials from `DEEPSEEK_API_KEY`, bounded
  timeouts and sizes, strict structured output, no automatic repair loop.
- **Secrets:** never in source, arguments, prompts, logs, errors, database, test
  fixtures, or command output.
- **Untrusted input:** validate before use; never execute model output.
- **Destructive actions:** require explicit, bound confirmation; fail closed in
  non-interactive environments.
- **Dependencies:** minimal, maintained, licensed, and free of known
  vulnerabilities. Commit the lockfile.
- **Language of artifacts:** English.

---

## 7. Quality gates

Run all applicable commands before declaring a change complete:

```sh
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo audit
cargo deny check
```

`make check` runs the same five commands in order (ADR-0014). Hosted CI is
active and supplements these local runs (ADR-0029); local gate evidence remains
mandatory.

Additional requirements:

- Tests must be deterministic and must not require a live provider or network
  access.
- Security-boundary tests must pass individually; never average them into a
  quality score.
- Do not suppress warnings, advisories, or license findings to pass a gate.
- If a command cannot run in the current environment, report that fact and the
  reason instead of claiming success.

---

## 8. Repository layout

Tracked content:

```text
AGENTS.md                       contributor and agent entry point
README.md, CONTRIBUTING.md      project overview and local setup
Makefile                        developer tasks; `make` lists them (ADR-0014)
.docs/                          authoritative project documentation
.docs/adr/                      architecture decisions and registry
.agents/skills/                 reusable, tool-portable procedures
.project/IMPLEMENTATION-PLAN.md ordered implementation plan
Cargo.toml, Cargo.lock          crate manifest and committed lockfile
rust-toolchain.toml             pinned toolchain (ADR-0008)
deny.toml                       cargo-deny policy (ADR-0012, ADR-0013)
migrations/                     embedded, forward-only schema migrations (ADR-0016)
vendor/                         temporary patched dependencies; provenance recorded (ADR-0017)
src/                            cli, app, domain, infra, obs (ADR-0001)
tests/                          integration tests; tests/support/ shared harness
.github/                        active CI and Dependabot (ADR-0029)
```

Project documentation, including documents the implementation plan places
under `docs/`, lives in `.docs/`. Migrations follow the structure defined by
the accepted ADRs and `.docs/STACK.md`.

### Version-control boundary

Everything listed above is tracked. The plan sequences work but must not
introduce rules or decisions that are not recorded in `.docs/` or an accepted
ADR. Local scratch directories, including `.tmp/` and any other content
under `.project/`, are outside version control. They are not authoritative
sources and must not be referenced by tracked guidance, skills, ADRs, or the
plan. The development data folder `.data/`, written by `make run`, is also
outside version control (ADR-0014).

---

## 9. Definition of done

A change is complete only when:

- Its acceptance criteria are met and covered by automated tests.
- Failure paths and security boundaries are tested, not only the happy path.
- Formatting, linting, tests, dependency audit, and license policy pass.
- Necessary ADRs are written and the registry is updated.
- Documentation and retention notes match the implemented behavior.
- No secrets, unrelated files, or unintended changes are included.
- The completion report accurately states validation performed and remaining
  limitations.

---

## 10. Maintaining this file

Update `AGENTS.md` when the documentation set, skill set, entry-point rules, or
tracked repository layout changes. Such a change is a technical decision
governed by ADR-0007; if it alters the governance model itself, supersede
ADR-0007 with a new ADR and update the registry.
