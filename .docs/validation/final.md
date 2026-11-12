# Final Deterministic Validation

Date: 2026-11-12
Scope: WP-1 through WP-6, deterministic suite (no live provider)
Environment: macOS (aarch64), Rust 1.99.1, bundled SQLite 3.54.1
Commit: working tree, no tag

## Executed commands

```sh
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo audit
cargo deny check
```

`make check` runs the five commands in order.

| Command | Result |
|---|---|
| `cargo fmt --all -- --check` | pass |
| `cargo clippy --locked --all-targets -- -D warnings` | pass |
| `cargo test --locked` | 319 passed, 0 failed |
| `cargo audit` | no findings |
| `cargo deny check` | advisories, bans, licenses, sources ok |

Security-boundary tests were also run individually (security_planning,
security_secrets, planning_cancel, provider_deepseek, db_schema_policy); they
are not averaged into a score.

## §17 acceptance criteria

| # | Criterion | Evidence | Result |
|---|---|---|---|
| 1 | Help documents all commands | `tests/cli_help.rs`, `tests/cli_contract.rs` | pass |
| 2 | Settings shown/edited without credential exposure | `tests/cli_settings_show.rs`, `tests/cli_settings_edit.rs`, `tests/security_secrets.rs` | pass |
| 3 | Invalid edits preserve the previous file | `tests/cli_settings_edit.rs` hash comparison | pass |
| 4 | Laboratories created/listed/opened/checked/deleted | `tests/cli_labs.rs`, `tests/cli_deletions.rs` | pass |
| 5 | Validation fails without agents or Tech Lead | `src/app/lab_validation.rs`, `tests/cli_labs.rs` | pass |
| 6 | Agents created via guided procedure and inspected | `src/app/agents/wizard.rs`, `tests/cli_agents.rs` | pass |
| 7 | Detail shows memberships, assignments, metrics/N/A | `tests/cli_agents.rs` | pass |
| 8 | Projects created/listed/deleted in active lab | `tests/cli_projects.rs`, `tests/cli_deletions.rs` | pass |
| 9 | Requests as quoted text or piped input | `tests/cli_new_request.rs` | pass |
| 10 | Original request persisted before provider | `tests/planning_flow.rs` (`the_request_is_persisted_before_the_provider_call`) | pass |
| 11 | Valid planning creates linked stories | `tests/planning_flow.rs`, `tests/cli_new_request.rs` | pass |
| 12 | Assignments reference real members | `tests/security_planning.rs`, `src/app/planning/rules.rs` | pass |
| 13 | Every story has a locally validated due date | `src/app/planning/rules.rs` leap/year-boundary tests | pass |
| 14 | Failed processing preserves the request | `tests/planning_flow.rs`, `tests/planning_cancel.rs` | pass |
| 15 | Kanban readable narrow/without color | `src/cli/render.rs`, `src/cli/dispatch.rs` board tests | pass |
| 16 | Questions target an agent or default lead | `tests/cli_ask.rs`, `tests/e2e_ask.rs` | pass |
| 17 | Destructive operations require confirmation | `src/cli/confirm.rs`, `tests/cli_confirmation.rs`, `tests/cli_deletions.rs` | pass |
| 18 | Provider execution bounded and observable | `tests/provider_deepseek.rs`, `tests/planning_cancel.rs`, interactions tests | pass |
| 19 | No model-generated content executed | no tools; `tests/security_planning.rs`, inert story test | pass |
| 20 | Core workflows pass deterministic tests | full suite | pass |
| 21 | Automatic memories are generated and cascade with their owner | `tests/cli_memories.rs`, `tests/db_memories.rs`, `tests/planning_flow.rs` | pass |
| 22 | Memory content bounded/untrusted/not executed | `tests/security_planning.rs`, memory tests | pass |
| 23 | Planning/questions use scoped memories within cap | prompt and ask tests | pass |
| 24 | Optional GitHub source with global key, clone-only network operation | `tests/cli_projects.rs`, `src/infra/workspace.rs`, migration test | pass |

## Adapted VALIDATION.md scenarios

- S-01 authorized planning: `tests/planning_flow.rs`.
- S-02 unauthorized/out-of-lab assignment: `tests/security_planning.rs`.
- S-03 prompt injection: `tests/security_planning.rs`.
- S-04 unsupported/fabricated output: schema/rules tests.
- S-05 budget/repeated action: `tests/planning_cancel.rs`, `tests/provider_deepseek.rs`.
- S-06 provider timeout: `tests/provider_deepseek.rs`, `tests/planning_cancel.rs`.
- S-07 user cancellation: `tests/planning_cancel.rs`; SIGINT wired in CLI.
- S-08 invalid/oversized input/response: `tests/cli_new_request.rs`, `tests/bounds.rs`, `tests/provider_deepseek.rs`.
- S-09 memory injection/privacy: `tests/security_planning.rs`, `tests/security_secrets.rs`.

## Residual limitations and Go-Live items

- Live DeepSeek endpoint/model, authentication mapping, retry cost, and semantic
  quality are GL-T2/GL-T3. The deterministic suite uses wiremock/scripted
  providers and requires no network or key.
- Linux was not run locally; hosted CI activation is GL-T1.
- Real PTY confirmation, literal-input TTY, and SIGINT interruption were not
  automated; non-TTY behavior, cancellation tokens, and scripted providers cover
  the contracts.
- SQLite is unencrypted at rest; backups and automatic purge are out of scope.
- No story transitions, JSON output, packaging, or Windows hardening.
- Semantic assignment/answer quality is an evaluation criterion, not proven.
- The release security gate and vulnerability-reporting channel are GL-T5.

## Completion statement

All deterministic §17 criteria pass with the evidence above. Live-provider
behavior, a real-user walkthrough, hosted CI, the release security gate, and
tagging remain explicitly assigned to Go-Live and are not claimed here.
