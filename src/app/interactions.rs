//! Project interaction history (ADR-0056).
//!
//! The history is a read model over persisted project data. It never invents
//! agent conversations: planning, work, and review are the real collaboration
//! events produced by the workflow.

use crate::app::AppError;
use crate::app::storage::{AppStateStore, StorageError};
use crate::domain::ids::{AgentId, LabId, ProjectId, RequestId, StoryId};

/// Longest recap shown for one interaction.
pub const RECAP_MAX_CHARS: usize = 4_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InteractionKind {
    Planning,
    Work,
    Review,
}

impl InteractionKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Planning => "PLANNING",
            Self::Work => "WORK",
            Self::Review => "REVIEW",
        }
    }

    fn rank(self) -> u8 {
        match self {
            Self::Planning => 0,
            Self::Work => 1,
            Self::Review => 2,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InteractionOutcome {
    Success,
    Failure,
    Pending,
}

impl InteractionOutcome {
    pub fn label(self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::Failure => "failure",
            Self::Pending => "pending",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Participant {
    pub id: AgentId,
    pub name: String,
    pub role: String,
}

impl Participant {
    pub fn label(&self) -> String {
        format!("{} ({})", self.name, self.role)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InteractionEntry {
    pub kind: InteractionKind,
    pub outcome: InteractionOutcome,
    pub participants: Vec<Participant>,
    pub at: String,
    pub reference: String,
    pub recap: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectInteractionHistory {
    pub project_id: ProjectId,
    pub project_name: String,
    pub entries: Vec<InteractionEntry>,
}

/// One request as returned by the store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestInteractionRow {
    pub id: RequestId,
    pub content: String,
    pub status: String,
    pub error_code: Option<String>,
    pub questions: Vec<String>,
    pub created_at: String,
    pub planner: Option<Participant>,
}

/// One story as returned by the store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoryInteractionRow {
    pub id: StoryId,
    pub request_id: RequestId,
    pub title: String,
    pub state: String,
    pub created_at: String,
    pub updated_at: String,
    pub assignee: Option<Participant>,
    pub work_outcome: Option<String>,
    pub review_outcome: Option<String>,
    pub reviewer: Option<Participant>,
    pub accepted: Option<bool>,
    pub completed_at: Option<String>,
}

/// Raw project data for the interaction read model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InteractionData {
    pub project_id: ProjectId,
    pub project_name: String,
    pub requests: Vec<RequestInteractionRow>,
    pub stories: Vec<StoryInteractionRow>,
}

/// Persistence for the project interaction read model.
pub trait InteractionStore {
    /// Returns `None` when the project does not belong to `lab`.
    fn project_interaction_data(
        &self,
        project: &ProjectId,
        lab: &LabId,
    ) -> Result<Option<InteractionData>, StorageError>;
}

/// Loads the history for one project of the active laboratory.
pub fn show(
    store: &impl InteractionStore,
    state: &impl AppStateStore,
    project: &ProjectId,
) -> Result<ProjectInteractionHistory, AppError> {
    let lab = crate::app::context::active_lab(state)?.require()?;
    let data = store
        .project_interaction_data(project, &lab)?
        .ok_or_else(|| AppError::NotFound {
            entity: crate::app::EntityKind::Project,
            id: project.to_string(),
        })?;
    Ok(build_history(data))
}

/// Builds the bounded, newest-first interaction history.
pub fn build_history(data: InteractionData) -> ProjectInteractionHistory {
    let mut entries = Vec::new();
    let mut stories_by_request: std::collections::HashMap<&RequestId, Vec<&StoryInteractionRow>> =
        std::collections::HashMap::new();
    for story in &data.stories {
        stories_by_request
            .entry(&story.request_id)
            .or_default()
            .push(story);
    }

    for request in &data.requests {
        let stories = stories_by_request
            .get(&request.id)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        entries.push(planning_entry(request, stories));
    }

    for story in &data.stories {
        if story.assignee.is_some() {
            entries.push(work_entry(story));
        }
        if story.review_outcome.is_some() || story.accepted.is_some() || story.state == "IN_REVIEW"
        {
            entries.push(review_entry(story));
        }
    }

    entries.sort_by(|left, right| {
        right
            .at
            .cmp(&left.at)
            .then_with(|| right.kind.rank().cmp(&left.kind.rank()))
            .then_with(|| left.reference.cmp(&right.reference))
    });
    ProjectInteractionHistory {
        project_id: data.project_id,
        project_name: data.project_name,
        entries,
    }
}

fn planning_entry(
    request: &RequestInteractionRow,
    stories: &[&StoryInteractionRow],
) -> InteractionEntry {
    let mut participants = Vec::new();
    add_participant(&mut participants, request.planner.as_ref());
    for story in stories {
        add_participant(&mut participants, story.assignee.as_ref());
    }
    let (outcome, recap) = match request.status.as_str() {
        "PLANNED" => (InteractionOutcome::Success, planned_recap(request, stories)),
        "FAILED" => (
            InteractionOutcome::Failure,
            format!(
                "Request: {}\nPlanning failed ({}).",
                request.content,
                request.error_code.as_deref().unwrap_or("unknown")
            ),
        ),
        "NEEDS_CLARIFICATION" => (InteractionOutcome::Pending, clarification_recap(request)),
        "TODO" | "PLANNING" => (
            InteractionOutcome::Pending,
            format!("Request: {}\nPlanning pending.", request.content),
        ),
        other => (
            InteractionOutcome::Pending,
            format!("Request: {}\nStatus: {other}.", request.content),
        ),
    };
    InteractionEntry {
        kind: InteractionKind::Planning,
        outcome,
        participants,
        at: request.created_at.clone(),
        reference: format!("Request {}", request.id),
        recap: truncate_recap(recap),
    }
}

fn planned_recap(request: &RequestInteractionRow, stories: &[&StoryInteractionRow]) -> String {
    if stories.is_empty() {
        return format!("Request: {}\nNo stories were planned.", request.content);
    }
    let mut recap = format!("Request: {}\nPlanned stories:", request.content);
    for story in stories {
        let assignee = story
            .assignee
            .as_ref()
            .map(Participant::label)
            .unwrap_or_else(|| "Unassigned".to_owned());
        recap.push_str(&format!("\n- {} → {}", story.title, assignee));
    }
    recap
}

fn clarification_recap(request: &RequestInteractionRow) -> String {
    let mut recap = format!("Request: {}\nClarification needed:", request.content);
    if request.questions.is_empty() {
        recap.push_str("\n- (no questions recorded)");
    } else {
        for question in &request.questions {
            recap.push_str(&format!("\n- {question}"));
        }
    }
    recap
}

fn work_entry(story: &StoryInteractionRow) -> InteractionEntry {
    let assignee = story.assignee.as_ref();
    let (outcome, recap) = match &story.work_outcome {
        Some(work) => (InteractionOutcome::Success, work.clone()),
        None => match story.state.as_str() {
            "TODO" => (
                InteractionOutcome::Pending,
                format!(
                    "Assigned to {}; work has not started.",
                    assignee.map(Participant::label).unwrap_or_default()
                ),
            ),
            "IN_PROGRESS" => (
                InteractionOutcome::Pending,
                "Work in progress; no outcome recorded yet.".to_owned(),
            ),
            "IN_REVIEW" => (
                InteractionOutcome::Pending,
                "Work finished; review pending.".to_owned(),
            ),
            "BLOCKED" => (
                InteractionOutcome::Failure,
                "Blocked; no work outcome recorded.".to_owned(),
            ),
            "DONE" => (InteractionOutcome::Success, "Completed.".to_owned()),
            other => (InteractionOutcome::Pending, format!("State: {other}.")),
        },
    };
    InteractionEntry {
        kind: InteractionKind::Work,
        outcome,
        participants: assignee.cloned().into_iter().collect(),
        at: story
            .completed_at
            .clone()
            .unwrap_or_else(|| story.updated_at.clone()),
        reference: format!("Story {} — {}", story.id, story.title),
        recap: truncate_recap(recap),
    }
}

fn review_entry(story: &StoryInteractionRow) -> InteractionEntry {
    let mut participants = Vec::new();
    add_participant(&mut participants, story.assignee.as_ref());
    add_participant(&mut participants, story.reviewer.as_ref());
    let (outcome, recap) = match &story.review_outcome {
        Some(review) => {
            let decision = match story.accepted {
                Some(true) => "accepted",
                Some(false) => "rejected",
                None => "decision unknown",
            };
            (
                match story.accepted {
                    Some(true) => InteractionOutcome::Success,
                    Some(false) => InteractionOutcome::Failure,
                    None => InteractionOutcome::Pending,
                },
                format!("{review}\nDecision: {decision}."),
            )
        }
        None if story.state == "IN_REVIEW" => {
            (InteractionOutcome::Pending, "Review pending.".to_owned())
        }
        None => (InteractionOutcome::Pending, "Not reviewed yet.".to_owned()),
    };
    InteractionEntry {
        kind: InteractionKind::Review,
        outcome,
        participants,
        at: story
            .completed_at
            .clone()
            .unwrap_or_else(|| story.updated_at.clone()),
        reference: format!("Story {} — {}", story.id, story.title),
        recap: truncate_recap(recap),
    }
}

fn add_participant(participants: &mut Vec<Participant>, participant: Option<&Participant>) {
    let Some(participant) = participant else {
        return;
    };
    if !participants
        .iter()
        .any(|existing| existing.id == participant.id)
    {
        participants.push(participant.clone());
    }
}

fn truncate_recap(mut recap: String) -> String {
    if recap.chars().count() <= RECAP_MAX_CHARS {
        return recap;
    }
    recap = recap.chars().take(RECAP_MAX_CHARS - 1).collect();
    recap.push('…');
    recap
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::ids::{LabId, RequestId, StoryId};

    fn participant(id: &str, name: &str, role: &str) -> Participant {
        Participant {
            id: AgentId::parse(id).unwrap(),
            name: name.to_owned(),
            role: role.to_owned(),
        }
    }

    fn request(status: &str) -> RequestInteractionRow {
        RequestInteractionRow {
            id: RequestId::parse("1c2d3e4f").unwrap(),
            content: "Add a reset flow".to_owned(),
            status: status.to_owned(),
            error_code: None,
            questions: Vec::new(),
            created_at: "2026-01-01T10:00:00Z".to_owned(),
            planner: Some(participant("10000001", "Klaus", "lead")),
        }
    }

    fn story(state: &str, accepted: Option<bool>) -> StoryInteractionRow {
        StoryInteractionRow {
            id: StoryId::parse("2d3e4f50").unwrap(),
            request_id: RequestId::parse("1c2d3e4f").unwrap(),
            title: "Reset link".to_owned(),
            state: state.to_owned(),
            created_at: "2026-01-01T10:01:00Z".to_owned(),
            updated_at: "2026-01-01T10:03:00Z".to_owned(),
            assignee: Some(participant("10000002", "Bram", "dev")),
            work_outcome: Some("Added the reset link and tests.".to_owned()),
            review_outcome: Some("The link expires and tests pass.".to_owned()),
            reviewer: Some(participant("10000003", "Trevor", "qa")),
            accepted,
            completed_at: Some("2026-01-01T10:05:00Z".to_owned()),
        }
    }

    fn data(request: RequestInteractionRow, stories: Vec<StoryInteractionRow>) -> InteractionData {
        InteractionData {
            project_id: ProjectId::parse("0b1c2d3e").unwrap(),
            project_name: "Portal".to_owned(),
            requests: vec![request],
            stories,
        }
    }

    #[test]
    fn planning_work_and_review_entries_carry_participants_and_recaps() {
        let history = build_history(data(request("PLANNED"), vec![story("DONE", Some(true))]));
        assert_eq!(history.project_name, "Portal");
        assert_eq!(history.entries.len(), 3);
        assert_eq!(history.entries[0].kind, InteractionKind::Review);
        assert_eq!(history.entries[1].kind, InteractionKind::Work);
        assert_eq!(history.entries[2].kind, InteractionKind::Planning);
        assert_eq!(history.entries[0].outcome, InteractionOutcome::Success);
        assert_eq!(history.entries[0].participants.len(), 2);
        assert!(history.entries[0].recap.contains("accepted"));
        assert!(history.entries[1].recap.contains("reset link"));
        assert!(history.entries[2].recap.contains("Reset link"));
        assert!(history.entries[2].recap.contains("Bram"));
    }

    #[test]
    fn failed_and_pending_entries_have_deterministic_recaps() {
        let mut failed = request("FAILED");
        failed.error_code = Some("provider_timeout".to_owned());
        let history = build_history(data(failed, Vec::new()));
        assert_eq!(history.entries[0].outcome, InteractionOutcome::Failure);
        assert!(history.entries[0].recap.contains("provider_timeout"));

        let mut pending = request("NEEDS_CLARIFICATION");
        pending.questions = vec!["Which users may reset?".to_owned()];
        let history = build_history(data(pending, Vec::new()));
        assert_eq!(history.entries[0].outcome, InteractionOutcome::Pending);
        assert!(history.entries[0].recap.contains("Which users may reset?"));
    }

    #[test]
    fn a_pending_story_has_work_and_review_placeholders() {
        let mut pending = story("IN_REVIEW", None);
        pending.work_outcome = None;
        pending.review_outcome = None;
        pending.reviewer = None;
        let history = build_history(data(request("PLANNED"), vec![pending]));
        assert_eq!(history.entries.len(), 3);
        assert!(history.entries[0].recap.contains("Review pending"));
        assert!(history.entries[1].recap.contains("review pending"));
        assert_eq!(history.entries[0].outcome, InteractionOutcome::Pending);
    }

    #[test]
    fn recaps_are_capped() {
        let mut long = request("PLANNED");
        long.content = "x".repeat(RECAP_MAX_CHARS + 100);
        let history = build_history(data(long, Vec::new()));
        assert_eq!(history.entries[0].recap.chars().count(), RECAP_MAX_CHARS);
        assert!(history.entries[0].recap.ends_with('…'));
    }

    #[test]
    fn show_requires_an_active_lab_and_resolves_the_project() {
        struct Store(Option<InteractionData>);
        impl InteractionStore for Store {
            fn project_interaction_data(
                &self,
                _: &ProjectId,
                _: &LabId,
            ) -> Result<Option<InteractionData>, StorageError> {
                Ok(self.0.clone())
            }
        }
        struct State(Option<&'static str>);
        impl AppStateStore for State {
            fn get(
                &self,
                _: crate::app::storage::AppStateKey,
            ) -> Result<Option<String>, StorageError> {
                Ok(self.0.map(str::to_owned))
            }
            fn set(
                &self,
                _: crate::app::storage::AppStateKey,
                _: &str,
            ) -> Result<(), StorageError> {
                Ok(())
            }
            fn clear(&self, _: crate::app::storage::AppStateKey) -> Result<(), StorageError> {
                Ok(())
            }
        }
        let project = ProjectId::parse("0b1c2d3e").unwrap();
        let store = Store(Some(data(request("PLANNED"), Vec::new())));
        let history = show(&store, &State(Some("a3f91c07")), &project).unwrap();
        assert_eq!(history.project_name, "Portal");
        assert!(matches!(
            show(&store, &State(None), &project),
            Err(AppError::NoActiveLab)
        ));
    }
}
