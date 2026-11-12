---
name: vlab-adr
description: "Record or revise a VLab technical decision. Use when choosing a technology, dependency, module boundary, schema, command interface, storage layout, security control, or provider contract; also use when an existing decision is questioned or must be superseded."
---

# VLab ADR workflow

Use this skill whenever a change selects or changes a technical direction.
Routine bug fixes with no alternatives do not need an ADR, but anything a future
contributor could reasonably ask "why was this chosen?" about does.

## Read first

- `AGENTS.md`
- `.docs/CONSTITUTION.md` (decision and documentation rules)
- `.docs/adr/registry.md` (existing decisions and next number)

## Procedure

1. **Search before writing.** Check the registry for an existing decision that
   already covers the topic. If it does, follow it. If it is wrong or obsolete,
   do not edit it; supersede it.
2. **State the decision in one sentence.** If the sentence needs "and" more
   than once, split the decision.
3. **Identify real alternatives.** Record at least the credible options,
   including doing nothing or deferring, and why each was rejected.
4. **Create the ADR** at `.docs/adr/adr-NNNN.md`, where `NNNN` is the next
   unused four-digit number.
5. **Update the registry** `.docs/adr/registry.md` in the same change: one row
   with ID, title, status, date, supersession, and relative link.
6. **If superseding**, update the old ADR's `Status` and `Superseded by` fields
   and add the relationship to the registry's supersession map.
7. **Implement only what the ADR authorizes.** If implementation reveals a
   different decision, revise or supersede the ADR before continuing.
8. **Link the ADR** from the pull request or commit description when useful.

## ADR template

```markdown
# ADR-NNNN: Short factual title

- Status: Proposed | Accepted | Superseded | Deprecated | Rejected
- Date: YYYY-MM-DD
- Supersedes: — | ADR-NNNN
- Superseded by: — | ADR-NNNN

## Context

What forces require a decision? Which constraints from `.docs/` apply?

## Decision

What is chosen, stated precisely enough to review. Include limits and
boundaries where they matter.

## Consequences

Positive outcomes, negative trade-offs, and follow-up obligations.

## Alternatives considered

Each credible alternative and why it was not chosen.

## References

Relative links to the documents this decision depends on.
```

## Quality bar

- The decision is specific enough that two reviewers would implement it the
  same way.
- Security, privacy, operability, and testability consequences are explicit.
- No secrets, credentials, or sensitive values appear in the ADR.
- The registry and the file agree on status and links.
- The ADR is written in English.
