//! Routes a parsed action to its use case and renders the result.

use std::io::{self, IsTerminal, Write};

use unicode_width::UnicodeWidthStr;

use crate::app::ProviderFailure;
use crate::app::agents::wizard;
use crate::app::agents::{self, AgentDetail, AgentListItem};
use crate::app::context::{self, ActiveLab};
use crate::app::deletions::{self, AgentDeletionPlan, LabDeletionPlan, ProjectDeletionPlan};
use crate::app::directory::Directory;
use crate::app::kanban::{self, Board, StoryState};
use crate::app::lab_validation::{LabCheck, RuleResult};
use crate::app::labs::{self, LabListItem, OpenSummary};
use crate::app::memories;
use crate::app::projects::{self, CreatedProject, ProjectOverview, ProjectStore};
use crate::app::requests::{self, PlanningOutcome};
use crate::app::settings::SettingsReport;
use crate::app::workflow;
use crate::app::{AppError, settings, settings_edit};
use crate::cli::args::Action;
use crate::cli::confirm::{self, ConfirmationRequest};
use crate::cli::input;
use crate::cli::prompt::{TerminalEditor, TerminalPrompter};
use crate::cli::render::{
    Column, empty_state, render_table, sanitize, terminal_width, truncate_cell,
};
use crate::cli::{write_stderr, write_stdout};
use crate::domain::agent::AgentProfile;
use crate::domain::budget::{CancelToken, ExecutionLimits};
use crate::domain::memory::MemoryScope;
use crate::infra::config::SettingsFile;
use crate::infra::db::Database;
use crate::infra::paths::Paths;
use crate::infra::provider::deepseek::{AdapterLimits, DEFAULT_ENDPOINT, DeepSeekProvider};
use crate::infra::workspace::{DEFAULT_CLONE_TIMEOUT, GitWorkspace};
use crate::obs::events::{AuditEvent, EntityRef, EntityType, EventKind, EventSink, Outcome};
use crate::obs::{ActionId, RunId};

