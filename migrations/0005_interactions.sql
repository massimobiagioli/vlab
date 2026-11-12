-- Schema v5 (WP-6): content-free interaction metadata.

CREATE TABLE interactions (
    id TEXT NOT NULL PRIMARY KEY,
    run_id TEXT NOT NULL,
    agent_id TEXT NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN ('planning', 'question')),
    provider TEXT NOT NULL,
    model TEXT NOT NULL,
    outcome TEXT NOT NULL CHECK (outcome IN ('success', 'failure', 'cancelled')),
    error_code TEXT,
    duration_ms INTEGER,
    input_tokens INTEGER,
    output_tokens INTEGER,
    created_at TEXT NOT NULL
) STRICT;

CREATE INDEX idx_interactions_agent ON interactions (agent_id);
CREATE INDEX idx_interactions_created ON interactions (created_at);
