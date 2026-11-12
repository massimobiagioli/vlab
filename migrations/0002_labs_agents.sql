-- Schema v2 (WP-3, ADR-0019): laboratories, agent profiles, and memberships.
--
-- Names and roles are constrained in SQL as a second line of defence after the
-- domain rules. `skills_json` is a bounded JSON array of lowercase slug tags.
-- The one-Tech-Lead rule spans two tables and is enforced by the application
-- before insert; `--check-lab` reports violations found in stored data.

CREATE TABLE labs (
    id TEXT NOT NULL PRIMARY KEY,
    name TEXT NOT NULL CHECK (length(trim(name)) BETWEEN 1 AND 100),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
) STRICT;

CREATE TABLE agents (
    id TEXT NOT NULL PRIMARY KEY,
    name TEXT NOT NULL CHECK (length(trim(name)) BETWEEN 1 AND 100),
    role TEXT NOT NULL CHECK (role IN ('lead', 'dev', 'analyst', 'qa')),
    description TEXT NOT NULL,
    skills_json TEXT NOT NULL,
    provider TEXT NOT NULL,
    model TEXT NOT NULL,
    cv TEXT,
    archived_at TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
) STRICT;

CREATE TABLE lab_memberships (
    lab_id TEXT NOT NULL REFERENCES labs (id) ON DELETE CASCADE,
    agent_id TEXT NOT NULL REFERENCES agents (id) ON DELETE CASCADE,
    joined_at TEXT NOT NULL,
    PRIMARY KEY (lab_id, agent_id)
) STRICT;

CREATE INDEX idx_lab_memberships_agent ON lab_memberships (agent_id);
CREATE INDEX idx_agents_active ON agents (archived_at) WHERE archived_at IS NULL;
