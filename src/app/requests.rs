//! Request intake and the planning state machine (WP-5-T8/T9).

use crate::app::AppError;
use crate::app::directory::Directory;
use crate::app::invocation::{self, InvocationKind, InvocationStore};
use crate::app::memories::MemoryStore;
use crate::app::planning::prompt::{self, PlanningContext, PlanningMember};
use crate::app::planning::rules;
use crate::app::planning::schema;
use crate::app::planning::{PlannedStoryRow, PlanningStore};
use crate::app::projects::ProjectStore;
use crate::app::storage::AppStateStore;
use crate::domain::budget::{Budget, CancelToken, ExecutionLimits};
use crate::domain::clock::Clock;
use crate::domain::ids::{ProjectId, RequestId, StoryId};
use crate::domain::memory::MemoryScope;
use crate::domain::provider::{ModelProvider, ProviderError};
use crate::domain::settings::Settings;
use crate::obs::events::{AuditEvent, EntityRef, EntityType, EventKind, Outcome};
use crate::obs::{ActionId, RunId};

pub const STALE_PLANNING_SECONDS: i64 = 240;

fn contains_secret(value: &str, secret: Option<&str>) -> bool {
    secret.is_some_and(|secret| !secret.is_empty() && value.contains(secret))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlanningOutcome {
    Planned {
        request_id: RequestId,
        stories: Vec<PlannedStoryRow>,
    },
    NeedsClarification {
        request_id: RequestId,
        questions: Vec<String>,
    },
    Failed {
        request_id: RequestId,
        code: &'static str,
        hint: &'static str,
        validation: bool,
    },
    Cancelled {
        request_id: RequestId,
    },
}

/// Runs one bounded planning operation. The request row is written before the
/// provider is called and is always preserved.
#[allow(clippy::too_many_arguments)]
pub fn plan(
    store: &(impl PlanningStore + Directory + ProjectStore + MemoryStore + InvocationStore),
    state: &impl AppStateStore,
    project_id: &ProjectId,
    request_text: &str,
    settings: &Settings,
    provider: &impl ModelProvider,
    limits: ExecutionLimits,
    clock: &impl Clock,
    cancel: CancelToken,
    run_id: RunId,
) -> Result<PlanningOutcome, AppError> {
    if request_text.trim().is_empty() {
        return Err(AppError::Usage {
            message: "a request must contain non-empty text.".to_owned(),
        });
    }
    if contains_secret(
        request_text,
        settings.credentials.deepseek_api_key.as_deref(),
    ) {
        return Err(AppError::Validation {
            problem: "the request contains a value that looks like the provider credential."
                .to_owned(),
            hint: "Remove the credential from the request and try again.".to_owned(),
        });
    }
    if request_text.len() > limits.request_max_bytes {
        return Err(AppError::Usage {
            message: format!(
                "a request may contain at most {} KiB.",
                limits.request_max_bytes / 1024
            ),
        });
    }
    let lab_id = crate::app::context::active_lab(state)?.require()?;
    let project = store
        .project_in_lab(project_id, &lab_id)?
        .ok_or_else(|| AppError::NotFound {
            entity: crate::app::EntityKind::Project,
            id: project_id.to_string(),
        })?;
    let lab = store.lab(&lab_id)?.ok_or_else(|| AppError::NotFound {
        entity: crate::app::EntityKind::Laboratory,
        id: lab_id.to_string(),
    })?;
    let members = store.lab_agents(&lab_id)?;
    let check = crate::app::lab_validation::check(&lab, &members);
    if !check.valid() {
        return Err(AppError::Validation {
            problem: "the laboratory is not ready for planning.".to_owned(),
            hint: "Run `vlab --check-lab <lab-id>` and fix the failed rules.".to_owned(),
        });
    }
    let lead = members
        .iter()
        .filter(|member| !member.archived)
        .find(|member| {
            member
                .role()
                .is_ok_and(|role| role == crate::domain::agent::Role::Lead)
        })
        .cloned()
        .ok_or_else(|| AppError::Validation {
            problem: "the laboratory needs a Tech Lead before planning.".to_owned(),
            hint: "Run `vlab --check-lab <lab-id>` and add a lead.".to_owned(),
        })?;
    let _ = store.recover_stale_planning(STALE_PLANNING_SECONDS)?;

    let request_id = RequestId::generate();
    store.insert_request(&request_id, project_id, request_text)?;
    store.mark_planning(&request_id)?;
    record(
        store,
        run_id,
        EventKind::PlanningStarted,
        Outcome::Success,
        Some(&request_id),
        None,
        None,
    );
    if cancel.is_cancelled() {
        finish_failed(
            store,
            &request_id,
            "cancelled",
            "the operation was cancelled.",
        )?;
        record(
            store,
            run_id,
            EventKind::PlanningFailed,
            Outcome::Cancelled,
            Some(&request_id),
            Some("cancelled"),
            None,
        );
        return Ok(PlanningOutcome::Cancelled { request_id });
    }

    let created_at = store
        .request_created_at(&request_id)?
        .unwrap_or_else(|| "1970-01-01T00:00:00Z".to_owned());
    let lab_memories = store.memories(&MemoryScope::Lab(lab_id.clone()))?;
    let project_memories = store.memories(&MemoryScope::Project(project_id.clone()))?;
    let lead_memories = store.memories(&MemoryScope::Agent(lead.id.clone()))?;
    if lab_memories
        .iter()
        .chain(project_memories.iter())
        .chain(lead_memories.iter())
        .any(|record| {
            contains_secret(
                &record.memory.content,
                settings.credentials.deepseek_api_key.as_deref(),
            )
        })
    {
        finish_failed(
            store,
            &request_id,
            "validation",
            "a selected memory contains the provider credential.",
        )?;
        record(
            store,
            run_id,
            EventKind::PlanningFailed,
            Outcome::Failure,
            Some(&request_id),
            Some("validation"),
            None,
        );
        return Ok(PlanningOutcome::Failed {
            request_id,
            code: "validation",
            hint: "remove credentials from memories before planning.",
            validation: true,
        });
    }
    let mut planning_members = Vec::new();
    for member in members.iter().filter(|member| !member.archived) {
        planning_members.push(PlanningMember {
            id: member.id.to_string(),
            name: member.name.clone(),
            role: member.role.clone(),
            skills: member
                .skills()
                .map(|tags| tags.iter().map(|tag| tag.as_str().to_owned()).collect())
                .unwrap_or_default(),
            open_stories: store.agent_open_story_count(&member.id)?,
        });
    }
    let prompt = prompt::build(&PlanningContext {
        project_id: project.id.to_string(),
        project_name: project.name,
        lab_name: lab.name,
        lead_name: Some(lead.name.clone()),
        members: planning_members,
        lab_memories,
        project_memories,
        lead_memories,
        request_text: request_text.to_owned(),
        request_date: created_at.chars().take(10).collect(),
        model: settings.model.to_string(),
        max_output_tokens: limits.max_output_tokens,
    });

    let cancel_check = cancel.clone();
    let mut budget = Budget::new(limits, cancel);
    let result = invocation::invoke(
        store,
        &lead.id,
        InvocationKind::Planning,
        settings.provider.as_str(),
        provider,
        &prompt,
        &mut budget,
        clock,
        run_id,
    );

    let response = match result {
        Ok(response) => {
            if cancel_check.is_cancelled() {
                finish_failed(
                    store,
                    &request_id,
                    "cancelled",
                    "the operation was cancelled.",
                )?;
                record(
                    store,
                    run_id,
                    EventKind::PlanningFailed,
                    Outcome::Cancelled,
                    Some(&request_id),
                    Some("cancelled"),
                    None,
                );
                return Ok(PlanningOutcome::Cancelled { request_id });
            }
            response
        }
        Err(ProviderError::Cancelled) => {
            finish_failed(
                store,
                &request_id,
                "cancelled",
                "the operation was cancelled.",
            )?;
            record(
                store,
                run_id,
                EventKind::PlanningFailed,
                Outcome::Cancelled,
                Some(&request_id),
                Some("cancelled"),
                None,
            );
            return Ok(PlanningOutcome::Cancelled { request_id });
        }
        Err(error) => {
            let (code, hint) = provider_failure(&error);
            finish_failed(store, &request_id, code, hint)?;
            record(
                store,
                run_id,
                EventKind::PlanningFailed,
                Outcome::Failure,
                Some(&request_id),
                Some(code),
                None,
            );
            return Ok(PlanningOutcome::Failed {
                request_id,
                code,
                hint,
                validation: false,
            });
        }
    };

    if contains_secret(
        &response.content,
        settings.credentials.deepseek_api_key.as_deref(),
    ) {
        finish_failed(
            store,
            &request_id,
            "validation",
            "the model output contained a value that looks like the provider credential.",
        )?;
        record(
            store,
            run_id,
            EventKind::PlanningFailed,
            Outcome::Failure,
            Some(&request_id),
            Some("validation"),
            None,
        );
        return Ok(PlanningOutcome::Failed {
            request_id,
            code: "validation",
            hint: "remove credentials from the provider response.",
            validation: true,
        });
    }
    let validated = match schema::validate(&response.content) {
        Ok(plan) => plan,
        Err(error) => {
            finish_failed(
                store,
                &request_id,
                "validation",
                "the model output did not pass local validation.",
            )?;
            record(
                store,
                run_id,
                EventKind::PlanningFailed,
                Outcome::Failure,
                Some(&request_id),
                Some("validation"),
                None,
            );
            return Ok(PlanningOutcome::Failed {
                request_id,
                code: "validation",
                hint: error.rule,
                validation: true,
            });
        }
    };
    if !validated.questions.is_empty() {
        store.finish_clarification(&request_id, &validated.questions)?;
        record(
            store,
            run_id,
            EventKind::PlanningCompleted,
            Outcome::Success,
            Some(&request_id),
            None,
            None,
        );
        return Ok(PlanningOutcome::NeedsClarification {
            request_id,
            questions: validated.questions,
        });
    }
    let applied = match rules::apply(validated, &members, &created_at) {
        Ok(stories) => stories,
        Err(error) => {
            finish_failed(
                store,
                &request_id,
                "validation",
                "the model output did not pass local assignment or due-date validation.",
            )?;
            record(
                store,
                run_id,
                EventKind::PlanningFailed,
                Outcome::Failure,
                Some(&request_id),
                Some("validation"),
                None,
            );
            return Ok(PlanningOutcome::Failed {
                request_id,
                code: "validation",
                hint: error.rule,
                validation: true,
            });
        }
    };
    let stories: Vec<PlannedStoryRow> = applied
        .into_iter()
        .map(|applied| PlannedStoryRow {
            id: StoryId::generate(),
            title: applied.story.title,
            description: applied.story.description,
            acceptance_criteria: applied.story.acceptance_criteria,
            priority: applied.story.priority.as_str().to_owned(),
            complexity: applied.story.complexity.as_str().to_owned(),
            required_skills: applied
                .story
                .required_skills
                .iter()
                .map(|skill| skill.as_str().to_owned())
                .collect(),
            assigned_agent_id: applied
                .story
                .assigned_agent_id
                .map(|agent| agent.to_string()),
            assignment_rationale: applied.assignment_rationale,
            due_date: applied.due_date,
        })
        .collect();
    store.finish_planned(&request_id, project_id, &stories)?;
    record(
        store,
        run_id,
        EventKind::PlanningCompleted,
        Outcome::Success,
        Some(&request_id),
        None,
        None,
    );
    Ok(PlanningOutcome::Planned {
        request_id,
        stories,
    })
}

fn finish_failed(
    store: &impl PlanningStore,
    request_id: &RequestId,
    code: &str,
    message: &str,
) -> Result<(), AppError> {
    store.finish_failed(request_id, code, message)?;
    Ok(())
}

fn record(
    store: &impl PlanningStore,
    run_id: RunId,
    kind: EventKind,
    outcome: Outcome,
    request: Option<&RequestId>,
    error_code: Option<&'static str>,
    attempt: Option<u32>,
) {
    // The planning store is the database adapter in production; event
    // persistence is best effort and never changes the terminal outcome.
    let mut event = AuditEvent::new(run_id, ActionId::generate(), kind, outcome);
    if let Some(request) = request
        && let Some(entity) = EntityRef::new(EntityType::Request, request.as_str())
    {
        event = event.entity(entity);
    }
    if let Some(code) = error_code {
        event = event.error_code(code);
    }
    if let Some(attempt) = attempt {
        event = event.attempt(attempt);
    }
    let _ = store.record_event(&event);
}

fn provider_failure(error: &ProviderError) -> (&'static str, &'static str) {
    match error {
        ProviderError::Authentication => (
            "provider_authentication",
            "check credentials.deepseek_api_key in settings.yaml.",
        ),
        ProviderError::RateLimited { .. } => ("provider_rate_limited", "retry later."),
        ProviderError::Timeout => ("provider_timeout", "retry later."),
        ProviderError::Transport => ("provider_transport", "check the network connection."),
        ProviderError::Server { .. } => ("provider_server", "retry later."),
        ProviderError::InvalidResponse => {
            ("provider_invalid_response", "submit a revised request.")
        }
        ProviderError::ResponseTooLarge => (
            "provider_response_too_large",
            "retry with a smaller request.",
        ),
        ProviderError::Cancelled => ("cancelled", "the operation was cancelled."),
        ProviderError::Configuration => ("provider_configuration", "check the provider settings."),
        ProviderError::BudgetExceeded => ("budget_exceeded", "retry later; the budget expired."),
    }
}

/// Uses the planning store's connection for event persistence when available.
pub fn recover_stale(store: &impl PlanningStore) -> Result<u64, AppError> {
    Ok(store.recover_stale_planning(STALE_PLANNING_SECONDS)?)
}

/// Default clock for production calls.
pub fn system_clock() -> crate::domain::clock::SystemClock {
    crate::domain::clock::SystemClock
}
