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
        "list_projects", "new_project", "show_project_kanban", "delete_project", "new_request",
        "list_memories", "new_memory", "delete_memory",
        "list_agents", "new_agent", "delete_agent", "show_agent_detail", "ask",
    ])),
)]
pub struct Cli {
    /// Show the configuration path, provider, and model
    #[arg(long, help_heading = "Settings")]
    show_settings: bool,

    /// Edit settings.yaml in $VISUAL or $EDITOR and validate it
    #[arg(long, help_heading = "Settings")]
    edit_settings: bool,

    /// List laboratories
    #[arg(long, help_heading = "Laboratories")]
    list_labs: bool,

    /// Create a laboratory and print its identifier
    #[arg(long, value_name = "name", help_heading = "Laboratories")]
    new_lab: Option<String>,

    /// Select the active laboratory
    #[arg(long, value_name = "lab-id", help_heading = "Laboratories")]
    open_lab: Option<String>,

    /// Delete a laboratory and its projects (asks for confirmation)
    #[arg(long, value_name = "lab-id", help_heading = "Laboratories")]
    delete_lab: Option<String>,

    /// Validate a laboratory's agents and Tech Lead
    #[arg(long, value_name = "lab-id", help_heading = "Laboratories")]
    check_lab: Option<String>,

    /// List projects in the active laboratory
    #[arg(long, help_heading = "Projects")]
    list_projects: bool,

    /// Create a project in the active laboratory
    #[arg(long, value_name = "name", help_heading = "Projects")]
    new_project: Option<String>,

    /// Optional GitHub repository URL for the new project
    #[arg(long, value_name = "url", help_heading = "Projects")]
    github_source: Option<String>,

    /// Show a project's read-only Kanban board
    #[arg(long, value_name = "project-id", help_heading = "Projects")]
    show_project_kanban: Option<String>,

    /// Delete a project (asks for confirmation)
    #[arg(long, value_name = "project-id", help_heading = "Projects")]
    delete_project: Option<String>,

    /// Submit a request for Tech Lead planning
    #[arg(
        long,
        value_names = ["project-id", "request-text"],
        num_args = 1..=2,
        help_heading = "Projects"
    )]
    new_request: Option<Vec<String>>,

    /// List memories for one laboratory, project, or agent
    #[arg(long, help_heading = "Memories")]
    list_memories: bool,

    /// Create a memory (use exactly one --for-* selector)
    #[arg(long, value_name = "text", help_heading = "Memories")]
    new_memory: Option<String>,

    /// Delete a memory (asks for confirmation)
    #[arg(long, value_name = "memory-id", help_heading = "Memories")]
    delete_memory: Option<String>,

    /// Scope a memory command to a laboratory
    #[arg(long, value_name = "lab-id", help_heading = "Memories")]
    for_lab: Option<String>,

    /// Scope a memory command to a project in the active laboratory
    #[arg(long, value_name = "project-id", help_heading = "Memories")]
    for_project: Option<String>,

    /// Scope a memory command to an agent
    #[arg(long, value_name = "agent-id", help_heading = "Memories")]
    for_agent: Option<String>,

    /// List agents in the active laboratory
    #[arg(long, help_heading = "Agents")]
    list_agents: bool,

    /// Create an agent with a guided procedure
    #[arg(long, help_heading = "Agents")]
    new_agent: bool,

    /// Delete an agent from every laboratory (asks for confirmation)
    #[arg(long, value_name = "agent-id", help_heading = "Agents")]
    delete_agent: Option<String>,

    /// Show an agent's profile, memberships, and assignments
    #[arg(long, value_name = "agent-id", help_heading = "Agents")]
    show_agent_detail: Option<String>,

    /// Ask the Tech Lead, or the agent given with --to-agent, a question
    #[arg(long, value_name = "question", help_heading = "Agents")]
    ask: Option<String>,

    /// Address --ask to a specific agent
    #[arg(long, value_name = "agent-id", help_heading = "Agents")]
    to_agent: Option<String>,

    /// Confirm a deletion without prompting
    #[arg(long)]
    yes: bool,
}

