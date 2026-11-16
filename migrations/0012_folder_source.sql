-- Schema v12 (ADR-0054): optional local folder source for a project.
--
-- A project has at most one source: `github_source`, `folder_source`, or
-- neither. Existing rows keep both columns NULL. The application enforces the
-- single-source invariant; the column length bound is a second line of defence.

ALTER TABLE projects ADD COLUMN folder_source TEXT
    CHECK (folder_source IS NULL OR length(folder_source) BETWEEN 1 AND 4096);
