---
name: vlab-sqlite-migration
description: "Change the VLab SQLite schema or repository layer safely. Use when adding tables, columns, indexes, constraints, or migrations, or when modifying persistence queries and transactions."
---

# VLab SQLite migration workflow

Schema changes are high-impact: they can lose data, break old databases, or
weaken security. Follow this skill for every persistence change.

## Read first

- `AGENTS.md`
- `.docs/STACK.md`
- `.docs/SECURITY.md`
- `.docs/adr/adr-0002.md`
- `.docs/adr/registry.md`

## Procedure

1. **Check whether the change is a technical decision.** A new table, column,
   retention rule, or storage location needs an ADR via the `vlab-adr` skill.
2. **Add a new migration.** Use the next number and a descriptive name:
   `migrations/NNNN_short_description.sql`. Never edit an applied migration.
3. **Prefer additive changes.** Add nullable columns or new tables. A
   destructive migration (drop, rename, type change, backfill with loss)
   requires explicit approval, a recovery plan, and an ADR.
4. **Enforce invariants in the schema.** Use `NOT NULL`, `CHECK`, primary keys,
   foreign keys, and unique constraints where they prevent invalid states.
   Keep application-level authorization checks in code as well.
5. **Use cascades deliberately.** Deleting an aggregate may cascade to owned
   rows; reusable profiles must not be silently destroyed. Document the intent
   in the migration.
6. **Index for actual queries.** Add indexes for foreign keys and the
   board/list queries that run on every command. Do not add speculative
   indexes.
7. **Keep queries parameterized.** Never interpolate values into SQL. Keep
   transactions short and make multi-row writes atomic.
8. **Preserve timestamps and formats.** Store timestamps as RFC 3339 UTC text
   and calendar dates as `YYYY-MM-DD`.
9. **Update retention documentation** if the change stores new personal or
   sensitive content.

## Required tests

- Fresh database reaches the latest version.
- Upgrade from the previous version preserves existing rows.
- Re-running migrations is idempotent.
- A database newer than the binary is refused with an actionable error.
- Constraints reject invalid rows: enums, lengths, foreign keys.
- Cascades produce the documented effect.
- A failed multi-row write rolls back completely.
- File-backed tests use temporary directories, never the developer's real data.

## Completion

- Migration is committed with its tests.
- Foreign keys are verified on every connection.
- Lock contention produces an actionable message.
- The ADR registry and retention document are updated when applicable.
- No credentials, prompts, or transcripts are added to storage.
