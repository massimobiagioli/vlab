-- Schema v13 (ADR-0056): the agent that planned each request.
--
-- The project interaction history shows the actual Tech Lead even if the
-- laboratory lead changes later. Existing requests keep NULL; their planning
-- entries still list the assignees derived from stories.

ALTER TABLE requests ADD COLUMN planned_by_agent_id TEXT REFERENCES agents (id);
