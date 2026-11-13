//! Kanban queries (WP-4).

use sqlx::SqliteConnection;

use crate::app::kanban::{Board, BoardCounts, Card, StoryDetail, StoryOutcome, StoryState};
use crate::domain::ids::{LabId, ProjectId, RequestId, StoryId};

type OutcomeRow = (
    String,
    String,
    Option<String>,
    String,
    Option<String>,
    Option<String>,
    Option<String>,
);

const SELECT_PROJECT: &str = "\
SELECT p.id, p.name, p.github_source, l.name FROM projects p JOIN labs l ON l.id = p.lab_id \
WHERE p.id = ?1 AND p.lab_id = ?2";

const SELECT_LEAD: &str = "\
SELECT a.name FROM agents a \
JOIN lab_memberships m ON m.agent_id = a.id \
WHERE m.lab_id = ?1 AND a.role = 'lead' AND a.archived_at IS NULL \
ORDER BY a.name COLLATE NOCASE, a.id LIMIT 1";

const SELECT_COUNTS: &str = "\
SELECT state, count(*) FROM stories WHERE project_id = ?1 GROUP BY state";

const SELECT_PENDING: &str = "\
SELECT count(*) FROM requests WHERE project_id = ?1 \
AND status IN ('TODO', 'PLANNING', 'NEEDS_CLARIFICATION', 'FAILED')";

const SELECT_CARDS: &str = "\
SELECT s.id, s.title, s.state, a.name, s.priority, s.due_date, \
       (s.due_date < date('now')) AS overdue \
FROM stories s LEFT JOIN agents a ON a.id = s.assigned_agent_id \
WHERE s.project_id = ?1 ORDER BY s.created_at, s.id";

const SELECT_STORY: &str = "\
SELECT s.id, s.request_id, s.title, s.description, s.state, a.name, s.priority, s.complexity, \
       s.due_date, (s.due_date < date('now')) AS overdue, s.acceptance_criteria_json, \
       s.required_skills_json, s.assignment_rationale \
FROM stories s JOIN projects p ON p.id = s.project_id \
LEFT JOIN agents a ON a.id = s.assigned_agent_id \
WHERE s.id = ?1 AND s.project_id = ?2 AND p.lab_id = ?3";

type StoryRow = (
    String,
    String,
    String,
    String,
    String,
    Option<String>,
    String,
    String,
    String,
    i64,
    String,
    String,
    Option<String>,
);

type CardRow = (String, String, String, Option<String>, String, String, i64);

pub async fn board(
    connection: &mut SqliteConnection,
    project: &ProjectId,
    lab: &LabId,
) -> Result<Option<Board>, sqlx::Error> {
    let row: Option<(String, String, Option<String>, String)> = sqlx::query_as(SELECT_PROJECT)
        .bind(project.as_str())
        .bind(lab.as_str())
        .fetch_optional(&mut *connection)
        .await?;
    let Some((project_id, project_name, github_source, lab_name)) = row else {
        return Ok(None);
    };
    let project_id = ProjectId::parse(&project_id).ok_or(sqlx::Error::RowNotFound)?;

    let lead_name: Option<String> = sqlx::query_scalar(SELECT_LEAD)
        .bind(lab.as_str())
        .fetch_optional(&mut *connection)
        .await?;

    let count_rows: Vec<(String, i64)> = sqlx::query_as(SELECT_COUNTS)
        .bind(project_id.as_str())
        .fetch_all(&mut *connection)
        .await?;
    let mut counts = BoardCounts::default();
    for (state, count) in count_rows {
        let count = u64::try_from(count).unwrap_or(0);
        match state.as_str() {
            "TODO" => counts.todo = count,
            "IN_PROGRESS" => counts.in_progress = count,
            "IN_REVIEW" => counts.in_review = count,
            "BLOCKED" => counts.blocked = count,
            "DONE" => counts.done = count,
            _ => return Err(sqlx::Error::RowNotFound),
        }
    }

    let pending_intake: i64 = sqlx::query_scalar(SELECT_PENDING)
        .bind(project_id.as_str())
        .fetch_one(&mut *connection)
        .await?;

    let card_rows: Vec<CardRow> = sqlx::query_as(SELECT_CARDS)
        .bind(project_id.as_str())
        .fetch_all(&mut *connection)
        .await?;
    let cards = card_rows
        .into_iter()
        .map(
            |(id, title, state, assignee, priority, due_date, overdue)| {
                Ok(Card {
                    id: StoryId::parse(&id).ok_or(sqlx::Error::RowNotFound)?,
                    title,
                    state: StoryState::parse(&state).ok_or(sqlx::Error::RowNotFound)?,
                    assignee,
                    priority,
                    due_date,
                    overdue: overdue != 0,
                })
            },
        )
        .collect::<Result<Vec<_>, sqlx::Error>>()?;

    Ok(Some(Board {
        project_id,
        project_name,
        github_source,
        lab_name,
        lead_name,
        counts,
        pending_intake: u64::try_from(pending_intake).unwrap_or(0),
        cards,
    }))
}

pub async fn story(
    connection: &mut SqliteConnection,
    project: &ProjectId,
    lab: &LabId,
    story: &StoryId,
) -> Result<Option<StoryDetail>, sqlx::Error> {
    let row: Option<StoryRow> = sqlx::query_as(SELECT_STORY)
        .bind(story.as_str())
        .bind(project.as_str())
        .bind(lab.as_str())
        .fetch_optional(&mut *connection)
        .await?;
    let Some((
        id,
        request_id,
        title,
        description,
        state,
        assignee,
        priority,
        complexity,
        due_date,
        overdue,
        criteria,
        skills,
        rationale,
    )) = row
    else {
        return Ok(None);
    };
    let acceptance_criteria: Vec<String> =
        serde_json::from_str(&criteria).map_err(|_| sqlx::Error::RowNotFound)?;
    let required_skills = crate::domain::skills::decode(&skills)
        .map_err(|_| sqlx::Error::RowNotFound)?
        .iter()
        .map(|tag| tag.as_str().to_owned())
        .collect();
    let outcome: Option<OutcomeRow> = sqlx::query_as(
        "SELECT o.work_outcome, o.review_outcome, a.name, o.completed_at, o.branch, \
                o.worktree_path, o.commit_sha \
         FROM story_outcomes o LEFT JOIN agents a ON a.id = o.reviewer_agent_id \
         WHERE o.story_id = ?1",
    )
    .bind(story.as_str())
    .fetch_optional(connection)
    .await?;
    let outcome = outcome.map(
        |(work, review, reviewer, completed_at, branch, worktree_path, commit_sha)| StoryOutcome {
            work,
            review,
            reviewer: reviewer.unwrap_or_else(|| "Unknown reviewer".to_owned()),
            completed_at,
            branch,
            worktree_path,
            commit_sha,
        },
    );
    Ok(Some(StoryDetail {
        id: StoryId::parse(&id).ok_or(sqlx::Error::RowNotFound)?,
        request_id: RequestId::parse(&request_id).ok_or(sqlx::Error::RowNotFound)?,
        title,
        description,
        state: StoryState::parse(&state).ok_or(sqlx::Error::RowNotFound)?,
        assignee,
        priority,
        complexity,
        due_date,
        overdue: overdue != 0,
        acceptance_criteria,
        required_skills,
        rationale,
        outcome,
    }))
}
