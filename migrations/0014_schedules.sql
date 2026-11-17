-- Schema v14 (ADR-0057): recurring project task schedules.
--
-- A due schedule creates a normal bounded request. The table stores no prompt,
-- provider response, file content, or command output. `last_error_code` is a
-- stable application code, never free text.

CREATE TABLE schedules (
    id TEXT NOT NULL PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
    agent_id TEXT NOT NULL REFERENCES agents (id),
    title TEXT NOT NULL CHECK (length(trim(title)) BETWEEN 1 AND 100),
    request_text TEXT NOT NULL CHECK (length(request_text) BETWEEN 1 AND 2000),
    interval_seconds INTEGER NOT NULL CHECK (
        interval_seconds IN (86400, 604800, 2592000)
    ),
    enabled INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0, 1)),
    next_run_at TEXT NOT NULL,
    last_run_at TEXT,
    last_request_id TEXT REFERENCES requests (id) ON DELETE SET NULL,
    last_error_code TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
) STRICT;

CREATE INDEX idx_schedules_project ON schedules (project_id);
CREATE INDEX idx_schedules_due ON schedules (enabled, next_run_at);
