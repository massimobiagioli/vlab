# Security Review

Date: 2026-11-12
Scope: WP-6 secrets, privacy, redaction, and the deterministic test suite.

## Trust boundaries and controls

| Boundary | Threat | Control | Evidence |
|---|---|---|---|
| Caller → CLI | Oversized/invalid input, control characters | UTF-8 and size caps before use; structured command validation; rendering sanitization | `tests/cli_new_request.rs`, `tests/cli_contract.rs`, `src/cli/render.rs` |
| Application → DeepSeek | Credential/data leak | `DEEPSEEK_API_KEY` only in the Authorization header; request size cap; no CV/unrelated data; TLS validation | `tests/provider_deepseek.rs`, `tests/security_planning.rs` |
| Model → application | Prompt injection, fabricated IDs, unsafe content | Strict schema, local membership/date rules, no tools, no execution, sanitized display | `tests/security_planning.rs`, `src/app/planning/*` |
| Memory → prompt | Injected instructions or secrets | Scoped selection, whole-memory 8 KiB cap, delimiters, configured-secret rejection | `src/app/planning/prompt.rs`, `src/app/memories.rs` |
| Application → storage | Secret/prompt retention | No prompt/response columns; content-free interactions/events; schema policy test | `tests/db_schema_policy.rs`, `tests/security_secrets.rs` |
| Application → logs | Payload leakage | Typed events, `Redacted`, content-free diagnostics; marker tests with `RUST_LOG=trace` | `tests/redaction.rs`, `tests/security_secrets.rs` |
| Application → confirmation | Bypass in scripts | Bound `y/N`, explicit `--yes`, non-TTY exit 4, snapshot re-check | `src/cli/confirm.rs`, `tests/cli_confirmation.rs` |

## Secret handling

- Synthetic marker `sk-vlab-SECRET-MARKER` is used in tests as the API key, in
  request text, and in a memory.
- The marker is absent from stdout, stderr, settings, requests, events,
  interactions, and rendered errors. Request/memory content containing the
  current key is rejected before provider dispatch or storage.
- `Debug` implementations for provider requests/responses redact content.
- The GitHub key is never stored; only `set`/`unset` is reported.

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
`DEEPSEEK_API_KEY`/`GITHUB_API_KEY` values; the synthetic
`sk-vlab-SECRET-MARKER` exists only in tests.

## Residual risks and limitations

- Prompt injection cannot be proven absent; mitigations are structural (no
  tools, strict validation) plus adversarial tests.
- Provider-side processing and retention are outside local control; the live
  contract is verified at GL-T2.
- No credential appears in the repository history by review; a synthetic marker
  is intentionally present in test files. The manual check must be repeated
  before Go-Live.
- Windows permission hardening is best effort and not tested here.
