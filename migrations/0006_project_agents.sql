-- Schema v6 (ADR-0034): project-specific agent roster.

CREATE TABLE project_agents (
    project_id TEXT NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
    agent_id TEXT NOT NULL REFERENCES agents (id) ON DELETE CASCADE,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    PRIMARY KEY (project_id, agent_id)
) STRICT;

CREATE INDEX idx_project_agents_agent ON project_agents (agent_id);