/// Runs one primary action, writing its result to `stdout` and warnings or
/// prompts to `stderr`.
pub fn dispatch(
    action: Action,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
    run_id: RunId,
) -> Result<(), AppError> {
    match action {
        Action::ShowSettings => {
            let report = settings::show(&settings_file()?)?;
            write_stdout(stdout, &render_settings(&report))
        }
        Action::EditSettings => {
            let editor = TerminalEditor::from_environment()?;
            let report = settings_edit::edit(&settings_file()?, &editor)?;
            let saved = format!("Settings saved.\n\n{}", render_settings(&report));
            write_stdout(stdout, &saved)
        }
        Action::ListLabs => {
            let db = open_database(run_id)?;
            let active = read_active(&db, stderr)?;
            let items = labs::list(&db)?;
            write_stdout(stdout, &render_labs(&items, &active))
        }
        Action::NewLab { name } => {
            // Validate before opening storage: invalid input must not create
            // a database file.
            labs::validated_name(&name)?;
            let mut db = open_database(run_id)?;
            let lab = labs::create(&db, &name)?;
            if let Some(memory) = memories::remember(
                &db,
                &db,
                MemoryScope::Lab(lab.id.clone()),
                &memories::lab_created_content(&lab),
                configured_secret().as_deref(),
            ) {
                record_event(
                    &mut db,
                    run_id,
                    EventKind::MemoryCreated,
                    EntityType::Memory,
                    memory.id.as_str(),
                );
            }
            record_event(
                &mut db,
                run_id,
                EventKind::LabCreated,
                EntityType::Laboratory,
                lab.id.as_str(),
            );
            write_stdout(stdout, &render_created_lab(&lab))
        }
        Action::OpenLab { lab_id } => {
            labs::parse_lab_id(&lab_id)?;
            let mut db = open_database(run_id)?;
            let summary = labs::open(&db, &db, &lab_id)?;
            context::clear_active_project(&db)?;
            record_event(
                &mut db,
                run_id,
                EventKind::LabOpened,
                EntityType::Laboratory,
                summary.lab.id.as_str(),
            );
            write_open_warnings(stderr, &summary)?;
            write_stdout(stdout, &render_open(&summary))
        }
        Action::CheckLab { lab_id } => {
            labs::parse_lab_id(&lab_id)?;
            let db = open_database(run_id)?;
            let mut report = labs::check(&db, &lab_id)?;
            let project_count = db.projects(&report.lab.id)?.len();
            report.rules.push(RuleResult::pass(
                "project references resolve",
                format!("{project_count} projects"),
            ));
            write_stdout(stdout, &render_check(&report))?;
            if report.valid() {
                Ok(())
            } else {
                Err(AppError::Validation {
                    problem: "the laboratory did not pass validation.".to_owned(),
                    hint: "Fix the failed rules, then run `vlab --check-lab <lab-id>` again."
                        .to_owned(),
                })
            }
        }
        Action::NewRequest { text } => {
            let limits = ExecutionLimits::default();
            let text = input::request_text(text.as_deref(), limits)?;
            let db = open_database(run_id)?;
            let project = context::active_project(&db)?.require()?;
            let settings = settings::show(&settings_file()?)?.settings;
            let provider = provider_from_settings(&settings, limits)?;
            let cancel = CancelToken::default();
            let handler_token = cancel.clone();
            let _ = ctrlc::set_handler(move || handler_token.cancel());
            let outcome = requests::plan(
                &db,
                &db,
                &project,
                &text,
                &settings,
                &provider,
                limits,
                &crate::domain::clock::SystemClock,
                cancel.clone(),
                run_id,
            )?;
            if matches!(&outcome, PlanningOutcome::Planned { .. }) {
                match workflow::run(
                    &db,
                    &db,
                    &project,
                    &settings,
                    &provider,
                    limits,
                    &crate::domain::clock::SystemClock,
                    cancel,
                    run_id,
                ) {
                    Ok(report) => write_stdout(
                        stdout,
                        &format!(
                            "Workflow: {} completed, {} failed, {} remaining.\n",
                            report.completed, report.failed, report.remaining
                        ),
                    )?,
                    Err(error) => write_stderr(
                        stderr,
                        &format!(
                            "Warning: the workflow could not start: {} {}\n",
                            error,
                            error.hint()
                        ),
                    )?,
                }
            }
            render_plan_outcome(stdout, stderr, &db, &outcome)
        }
        Action::RunWorkflow => {
            let limits = ExecutionLimits::default();
            let db = open_database(run_id)?;
            let project = context::active_project(&db)?.require()?;
            let settings = settings::show(&settings_file()?)?.settings;
            let provider = provider_from_settings(&settings, limits)?;
            let cancel = CancelToken::default();
            let handler_token = cancel.clone();
            let _ = ctrlc::set_handler(move || handler_token.cancel());
            let report = workflow::run(
                &db,
                &db,
                &project,
                &settings,
                &provider,
                limits,
                &crate::domain::clock::SystemClock,
                cancel,
                run_id,
            )?;
            write_stdout(
                stdout,
                &format!(
                    "Workflow: {} completed, {} failed, {} remaining.\n",
                    report.completed, report.failed, report.remaining
                ),
            )
        }
        Action::ListProjects => {
            let db = open_database(run_id)?;
            let _active = read_active(&db, stderr)?;
            let projects = projects::list(&db, &db)?;
            write_stdout(stdout, &render_projects(&projects))
        }
        Action::NewProject {
            name,
            github_source,
        } => {
            projects::validated_name(&name)?;
            let github_source = match github_source {
                None => (None, None),
                Some(raw) => {
                    let source =
                        crate::domain::project::GithubSource::parse(&raw).map_err(|error| {
                            AppError::Usage {
                                message: error.to_string(),
                            }
                        })?;
                    let settings = settings::show(&settings_file()?)?.settings;
                    if settings.source_control.provider
                        != crate::domain::settings::SourceControlProvider::Github
                    {
                        return Err(AppError::Configuration {
                            problem: "the configured source-control provider is not supported."
                                .to_owned(),
                            hint: "Set `source_control.provider` to `github` with `vlab --edit-settings`."
                                .to_owned(),
                        });
                    }
                    if !settings.credentials.github_present() {
                        return Err(AppError::Configuration {
                            problem: "`credentials.github_api_key` is not set.".to_owned(),
                            hint: "Add credentials.github_api_key to settings.yaml with `vlab --edit-settings`."
                                .to_owned(),
                        });
                    }
                    let token = settings.credentials.github_api_key.clone();
                    (Some(source), token)
                }
            };
            let paths = Paths::from_environment()?;
            let workspace = GitWorkspace::new(
                paths.workspace_dir(),
                github_source.1,
                DEFAULT_CLONE_TIMEOUT,
            );
            let mut db = open_database(run_id)?;
            let created = projects::create(&db, &db, &name, github_source.0, &workspace)?;
            context::save_active_project(&db, &created.project.id)?;
            if let Some(memory) = memories::remember(
                &db,
                &db,
                MemoryScope::Project(created.project.id.clone()),
                &memories::project_created_content(&created.project),
                configured_secret().as_deref(),
            ) {
                record_event(
                    &mut db,
                    run_id,
                    EventKind::MemoryCreated,
                    EntityType::Memory,
                    memory.id.as_str(),
                );
            }
            record_event(
                &mut db,
                run_id,
                EventKind::ProjectCreated,
                EntityType::Project,
                created.project.id.as_str(),
            );
            write_stdout(stdout, &render_created_project(&created))
        }
        Action::OpenProject { project_id } => {
            projects::parse_project_id(&project_id)?;
            let db = open_database(run_id)?;
            let summary = projects::open(&db, &db, &project_id)?;
            write_stdout(stdout, &render_opened_project(&summary))
        }
        Action::ShowProjectKanban => {
            let db = open_database(run_id)?;
            let project = context::active_project(&db)?.require()?;
            let board = kanban::show(&db, &db, project.as_str())?;
            write_stdout(stdout, &render_board(&board, terminal_width()))
        }
        Action::DeleteLab { lab_id, assume_yes } => {
            labs::parse_lab_id(&lab_id)?;
            let mut db = open_database(run_id)?;
            let plan = deletions::preflight_lab(&db, &lab_id)?;
            let mut prompter = TerminalPrompter;
            confirm::confirm(
                &mut prompter,
                &ConfirmationRequest {
                    command: "--delete-lab",
                    summary: lab_delete_summary(&plan),
                    assume_yes,
                },
            )?;
            deletions::delete_lab(&db, &plan)?;
            context::clear_active_project(&db)?;
            record_event(
                &mut db,
                run_id,
                EventKind::LabDeleted,
                EntityType::Laboratory,
                plan.id.as_str(),
            );
            write_stdout(stdout, &render_deleted_lab(&plan))
        }
        Action::AssignAgent { agent_id } => {
            let db = open_database(run_id)?;
            let project = projects::assign_agent(&db, &db, &agent_id)?;
            write_stdout(
                stdout,
                &format!(
                    "Agent assigned.\nProject: {} — {}\nAgent:   {}\n",
                    project.id,
                    sanitize(&project.name),
                    agent_id
                ),
            )
        }
        Action::DeleteProject { assume_yes } => {
            let mut db = open_database(run_id)?;
            let project = context::active_project(&db)?.require()?;
            let plan = deletions::preflight_project(&db, &db, &project)?;
            let mut prompter = TerminalPrompter;
            confirm::confirm(
                &mut prompter,
                &ConfirmationRequest {
                    command: "--delete-project",
                    summary: project_delete_summary(&plan),
                    assume_yes,
                },
            )?;
            deletions::delete_project(&db, &plan)?;
            context::clear_active_project(&db)?;
            record_event(
                &mut db,
                run_id,
                EventKind::ProjectDeleted,
                EntityType::Project,
                plan.id.as_str(),
            );
            write_stdout(stdout, &render_deleted_project(&plan))
        }
        Action::DeleteAgent {
            agent_id,
            assume_yes,
        } => {
            agents::parse_agent_id(&agent_id)?;
            let mut db = open_database(run_id)?;
            let plan = deletions::preflight_agent(&db, &agent_id)?;
            let mut prompter = TerminalPrompter;
            confirm::confirm(
                &mut prompter,
                &ConfirmationRequest {
                    command: "--delete-agent",
                    summary: agent_delete_summary(&plan),
                    assume_yes,
                },
            )?;
            if !plan.lead_labs.is_empty() {
                write_stderr(
                    stderr,
                    &format!(
                        "Warning: this agent is a Tech Lead in {}; those laboratories will need a new lead.\n",
                        plan.lead_labs.len()
                    ),
                )?;
            }
            deletions::delete_agent(&db, &plan)?;
            record_event(
                &mut db,
                run_id,
                EventKind::AgentArchived,
                EntityType::Agent,
                plan.id.as_str(),
            );
            write_stdout(stdout, &render_archived_agent(&plan))
        }
        Action::Ask { question, to_agent } => {
            if let Some(raw) = &to_agent {
                agents::parse_agent_id(raw)?;
            }
            let settings = settings::show(&settings_file()?)?.settings;
            let limits = ExecutionLimits::default();
            let provider = provider_from_settings(&settings, limits)?;
            let cancel = CancelToken::default();
            let handler_token = cancel.clone();
            let _ = ctrlc::set_handler(move || handler_token.cancel());
            let db = open_database(run_id)?;
            let outcome = match crate::app::ask::ask(
                &db,
                &db,
                to_agent.as_deref(),
                &question,
                &settings,
                &provider,
                limits,
                cancel,
                &crate::domain::clock::SystemClock,
                run_id,
            ) {
                Ok(outcome) => outcome,
                Err(AppError::Interrupted) => {
                    write_stderr(
                        stderr,
                        "Warning: provider-side processing may have continued after local cancellation.\n",
                    )?;
                    return Err(AppError::Interrupted);
                }
                Err(error) => return Err(error),
            };
            let text = format!(
                "Agent {} ({}) — model {}\n{}\n",
                sanitize(&outcome.agent_name),
                outcome.agent_id,
                sanitize(&outcome.model),
                sanitize(&outcome.answer)
            );
            write_stdout(stdout, &text)
        }
        Action::ListAgents => {
            let db = open_database(run_id)?;
            // The command requires context; a corrupt selection is warned
            // about and then reported as missing.
            let active = read_active(&db, stderr)?;
            let _lab = active.require()?;
            let records = agents::list(&db, &db)?;
            write_stdout(stdout, &render_agents(&records))
        }
        Action::NewAgent => {
            let mut prompter = TerminalPrompter;
            if !io::stdin().is_terminal() {
                return wizard::ensure_interactive(&prompter);
            }
            let settings = settings::show(&settings_file()?)?.settings;
            let mut db = open_database(run_id)?;
            let profile = wizard::run(&mut prompter, &db, &db, &settings)?;
            let memberships: Vec<crate::domain::ids::LabId> = db
                .agent_memberships(&profile.id)?
                .into_iter()
                .map(|lab| lab.id)
                .collect();
            if let Some(memory) = memories::remember(
                &db,
                &db,
                MemoryScope::Agent(profile.id.clone()),
                &memories::agent_created_content(&profile, &memberships),
                settings.credentials.deepseek_api_key.as_deref(),
            ) {
                record_event(
                    &mut db,
                    run_id,
                    EventKind::MemoryCreated,
                    EntityType::Memory,
                    memory.id.as_str(),
                );
            }
            record_event(
                &mut db,
                run_id,
                EventKind::AgentCreated,
                EntityType::Agent,
                profile.id.as_str(),
            );
            write_stdout(stdout, &render_created_agent(&profile))
        }
        Action::ShowAgentDetail { agent_id } => {
            agents::parse_agent_id(&agent_id)?;
            let db = open_database(run_id)?;
            let detail = agents::detail(&db, &agent_id)?;
            write_stdout(stdout, &render_agent_detail(&detail))
        }
    }
}

