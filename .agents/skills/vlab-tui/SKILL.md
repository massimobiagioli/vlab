---
name: vlab-tui
description: Implement or change VLab interactive terminal dashboards, themes, keymaps, and plain fallbacks with ratatui/crossterm. Use for any dashboard screen, terminal-color policy, key handling, or TUI test.
---

# VLab terminal dashboards

Use this skill when adding or changing interactive terminal output. The
authoritative decisions are ADR-0038 (dashboards), ADR-0005 (superseded), and
the feature plan `00001-feat-interactive-dashboards.md`.

## Read first

- `AGENTS.md`
- `.docs/STACK.md` (Textual User Interface)
- `.docs/DEPENDENCIES.md`
- `.docs/SECURITY.md` (confirmation, secrets, untrusted text)
- `.docs/adr/adr-0038.md`, `adr-0009.md`, `adr-0021.md`
- `.project/features/00001-feat-interactive-dashboards.md`

## Architecture

- TUI code belongs in `src/cli/tui/`; it is presentation only.
- Call application use cases from `app`; never duplicate domain validation or
  authorization.
- Never call the model provider from a list, detail, filter, or navigation
  screen.
- Use `ratatui` and its re-exported `crossterm` backend only. Do not add a
  second terminal library.
- Keep a plain renderer for `--plain`, non-TTY streams, and tests.

## Rules

1. **Two modes.** Interactive dashboards only when stdin and stdout are TTYs;
   otherwise fall back to plain output. `--plain` wins even in a TTY.
2. **Terminal lifecycle.** Use `ratatui::try_init`/`try_restore` or an RAII
   guard. Restore alternate screen, raw mode, and cursor on success, error,
   panic, SIGINT, and editor/wizard suspension.
3. **Color.** Semantic tokens only. Honor `NO_COLOR`, `--no-color`,
   `TERM=dumb`, and non-TTY. Never rely on color alone. Plain output contains no
   ANSI or cursor sequences.
4. **Sanitize.** Pass every stored or model-generated string through
   `cli::render::sanitize` before rendering. Identifiers are never truncated
   silently.
5. **Secrets.** Never render a credential; show only `set`/`unset`. Do not log
   screen content.
6. **Destructive actions.** Use the same bound confirmation as the flag path.
   Show the exact target and scope. `Esc` cancels.
7. **Minimum size.** Test 80×24 and 60×20; show an explicit “terminal too
   small” screen below the minimum.
8. **Pagination.** The action bar is persistent and shows the active keymap.
   Navigation never mutates the database.
9. **Error handling.** Convert application errors to a sanitized modal or toast.
   A dashboard action must not leave the terminal broken.

## Testing

- Unit-test theme tokens, sanitization, table layout, and key transitions.
- Render with `ratatui::backend::TestBackend` at fixed sizes; assert visible
  text, not escape bytes.
- Test plain and non-TTY paths through the CLI with `assert_cmd`.
- Add PTY tests when the harness is available, but never make CI depend on an
  interactive terminal.
- Verify `NO_COLOR`, `--no-color`, `--plain`, `TERM=dumb`, and redirected
  streams.
- Run `make check` and the hosted CI matrix.

## Completion

- Every dashboard action has a tested path and a plain/non-TTY equivalent where
  documented.
- Terminal restoration is verified after error and panic.
- No credential, model content, or control sequence leaks into output.
- ADRs, feature checklist, and CLI documentation are updated.
