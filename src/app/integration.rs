//! Develop integration and sandbox reconciliation (ADR-0070).
//!
//! The use case decides which DONE stories still need integration and which
//! projects need a testable checkout. The infrastructure adapter owns git.

use crate::app::AppError;
use crate::app::invocation::InvocationStore;
use crate::app::projects::ProjectStore;
use crate::app::storage::AppStateStore;
use crate::app::workflow::WorkflowStore;
use crate::app::worktrees::{ProjectRepository, RepositoryError};
use crate::domain::ids::{ProjectId, StoryId};
use crate::obs::events::{AuditEvent, EntityRef, EntityType, EventKind, Outcome};
use crate::obs::{ActionId, RunId};

/// Counts from one active-laboratory reconciliation pass.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SyncReport {
    pub projects: usize,
    pub integrated: usize,
    pub push_failed: usize,
    pub failed: usize,
    pub synced: usize,
    pub skipped: usize,
}

impl SyncReport {
    pub fn summary(&self) -> String {
        format!(
            "Sandbox: {} projects, {} integrated, {} push pending, {} failed, {} synced, {} skipped.",
            self.projects,
            self.integrated,
            self.push_failed,
            self.failed,
            self.synced,
            self.skipped
        )
    }
}

/// Integrates pending DONE stories and refreshes the sandbox for every project
/// of the active laboratory.
pub fn sync_active_lab(
    store: &(impl WorkflowStore + ProjectStore + InvocationStore),
    state: &impl AppStateStore,
    repository: &impl ProjectRepository,
    run_id: RunId,
) -> Result<SyncReport, AppError> {
    let lab = crate::app::context::active_lab(state)?.require()?;
    let projects = store.projects(&lab)?;
    let mut report = SyncReport::default();
    for project in projects {
        report.projects += 1;
        if project.source_kind() == "none" {
            report.skipped += 1;
            continue;
        }
        for pending in store.pending_integrations(&project.id)? {
            match repository.integrate_story(
                &project.id,
                &pending.story_id,
                &pending.branch,
                &pending.commit,
            ) {
                Ok(integration) => {
                    let push_ok = !integration.remote || integration.pushed;
                    store.finish_integration(
                        &pending.story_id,
                        Some(&integration.merge_commit),
                        push_ok,
                        (!push_ok).then_some("develop_push_failed"),
                    )?;
                    record_story(
                        store,
                        run_id,
                        EventKind::DevelopIntegrated,
                        Outcome::Success,
                        &pending.story_id,
                        None,
                    );
                    report.integrated += 1;
                    if !push_ok {
                        record_story(
                            store,
                            run_id,
                            EventKind::DevelopPushFailed,
                            Outcome::Failure,
                            &pending.story_id,
                            Some("develop_push_failed"),
                        );
                        report.push_failed += 1;
                    }
                }
                Err(error) => {
                    store.finish_integration(&pending.story_id, None, false, Some(error.code()))?;
                    record_story(
                        store,
                        run_id,
                        EventKind::DevelopIntegrationFailed,
                        Outcome::Failure,
                        &pending.story_id,
                        Some(error.code()),
                    );
                    report.failed += 1;
                }
            }
        }
        match repository.sync_sandbox(&project.id) {
            Ok(_) => {
                record_project(
                    store,
                    run_id,
                    EventKind::SandboxSynced,
                    Outcome::Success,
                    &project.id,
                    None,
                );
                report.synced += 1;
            }
            Err(RepositoryError::NoDevelop) => {
                // A project without a develop branch has no integrated work yet.
                report.skipped += 1;
            }
            Err(error) => {
                record_project(
                    store,
                    run_id,
                    EventKind::SandboxSyncFailed,
                    Outcome::Failure,
                    &project.id,
                    Some(error.code()),
                );
                report.failed += 1;
            }
        }
    }
    Ok(report)
}

fn record_story(
    store: &impl InvocationStore,
    run_id: RunId,
    kind: EventKind,
    outcome: Outcome,
    story: &StoryId,
    error_code: Option<&'static str>,
) {
    let mut event = AuditEvent::new(run_id, ActionId::generate(), kind, outcome);
    if let Some(entity) = EntityRef::new(EntityType::Story, story.as_str()) {
        event = event.entity(entity);
    }
    if let Some(code) = error_code {
        event = event.error_code(code);
    }
    let _ = store.record_event(&event);
}

fn record_project(
    store: &impl InvocationStore,
    run_id: RunId,
    kind: EventKind,
    outcome: Outcome,
    project: &ProjectId,
    error_code: Option<&'static str>,
) {
    let mut event = AuditEvent::new(run_id, ActionId::generate(), kind, outcome);
    if let Some(entity) = EntityRef::new(EntityType::Project, project.as_str()) {
        event = event.entity(entity);
    }
    if let Some(code) = error_code {
        event = event.error_code(code);
    }
    let _ = store.record_event(&event);
}
