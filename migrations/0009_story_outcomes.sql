-- Schema v9 (ADR-0048): simulated story outcomes and workflow interactions.
--
-- The outcome text is project data, not audit metadata. Interactions stay
-- content-free; the only change is the new `workflow` kind.

CREATE TABLE story_outcomes (
    story_id TEXT NOT NULL PRIMARY KEY REFERENCES stories (id) ON DELETE CASCADE,
    work_outcome TEXT NOT NULL,
    review_outcome TEXT NOT NULL,
    reviewer_agent_id TEXT NOT NULL REFERENCES agents (id),
    completed_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
) STRICT;

CREATE TABLE interactions_v9 (
    id TEXT NOT NULL PRIMARY KEY,
    run_id TEXT NOT NULL,
    agent_id TEXT NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN ('planning', 'question', 'workflow')),
    provider TEXT NOT NULL,
    model TEXT NOT NULL,
    outcome TEXT NOT NULL CHECK (outcome IN ('success', 'failure', 'cancelled')),
    error_code TEXT,
    duration_ms INTEGER,
    input_tokens INTEGER,
    output_tokens INTEGER,
    created_at TEXT NOT NULL
) STRICT;

INSERT INTO interactions_v9 (
    id, run_id, agent_id, kind, provider, model, outcome, error_code,
    duration_ms, input_tokens, output_tokens, created_at
)
SELECT
    id, run_id, agent_id, kind, provider, model, outcome, error_code,
    duration_ms, input_tokens, output_tokens, created_at
FROM interactions;

DROP TABLE interactions;
ALTER TABLE interactions_v9 RENAME TO interactions;

CREATE INDEX idx_interactions_agent ON interactions (agent_id);
CREATE INDEX idx_interactions_created ON interactions (created_at);
CREATE INDEX idx_story_outcomes_reviewer ON story_outcomes (reviewer_agent_id);
