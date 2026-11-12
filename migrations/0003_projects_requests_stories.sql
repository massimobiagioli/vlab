-- Schema v3 (WP-4, ADR-0023): projects, requests, stories, and scoped memories.
--
-- Planning rows exist before the provider integration so WP-5 persists into a
-- tested schema. Memories are user-authored context with exactly one owner.

CREATE TABLE projects (
    id TEXT NOT NULL PRIMARY KEY,
    lab_id TEXT NOT NULL REFERENCES labs (id) ON DELETE CASCADE,
    name TEXT NOT NULL CHECK (length(trim(name)) BETWEEN 1 AND 100),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
) STRICT;

CREATE TABLE requests (
    id TEXT NOT NULL PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
    content TEXT NOT NULL,
    status TEXT NOT NULL CHECK (
        status IN ('TODO', 'PLANNING', 'PLANNED', 'NEEDS_CLARIFICATION', 'FAILED')
    ),
    questions_json TEXT,
    error_code TEXT,
    error_message TEXT,
    planning_started_at TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
) STRICT;

CREATE TABLE stories (
    id TEXT NOT NULL PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
    request_id TEXT NOT NULL REFERENCES requests (id) ON DELETE CASCADE,
    title TEXT NOT NULL,
    description TEXT NOT NULL,
    acceptance_criteria_json TEXT NOT NULL,
    priority TEXT NOT NULL CHECK (priority IN ('low', 'medium', 'high')),
    complexity TEXT NOT NULL CHECK (complexity IN ('small', 'medium', 'large')),
    required_skills_json TEXT NOT NULL,
    assigned_agent_id TEXT REFERENCES agents (id) ON DELETE SET NULL,
    assignment_rationale TEXT,
    state TEXT NOT NULL CHECK (
        state IN ('TODO', 'IN_PROGRESS', 'IN_REVIEW', 'BLOCKED', 'DONE')
    ),
    due_date TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
) STRICT;

CREATE TABLE memories (
    id TEXT NOT NULL PRIMARY KEY,
    scope_type TEXT NOT NULL CHECK (scope_type IN ('lab', 'project', 'agent')),
    lab_id TEXT REFERENCES labs (id) ON DELETE CASCADE,
    project_id TEXT REFERENCES projects (id) ON DELETE CASCADE,
    agent_id TEXT REFERENCES agents (id) ON DELETE CASCADE,
    content TEXT NOT NULL,
    created_at TEXT NOT NULL,
    CHECK (
        (scope_type = 'lab' AND lab_id IS NOT NULL AND project_id IS NULL AND agent_id IS NULL)
        OR (scope_type = 'project' AND lab_id IS NULL AND project_id IS NOT NULL AND agent_id IS NULL)
        OR (scope_type = 'agent' AND lab_id IS NULL AND project_id IS NULL AND agent_id IS NOT NULL)
    )
) STRICT;

CREATE INDEX idx_projects_lab ON projects (lab_id);
CREATE INDEX idx_requests_project ON requests (project_id);
CREATE INDEX idx_requests_project_status ON requests (project_id, status);
CREATE INDEX idx_stories_project_state ON stories (project_id, state);
CREATE INDEX idx_stories_request ON stories (request_id);
CREATE INDEX idx_stories_assigned_agent ON stories (assigned_agent_id);
CREATE INDEX idx_memories_scope_lab ON memories (scope_type, lab_id);
CREATE INDEX idx_memories_scope_project ON memories (scope_type, project_id);
CREATE INDEX idx_memories_scope_agent ON memories (scope_type, agent_id);
CREATE INDEX idx_memories_created ON memories (created_at);
