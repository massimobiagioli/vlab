-- Schema v1 (WP-2, ADR-0016): application state and the audit-event log.
--
-- STRICT tables reject values of the wrong SQLite storage class. Timestamps
-- are RFC 3339 UTC text; all other values are identifiers, typed codes, or
-- bounded counters. No column stores credentials, prompts, or provider
-- payloads.

CREATE TABLE app_state (
    key TEXT NOT NULL PRIMARY KEY,
    value TEXT NOT NULL,
    updated_at TEXT NOT NULL
) STRICT;

CREATE TABLE events (
    id TEXT NOT NULL PRIMARY KEY,
    run_id TEXT NOT NULL,
    action_id TEXT,
    kind TEXT NOT NULL,
    entity_type TEXT,
    entity_id TEXT,
    outcome TEXT,
    error_code TEXT,
    duration_ms INTEGER,
    attempt INTEGER,
    provider TEXT,
    model TEXT,
    schema_from INTEGER,
    schema_to INTEGER,
    created_at TEXT NOT NULL
) STRICT;

CREATE INDEX idx_events_run_id ON events (run_id);
CREATE INDEX idx_events_entity ON events (entity_type, entity_id);
CREATE INDEX idx_events_created_at ON events (created_at);