/// The scope selector supplied to a memory command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MemoryScopeArg {
    Lab(String),
    Project(String),
    Agent(String),
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
    },
    ShowProjectKanban {
        project_id: String,
    },
    DeleteProject {
        project_id: String,
        assume_yes: bool,
    },
    NewRequest {
        project_id: String,
        text: Option<String>,
    },
    ListMemories {
        scope: MemoryScopeArg,
    },
    NewMemory {
        text: String,
        scope: MemoryScopeArg,
    },
    DeleteMemory {
        memory_id: String,
        assume_yes: bool,
    },
    ListAgents,
    NewAgent,
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
            Self::ShowProjectKanban { .. } => "--show-project-kanban",
            Self::DeleteProject { .. } => "--delete-project",
            Self::NewRequest { .. } => "--new-request",
            Self::ListMemories { .. } => "--list-memories",
            Self::NewMemory { .. } => "--new-memory",
            Self::DeleteMemory { .. } => "--delete-memory",
            Self::ListAgents => "--list-agents",
            Self::NewAgent => "--new-agent",
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
        let memory_action =
            self.list_memories || self.new_memory.is_some() || self.delete_memory.is_some();
        let scope_count = self.for_lab.is_some() as u8
            + self.for_project.is_some() as u8
            + self.for_agent.is_some() as u8;
        if (self.for_lab.is_some() || self.for_project.is_some() || self.for_agent.is_some())
            && !memory_action
        {
            return Err(modifier_error(
                "`--for-lab`, `--for-project`, and `--for-agent` can only be used with a memory command.",
            ));
        }
        if (self.list_memories || self.new_memory.is_some()) && scope_count != 1 {
            return Err(modifier_error(
                "a memory command needs exactly one of `--for-lab`, `--for-project`, or `--for-agent`.",
            ));
        }
        if self.delete_memory.is_some() && scope_count != 0 {
            return Err(modifier_error(
                "`--delete-memory` takes only the memory identifier.",
            ));
        }
        let scope = || -> MemoryScopeArg {
            if let Some(value) = self.for_lab.clone() {
                MemoryScopeArg::Lab(value)
            } else if let Some(value) = self.for_project.clone() {
                MemoryScopeArg::Project(value)
            } else if let Some(value) = self.for_agent.clone() {
                MemoryScopeArg::Agent(value)
            } else {
                unreachable!("scope_count validated above")
            }
        };

        let deletion = self.delete_lab.is_some()
            || self.delete_project.is_some()
            || self.delete_agent.is_some()
            || self.delete_memory.is_some();
        if self.github_source.is_some() && self.new_project.is_none() {
            return Err(modifier_error(
                "`--github-source` can only be used with `--new-project`.",
            ));
        }
        if self.yes && !deletion {
            return Err(modifier_error(
                "`--yes` can only be used with `--delete-lab`, `--delete-project`, `--delete-agent`, or `--delete-memory`.",
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
            }
        } else if let Some(project_id) = self.show_project_kanban {
            Action::ShowProjectKanban { project_id }
        } else if let Some(project_id) = self.delete_project {
            Action::DeleteProject {
                project_id,
                assume_yes,
            }
        } else if let Some(values) = self.new_request {
            let mut values = values.into_iter();
            match values.next() {
                Some(project_id) => Action::NewRequest {
                    project_id,
                    text: values.next(),
                },
                None => return Ok(None),
            }
        } else if self.list_memories {
            Action::ListMemories { scope: scope() }
        } else if let Some(text) = self.new_memory {
            Action::NewMemory {
                text,
                scope: scope(),
            }
        } else if let Some(memory_id) = self.delete_memory {
            Action::DeleteMemory {
                memory_id,
                assume_yes,
            }
        } else if self.list_agents {
            Action::ListAgents
        } else if self.new_agent {
            Action::NewAgent
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
            parse(&["--new-request", "a3f91c07", "Add reset"]),
            Some(Action::NewRequest {
                project_id: "a3f91c07".into(),
                text: Some("Add reset".into()),
            })
        );
        assert_eq!(
            parse(&["--new-request", "a3f91c07"]),
            Some(Action::NewRequest {
                project_id: "a3f91c07".into(),
                text: None
            })
        );
        assert_eq!(
            parse(&["--ask", "Why?", "--to-agent", "0b1c2d3e"]),
            Some(Action::Ask {
                question: "Why?".into(),
                to_agent: Some("0b1c2d3e".into())
            })
        );
    }

    #[test]
    fn memory_commands_capture_exactly_one_scope() {
        assert_eq!(
            parse(&["--new-memory", "Keep it small", "--for-lab", "a3f91c07"]),
            Some(Action::NewMemory {
                text: "Keep it small".into(),
                scope: MemoryScopeArg::Lab("a3f91c07".into())
            })
        );
        assert_eq!(
            parse(&["--list-memories", "--for-agent", "0b1c2d3e"]),
            Some(Action::ListMemories {
                scope: MemoryScopeArg::Agent("0b1c2d3e".into())
            })
        );
        assert_eq!(
            parse(&["--delete-memory", "1c2d3e4f", "--yes"]),
            Some(Action::DeleteMemory {
                memory_id: "1c2d3e4f".into(),
                assume_yes: true
            })
        );
    }

    #[test]
    fn memory_modifiers_require_the_right_action_and_scope() {
        for args in [
            &["--new-memory", "x"][..],
            &[
                "--new-memory",
                "x",
                "--for-lab",
                "a3f91c07",
                "--for-agent",
                "0b1c2d3e",
            ],
            &["--list-memories"],
            &["--delete-memory", "1c2d3e4f", "--for-lab", "a3f91c07"],
        ] {
            let error = match args[0] {
                "--new-memory" | "--list-memories" if args.len() == 1 => {
                    // parse the action first, then the modifier validation
                    let argv = std::iter::once("vlab").chain(args.iter().copied());
                    Cli::try_parse_from(argv)
                        .unwrap()
                        .into_action()
                        .unwrap_err()
                }
                _ => {
                    let argv = std::iter::once("vlab").chain(args.iter().copied());
                    Cli::try_parse_from(argv)
                        .unwrap()
                        .into_action()
                        .unwrap_err()
                }
            };
            assert!(
                error.to_string().contains("memory") || error.to_string().contains("exactly one"),
                "{error}"
            );
        }
        let error = modifier_misuse(&["--list-labs", "--for-lab", "a3f91c07"]);
        assert!(error.to_string().contains("memory command"), "{error}");
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
        let argv = ["vlab", "--new-request", "a3f91c07", "--list-labs"];
        assert!(Cli::try_parse_from(argv).is_err());
    }
}
