---
name: vlab-cli-contract
description: "Add or change VLab commands, flags, help text, output, or exit codes without breaking the CLI contract. Use for any user-facing command-line behavior, rendering change, confirmation flow, or non-interactive behavior."
---

# VLab CLI contract

The CLI is the product interface. Treat its commands, streams, exit codes, and
non-interactive behavior as a stable contract.

## Read first

- `AGENTS.md`
- `.docs/STACK.md` (textual UI conventions)
- `.docs/SECURITY.md` (confirmation and failure behavior)
- `.docs/adr/adr-0005.md` (rendering)

## Rules

1. **One primary action per invocation.** Modifiers such as `--to-agent` or
   `--yes` are valid only with their declaring command.
2. **Help is part of the contract.** Keep command groups consistent, describe
   preconditions, and update help when a flag changes.
3. **Streams are strict.** Results go to stdout; diagnostics, warnings,
   progress, and errors go to stderr. Never mix them.
4. **Exit codes are stable.** Use the documented code for success, operation
   failure, invalid input, missing entity or context, validation or
   precondition failure, user cancellation, and interruption. Do not invent new
   codes without an ADR.
5. **No secrets on the command line.** Read credentials from the documented
   environment variable and never echo them.
6. **Non-interactive safety.** Do not prompt when stdin is not a terminal.
   Destructive commands fail closed unless explicitly authorized; they never
   hang waiting for input.
7. **Cancellation is explicit.** Report cancellation as cancellation, not as
   success or a generic failure. Do not retry after cancellation.
8. **Untrusted text is inert.** Sanitize control characters before rendering
   names, titles, questions, or model output. Never execute model output.
9. **Rendering is deterministic.** Use the shared renderer with an injected
   width. Do not emit ANSI sequences. Provide an actionable empty state.
10. **No hidden state changes.** Help, version, validation, and read-only
    commands must not create files, open databases unnecessarily, or mutate
    settings.

## Tests

- Snapshot the help output and the main command groups.
- Table-drive invalid combinations and assert exit code, stdout, and stderr
  separately.
- Test empty states, narrow widths, redirected output, and control characters.
- Test confirmation: yes, no, EOF, `--yes`, and non-TTY without `--yes`.
- Test that read-only commands leave paths and database rows untouched.

## Completion

- Update the command reference and any affected examples.
- Confirm no ANSI bytes appear in captured output.
- Confirm exit codes and stream separation match the documented contract.
- If the change alters the interface contract, record an ADR first.
