//! The complete argument surface of specification §4.1 (ADR-0009).
//!
//! Every primary action is a long flag in the `action` group, so clap rejects
//! a second action. Modifiers are checked in [`Cli::into_action`]: clap skips
//! a `requires` rule whose target conflicts with an argument already present,
//! so it cannot enforce them alone.

use clap::{ArgGroup, Parser};

use crate::app::AppError;

#[derive(Debug, Parser)]
#[command(
    name = "vlab",
    version,
    about = "Manage virtual software laboratories staffed by AI agents.",
    group(ArgGroup::new("action").multiple(false).args([
        "show_settings", "edit_settings",
        "list_labs", "new_lab", "open_lab", "delete_lab", "check_lab",
        "list_projects", "new_project", "open_project", "show_project_kanban",
        "delete_project", "new_request", "assign_agent", "run_schedules", "run_queue",
        "sync_sandbox", "import_files",
        "agent_create_from_file", "logs",
        "list_agents", "new_agent", "load_preset", "delete_agent", "show_agent_detail", "ask",
    ])),
)]
pub struct Cli {
    /// Show the configuration path, provider, and model
    #[arg(hide = true, long, help_heading = "Settings")]
    show_settings: bool,

    /// Edit settings.yaml in $VISUAL or $EDITOR and validate it
    #[arg(hide = true, long, help_heading = "Settings")]
    edit_settings: bool,

    /// List laboratories
    #[arg(hide = true, long, help_heading = "Laboratories")]
    list_labs: bool,

    /// Create a laboratory and print its identifier
    #[arg(hide = true, long, value_name = "name", help_heading = "Laboratories")]
    new_lab: Option<String>,

    /// Select the active laboratory
    #[arg(
        hide = true,
        long,
        value_name = "lab-id",
        help_heading = "Laboratories"
    )]
    open_lab: Option<String>,

    /// Delete a laboratory and its projects (asks for confirmation)
    #[arg(
        hide = true,
        long,
        value_name = "lab-id",
        help_heading = "Laboratories"
    )]
    delete_lab: Option<String>,

    /// Validate a laboratory's agents and Tech Lead
    #[arg(
        hide = true,
        long,
        value_name = "lab-id",
        help_heading = "Laboratories"
    )]
    check_lab: Option<String>,

    /// List projects in the active laboratory
    #[arg(hide = true, long, help_heading = "Projects")]
    list_projects: bool,

    /// Create a project in the active laboratory
    #[arg(hide = true, long, value_name = "name", help_heading = "Projects")]
    new_project: Option<String>,

    /// Optional GitHub repository URL for the new project
    #[arg(hide = true, long, value_name = "url", help_heading = "Projects")]
    github_source: Option<String>,

    /// Optional absolute folder path for the new project
    #[arg(hide = true, long, value_name = "path", help_heading = "Projects")]
    folder_source: Option<String>,

    /// Select the current project
    #[arg(
        hide = true,
        long,
        value_name = "project-id",
        help_heading = "Projects"
    )]
    open_project: Option<String>,

    /// Show the current project's read-only Kanban board
    #[arg(hide = true, long, help_heading = "Projects")]
    show_project_kanban: bool,

    /// Delete the current project (asks for confirmation)
    #[arg(hide = true, long, help_heading = "Projects")]
    delete_project: bool,

    /// Submit a request for the current project
    #[arg(hide = true,
        long,
        value_name = "request-text",
        num_args = 0..=1,
        default_missing_value = "",
        help_heading = "Projects"
    )]
    new_request: Option<String>,

    /// Assign an agent to the current project
    #[arg(hide = true, long, value_name = "agent-id", help_heading = "Projects")]
    assign_agent: Option<String>,

    /// Run due project schedules
    #[arg(hide = true, long, help_heading = "Projects")]
    run_schedules: bool,

    /// Process queued TODO requests
    #[arg(hide = true, long, help_heading = "Projects")]
    run_queue: bool,

    /// Integrate DONE stories into develop and refresh .sandbox/
    #[arg(hide = true, long, help_heading = "Projects")]
    sync_sandbox: bool,

    /// Import a markdown document into the data file store
    #[arg(hide = true, long = "import", help_heading = "Projects")]
    import_files: bool,

    /// Import an agent definition from a markdown file
    #[arg(
        hide = true,
        long = "create-agent-from-file",
        value_name = "file",
        help_heading = "Agents"
    )]
    agent_create_from_file: Option<String>,

    /// Source markdown file for --import
    #[arg(hide = true, long, value_name = "path", help_heading = "Projects")]
    src: Option<String>,

    /// Destination for --import: agent, lab/<name>, or project/<name>
    #[arg(hide = true, long, value_name = "dest", help_heading = "Projects")]
    into: Option<String>,

    /// Agent name or identifier when --into=agent
    #[arg(hide = true, long, value_name = "agent", help_heading = "Projects")]
    agent: Option<String>,

    /// Show the recent application audit log
    #[arg(hide = true, long, help_heading = "Diagnostics")]
    logs: bool,

    /// List agents in the active laboratory
    #[arg(hide = true, long, help_heading = "Agents")]
    list_agents: bool,

    /// Create an agent with a guided procedure
    #[arg(hide = true, long, help_heading = "Agents")]
    new_agent: bool,

    /// Load the preset agent catalog from a YAML file
    #[arg(hide = true, long, value_name = "file", help_heading = "Agents")]
    load_preset: Option<String>,

    /// Delete an agent from every laboratory (asks for confirmation)
    #[arg(hide = true, long, value_name = "agent-id", help_heading = "Agents")]
    delete_agent: Option<String>,

    /// Show an agent's profile, memberships, and assignments
    #[arg(hide = true, long, value_name = "agent-id", help_heading = "Agents")]
    show_agent_detail: Option<String>,

    /// Ask the Tech Lead, or the agent given with --to-agent, a question
    #[arg(hide = true, long, value_name = "question", help_heading = "Agents")]
    ask: Option<String>,

    /// Address --ask to a specific agent
    #[arg(hide = true, long, value_name = "agent-id", help_heading = "Agents")]
    to_agent: Option<String>,

    /// Confirm a deletion without prompting
    #[arg(hide = true, long)]
    yes: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DashboardGroup {
    Labs,
    Projects,
    Agents,
    Settings,
}

