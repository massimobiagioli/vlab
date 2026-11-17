-- Schema v19 (ADR-0070): do not retroactively publish historical outcomes.
--
-- Cards that reached DONE before develop integration existed were often
-- partial, rejected, or insecure work. They are marked as intentionally not
-- scheduled for a retroactive merge/push. New outcomes default to pending, and
-- explicit reconciliation still works after a manual reset.

UPDATE story_outcomes
SET push_ok = 1,
    integration_error = 'legacy_not_integrated'
WHERE integrated_at IS NULL AND push_ok = 0;
