-- Schema v8 (ADR-0047): assigned stories start in progress.
--
-- Planning used to insert every story as TODO. The new policy is that an
-- assigned story is ready to be worked, so existing assigned TODO rows are
-- moved once. Unassigned stories remain TODO.

UPDATE stories
SET state = 'IN_PROGRESS'
WHERE state = 'TODO' AND assigned_agent_id IS NOT NULL;