/// One validated primary action with its operands.
///
/// Operands are raw user input; later packages validate them (identifiers per
/// ADR-0004) before any use.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    ShowSettings,
    EditSettings,
    ListLabs,
    NewLab {
        name: String,
    },
    OpenLab {
        lab_id: String,
    },
    DeleteLab {
        lab_id: String,
        assume_yes: bool,
    },
    CheckLab {
        lab_id: String,
    },
    ListProjects,
    NewProject {
        name: String,
        github_source: Option<String>,
        folder_source: Option<String>,
    },
    OpenProject {
        project_id: String,
    },
    ShowProjectKanban,
    DeleteProject {
        assume_yes: bool,
    },
    NewRequest {
        text: Option<String>,
    },
    AssignAgent {
        agent_id: String,
    },
    RunSchedules,
    RunQueue,
    SyncSandbox,
    ImportDocument {
        src: String,
        into: String,
        agent: Option<String>,
    },
    CreateAgentFromFile {
        path: String,
    },
    Logs,
    ListAgents,
    NewAgent,
    LoadPreset {
        path: String,
    },
    DeleteAgent {
        agent_id: String,
        assume_yes: bool,
    },
    ShowAgentDetail {
        agent_id: String,
    },
    Ask {
        question: String,
        to_agent: Option<String>,
    },
}

impl Action {
    /// The flag that selects this action, for messages and diagnostics.
    pub fn flag(&self) -> &'static str {
        match self {
            Self::ShowSettings => "--show-settings",
            Self::EditSettings => "--edit-settings",
            Self::ListLabs => "--list-labs",
            Self::NewLab { .. } => "--new-lab",
            Self::OpenLab { .. } => "--open-lab",
            Self::DeleteLab { .. } => "--delete-lab",
            Self::CheckLab { .. } => "--check-lab",
            Self::ListProjects => "--list-projects",
            Self::NewProject { .. } => "--new-project",
            Self::OpenProject { .. } => "--open-project",
            Self::ShowProjectKanban => "--show-project-kanban",
            Self::DeleteProject { .. } => "--delete-project",
            Self::NewRequest { .. } => "--new-request",
            Self::AssignAgent { .. } => "--assign-agent",
            Self::RunSchedules => "--run-schedules",
            Self::RunQueue => "--run-queue",
            Self::SyncSandbox => "--sync-sandbox",
            Self::ImportDocument { .. } => "--import",
            Self::CreateAgentFromFile { .. } => "--create-agent-from-file",
            Self::Logs => "--logs",
            Self::ListAgents => "--list-agents",
            Self::NewAgent => "--new-agent",
            Self::LoadPreset { .. } => "--load-preset",
            Self::DeleteAgent { .. } => "--delete-agent",
            Self::ShowAgentDetail { .. } => "--show-agent-detail",
            Self::Ask { .. } => "--ask",
        }
    }
}

