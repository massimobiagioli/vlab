# Security Review

Date: 2026-11-12
Scope: WP-6 secrets, privacy, redaction, the settings-file credential change
(ADR-0030), the project clone boundary (ADR-0032), current-project commands
(ADR-0033), project rosters (ADR-0034), automatic memories (ADR-0035,
ADR-0036), and interactive dashboards (ADR-0038).

## Trust boundaries and controls

| Boundary | Threat | Control | Evidence |
|---|---|---|---|
| Executable layout → paths | Debug build selecting attacker-controlled data | Fallback derived from the executable path, never the working directory; explicit overrides win; release builds unaffected | `src/infra/paths.rs`, `tests/paths.rs` |
| Caller → CLI | Oversized/invalid input, control characters | UTF-8 and size caps before use; structured command validation; rendering sanitization | `tests/cli_new_request.rs`, `tests/cli_contract.rs`, `src/cli/render.rs` |
| Application → git/GitHub | Token leak, command injection, unbounded clone | `git` without a shell, validated HTTPS source, `--` separator, private askpass, `GIT_TERMINAL_PROMPT=0`, credential helpers disabled, 300 s timeout, output discarded | `src/infra/workspace.rs`, `tests/cli_projects.rs` |
| Application → DeepSeek | Credential/data leak | `credentials.deepseek_api_key` from private `settings.yaml`, used only in the Authorization header; request size cap; no CV/unrelated data; TLS validation | `tests/provider_deepseek.rs`, `tests/security_planning.rs` |
| Model → application | Prompt injection, fabricated IDs, unsafe content | Strict schema, local membership/date rules, no tools, no execution, sanitized display | `tests/security_planning.rs`, `src/app/planning/*` |
| Memory → prompt | Injected instructions or secrets | Scoped selection, whole-memory 8 KiB cap, delimiters, configured-secret rejection; automatic writes are best effort and deterministic except the bounded planning summary | `src/app/planning/prompt.rs`, `src/app/memories.rs`, `src/app/requests.rs` |
| Application → storage | Secret/prompt retention | No prompt/response columns; content-free interactions/events; schema policy test | `tests/db_schema_policy.rs`, `tests/security_secrets.rs` |
| Current project → commands | Stale or cross-laboratory project | Typed state reloaded per command; project must resolve in the active laboratory; switching labs clears it | `src/app/context.rs`, `tests/cli_kanban.rs`, `tests/cli_projects.rs` |
| Project roster → planning | Unauthorized or unknown agent | Membership, archive, and laboratory checks; planning filters to the roster; empty result fails closed | `src/app/projects.rs`, `src/app/requests.rs`, `tests/cli_memories.rs`, `tests/planning_flow.rs` |
| Automatic memory → prompt | Injected or secret content | Deterministic structural text; bounded model summary; secret check; 20-per-scope and 8 KiB prompt caps | `src/app/memories.rs`, `src/app/requests.rs`, `tests/planning_flow.rs` |
| Stored/model text → terminal | Escape-sequence injection, misleading UI | All rendered text sanitized; identifiers never silently truncated; color never the only signal | `src/cli/tui/mod.rs`, `tests/cli_dashboards.rs` |
| Dashboard → destructive action | Accidental or bypassed deletion | Bound confirmation modal; exact target; `Esc` cancels; same app use cases as flags | `src/cli/tui/mod.rs` |
| Settings file → application | Credential disclosure or malformed config | Strict bounded parser, token validation, content-free errors, `0600`/`0700`, Unix refusal of group/other-readable files | `tests/cli_settings_show.rs`, `src/infra/config.rs` |
| Application → logs | Payload leakage | Typed events, `Redacted`, content-free diagnostics; marker tests with `RUST_LOG=trace` | `tests/redaction.rs`, `tests/security_secrets.rs` |
| Application → confirmation | Bypass in scripts | Bound `y/N`, explicit `--yes`, non-TTY exit 4, snapshot re-check | `src/cli/confirm.rs`, `tests/cli_confirmation.rs` |

## Secret handling

- Synthetic marker `sk-vlab-SECRET-MARKER` is used in tests as the API key, in
  request text, and in a memory. Test settings files are private fixtures.
- The marker is allowed only in a private test settings file. It is absent
  from stdout, stderr, requests, events, interactions, database rows, and
  rendered errors. Request/memory content containing the current key is
  rejected before provider dispatch or storage.
- On Unix, `--show-settings` refuses a settings file with group or other
  permissions and reports only the `chmod 600` remedy.
- `Debug` implementations for settings, credentials, and provider
  requests/responses redact content.
- The GitHub key is stored only in the private settings file; only `set`/`unset`
  is reported.
- Automatic planning summaries use one bounded provider call; failure or a
  secret-containing response skips the write and never changes the request
  outcome.

## Filesystem and data

- Settings/database directories `0700`; files `0600` on Unix.
- Editor drafts are private, removed on failure, and validated before an atomic
  rename.
- SQLite is not encrypted at rest; OS disk encryption is recommended.
- No backups or automatic purge exist; retention is documented in
  [`data-retention.md`](data-retention.md).

## Manual repository-history check

Before Go-Live, inspect committed history for credential patterns, for example
`git log -p -- . ':!tests' | rg -i 'api[_-]?key|secret|password|token'` and
confirm any hit is a field name or a synthetic test marker. VLab never commits
real credential values; `settings.yaml` is git-ignored, and the synthetic
`sk-vlab-SECRET-MARKER` exists only in tests and the placeholder example.

## Residual risks and limitations

- Prompt injection cannot be proven absent; mitigations are structural (no
  tools, strict validation) plus adversarial tests.
- Provider-side processing and retention are outside local control; the live
  contract is verified at GL-T2.
- No real credential appears in the repository history by review; synthetic
  markers are intentionally present in test files and the example uses
  placeholders. The manual check must be repeated before Go-Live.
- The settings file is the credential store. Backups, editor swap files, crash
  dumps, and processes running as the same operating-system user are outside
  VLab's control; users must protect those artifacts.
- Credentials are not encrypted at rest beyond file permissions; an
  operating-system keyring is deferred.
- A debug build selects `.data` next to its `target/debug` directory. Debug
  binaries must not be distributed as release artifacts.
- The dashboard creates the SQLite file even when only the Settings screen is
  opened; this is a local side effect, not a secret exposure. A later refinement
  can make database opening lazy for Settings.
- A clone writes a full checkout under `.workspace/`. The checkout is not
  encrypted or backed up, and project deletion does not remove it yet; users
  must use `make clean-workspace` for an explicit reset.
- Killing a clone can leave a temporary directory under `.workspace/`; the
  next clone removes only its own temporary directory.
- Automatic memories can accumulate model-generated summaries. They remain
  untrusted and within the 20-per-scope and 8 KiB prompt caps; there is no
  manual cleanup command, so a full scope stops further automatic writes.
- Windows permission hardening is best effort and not tested here.
