# Feature 00009 — MSX-style laboratory floor

| Field | Value |
|---|---|
| Identifier | `00009` |
| Branch | `feat/msx-floor` |
| Status | `In progress` |
| Date | 2026-11-13 |
| Related ADRs | ADR-0045 (laboratory floor), ADR-0038 (dashboards) |
| Scope | Visual density of the terminal floor |

## Goal

Make the laboratory floor read as a friendly MSX-inspired 2D scene inside the
terminal: a tiled floor, bright project pods, blocky pixel sprites, desks, and
a CRT-like frame, while keeping the existing data, navigation, and ASCII
fallback. No GUI, image protocol, or new dependency is introduced.

## Acceptance criteria

1. At 80×24 and 100×30 the scene shows a tiled floor background, colored
   project pods, and two-line block sprites for agents; at 60×20 it uses the
   compact one-line sprites to preserve legibility.
2. The lead desk, focus desk, `(zzz)`, exchange `( ? )`/`( ! )`, and `+N more`
   indicators remain visible and readable.
3. Project accent colors never carry meaning alone; the project name and
   symbols remain the primary signal. The ASCII fallback uses `+ - |` and
   `(o_o)`.
4. All existing tests pass; `TestBackend` renders at 60×20, 80×24, and 100×30
   without panicking or clipping the Actions bar.
5. `make check` and hosted CI pass.

## Non-goals

- No real bitmap graphics, Sixel, Kitty, or iTerm2 image protocol.
- No animation or timers.
- No new data, provider calls, or persistence.

## Risks

- Two-line sprites reduce how many agents fit in a pod; pagination and
  `+N more` must still work.
- Background tiles can reduce contrast on monochrome terminals; agent rows
  clear the tile behind text.
- Color is a visual accent, not the only information channel.

## Validation

- Local gate (2026-11-13): `make check` passed (fmt, clippy, tests, audit,
  deny).
- `TestBackend` renders at 60×20, 80×24, and 100×30.
- Scene assertions for the tiled floor, two-line sprite, compact fallback, and
  ASCII fallback.
- Hosted CI is required before the feature is marked `Done`.

## Task checklist

- [x] **F01-01** Add the tiled floor and accent-colored pods.
- [x] **F01-02** Add two-line block sprites at wide sizes and keep the compact
  fallback at 60×20.
- [x] **F02-01** Update the visual tests and the feature/spec docs.
- [x] **F03-01** Run `make check` and fix every failure.
- [ ] **F04-01** Push, confirm hosted CI is green, and set the status to `Done`.
