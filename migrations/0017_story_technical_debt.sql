-- Schema v17 (ADR-0067): technical-debt follow-up created when a story
-- exhausts its remediation budget.
--
-- The original story closes with a documented Tech Lead decision. The follow-up
-- is a normal story linked to its parent through `debt_of_story_id`; the column
-- stays null for every other story.

ALTER TABLE stories ADD COLUMN debt_of_story_id TEXT REFERENCES stories (id) ON DELETE SET NULL;

CREATE INDEX idx_stories_debt_of ON stories (debt_of_story_id);