fn configured_secret() -> Option<String> {
    settings_file()
        .ok()
        .and_then(|file| crate::app::settings::SettingsStore::load(&file).ok())
        .and_then(|settings| settings.credentials.deepseek_api_key)
}

fn settings_file() -> Result<SettingsFile, AppError> {
    let paths = Paths::from_environment()?;
    Ok(SettingsFile::new(paths.config_dir(), paths.settings_file()))
}

fn open_database(run_id: RunId) -> Result<Database, AppError> {
    let paths = Paths::from_environment()?;
    let (database, _) = Database::open_and_migrate(paths.database_file(), run_id)?;
    Ok(database)
}

fn read_active(db: &Database, stderr: &mut dyn Write) -> Result<ActiveLab, AppError> {
    let active = context::active_lab(db)?;
    if let Some(warning) = active.warning() {
        write_stderr(stderr, &format!("Warning: {}\n", sanitize(warning)))?;
    }
    Ok(active)
}

fn record_event(
    db: &mut Database,
    run_id: RunId,
    kind: EventKind,
    entity_type: EntityType,
    id: &str,
) {
    if let Some(entity) = EntityRef::new(entity_type, id) {
        let event =
            AuditEvent::new(run_id, ActionId::generate(), kind, Outcome::Success).entity(entity);
        EventSink::record(db, &event);
    }
}

