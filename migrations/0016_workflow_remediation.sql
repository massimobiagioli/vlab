-- Schema v16 (ADR-0067): autonomous Tech Lead remediation of blocked stories.
--
-- A blocked story keeps a bounded remediation counter and the latest Tech Lead
-- decision/context. The context is project data and is included in the next
-- execution prompt; it is not audit metadata.

ALTER TABLE stories ADD COLUMN remediation_attempts INTEGER NOT NULL DEFAULT 0;
ALTER TABLE stories ADD COLUMN tech_lead_context TEXT;