impl Cli {
    /// The selected primary action, or `None` when no action was given.
    ///
    /// Fails when a modifier is used without the action that declares it.
    pub fn into_action(self) -> Result<Option<Action>, AppError> {
        if self.to_agent.is_some() && self.ask.is_none() {
            return Err(modifier_error(
                "`--to-agent` can only be used with `--ask`.",
            ));
        }
        let deletion =
            self.delete_lab.is_some() || self.delete_project || self.delete_agent.is_some();
        if self.github_source.is_some() && self.new_project.is_none() {
            return Err(modifier_error(
                "`--github-source` can only be used with `--new-project`.",
            ));
        }
        if self.folder_source.is_some() && self.new_project.is_none() {
            return Err(modifier_error(
                "`--folder-source` can only be used with `--new-project`.",
            ));
        }
        if !self.import_files && (self.src.is_some() || self.into.is_some() || self.agent.is_some())
        {
            return Err(modifier_error(
                "`--src`, `--into`, and `--agent` can only be used with `--import`.",
            ));
        }
        if self.import_files && (self.src.is_none() || self.into.is_none()) {
            return Err(modifier_error(
                "`--import` requires both `--src` and `--into`.",
            ));
        }
        if self.github_source.is_some() && self.folder_source.is_some() {
            return Err(modifier_error(
                "`--github-source` and `--folder-source` are mutually exclusive.",
            ));
        }
        if self.yes && !deletion {
            return Err(modifier_error(
                "`--yes` can only be used with `--delete-lab`, `--delete-project`, or `--delete-agent`.",
            ));
        }

        let assume_yes = self.yes;
        let action = if self.show_settings {
            Action::ShowSettings
        } else if self.edit_settings {
            Action::EditSettings
        } else if self.list_labs {
            Action::ListLabs
        } else if let Some(name) = self.new_lab {
            Action::NewLab { name }
        } else if let Some(lab_id) = self.open_lab {
            Action::OpenLab { lab_id }
        } else if let Some(lab_id) = self.delete_lab {
            Action::DeleteLab { lab_id, assume_yes }
        } else if let Some(lab_id) = self.check_lab {
            Action::CheckLab { lab_id }
        } else if self.list_projects {
            Action::ListProjects
        } else if let Some(name) = self.new_project {
            Action::NewProject {
                name,
                github_source: self.github_source,
                folder_source: self.folder_source,
            }
        } else if let Some(project_id) = self.open_project {
            Action::OpenProject { project_id }
        } else if self.show_project_kanban {
            Action::ShowProjectKanban
        } else if self.delete_project {
            Action::DeleteProject { assume_yes }
        } else if let Some(text) = self.new_request {
            Action::NewRequest {
                text: (!text.is_empty()).then_some(text),
            }
        } else if let Some(agent_id) = self.assign_agent {
            Action::AssignAgent { agent_id }
        } else if self.run_schedules {
            Action::RunSchedules
        } else if self.run_queue {
            Action::RunQueue
        } else if self.sync_sandbox {
            Action::SyncSandbox
        } else if self.import_files {
            Action::ImportDocument {
                src: self.src.ok_or(AppError::Internal)?,
                into: self.into.ok_or(AppError::Internal)?,
                agent: self.agent,
            }
        } else if let Some(path) = self.agent_create_from_file {
            Action::CreateAgentFromFile { path }
        } else if self.logs {
            Action::Logs
        } else if self.list_agents {
            Action::ListAgents
        } else if self.new_agent {
            Action::NewAgent
        } else if let Some(path) = self.load_preset {
            Action::LoadPreset { path }
        } else if let Some(agent_id) = self.delete_agent {
            Action::DeleteAgent {
                agent_id,
                assume_yes,
            }
        } else if let Some(agent_id) = self.show_agent_detail {
            Action::ShowAgentDetail { agent_id }
        } else if let Some(question) = self.ask {
            Action::Ask {
                question,
                to_agent: self.to_agent,
            }
        } else {
            return Ok(None);
        };
        Ok(Some(action))
    }
}

