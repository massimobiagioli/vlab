-- Schema v4 (WP-5): optional GitHub source for a project.
--
-- The GitHub credential is never stored in the database; it is read from
-- GITHUB_API_KEY when a GitHub-backed operation is added.

ALTER TABLE projects ADD COLUMN github_source TEXT;