fn render_settings(report: &SettingsReport) -> String {
    let credential = if report.settings.credentials.deepseek_present() {
        "set"
    } else {
        "unset"
    };
    let github_credential = if report.settings.credentials.github_present() {
        "set"
    } else {
        "unset"
    };
    format!(
        "Settings file:     {}\n\
         Status:            valid\n\
         Provider:          {}\n\
         Model:             {}\n\
         Source control:    {}\n\
         DeepSeek API key:  {credential}\n\
         GitHub API key:    {github_credential}\n",
        sanitize(&report.path.display().to_string()),
        report.settings.provider,
        report.settings.model,
        report.settings.source_control.provider,
    )
}

fn render_labs(items: &[LabListItem], active: &ActiveLab) -> String {
    if items.is_empty() {
        return empty_state(
            "No laboratories yet.",
            "Run `vlab labs` to create the first one.",
        );
    }
    let columns = [
        Column::identifier("ID"),
        Column::text("Name"),
        Column::text("Agents"),
        Column::text("Projects"),
        Column::text("Valid"),
        Column::text("Active"),
    ];
    let rows: Vec<Vec<String>> = items
        .iter()
        .map(|item| {
            let marker = matches!(active, ActiveLab::Selected(id) if id == &item.lab.id);
            vec![
                item.lab.id.to_string(),
                item.lab.name.clone(),
                item.lab.agent_count.to_string(),
                item.lab.project_count.to_string(),
                if item.check.valid() { "yes" } else { "no" }.to_owned(),
                if marker { "*" } else { "" }.to_owned(),
            ]
        })
        .collect();
    render_table(terminal_width(), &columns, &rows)
}

