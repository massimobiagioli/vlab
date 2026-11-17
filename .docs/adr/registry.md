# Architecture Decision Record Registry

This registry is the authoritative index of technical decisions for the
project. `AGENTS.md` links to it, and every tracked change that makes or
revises a technical decision must keep it current.

## Rules

1. Read this registry before making an architectural, dependency, schema,
   interface, security, or repository-layout decision.
2. Create the next ADR as `.docs/adr/adr-NNNN.md`, using the next unused
   four-digit number.
3. Add exactly one row here in the same change that adds the ADR.
4. Never rewrite an accepted decision. If a decision changes, create a new ADR
   and mark the old one `Superseded` with a link to its replacement.
5. Keep titles short, factual, and in English.

## Status values

| Status | Meaning |
|---|---|
| `Proposed` | Under review; not yet binding. |
| `Accepted` | Binding for current work. |
| `Superseded` | Replaced by a newer ADR; retained for history. |
| `Deprecated` | Still documented but should not be used for new work. |
| `Rejected` | Considered and deliberately not adopted. |

## Decisions

| ID | Title | Status | Date | Supersedes | File |
|---|---|---|---|---|---|
| ADR-0001 | Rust modular monolith with explicit layer boundaries | Accepted | 2026-11-12 | — | [adr-0001.md](adr-0001.md) |
| ADR-0002 | Local SQLite persistence and per-user configuration storage | Accepted | 2026-11-12 | — | [adr-0002.md](adr-0002.md) |
| ADR-0003 | Bounded DeepSeek integration with no agent tools | Accepted | 2026-11-12 | — | [adr-0003.md](adr-0003.md) |
| ADR-0004 | Short hexadecimal entity identifiers | Accepted | 2026-11-12 | — | [adr-0004.md](adr-0004.md) |
| ADR-0005 | Deterministic terminal rendering without color | Accepted | 2026-11-12 | — | [adr-0005.md](adr-0005.md) |
| ADR-0006 | Agent guidance, portable skills, and ADR governance | Superseded | 2026-11-12 | — | [adr-0006.md](adr-0006.md) |
| ADR-0007 | Tracked implementation plan in agent guidance | Accepted | 2026-11-12 | ADR-0006 | [adr-0007.md](adr-0007.md) |
| ADR-0008 | Pinned toolchain and incremental dependency baseline | Accepted | 2026-11-12 | — | [adr-0008.md](adr-0008.md) |
| ADR-0009 | Flag-based command surface with one primary action | Accepted | 2026-11-12 | — | [adr-0009.md](adr-0009.md) |
| ADR-0010 | Absolute path overrides and fail-closed private directories | Accepted | 2026-11-12 | — | [adr-0010.md](adr-0010.md) |
| ADR-0011 | Default-off diagnostics and typed audit events | Accepted | 2026-11-12 | — | [adr-0011.md](adr-0011.md) |
| ADR-0012 | GitHub Actions CI and supply-chain policy | Superseded | 2026-11-12 | — | [adr-0012.md](adr-0012.md) |
| ADR-0013 | Local quality gates until Go-Live; hosted CI deferred | Superseded | 2026-11-12 | ADR-0012 | [adr-0013.md](adr-0013.md) |
| ADR-0014 | Makefile task runner and project-local development data | Accepted | 2026-11-12 | — | [adr-0014.md](adr-0014.md) |
| ADR-0015 | Strict settings file and atomic in-place editing | Superseded | 2026-11-12 | — | [adr-0015.md](adr-0015.md) |
| ADR-0016 | SQLite schema v1 and the synchronous database adapter | Accepted | 2026-11-12 | — | [adr-0016.md](adr-0016.md) |
| ADR-0017 | Temporary vendored sqlx-sqlite patch for RUSTSEC-2026-0459 | Accepted | 2026-11-12 | — | [adr-0017.md](adr-0017.md) |
| ADR-0018 | Allow the BSD-3-Clause and Zlib transitive licenses | Accepted | 2026-11-12 | — | [adr-0018.md](adr-0018.md) |
| ADR-0019 | Schema v2 for laboratories, agents, and memberships | Accepted | 2026-11-12 | — | [adr-0019.md](adr-0019.md) |
| ADR-0020 | Interactive agent wizard and laboratory skill policy | Accepted | 2026-11-12 | — | [adr-0020.md](adr-0020.md) |
| ADR-0021 | Width-aware aligned tables with stacked fallback | Accepted | 2026-11-12 | — | [adr-0021.md](adr-0021.md) |
| ADR-0022 | Scoped memories as bounded, untrusted prompt context | Accepted | 2026-11-12 | — | [adr-0022.md](adr-0022.md) |
| ADR-0023 | Schema v3 for projects, requests, stories, and planning states | Accepted | 2026-11-12 | — | [adr-0023.md](adr-0023.md) |
| ADR-0024 | Bound confirmations and the active-planning deletion guard | Accepted | 2026-11-12 | — | [adr-0024.md](adr-0024.md) |
| ADR-0025 | Optional project GitHub source with a global GitHub credential | Accepted | 2026-11-12 | — | [adr-0025.md](adr-0025.md) |
| ADR-0026 | Bounded synchronous provider port and planning contract | Accepted | 2026-11-12 | — | [adr-0026.md](adr-0026.md) |
| ADR-0027 | Allow ISC and CDLA-Permissive-2.0 for the TLS dependency graph | Accepted | 2026-11-12 | — | [adr-0027.md](adr-0027.md) |
| ADR-0028 | Content-free interaction metadata (schema v5) | Accepted | 2026-11-12 | — | [adr-0028.md](adr-0028.md) |
| ADR-0029 | Activate hosted CI before the release tag | Accepted | 2026-11-12 | ADR-0013 | [adr-0029.md](adr-0029.md) |
| ADR-0030 | Credentials in the private settings file | Accepted | 2026-11-12 | ADR-0015 | [adr-0030.md](adr-0030.md) |
| ADR-0031 | Repository-local data for debug builds | Accepted | 2026-11-12 | — | [adr-0031.md](adr-0031.md) |
| ADR-0032 | Clone GitHub projects into .workspace | Accepted | 2026-11-12 | — | [adr-0032.md](adr-0032.md) |
| ADR-0033 | Current project context and project-scoped commands | Accepted | 2026-11-12 | — | [adr-0033.md](adr-0033.md) |
| ADR-0034 | Project agent assignments and planning roster | Accepted | 2026-11-12 | — | [adr-0034.md](adr-0034.md) |
| ADR-0035 | Remove manual memory commands | Accepted | 2026-11-12 | — | [adr-0035.md](adr-0035.md) |
| ADR-0036 | Automatic memory generation | Accepted | 2026-11-12 | — | [adr-0036.md](adr-0036.md) |
| ADR-0037 | Tracked feature plans with task checklists | Accepted | 2026-11-13 | — | [adr-0037.md](adr-0037.md) |
| ADR-0038 | Interactive colored terminal dashboards | Accepted | 2026-11-13 | ADR-0005 | [adr-0038.md](adr-0038.md) |
| ADR-0039 | Dashboard-only entry point | Accepted | 2026-11-13 | — | [adr-0039.md](adr-0039.md) |
| ADR-0040 | Track the entire .project directory | Accepted | 2026-11-13 | — | [adr-0040.md](adr-0040.md) |
| ADR-0041 | Optional laboratory description and dashboard actions | Accepted | 2026-11-13 | — | [adr-0041.md](adr-0041.md) |
| ADR-0042 | Reusable form modal for dashboard editing | Accepted | 2026-11-13 | — | [adr-0042.md](adr-0042.md) |
| ADR-0043 | Project editing and full-screen Kanban board | Accepted | 2026-11-13 | — | [adr-0043.md](adr-0043.md) |
| ADR-0044 | Request intake from the dashboards | Accepted | 2026-11-13 | — | [adr-0044.md](adr-0044.md) |
| ADR-0045 | Read-only laboratory floor view | Superseded | 2026-11-13 | — | [adr-0045.md](adr-0045.md) |
| ADR-0046 | Manual laboratory and project memories from the floor | Accepted | 2026-11-13 | — | [adr-0046.md](adr-0046.md) |
| ADR-0047 | Assigned stories start in IN PROGRESS | Accepted | 2026-11-13 | — | [adr-0047.md](adr-0047.md) |
| ADR-0048 | Bounded no-tool story workflow simulation | Superseded | 2026-11-13 | — | [adr-0048.md](adr-0048.md) |
| ADR-0049 | Automatic workflow resume on startup and application logs | Accepted | 2026-11-13 | — | [adr-0049.md](adr-0049.md) |
| ADR-0050 | Real bounded agent execution in project git worktrees | Accepted | 2026-11-13 | — | [adr-0050.md](adr-0050.md) |
| ADR-0051 | Laboratory metrics dashboard | Accepted | 2026-11-13 | — | [adr-0051.md](adr-0051.md) |
| ADR-0052 | Preset agent catalog loaded from a tracked YAML file | Accepted | 2026-11-16 | — | [adr-0052.md](adr-0052.md) |
| ADR-0053 | Laboratory populate screen with a two-pane agent transfer | Accepted | 2026-11-16 | — | [adr-0053.md](adr-0053.md) |
| ADR-0054 | Project source types and isolated folder import | Accepted | 2026-11-16 | — | [adr-0054.md](adr-0054.md) |
| ADR-0055 | Larger preset catalog and populate profile preview | Accepted | 2026-11-16 | — | [adr-0055.md](adr-0055.md) |
| ADR-0056 | Project interaction history with recaps | Accepted | 2026-11-16 | — | [adr-0056.md](adr-0056.md) |
| ADR-0057 | Recurring project task schedules | Accepted | 2026-11-16 | — | [adr-0057.md](adr-0057.md) |
| ADR-0058 | Agent duplication with optional memory preservation | Accepted | 2026-11-17 | — | [adr-0058.md](adr-0058.md) |
| ADR-0059 | Markdown file store for memories and documents | Accepted | 2026-11-17 | — | [adr-0059.md](adr-0059.md) |
| ADR-0060 | Agent definitions from markdown files | Accepted | 2026-11-17 | — | [adr-0060.md](adr-0060.md) |
| ADR-0061 | Background request queue with serial Tech Lead processing | Accepted | 2026-11-17 | — | [adr-0061.md](adr-0061.md) |
| ADR-0062 | Fixed three-room MSX floor with bidirectional Kanban | Accepted | 2026-11-17 | ADR-0045 | [adr-0062.md](adr-0062.md) |
| ADR-0063 | Maintain the Tech Lead skill after every feature | Accepted | 2026-11-17 | — | [adr-0063.md](adr-0063.md) |
| ADR-0064 | Non-blocking startup resume | Accepted | 2026-11-17 | — | [adr-0064.md](adr-0064.md) |
| ADR-0065 | Live Kanban refresh and initial-based view keys | Accepted | 2026-11-17 | — | [adr-0065.md](adr-0065.md) |
| ADR-0066 | Autonomous clarification resolution | Accepted | 2026-11-17 | — | [adr-0066.md](adr-0066.md) |
| ADR-0067 | Autonomous remediation of blocked stories | Superseded | 2026-11-17 | — | [adr-0067.md](adr-0067.md) |
| ADR-0068 | Markdown recap memories for planning and completed cards | Accepted | 2026-11-17 | — | [adr-0068.md](adr-0068.md) |
| ADR-0069 | Close exhausted stories with a Tech Lead compromise and technical debt | Accepted | 2026-11-17 | ADR-0067 | [adr-0069.md](adr-0069.md) |
| ADR-0070 | Integrate DONE stories into develop and materialize a testable sandbox | Accepted | 2026-11-17 | — | [adr-0070.md](adr-0070.md) |

## Supersession map

| Superseded | Superseded by |
|---|---|
| ADR-0006 | ADR-0007 |
| ADR-0012 | ADR-0013 |
| ADR-0013 | ADR-0029 |
| ADR-0015 | ADR-0030 |
| ADR-0005 | ADR-0038 |
| ADR-0048 | ADR-0050 |
| ADR-0045 | ADR-0062 |
| ADR-0067 | ADR-0069 |
