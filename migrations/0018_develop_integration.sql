-- Schema v18 (ADR-0070): develop integration state for each story outcome.
--
-- `integrated_at` is set when the story commit is in the local `develop`
-- branch. `push_ok` is true when `develop` was pushed to the remote, or when
-- the project has no remote. `integration_error` stores a content-free error
-- code for the next retry. Git output and credentials are never stored.

ALTER TABLE story_outcomes ADD COLUMN integrated_at TEXT;
ALTER TABLE story_outcomes ADD COLUMN push_ok INTEGER NOT NULL DEFAULT 0;
ALTER TABLE story_outcomes ADD COLUMN integration_error TEXT;