fn modifier_error(message: &str) -> AppError {
    AppError::Usage {
        message: message.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    fn parse(args: &[&str]) -> Option<Action> {
        let argv = std::iter::once("vlab").chain(args.iter().copied());
        Cli::try_parse_from(argv).unwrap().into_action().unwrap()
    }

    fn modifier_misuse(args: &[&str]) -> AppError {
        let argv = std::iter::once("vlab").chain(args.iter().copied());
        Cli::try_parse_from(argv)
            .unwrap()
            .into_action()
            .unwrap_err()
    }

    #[test]
    fn command_definition_is_consistent() {
        Cli::command().debug_assert();
    }

    #[test]
    fn no_flags_selects_no_action() {
        assert_eq!(parse(&[]), None);
    }

    #[test]
    fn operands_are_captured() {
        assert_eq!(
            parse(&["--new-request", "Add reset"]),
            Some(Action::NewRequest {
                text: Some("Add reset".into()),
            })
        );
        assert_eq!(
            parse(&["--new-request"]),
            Some(Action::NewRequest { text: None })
        );
        assert_eq!(
            parse(&["--open-project", "a3f91c07"]),
            Some(Action::OpenProject {
                project_id: "a3f91c07".into()
            })
        );
        assert_eq!(
            parse(&["--new-project", "Portal", "--folder-source", "/tmp/portal"]),
            Some(Action::NewProject {
                name: "Portal".into(),
                github_source: None,
                folder_source: Some("/tmp/portal".into()),
            })
        );
        assert_eq!(
            parse(&["--assign-agent", "0b1c2d3e"]),
            Some(Action::AssignAgent {
                agent_id: "0b1c2d3e".into()
            })
        );
        assert_eq!(
            parse(&["--show-project-kanban"]),
            Some(Action::ShowProjectKanban)
        );
        assert_eq!(
            parse(&["--delete-project", "--yes"]),
            Some(Action::DeleteProject { assume_yes: true })
        );
        assert_eq!(
            parse(&["--ask", "Why?", "--to-agent", "0b1c2d3e"]),
            Some(Action::Ask {
                question: "Why?".into(),
                to_agent: Some("0b1c2d3e".into())
            })
        );
        assert_eq!(
            parse(&["--load-preset", "assets/agents.yaml"]),
            Some(Action::LoadPreset {
                path: "assets/agents.yaml".into()
            })
        );
        assert_eq!(parse(&["--run-schedules"]), Some(Action::RunSchedules));
        assert_eq!(parse(&["--sync-sandbox"]), Some(Action::SyncSandbox));
    }

    #[test]
    fn yes_is_bound_to_the_deletion() {
        assert_eq!(
            parse(&["--delete-lab", "a3f91c07", "--yes"]),
            Some(Action::DeleteLab {
                lab_id: "a3f91c07".into(),
                assume_yes: true
            })
        );
        assert_eq!(
            parse(&["--delete-agent", "a3f91c07"]),
            Some(Action::DeleteAgent {
                agent_id: "a3f91c07".into(),
                assume_yes: false
            })
        );
        assert_eq!(
            parse(&["--delete-project"]),
            Some(Action::DeleteProject { assume_yes: false })
        );
    }

    #[test]
    fn modifiers_without_their_action_are_rejected() {
        for args in [
            &["--to-agent", "a3f91c07"][..],
            &["--list-labs", "--to-agent", "a3f91c07"],
        ] {
            let error = modifier_misuse(args);
            assert_eq!(
                error.to_string(),
                "`--to-agent` can only be used with `--ask`."
            );
        }
        for args in [
            &["--folder-source", "/tmp/portal"][..],
            &["--list-labs", "--folder-source", "/tmp/portal"],
        ] {
            let error = modifier_misuse(args);
            assert_eq!(
                error.to_string(),
                "`--folder-source` can only be used with `--new-project`."
            );
        }
        let error = modifier_misuse(&[
            "--new-project",
            "Portal",
            "--github-source",
            "https://github.com/acme/portal",
            "--folder-source",
            "/tmp/portal",
        ]);
        assert_eq!(
            error.to_string(),
            "`--github-source` and `--folder-source` are mutually exclusive."
        );
        for args in [
            &["--yes"][..],
            &["--list-labs", "--yes"],
            &["--ask", "Why?", "--yes"],
        ] {
            let error = modifier_misuse(args);
            assert!(
                error
                    .to_string()
                    .starts_with("`--yes` can only be used with"),
                "{error}"
            );
        }
    }

    #[test]
    fn a_flag_after_new_request_is_parsed_as_a_second_action() {
        let argv = ["vlab", "--new-request", "Add reset", "--list-labs"];
        assert!(Cli::try_parse_from(argv).is_err());
    }
}