fn render_created_lab(lab: &crate::domain::lab::Lab) -> String {
    format!(
        "Laboratory created.\n\
         ID:   {}\n\
         Name: {}\n\
         \n\
         A Tech Lead is required before requests can be processed.\n\
         Run `vlab agents` to add one.\n",
        lab.id,
        sanitize(lab.name.as_str()),
    )
}

fn render_open(summary: &OpenSummary) -> String {
    format!(
        "Active laboratory: {} — {}\nAgents: {}\n",
        summary.lab.id,
        sanitize(&summary.lab.name),
        summary.lab.agent_count,
    )
}

fn write_open_warnings(stderr: &mut dyn Write, summary: &OpenSummary) -> Result<(), AppError> {
    if summary.warnings.is_empty() {
        return Ok(());
    }
    write_stderr(
        stderr,
        "Warning: this laboratory is not ready for planning.\n",
    )?;
    for warning in &summary.warnings {
        match &warning.detail {
            Some(detail) => write_stderr(
                stderr,
                &format!("Warning: {}: {}\n", warning.rule, sanitize(detail)),
            )?,
            None => write_stderr(stderr, &format!("Warning: {}\n", warning.rule))?,
        }
    }
    write_stderr(
        stderr,
        &format!(
            "Hint: Run `vlab --check-lab {}` to see every rule.\n",
            summary.lab.id
        ),
    )
}

fn render_check(report: &LabCheck) -> String {
    let mut out = format!(
        "Laboratory {}: {}\n",
        report.lab.id,
        sanitize(&report.lab.name)
    );
    for rule in &report.rules {
        let status = if rule.passed { "pass" } else { "fail" };
        out.push_str(&format!("  [{status}] {}", rule.rule));
        if let Some(detail) = &rule.detail {
            out.push_str(&format!(" ({})", sanitize(detail)));
        }
        out.push('\n');
    }
    out.push_str(if report.valid() {
        "Result: valid\n"
    } else {
        "Result: invalid\n"
    });
    out
}

fn render_agents(records: &[AgentListItem]) -> String {
    if records.is_empty() {
        return empty_state(
            "No agents in the active laboratory.",
            "Run `vlab agents` to create one.",
        );
    }
    let columns = [
        Column::identifier("ID"),
        Column::text("Name"),
        Column::text("Role"),
        Column::text("Provider"),
        Column::text("Model"),
        Column::text("Projects"),
        Column::text("Stories"),
    ];
    let rows: Vec<Vec<String>> = records
        .iter()
        .map(|item| {
            let record = &item.record;
            vec![
                record.id.to_string(),
                record.name.clone(),
                record.role.clone(),
                record.provider.clone(),
                record.model.clone(),
                item.projects.to_string(),
                item.open_stories.to_string(),
            ]
        })
        .collect();
    render_table(terminal_width(), &columns, &rows)
}

