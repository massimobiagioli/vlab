# Feature 00003 — Form-based dashboard editing

| Field | Value |
|---|---|
| Identifier | `00003` |
| Branch | `feat/form-editor` |
| Status | `Done` |
| Date | 2026-11-13 |
| Related ADRs | ADR-0042 (form modal), ADR-0038/0039 (dashboards), ADR-0041 (lab description) |
| Scope | Dashboard input presentation and key handling |

## Goal

Replace inline “Label: value” edit screens with a proper form modal:

- each field has a visible bordered input box;
- the focused field is clearly highlighted;
- a visible cursor supports insertion and editing;
- keyboard navigation works with Tab, Shift-Tab, arrows, Home/End;
- Save/Cancel are explicit and discoverable.

## Acceptance criteria

1. New/Edit laboratory and New project use the same form component.
2. Fields render as labeled bordered boxes; the focused box has an accent border.
3. The focused field shows a cursor and supports insert, Backspace, Delete, Left/Right, Home/End.
4. `Tab`/`↓` and `Shift-Tab`/`↑` move focus; `Enter` saves; `Esc` cancels.
5. Existing PTY flows (`n` name Enter, `e` Tab description Enter, project `n` name Enter) keep working.
6. Validation errors remain actionable and do not leak field values.
7. TestBackend and PTY tests cover rendering, focus, editing, save, and cancel.
8. `make check` and hosted CI pass.

## Non-goals

- No mouse editing, clipboard integration, or multi-line editor.
- No rich text.
- No change to domain validation or persistence.

## Validation

Local gate (2026-11-13): `make check` passed (fmt, clippy, tests, audit, deny).
Hosted CI is recorded after the closing push.

## Task checklist

- [x] **F00-01** Write ADR-0042 for the reusable form modal and keymap; update the
  registry.
- [x] **F01-01** Add a reusable `Form`/`FormField` state with char cursor,
  insert/delete, and focus movement.
- [x] **F02-01** Replace the laboratory and project input modes with the form
  component and keep create/update/project actions unchanged.
- [x] **F03-01** Render the form as an outer modal with labeled bordered fields,
  focused accent, cursor, and a Save/Cancel hint bar.
- [x] **F04-01** Add unit tests for cursor editing and focus navigation.
- [x] **F04-02** Add TestBackend and PTY tests for form rendering and the
  existing create/edit flows.
- [x] **F05-01** Update dashboard documentation and run `make check`/CI; close
  the feature.
