-- Schema v20 (ADR-0071): bounded rework from structured review findings.
--
-- `rework_attempts` counts failed attempts that must be reworked (review
-- rejection or work-limit stop). The findings and retest scope are bounded
-- project data used by the next work and review prompts; they are never audit
-- metadata.

ALTER TABLE stories ADD COLUMN rework_attempts INTEGER NOT NULL DEFAULT 0;
ALTER TABLE story_outcomes ADD COLUMN review_findings_json TEXT;
ALTER TABLE story_outcomes ADD COLUMN review_retest TEXT;
