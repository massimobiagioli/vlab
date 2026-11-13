-- Schema v10 (ADR-0050): real worktree execution state.
--
-- `stories.execution_attempts` bounds paid retries. Outcome rows record the
-- branch, worktree, and commit produced by real work. File content and command
-- output are never stored in these columns.

ALTER TABLE stories ADD COLUMN execution_attempts INTEGER NOT NULL DEFAULT 0;
ALTER TABLE story_outcomes ADD COLUMN branch TEXT;
ALTER TABLE story_outcomes ADD COLUMN worktree_path TEXT;
ALTER TABLE story_outcomes ADD COLUMN commit_sha TEXT;
