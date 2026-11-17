//! Recurring project task schedules (ADR-0057).
//!
//! A due schedule creates a normal bounded request and runs the existing
//! planning/workflow path. The schedule table stores no prompt, response, file,
//! or command content beyond the operator-provided task text.

use crate::app::AppError;
use crate::app::directory::{AgentRecord, Directory};
use crate::app::invocation::InvocationStore;
use crate::app::memories::MemoryStore;
use crate::app::planning::PlanningStore;
use crate::app::projects::ProjectStore;
use crate::app::requests::{self, PlanningOutcome};
use crate::app::storage::{AppStateStore, StorageError};
use crate::app::workflow::{self, WorkflowStore};
use crate::app::worktrees::WorkflowEnvironment;
use crate::domain::agent::Description;
use crate::domain::budget::{CancelToken, ExecutionLimits};
use crate::domain::clock::Clock;
use crate::domain::ids::{
    AgentId, LabId, ProjectId, RequestId, ScheduleId, UniqueIdError, unique_short_id,
};
use crate::domain::name::Name;
use crate::domain::provider::ModelProvider;
use crate::domain::schedule::{Schedule, ScheduleInterval};
use crate::domain::settings::Settings;
use crate::obs::RunId;

/// At most this many due schedules run in one pass.
pub const MAX_DUE_PER_RUN: usize = 3;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectSchedules {
    pub project_name: String,
    pub schedules: Vec<Schedule>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ScheduleRunReport {
    pub ran: usize,
    pub failed: usize,
    pub remaining: usize,
}

/// Persistence for project schedules.
pub trait ScheduleStore {
    /// Returns `None` when the project does not belong to `lab`.
    fn project_schedules(
        &self,
        project: &ProjectId,
        lab: &LabId,
    ) -> Result<Option<ProjectSchedules>, StorageError>;

    /// Due, enabled schedules for one project. The caller bounds the batch.
    fn due_schedules(&self, project: &ProjectId) -> Result<Vec<Schedule>, StorageError>;

    fn schedule(&self, id: &ScheduleId) -> Result<Option<Schedule>, StorageError>;

    fn insert_schedule(&self, schedule: &Schedule) -> Result<(), StorageError>;

    fn set_schedule_enabled(&self, id: &ScheduleId, enabled: bool) -> Result<(), StorageError>;

    fn delete_schedule(&self, id: &ScheduleId) -> Result<(), StorageError>;

    /// Sets `last_run_at` and advances `next_run_at` by the interval.
    fn record_schedule_run(
        &self,
        id: &ScheduleId,
        request: Option<&RequestId>,
        error_code: Option<&str>,
    ) -> Result<(), StorageError>;
}

/// Lists schedules for a project of the active laboratory.
pub fn show(
    store: &impl ScheduleStore,
    state: &impl AppStateStore,
    project: &ProjectId,
) -> Result<ProjectSchedules, AppError> {
    let lab = crate::app::context::active_lab(state)?.require()?;
    store
        .project_schedules(project, &lab)?
        .ok_or_else(|| AppError::NotFound {
            entity: crate::app::EntityKind::Project,
            id: project.to_string(),
        })
}

/// Creates a schedule owned by an active laboratory member. The first run is
/// one interval after creation.
pub fn create(
    store: &(impl ScheduleStore + Directory),
    state: &impl AppStateStore,
    project: &ProjectId,
    agent: &AgentId,
    raw_title: &str,
    raw_request: &str,
    interval: ScheduleInterval,
) -> Result<Schedule, AppError> {
    let lab = crate::app::context::active_lab(state)?.require()?;
    store
        .project_schedules(project, &lab)?
        .ok_or_else(|| AppError::NotFound {
            entity: crate::app::EntityKind::Project,
            id: project.to_string(),
        })?;
    let title = Name::parse(raw_title).map_err(|error| AppError::Validation {
        problem: error.to_string(),
        hint: "Use a short schedule title.".to_owned(),
    })?;
    let request_text = Description::parse(raw_request).map_err(|error| AppError::Validation {
        problem: error.to_string(),
        hint: "Use a single-line task description of at most 2000 characters.".to_owned(),
    })?;
    let members = store.lab_agents(&lab)?;
    if !members
        .iter()
        .any(|member| !member.archived && &member.id == agent)
    {
        return Err(AppError::Precondition {
            problem: "the schedule owner is not an active laboratory member.".to_owned(),
            hint: "Choose an active member of the project's laboratory.".to_owned(),
        });
    }
    let generated = unique_short_id(|candidate| match ScheduleId::parse(candidate) {
        Some(id) => Ok(store.schedule(&id)?.is_some()),
        None => Ok(true),
    })
    .map_err(map_unique)?;
    let id = ScheduleId::parse(&generated).ok_or(AppError::Internal)?;
    let schedule = Schedule {
        id: id.clone(),
        project_id: project.clone(),
        agent_id: agent.clone(),
        title,
        request_text,
        interval,
        enabled: true,
        next_run_at: String::new(),
        last_run_at: None,
        last_request_id: None,
        last_request_status: None,
        last_error_code: None,
    };
    store.insert_schedule(&schedule)?;
    store.schedule(&id)?.ok_or(AppError::Internal)
}

/// Pauses or resumes one schedule of the active laboratory.
pub fn set_enabled(
    store: &(impl ScheduleStore + ProjectStore),
    state: &impl AppStateStore,
    raw_id: &str,
    enabled: bool,
) -> Result<Schedule, AppError> {
    let id = parse_schedule_id(raw_id)?;
    let lab = crate::app::context::active_lab(state)?.require()?;
    let mut schedule = store.schedule(&id)?.ok_or_else(|| not_found(&id))?;
    require_project(store, &schedule.project_id, &lab)?;
    store.set_schedule_enabled(&id, enabled)?;
    schedule.enabled = enabled;
    Ok(schedule)
}

/// Deletes one schedule of the active laboratory.
pub fn delete(
    store: &(impl ScheduleStore + ProjectStore),
    state: &impl AppStateStore,
    raw_id: &str,
) -> Result<Schedule, AppError> {
    let id = parse_schedule_id(raw_id)?;
    let lab = crate::app::context::active_lab(state)?.require()?;
    let schedule = store.schedule(&id)?.ok_or_else(|| not_found(&id))?;
    require_project(store, &schedule.project_id, &lab)?;
    store.delete_schedule(&id)?;
    Ok(schedule)
}

/// Parses a schedule identifier; malformed input is a usage error.
pub fn parse_schedule_id(raw: &str) -> Result<ScheduleId, AppError> {
    ScheduleId::parse(raw).ok_or_else(|| AppError::Usage {
        message: "a schedule identifier must be 8 lowercase hexadecimal characters.".to_owned(),
    })
}

/// Runs due schedules for one project. Each due schedule creates a normal
/// request and, when planned, runs the bounded workflow.
#[allow(clippy::too_many_arguments)]
pub fn run_due(
    store: &(
         impl ScheduleStore
         + PlanningStore
         + Directory
         + ProjectStore
         + MemoryStore
         + InvocationStore
         + WorkflowStore
         + AppStateStore
     ),
    state: &impl AppStateStore,
    project: &ProjectId,
    settings: &Settings,
    provider: &impl ModelProvider,
    environment: &impl WorkflowEnvironment,
    limits: ExecutionLimits,
    clock: &impl Clock,
    cancel: CancelToken,
    run_id: RunId,
) -> Result<ScheduleRunReport, AppError> {
    let lab = crate::app::context::active_lab(state)?.require()?;
    store
        .project_schedules(project, &lab)?
        .ok_or_else(|| AppError::NotFound {
            entity: crate::app::EntityKind::Project,
            id: project.to_string(),
        })?;
    let due = store.due_schedules(project)?;
    let members: Vec<AgentRecord> = store
        .lab_agents(&lab)?
        .into_iter()
        .filter(|member| !member.archived)
        .collect();
    let mut report = ScheduleRunReport::default();
    let mut processed = 0;
    for schedule in due.iter().take(MAX_DUE_PER_RUN) {
        if cancel.is_cancelled() {
            break;
        }
        processed += 1;
        let Some(member) = members.iter().find(|member| member.id == schedule.agent_id) else {
            store.record_schedule_run(&schedule.id, None, Some("agent_inactive"))?;
            report.failed += 1;
            continue;
        };
        let text = format!(
            "Scheduled task: {} (owner: {}).\n{}",
            schedule.title.as_str(),
            member.name,
            schedule.request_text.as_str()
        );
        match requests::plan(
            store,
            state,
            project,
            &text,
            settings,
            provider,
            limits,
            clock,
            cancel.clone(),
            run_id,
        ) {
            Ok(outcome) => {
                let request_id = outcome.request_id().clone();
                let mut error = None;
                if matches!(outcome, PlanningOutcome::Planned { .. })
                    && let Err(workflow_error) = workflow::run(
                        store,
                        state,
                        project,
                        settings,
                        provider,
                        environment,
                        limits,
                        clock,
                        cancel.clone(),
                        run_id,
                    )
                {
                    error = Some(workflow_error.code());
                }
                let failed = error.is_some() || matches!(outcome, PlanningOutcome::Failed { .. });
                store.record_schedule_run(&schedule.id, Some(&request_id), error)?;
                report.ran += 1;
                if failed {
                    report.failed += 1;
                }
            }
            Err(error) => {
                let _ = store.record_schedule_run(&schedule.id, None, Some(error.code()));
                report.failed += 1;
            }
        }
    }
    report.remaining = due.len().saturating_sub(processed);
    Ok(report)
}

fn require_project(
    store: &impl ProjectStore,
    project: &ProjectId,
    lab: &LabId,
) -> Result<(), AppError> {
    if store.project_in_lab(project, lab)?.is_none() {
        return Err(AppError::NotFound {
            entity: crate::app::EntityKind::Project,
            id: project.to_string(),
        });
    }
    Ok(())
}

fn not_found(id: &ScheduleId) -> AppError {
    AppError::NotFound {
        entity: crate::app::EntityKind::Schedule,
        id: id.to_string(),
    }
}

fn map_unique(error: UniqueIdError<StorageError>) -> AppError {
    match error {
        UniqueIdError::Lookup(storage) => AppError::from(storage),
        UniqueIdError::Exhausted => AppError::Internal,
    }
}