fn render_agent_detail(detail: &AgentDetail) -> String {
    let profile = &detail.profile;
    let skills = if profile.skills.is_empty() {
        "none".to_owned()
    } else {
        profile
            .skills
            .iter()
            .map(|skill| skill.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    };
    let mut out = format!(
        "Agent {}\n\
         Name:        {}\n\
         Role:        {}\n\
         Description: {}\n\
         Provider:    {}\n\
         Model:       {}\n\
         Status:      {}\n\
         Skills:      {skills}\n",
        profile.id,
        sanitize(profile.name.as_str()),
        profile.role.as_str(),
        sanitize(profile.description.as_str()),
        profile.provider,
        profile.model,
        if detail.archived {
            "archived"
        } else {
            "active"
        },
    );
    match &profile.cv {
        Some(cv) => {
            out.push_str("CV:\n");
            out.push_str(&sanitize(cv));
            if !cv.ends_with('\n') {
                out.push('\n');
            }
        }
        None => out.push_str("CV:          (none)\n"),
    }
    out.push_str(&format!("Memberships: {}\n", detail.memberships.len()));
    for lab in &detail.memberships {
        out.push_str(&format!("  {} — {}\n", lab.id, sanitize(&lab.name)));
    }
    let projects = if detail.projects.is_empty() {
        "N/A".to_owned()
    } else {
        detail
            .projects
            .iter()
            .map(|(id, name)| format!("{id} — {}", sanitize(name)))
            .collect::<Vec<_>>()
            .join(", ")
    };
    out.push_str(&format!(
        "Projects:    {projects}\nAssigned stories: {} (open {})\nCompleted stories: N/A\nMemories:    {}\n",
        detail.assigned_stories, detail.open_stories, detail.memories
    ));
    out
}

fn render_deleted_lab(plan: &LabDeletionPlan) -> String {
    format!(
        "Laboratory {} — {} deleted.\nProjects: {}\nMemories: {}\n",
        plan.id,
        sanitize(&plan.name),
        plan.projects,
        plan.memories
    )
}

fn render_deleted_project(plan: &ProjectDeletionPlan) -> String {
    format!(
        "Project {} — {} deleted.\nRequests: {}\nStories: {}\nMemories: {}\n",
        plan.id,
        sanitize(&plan.name),
        plan.requests,
        plan.stories,
        plan.memories
    )
}

fn render_archived_agent(plan: &AgentDeletionPlan) -> String {
    let labs = if plan.labs.is_empty() {
        "none".to_owned()
    } else {
        plan.labs
            .iter()
            .map(|lab| format!("{} — {}", lab.id, sanitize(&lab.name)))
            .collect::<Vec<_>>()
            .join(", ")
    };
    format!(
        "Agent {} — {} archived.\nLaboratories: {labs}\nMemories retained: {}\n",
        plan.id,
        sanitize(&plan.name),
        plan.memories
    )
}

fn render_created_agent(profile: &AgentProfile) -> String {
    format!(
        "Agent created.\n\
         ID:       {}\n\
         Name:     {}\n\
         Role:     {}\n\
         Provider: {}\n\
         Model:    {}\n",
        profile.id,
        sanitize(profile.name.as_str()),
        profile.role.as_str(),
        profile.provider,
        profile.model,
    )
}

fn lab_delete_summary(plan: &LabDeletionPlan) -> String {
    format!(
        "Delete laboratory {} — {}?\nProjects: {}\nMemberships: {}\nMemories: {}\nAgent profiles are preserved.",
        plan.id,
        sanitize(&plan.name),
        plan.projects,
        plan.memberships,
        plan.memories
    )
}

fn project_delete_summary(plan: &ProjectDeletionPlan) -> String {
    format!(
        "Delete project {} — {}?\nRequests: {}\nStories: {}\nMemories: {}\nAgent profiles are preserved.",
        plan.id,
        sanitize(&plan.name),
        plan.requests,
        plan.stories,
        plan.memories
    )
}

fn agent_delete_summary(plan: &AgentDeletionPlan) -> String {
    let labs = if plan.labs.is_empty() {
        "none".to_owned()
    } else {
        plan.labs
            .iter()
            .map(|lab| format!("{} — {}", lab.id, sanitize(&lab.name)))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let lead_warning = if plan.lead_labs.is_empty() {
        String::new()
    } else {
        format!(
            "\nWarning: this agent is a Tech Lead in {}; those laboratories will need a new lead.",
            plan.lead_labs.len()
        )
    };
    format!(
        "Archive agent {} — {}?\nLaboratories: {labs}\nMemories retained: {}\nAssigned unfinished stories: {}{lead_warning}",
        plan.id,
        sanitize(&plan.name),
        plan.memories,
        plan.assigned_open_stories
    )
}

fn render_opened_project(project: &ProjectOverview) -> String {
    format!(
        "Project opened.\nID:   {}\nName: {}\n{}\n",
        project.id,
        sanitize(&project.name),
        project
            .github_source
            .as_ref()
            .map(|source| format!("GitHub: {}\n", sanitize(source)))
            .unwrap_or_default()
    )
}

fn render_projects(projects: &[ProjectOverview]) -> String {
    if projects.is_empty() {
        return empty_state(
            "No projects in the active laboratory.",
            "Run `vlab projects` to create one.",
        );
    }
    let columns = [
        Column::identifier("ID"),
        Column::text("Name"),
        Column::identifier("GitHub"),
        Column::text("Requests"),
        Column::text("Stories"),
        Column::text("TODO"),
        Column::text("PROG"),
        Column::text("REVIEW"),
        Column::text("BLOCKED"),
        Column::text("DONE"),
    ];
    let rows: Vec<Vec<String>> = projects
        .iter()
        .map(|project| {
            vec![
                project.id.to_string(),
                project.name.clone(),
                project
                    .github_source
                    .clone()
                    .unwrap_or_else(|| "-".to_owned()),
                project.request_count.to_string(),
                project.story_count.to_string(),
                project.todo.to_string(),
                project.in_progress.to_string(),
                project.in_review.to_string(),
                project.blocked.to_string(),
                project.done.to_string(),
            ]
        })
        .collect();
    render_table(terminal_width(), &columns, &rows)
}

fn render_created_project(created: &CreatedProject) -> String {
    let project = &created.project;
    format!(
        "Project created.\nID:   {}\nName: {}\n{}{}\n",
        project.id,
        sanitize(project.name.as_str()),
        project
            .github_source
            .as_ref()
            .map(|source| format!("GitHub: {source}\n"))
            .unwrap_or_default(),
        created
            .workspace
            .as_ref()
            .map(|path| format!("Workspace: {}\n", sanitize(&path.display().to_string())))
            .unwrap_or_default()
    )
}

fn render_board(board: &Board, width: usize) -> String {
    let mut out = format!(
        "Project {} — {}\nLaboratory: {}\nTech Lead: {}\n",
        board.project_id,
        sanitize(&board.project_name),
        sanitize(&board.lab_name),
        board
            .lead_name
            .as_deref()
            .map(sanitize)
            .unwrap_or_else(|| "N/A (missing Tech Lead)".to_owned()),
    );
    if let Some(source) = &board.github_source {
        out.push_str(&format!("GitHub: {}\n", sanitize(source)));
    }
    out.push_str(&format!(
        "Stories: {} (TODO {}, IN PROGRESS {}, IN REVIEW {}, BLOCKED {}, DONE {})\n",
        board.counts.total(),
        board.counts.todo,
        board.counts.in_progress,
        board.counts.in_review,
        board.counts.blocked,
        board.counts.done,
    ));
    out.push_str(&format!(
        "Completed: {}\nPending intake: {}\n\n",
        board.counts.done, board.pending_intake
    ));
    if board.counts.total() == 0 {
        out.push_str(
            "No stories yet. Stories appear here after request planning is implemented.\n",
        );
        return out;
    }
    if width >= 100 {
        out.push_str(&render_board_columns(board, width));
    } else {
        out.push_str(&render_board_sections(board));
    }
    out
}

fn render_board_columns(board: &Board, width: usize) -> String {
    let states = StoryState::ALL;
    let gaps = 2 * (states.len() - 1);
    let column_width = width.saturating_sub(gaps) / states.len();
    let mut columns: Vec<Vec<String>> = Vec::new();
    for state in states {
        let mut lines = vec![format!("{} ({})", state.label(), board.counts.get(state))];
        let mut any = false;
        for card in board.cards_in(state) {
            any = true;
            let title_width = column_width.saturating_sub(card.id.as_str().len() + 1);
            lines.push(format!(
                "{} {}",
                card.id,
                truncate_cell(&card.title, title_width)
            ));
            lines.push(truncate_cell(
                &format!(
                    "{} · {}",
                    card.assignee.as_deref().unwrap_or("Unassigned"),
                    card.priority
                ),
                column_width,
            ));
            lines.push(format!("Due: {}", card.due_date));
            if card.overdue {
                lines.push("OVERDUE".to_owned());
            }
        }
        if !any {
            lines.push("(empty)".to_owned());
        }
        columns.push(lines);
    }
    let height = columns.iter().map(Vec::len).max().unwrap_or(0);
    let mut out = String::new();
    for row in 0..height {
        let cells: Vec<String> = columns
            .iter()
            .map(|column| column.get(row).cloned().unwrap_or_default())
            .collect();
        let mut line = String::new();
        for (index, cell) in cells.iter().enumerate() {
            if index > 0 {
                line.push_str("  ");
            }
            let cell = truncate_cell(cell, column_width);
            line.push_str(&cell);
            line.push_str(&" ".repeat(column_width.saturating_sub(cell.width())));
        }
        out.push_str(line.trim_end());
        out.push('\n');
    }
    out
}

fn render_board_sections(board: &Board) -> String {
    let mut out = String::new();
    for state in StoryState::ALL {
        out.push_str(&format!(
            "{} ({})\n",
            state.label(),
            board.counts.get(state)
        ));
        let mut any = false;
        for card in board.cards_in(state) {
            any = true;
            out.push_str(&format!(
                "  {} {}\n    Assignee: {} · Priority: {} · Due: {}{}\n",
                card.id,
                sanitize(&card.title),
                card.assignee
                    .as_deref()
                    .map(sanitize)
                    .unwrap_or_else(|| "Unassigned".to_owned()),
                card.priority,
                card.due_date,
                if card.overdue { " · overdue" } else { "" }
            ));
        }
        if !any {
            out.push_str("  (empty)\n");
        }
        out.push('\n');
    }
    out
}

pub(crate) fn provider_from_settings(
    settings: &crate::domain::settings::Settings,
    limits: ExecutionLimits,
) -> Result<DeepSeekProvider, AppError> {
    let key = settings
        .credentials
        .deepseek_api_key
        .clone()
        .ok_or_else(|| AppError::Configuration {
            problem: "`credentials.deepseek_api_key` is not set.".to_owned(),
            hint: "Add credentials.deepseek_api_key to settings.yaml with `vlab --edit-settings`."
                .to_owned(),
        })?;
    // The test override may only point at a loopback mock; it must never
    // redirect the credential to an arbitrary host.
    let endpoint = std::env::var("VLAB_DEEPSEEK_ENDPOINT")
        .ok()
        .filter(|value| {
            value.starts_with("http://127.0.0.1:") || value.starts_with("http://localhost:")
        })
        .unwrap_or_else(|| DEFAULT_ENDPOINT.to_owned());
    DeepSeekProvider::new(
        endpoint,
        key,
        AdapterLimits {
            attempt_timeout: limits.attempt_timeout,
            connect_timeout: std::time::Duration::from_secs(10),
            response_max_bytes: limits.response_max_bytes,
        },
    )
    .map_err(|_| AppError::Provider(ProviderFailure::Configuration))
}

fn render_plan_outcome(
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
    db: &Database,
    outcome: &PlanningOutcome,
) -> Result<(), AppError> {
    match outcome {
        PlanningOutcome::Planned {
            request_id,
            stories,
        } => {
            let members = db.lab_agents(&crate::app::context::active_lab(db)?.require()?)?;
            let names: std::collections::HashMap<String, String> = members
                .into_iter()
                .map(|member| (member.id.to_string(), member.name))
                .collect();
            let columns = [
                Column::identifier("ID"),
                Column::text("Title"),
                Column::text("Assignee"),
                Column::text("Priority"),
                Column::text("Complexity"),
                Column::text("Due"),
            ];
            let rows: Vec<Vec<String>> = stories
                .iter()
                .map(|story| {
                    let assignee = story
                        .assigned_agent_id
                        .as_deref()
                        .and_then(|id| names.get(id).cloned())
                        .unwrap_or_else(|| "Unassigned".to_owned());
                    vec![
                        story.id.to_string(),
                        story.title.clone(),
                        assignee,
                        story.priority.clone(),
                        story.complexity.clone(),
                        story.due_date.clone(),
                    ]
                })
                .collect();
            write_stdout(
                stdout,
                &format!(
                    "Request {request_id}: planned\n{}",
                    render_table(terminal_width(), &columns, &rows)
                ),
            )
        }
        PlanningOutcome::NeedsClarification {
            request_id,
            questions,
        } => {
            let mut out = format!("Request {request_id}: needs clarification\nQuestions:\n");
            for (index, question) in questions.iter().enumerate() {
                out.push_str(&format!("  {}. {}\n", index + 1, sanitize(question)));
            }
            write_stdout(stdout, &out)
        }
        PlanningOutcome::Failed {
            request_id,
            code,
            hint,
            validation,
        } => {
            write_stdout(
                stdout,
                &format!(
                    "Request {request_id}: failed\nCode: {code}\nHint: {}\n",
                    sanitize(hint)
                ),
            )?;
            if *validation {
                Err(AppError::Validation {
                    problem: "the request was not planned; local validation failed.".to_owned(),
                    hint: (*hint).to_owned(),
                })
            } else {
                Err(AppError::Provider(provider_failure_from_code(code)))
            }
        }
        PlanningOutcome::Cancelled { request_id } => {
            write_stdout(stdout, &format!("Request {request_id}: cancelled\n"))?;
            write_stderr(
                stderr,
                "Warning: provider-side processing may have continued after local cancellation.\n",
            )?;
            Err(AppError::Interrupted)
        }
    }
}

fn provider_failure_from_code(code: &str) -> ProviderFailure {
    match code {
        "provider_authentication" => ProviderFailure::Authentication,
        "provider_rate_limited" => ProviderFailure::RateLimited,
        "provider_timeout" => ProviderFailure::Timeout,
        "provider_transport" => ProviderFailure::Transport,
        "provider_server" => ProviderFailure::Server,
        "provider_response_too_large" => ProviderFailure::ResponseTooLarge,
        "provider_configuration" => ProviderFailure::Configuration,
        "budget_exceeded" => ProviderFailure::BudgetExceeded,
        _ => ProviderFailure::InvalidOutput,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::kanban::BoardCounts;
    use crate::domain::ids::{ProjectId, StoryId};

    fn board() -> Board {
        Board {
            project_id: ProjectId::parse("0b1c2d3e").unwrap(),
            project_name: "Portal".to_owned(),
            github_source: None,
            lab_name: "Payments".to_owned(),
            lead_name: Some("Ada".to_owned()),
            counts: BoardCounts {
                todo: 1,
                ..BoardCounts::default()
            },
            pending_intake: 1,
            cards: vec![crate::app::kanban::Card {
                id: StoryId::parse("a3f91c07").unwrap(),
                title: "Add reset".to_owned(),
                state: StoryState::Todo,
                assignee: None,
                priority: "high".to_owned(),
                due_date: "2026-01-03".to_owned(),
                overdue: false,
            }],
        }
    }

    #[test]
    fn the_board_renders_at_narrow_and_wide_widths_without_ansi() {
        let narrow = render_board(&board(), 50);
        assert!(narrow.contains("TODO (1)"), "{narrow}");
        assert!(narrow.contains("a3f91c07"), "{narrow}");
        assert!(!narrow.contains('\u{1b}'));
        let wide = render_board(&board(), 120);
        assert!(wide.contains("TODO (1)"), "{wide}");
        assert!(wide.contains("IN PROGRESS (0)"), "{wide}");
        assert!(!wide.contains('\u{1b}'));
    }
}
