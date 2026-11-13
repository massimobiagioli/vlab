-- Schema v7 (ADR-0041): optional laboratory description.

ALTER TABLE labs ADD COLUMN description TEXT NOT NULL DEFAULT '';
