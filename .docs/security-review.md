# Security Review

Date: 2026-11-12
Scope: WP-6 secrets, privacy, redaction, and the settings-file credential change
(ADR-0030).

## Trust boundaries and controls

| Boundary | Threat | Control | Evidence |
|---|---|---|---|
| Executable layout → paths | Debug build selecting attacker-controlled data | Fallback derived from the executable path, never the working directory; explicit overrides win; release builds unaffected | `src/infra/paths.rs`, `tests/paths.rs` |
| Caller → CLI | Oversized/invalid input, control characters | UTF-8 and size caps before use; structured command validation; rendering sanitization | `tests/cli_new_request.rs`, `tests/cli_contract.rs`, `src/cli/render.rs` |
| Application → DeepSeek | Credential/data leak | `credentials.deepseek_api_key` from private `settings.yaml`, used only in the Authorization header; request size cap; no CV/unrelated data; TLS validation | `tests/provider_deepseek.rs`, `tests/security_planning.rs` |
| Model → application | Prompt injection, fabricated IDs, unsafe content | Strict schema, local membership/date rules, no tools, no execution, sanitized display | `tests/security_planning.rs`, `src/app/planning/*` |
| Memory → prompt | Injected instructions or secrets | Scoped selection, whole-memory 8 KiB cap, delimiters, configured-secret rejection | `src/app/planning/prompt.rs`, `src/app/memories.rs` |
| Application → storage | Secret/prompt retention | No prompt/response columns; content-free interactions/events; schema policy test | `tests/db_schema_policy.rs`, `tests/security_secrets.rs` |
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
- Windows permission hardening is best effort and not tested here.
