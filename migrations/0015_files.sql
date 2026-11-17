-- Schema v15 (ADR-0059): file-backed memories and documents.
--
-- Memory content moves to markdown files under the data directory. The
-- `memories.path` column stores a path relative to that directory; the legacy
-- `content` column is retained but always empty after materialization. Entity
-- folders are allocated once so duplicate names can coexist with an ID suffix.
-- Documents are markdown files with the same scope kinds as memories.

ALTER TABLE memories ADD COLUMN path TEXT;

CREATE TABLE entity_folders (
    kind TEXT NOT NULL CHECK (kind IN ('agent', 'project', 'lab')),
    entity_id TEXT NOT NULL,
    folder TEXT NOT NULL,
    PRIMARY KEY (kind, entity_id),
    UNIQUE (kind, folder)
) STRICT;

CREATE INDEX idx_entity_folders_entity ON entity_folders (kind, entity_id);

CREATE TABLE documents (
    id TEXT NOT NULL PRIMARY KEY,
    scope_type TEXT NOT NULL CHECK (scope_type IN ('agent', 'project', 'lab')),
    agent_id TEXT REFERENCES agents (id) ON DELETE CASCADE,
    project_id TEXT REFERENCES projects (id) ON DELETE CASCADE,
    lab_id TEXT REFERENCES labs (id) ON DELETE CASCADE,
    path TEXT NOT NULL UNIQUE,
    title TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    CHECK (
        (scope_type = 'agent' AND agent_id IS NOT NULL AND project_id IS NULL AND lab_id IS NULL)
        OR (scope_type = 'project' AND agent_id IS NULL AND project_id IS NOT NULL AND lab_id IS NULL)
        OR (scope_type = 'lab' AND agent_id IS NULL AND project_id IS NULL AND lab_id IS NOT NULL)
    )
) STRICT;

CREATE INDEX idx_documents_agent ON documents (agent_id);
CREATE INDEX idx_documents_project ON documents (project_id);
CREATE INDEX idx_documents_lab ON documents (lab_id);
