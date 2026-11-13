-- Schema v11 (ADR-0050): the reviewer's binding decision.
--
-- A rejected review leaves the committed work on the story branch and blocks
-- the story for human attention instead of marking it DONE.

ALTER TABLE story_outcomes ADD COLUMN review_accepted INTEGER NOT NULL DEFAULT 0;
