//! Interactive group dashboards (ADR-0038).
//!
//! The dashboards are a presentation layer over the existing application use
//! cases. They never add provider calls or bypass confirmation rules.

use ratatui::crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Clear, Paragraph, Wrap};
use ratatui::{DefaultTerminal, Frame};

use crate::app::AppError;
use crate::app::agents::{self, AgentListItem};
use crate::app::context;
use crate::app::deletions;
use crate::app::directory::Directory;
use crate::app::kanban;
use crate::app::labs::{self, LabListItem};
use crate::app::memories;
use crate::app::projects::{self, CreatedProject, ProjectOverview};
use crate::app::settings::{self, SettingsReport, SettingsStore};
use crate::app::settings_edit;
use crate::cli::args::DashboardGroup;
use crate::cli::prompt::{TerminalEditor, TerminalPrompter};
use crate::domain::ids::{AgentId, LabId, ProjectId};
use crate::domain::memory::MemoryScope;
use crate::domain::project::GithubSource;
use crate::infra::config::SettingsFile;
use crate::infra::db::Database;
use crate::infra::paths::Paths;
use crate::infra::workspace::{DEFAULT_CLONE_TIMEOUT, GitWorkspace};
use crate::obs::events::{AuditEvent, EntityRef, EntityType, EventKind, Outcome};
use crate::obs::{ActionId, RunId};

/// Opens a group dashboard.
pub fn run(group: DashboardGroup, no_color: bool, run_id: RunId) -> Result<(), AppError> {
    run_screen(Screen::Group(group), no_color, run_id)
}

/// Opens the home dashboard.
pub fn run_home(run_id: RunId) -> Result<(), AppError> {
    run_screen(Screen::Home, false, run_id)
}

fn run_screen(screen: Screen, no_color: bool, run_id: RunId) -> Result<(), AppError> {
    let mut terminal = ratatui::try_init().map_err(|error| AppError::Io {
        context: "starting the terminal dashboard",
        kind: error.kind(),
    })?;
    let result = App::new(screen, no_color, run_id).and_then(|mut app| app.run(&mut terminal));
    let _ = ratatui::try_restore();
    result
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Screen {
    Home,
    Group(DashboardGroup),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScreenItem {
    Group(DashboardGroup),
}

impl ScreenItem {
    fn label(self) -> &'static str {
        match self {
            Self::Group(DashboardGroup::Labs) => "Laboratories",
            Self::Group(DashboardGroup::Projects) => "Projects",
            Self::Group(DashboardGroup::Agents) => "Agents",
            Self::Group(DashboardGroup::Settings) => "Settings",
        }
    }

    fn home_description(self) -> &'static str {
        match self {
            Self::Group(DashboardGroup::Labs) => "create, check, activate, delete",
            Self::Group(DashboardGroup::Projects) => "create, open, assign, delete",
            Self::Group(DashboardGroup::Agents) => "inspect, create, assign, archive",
            Self::Group(DashboardGroup::Settings) => "inspect and edit settings",
        }
    }
}

const HOME_ITEMS: [ScreenItem; 4] = [
    ScreenItem::Group(DashboardGroup::Labs),
    ScreenItem::Group(DashboardGroup::Projects),
    ScreenItem::Group(DashboardGroup::Agents),
    ScreenItem::Group(DashboardGroup::Settings),
];

#[derive(Debug, Clone)]
enum Mode {
    Browse,
    Filter(String),
    LabForm {
        id: Option<LabId>,
        name: String,
        description: String,
        description_focus: bool,
    },
    ProjectForm {
        name: String,
        source: String,
        source_focus: bool,
    },
    Confirm(ConfirmAction),
    Message {
        title: String,
        lines: Vec<String>,
    },
    Help,
    Picker {
        agents: Vec<(String, String)>,
        selected: usize,
    },
}

#[derive(Debug, Clone)]
enum ConfirmAction {
    Lab(LabId, String),
    Project(ProjectId, String),
    Agent(AgentId, String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Deferred {
    EditSettings,
    NewAgent,
}

struct App {
    screen: Screen,
    mode: Mode,
    quit: bool,
    theme: Theme,
    run_id: RunId,
    db: Database,
    paths: Paths,
    labs: Vec<LabListItem>,
    projects: Vec<ProjectOverview>,
    agents: Vec<AgentListItem>,
    settings: Option<SettingsReport>,
    settings_error: Option<String>,
    active_lab: Option<LabId>,
    active_project: Option<ProjectId>,
    selected: usize,
    page: usize,
    page_size: usize,
    filter: String,
    status: Option<String>,
    home_selected: usize,
}

impl App {
    fn new(screen: Screen, no_color: bool, run_id: RunId) -> Result<Self, AppError> {
        let paths = Paths::from_environment()?;
        let (db, _) = Database::open_and_migrate(paths.database_file(), run_id)?;
        let no_color = no_color
            || std::env::var_os("NO_COLOR").is_some()
            || std::env::var("CLICOLOR").is_ok_and(|value| value == "0")
            || std::env::var("TERM").is_ok_and(|term| term == "dumb");
        let mut app = Self {
            screen,
            mode: Mode::Browse,
            quit: false,
            theme: Theme {
                color: !no_color,
                unicode: supports_unicode(),
            },
            run_id,
            db,
            paths,
            labs: Vec::new(),
            projects: Vec::new(),
            agents: Vec::new(),
            settings: None,
            settings_error: None,
            active_lab: None,
            active_project: None,
            selected: 0,
            page: 0,
            page_size: 8,
            filter: String::new(),
            status: None,
            home_selected: 0,
        };
        app.reload();
        Ok(app)
    }

    fn run(&mut self, terminal: &mut DefaultTerminal) -> Result<(), AppError> {
        loop {
            terminal
                .draw(|frame| self.render(frame))
                .map_err(|error| AppError::Io {
                    context: "drawing the terminal dashboard",
                    kind: error.kind(),
                })?;
            if self.quit {
                return Ok(());
            }
            let event = event::read().map_err(|error| AppError::Io {
                context: "reading terminal input",
                kind: error.kind(),
            })?;
            match event {
                Event::Key(key) if key.kind == KeyEventKind::Press => {
                    if let Some(deferred) = self.handle_key(key) {
                        let _ = ratatui::try_restore();
                        let result = self.execute_deferred(deferred);
                        *terminal = ratatui::try_init().map_err(|error| AppError::Io {
                            context: "restarting the terminal dashboard",
                            kind: error.kind(),
                        })?;
                        match result {
                            Ok(message) => {
                                self.status = Some(message);
                                self.mode = Mode::Browse;
                                self.reload();
                            }
                            Err(error) => self.show_error(error),
                        }
                    }
                }
                Event::Resize(_, _) => {}
                _ => {}
            }
        }
    }

    fn reload(&mut self) {
        self.active_lab = context::active_lab(&self.db)
            .ok()
            .and_then(|active| active.require().ok());
        self.active_project = context::active_project(&self.db)
            .ok()
            .and_then(|active| active.require().ok());
        match self.screen {
            Screen::Home => {}
            Screen::Group(DashboardGroup::Labs) => {
                self.labs = labs::list(&self.db).unwrap_or_default();
            }
            Screen::Group(DashboardGroup::Projects) => {
                self.projects = projects::list(&self.db, &self.db).unwrap_or_default();
                self.agents = agents::list(&self.db, &self.db).unwrap_or_default();
            }
            Screen::Group(DashboardGroup::Agents) => {
                self.agents = agents::list(&self.db, &self.db).unwrap_or_default();
            }
            Screen::Group(DashboardGroup::Settings) => {
                match settings::show(&self.settings_file()) {
                    Ok(report) => {
                        self.settings = Some(report);
                        self.settings_error = None;
                    }
                    Err(error) => {
                        self.settings = None;
                        self.settings_error = Some(format!("{} {}", error, error.hint()));
                    }
                }
            }
        }
        self.clamp_selection();
    }

    fn settings_file(&self) -> SettingsFile {
        SettingsFile::new(self.paths.config_dir(), self.paths.settings_file())
    }

    fn clamp_selection(&mut self) {
        let len = self.visible_len();
        if len == 0 {
            self.selected = 0;
            self.page = 0;
            return;
        }
        self.selected = self.selected.min(len - 1);
        let max_page = len.saturating_sub(1) / self.page_size.max(1);
        self.page = self.page.min(max_page);
        if self.selected < self.page * self.page_size {
            self.selected = self.page * self.page_size;
        }
    }

    fn visible_len(&self) -> usize {
        match self.screen {
            Screen::Home => HOME_ITEMS.len(),
            Screen::Group(DashboardGroup::Labs) => self
                .labs
                .iter()
                .filter(|item| self.matches(&format!("{} {}", item.lab.id.as_str(), item.lab.name)))
                .count(),
            Screen::Group(DashboardGroup::Projects) => self
                .projects
                .iter()
                .filter(|item| {
                    self.matches(&format!(
                        "{} {} {}",
                        item.id.as_str(),
                        item.name,
                        item.github_source.as_deref().unwrap_or_default()
                    ))
                })
                .count(),
            Screen::Group(DashboardGroup::Agents) => self
                .agents
                .iter()
                .filter(|item| {
                    self.matches(&format!(
                        "{} {} {}",
                        item.record.id.as_str(),
                        item.record.name,
                        item.record.role
                    ))
                })
                .count(),
            Screen::Group(DashboardGroup::Settings) => 1,
        }
    }

    fn matches(&self, value: &str) -> bool {
        self.filter.is_empty()
            || value
                .to_ascii_lowercase()
                .contains(&self.filter.to_ascii_lowercase())
    }

    fn selected_lab(&self) -> Option<&LabListItem> {
        self.filtered_index(self.selected)
            .and_then(|index| self.labs.get(index))
    }

    fn selected_project(&self) -> Option<&ProjectOverview> {
        self.filtered_index(self.selected)
            .and_then(|index| self.projects.get(index))
    }

    fn selected_agent(&self) -> Option<&AgentListItem> {
        self.filtered_index(self.selected)
            .and_then(|index| self.agents.get(index))
    }

    fn filtered_index(&self, visible: usize) -> Option<usize> {
        match self.screen {
            Screen::Group(DashboardGroup::Labs) => self
                .labs
                .iter()
                .enumerate()
                .filter(|(_, item)| {
                    self.matches(&format!("{} {}", item.lab.id.as_str(), item.lab.name))
                })
                .nth(visible)
                .map(|(index, _)| index),
            Screen::Group(DashboardGroup::Projects) => self
                .projects
                .iter()
                .enumerate()
                .filter(|(_, item)| {
                    self.matches(&format!(
                        "{} {} {}",
                        item.id.as_str(),
                        item.name,
                        item.github_source.as_deref().unwrap_or_default()
                    ))
                })
                .nth(visible)
                .map(|(index, _)| index),
            Screen::Group(DashboardGroup::Agents) => self
                .agents
                .iter()
                .enumerate()
                .filter(|(_, item)| {
                    self.matches(&format!(
                        "{} {} {}",
                        item.record.id.as_str(),
                        item.record.name,
                        item.record.role
                    ))
                })
                .nth(visible)
                .map(|(index, _)| index),
            _ => None,
        }
    }

    fn handle_key(&mut self, key: KeyEvent) -> Option<Deferred> {
        let mode = std::mem::replace(&mut self.mode, Mode::Browse);
        match mode {
            Mode::Browse => self.handle_browse(key),
            Mode::Filter(mut text) => {
                match key.code {
                    KeyCode::Esc => {
                        self.filter.clear();
                        self.selected = 0;
                        self.page = 0;
                    }
                    KeyCode::Enter => {
                        self.filter = text;
                        self.selected = 0;
                        self.page = 0;
                    }
                    KeyCode::Backspace => {
                        text.pop();
                        self.mode = Mode::Filter(text);
                    }
                    KeyCode::Char(ch) => {
                        text.push(ch);
                        self.mode = Mode::Filter(text);
                    }
                    _ => self.mode = Mode::Filter(text),
                }
                self.clamp_selection();
                None
            }
            Mode::LabForm {
                id,
                mut name,
                mut description,
                description_focus,
            } => {
                match key.code {
                    KeyCode::Esc => {}
                    KeyCode::Tab => {
                        self.mode = Mode::LabForm {
                            id,
                            name,
                            description,
                            description_focus: !description_focus,
                        };
                    }
                    KeyCode::Enter => {
                        if !name.trim().is_empty() {
                            let result = match &id {
                                Some(id) => self.update_lab(id, &name, &description),
                                None => self.create_lab(&name, &description),
                            };
                            self.apply_result(result);
                        }
                    }
                    KeyCode::Backspace => {
                        if description_focus {
                            description.pop();
                        } else {
                            name.pop();
                        }
                        self.mode = Mode::LabForm {
                            id,
                            name,
                            description,
                            description_focus,
                        };
                    }
                    KeyCode::Char(ch) => {
                        if description_focus {
                            description.push(ch);
                        } else {
                            name.push(ch);
                        }
                        self.mode = Mode::LabForm {
                            id,
                            name,
                            description,
                            description_focus,
                        };
                    }
                    _ => {
                        self.mode = Mode::LabForm {
                            id,
                            name,
                            description,
                            description_focus,
                        };
                    }
                }
                None
            }
            Mode::ProjectForm {
                mut name,
                mut source,
                source_focus,
            } => {
                match key.code {
                    KeyCode::Esc => {}
                    KeyCode::Tab => {
                        self.mode = Mode::ProjectForm {
                            name,
                            source,
                            source_focus: !source_focus,
                        };
                    }
                    KeyCode::Enter => {
                        if !name.trim().is_empty() {
                            let result = self.create_project(&name, &source);
                            self.apply_result(result);
                        }
                    }
                    KeyCode::Backspace => {
                        if source_focus {
                            source.pop();
                        } else {
                            name.pop();
                        }
                        self.mode = Mode::ProjectForm {
                            name,
                            source,
                            source_focus,
                        };
                    }
                    KeyCode::Char(ch) => {
                        if source_focus {
                            source.push(ch);
                        } else {
                            name.push(ch);
                        }
                        self.mode = Mode::ProjectForm {
                            name,
                            source,
                            source_focus,
                        };
                    }
                    _ => {
                        self.mode = Mode::ProjectForm {
                            name,
                            source,
                            source_focus,
                        };
                    }
                }
                None
            }
            Mode::Confirm(action) => {
                match key.code {
                    KeyCode::Char('y') | KeyCode::Char('Y') => {
                        let result = self.execute_confirm(action);
                        self.apply_result(result);
                    }
                    KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {}
                    _ => self.mode = Mode::Confirm(action),
                }
                None
            }
            Mode::Message { .. } | Mode::Help => None,
            Mode::Picker {
                agents,
                mut selected,
            } => {
                match key.code {
                    KeyCode::Esc => {}
                    KeyCode::Up | KeyCode::Char('k') => {
                        selected = selected.saturating_sub(1);
                        self.mode = Mode::Picker { agents, selected };
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        if selected + 1 < agents.len() {
                            selected += 1;
                        }
                        self.mode = Mode::Picker { agents, selected };
                    }
                    KeyCode::Enter => {
                        if let Some((id, _)) = agents.get(selected) {
                            let result = self.assign_agent(id);
                            self.apply_result(result);
                        }
                    }
                    _ => self.mode = Mode::Picker { agents, selected },
                }
                None
            }
        }
    }

    fn handle_browse(&mut self, key: KeyEvent) -> Option<Deferred> {
        match self.screen {
            Screen::Home => {
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => self.quit = true,
                    KeyCode::Up | KeyCode::Char('k') => {
                        self.home_selected = self.home_selected.saturating_sub(1)
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        self.home_selected = (self.home_selected + 1).min(HOME_ITEMS.len() - 1)
                    }
                    KeyCode::Enter => {
                        self.screen = Screen::Group(match HOME_ITEMS[self.home_selected] {
                            ScreenItem::Group(group) => group,
                        });
                        self.selected = 0;
                        self.page = 0;
                        self.filter.clear();
                        self.reload();
                    }
                    KeyCode::Char('1') => self.open_group(DashboardGroup::Labs),
                    KeyCode::Char('2') => self.open_group(DashboardGroup::Projects),
                    KeyCode::Char('3') => self.open_group(DashboardGroup::Agents),
                    KeyCode::Char('4') => self.open_group(DashboardGroup::Settings),
                    _ => {}
                }
                None
            }
            Screen::Group(group) => {
                match key.code {
                    KeyCode::Char('q') => self.quit = true,
                    KeyCode::Esc => {
                        if self.filter.is_empty() {
                            self.home_selected = match group {
                                DashboardGroup::Labs => 0,
                                DashboardGroup::Projects => 1,
                                DashboardGroup::Agents => 2,
                                DashboardGroup::Settings => 3,
                            };
                            self.screen = Screen::Home;
                            self.selected = 0;
                            self.page = 0;
                            self.status = None;
                        } else {
                            self.filter.clear();
                            self.selected = 0;
                            self.page = 0;
                        }
                    }
                    KeyCode::Char('?') => self.mode = Mode::Help,
                    KeyCode::Char('/') => self.mode = Mode::Filter(self.filter.clone()),
                    KeyCode::Up | KeyCode::Char('k') => self.move_selection(-1),
                    KeyCode::Down | KeyCode::Char('j') => self.move_selection(1),
                    KeyCode::Left | KeyCode::Char('h') => self.change_page(-1),
                    KeyCode::Right | KeyCode::Char('l') => self.change_page(1),
                    KeyCode::Char('g') => {
                        self.page = 0;
                        self.selected = 0;
                    }
                    KeyCode::Char('G') => {
                        let len = self.visible_len();
                        if len > 0 {
                            self.page = len.saturating_sub(1) / self.page_size.max(1);
                            self.selected = len - 1;
                        }
                    }
                    KeyCode::Enter => {
                        let result = self.open_selected(group);
                        self.apply_result(result);
                    }
                    KeyCode::Char('n') => match group {
                        DashboardGroup::Labs => {
                            self.mode = Mode::LabForm {
                                id: None,
                                name: String::new(),
                                description: String::new(),
                                description_focus: false,
                            }
                        }
                        DashboardGroup::Projects => {
                            self.mode = Mode::ProjectForm {
                                name: String::new(),
                                source: String::new(),
                                source_focus: false,
                            }
                        }
                        DashboardGroup::Agents => return Some(Deferred::NewAgent),
                        DashboardGroup::Settings => return Some(Deferred::EditSettings),
                    },
                    KeyCode::Char('c') if group == DashboardGroup::Labs => {
                        let result = self.check_selected_lab();
                        self.apply_result(result);
                    }
                    KeyCode::Char('e') if group == DashboardGroup::Labs => {
                        if let Some(lab) = self.selected_lab() {
                            self.mode = Mode::LabForm {
                                id: Some(lab.lab.id.clone()),
                                name: lab.lab.name.clone(),
                                description: lab.lab.description.clone(),
                                description_focus: false,
                            };
                        } else {
                            self.status = Some("Select a laboratory to edit.".to_owned());
                        }
                    }
                    KeyCode::Char('p') if group == DashboardGroup::Labs => {
                        let result = self.open_projects_for_selected_lab();
                        self.apply_result(result);
                    }
                    KeyCode::Char('a') => match group {
                        DashboardGroup::Labs => {
                            let result = self.open_selected_lab();
                            self.apply_result(result);
                        }
                        DashboardGroup::Projects => {
                            if self.active_project.is_none() {
                                self.status = Some(
                                    "Open a project first; `a` assigns an agent to the current project."
                                        .to_owned(),
                                );
                            } else {
                                self.mode = Mode::Picker {
                                    agents: self
                                        .agents
                                        .iter()
                                        .map(|agent| {
                                            (agent.record.id.to_string(), agent.record.name.clone())
                                        })
                                        .collect(),
                                    selected: 0,
                                };
                            }
                        }
                        DashboardGroup::Agents => {
                            if let Some(id) = self
                                .selected_agent()
                                .map(|agent| agent.record.id.to_string())
                            {
                                let result = self.assign_agent(&id);
                                self.apply_result(result);
                            }
                        }
                        DashboardGroup::Settings => {}
                    },
                    KeyCode::Char('d') => {
                        let result = self.confirm_selected();
                        if let Err(error) = result {
                            self.show_error(error);
                        }
                    }
                    KeyCode::Char('v') if group == DashboardGroup::Projects => {
                        let result = self.show_kanban();
                        self.apply_result(result);
                    }
                    KeyCode::Char('e') if group == DashboardGroup::Settings => {
                        return Some(Deferred::EditSettings);
                    }
                    KeyCode::Char('r') if group == DashboardGroup::Settings => {
                        self.reload();
                        self.status = Some("Settings reloaded.".to_owned());
                    }
                    KeyCode::Char('p') if group == DashboardGroup::Settings => {
                        self.status = Some(self.settings_file().path().display().to_string());
                    }
                    KeyCode::Char('c') if group == DashboardGroup::Settings => {
                        self.status = Some(self.credential_summary());
                    }
                    _ => {}
                }
                None
            }
        }
    }

    fn open_group(&mut self, group: DashboardGroup) {
        self.screen = Screen::Group(group);
        self.selected = 0;
        self.page = 0;
        self.filter.clear();
        self.reload();
    }

    fn move_selection(&mut self, delta: isize) {
        let len = self.visible_len();
        if len == 0 {
            return;
        }
        if delta < 0 {
            self.selected = self.selected.saturating_sub(1);
        } else {
            self.selected = (self.selected + 1).min(len - 1);
        }
        let page_start = self.page * self.page_size;
        if self.selected < page_start {
            self.page = self.page.saturating_sub(1);
        } else if self.selected >= page_start + self.page_size {
            self.page += 1;
        }
    }

    fn change_page(&mut self, delta: isize) {
        let len = self.visible_len();
        let max_page = len.saturating_sub(1) / self.page_size.max(1);
        if delta < 0 {
            self.page = self.page.saturating_sub(1);
        } else if self.page < max_page {
            self.page += 1;
        }
        self.selected = (self.page * self.page_size).min(len.saturating_sub(1));
    }

    fn open_selected(&mut self, group: DashboardGroup) -> Result<(), AppError> {
        match group {
            DashboardGroup::Labs => self.open_selected_lab(),
            DashboardGroup::Projects => {
                let project_id = self
                    .selected_project()
                    .map(|project| project.id.clone())
                    .ok_or_else(|| AppError::Precondition {
                        problem: "no project is selected.".to_owned(),
                        hint: "Create a project or move the selection.".to_owned(),
                    })?;
                projects::open(&self.db, &self.db, project_id.as_str())?;
                self.reload();
                self.status = Some(format!("Current project: {project_id}."));
                Ok(())
            }
            DashboardGroup::Agents => {
                let agent = self
                    .selected_agent()
                    .ok_or_else(|| AppError::Precondition {
                        problem: "no agent is selected.".to_owned(),
                        hint: "Move the selection.".to_owned(),
                    })?;
                let detail = agents::detail(&self.db, agent.record.id.as_str())?;
                self.mode = Mode::Message {
                    title: format!("Agent {}", detail.profile.id),
                    lines: agent_detail_lines(&detail),
                };
                Ok(())
            }
            DashboardGroup::Settings => {
                self.mode = Mode::Message {
                    title: "Settings".to_owned(),
                    lines: self.settings_lines(),
                };
                Ok(())
            }
        }
    }

    fn open_selected_lab(&mut self) -> Result<(), AppError> {
        let lab = self.selected_lab().ok_or_else(|| AppError::Precondition {
            problem: "no laboratory is selected.".to_owned(),
            hint: "Move the selection.".to_owned(),
        })?;
        let summary = labs::open(&self.db, &self.db, lab.lab.id.as_str())?;
        context::clear_active_project(&self.db)?;
        record_event(
            &mut self.db,
            self.run_id,
            EventKind::LabOpened,
            EntityType::Laboratory,
            summary.lab.id.as_str(),
        );
        self.reload();
        self.status = Some(format!(
            "Active laboratory: {} — {}",
            summary.lab.id, summary.lab.name
        ));
        if !summary.warnings.is_empty() {
            self.status = Some(format!(
                "Active laboratory set; {} validation warning(s).",
                summary.warnings.len()
            ));
        }
        Ok(())
    }

    fn check_selected_lab(&mut self) -> Result<(), AppError> {
        let lab = self.selected_lab().ok_or_else(|| AppError::Precondition {
            problem: "no laboratory is selected.".to_owned(),
            hint: "Move the selection.".to_owned(),
        })?;
        let check = labs::check(&self.db, lab.lab.id.as_str())?;
        let mut lines = vec![format!("{} — {}", check.lab.id, check.lab.name)];
        for rule in &check.rules {
            lines.push(format!(
                "{} {}{}",
                if rule.passed { "ok" } else { "fail" },
                rule.rule,
                rule.detail
                    .as_ref()
                    .map(|detail| format!(": {detail}"))
                    .unwrap_or_default()
            ));
        }
        self.mode = Mode::Message {
            title: "Laboratory check".to_owned(),
            lines,
        };
        Ok(())
    }

    fn confirm_selected(&mut self) -> Result<(), AppError> {
        match self.screen {
            Screen::Group(DashboardGroup::Labs) => {
                let lab = self.selected_lab().ok_or_else(|| AppError::Precondition {
                    problem: "no laboratory is selected.".to_owned(),
                    hint: "Move the selection.".to_owned(),
                })?;
                self.mode =
                    Mode::Confirm(ConfirmAction::Lab(lab.lab.id.clone(), lab.lab.name.clone()));
            }
            Screen::Group(DashboardGroup::Projects) => {
                let project = self
                    .selected_project()
                    .ok_or_else(|| AppError::Precondition {
                        problem: "no project is selected.".to_owned(),
                        hint: "Move the selection.".to_owned(),
                    })?;
                self.mode = Mode::Confirm(ConfirmAction::Project(
                    project.id.clone(),
                    project.name.clone(),
                ));
            }
            Screen::Group(DashboardGroup::Agents) => {
                let agent = self
                    .selected_agent()
                    .ok_or_else(|| AppError::Precondition {
                        problem: "no agent is selected.".to_owned(),
                        hint: "Move the selection.".to_owned(),
                    })?;
                self.mode = Mode::Confirm(ConfirmAction::Agent(
                    agent.record.id.clone(),
                    agent.record.name.clone(),
                ));
            }
            Screen::Group(DashboardGroup::Settings) | Screen::Home => {
                self.status = Some("This group has no delete action.".to_owned());
            }
        }
        Ok(())
    }

    fn execute_confirm(&mut self, action: ConfirmAction) -> Result<(), AppError> {
        match action {
            ConfirmAction::Lab(id, name) => {
                let plan = deletions::preflight_lab(&self.db, id.as_str())?;
                deletions::delete_lab(&self.db, &plan)?;
                context::clear_active_project(&self.db)?;
                record_event(
                    &mut self.db,
                    self.run_id,
                    EventKind::LabDeleted,
                    EntityType::Laboratory,
                    plan.id.as_str(),
                );
                self.status = Some(format!("Laboratory {name} deleted."));
            }
            ConfirmAction::Project(id, name) => {
                let plan = deletions::preflight_project(&self.db, &self.db, &id)?;
                deletions::delete_project(&self.db, &plan)?;
                context::clear_active_project(&self.db)?;
                record_event(
                    &mut self.db,
                    self.run_id,
                    EventKind::ProjectDeleted,
                    EntityType::Project,
                    plan.id.as_str(),
                );
                self.status = Some(format!("Project {name} deleted."));
            }
            ConfirmAction::Agent(id, name) => {
                let plan = deletions::preflight_agent(&self.db, id.as_str())?;
                deletions::delete_agent(&self.db, &plan)?;
                record_event(
                    &mut self.db,
                    self.run_id,
                    EventKind::AgentArchived,
                    EntityType::Agent,
                    plan.id.as_str(),
                );
                self.status = Some(format!("Agent {name} archived."));
            }
        }
        self.reload();
        Ok(())
    }

    fn create_lab(&mut self, name: &str, description: &str) -> Result<(), AppError> {
        let lab = labs::create_with_description(&self.db, name, description)?;
        if let Some(memory) = memories::remember(
            &self.db,
            &self.db,
            MemoryScope::Lab(lab.id.clone()),
            &memories::lab_created_content(&lab),
            self.secret().as_deref(),
        ) {
            record_event(
                &mut self.db,
                self.run_id,
                EventKind::MemoryCreated,
                EntityType::Memory,
                memory.id.as_str(),
            );
        }
        record_event(
            &mut self.db,
            self.run_id,
            EventKind::LabCreated,
            EntityType::Laboratory,
            lab.id.as_str(),
        );
        self.reload();
        self.status = Some(format!("Laboratory {} created.", lab.id));
        Ok(())
    }

    fn update_lab(&mut self, id: &LabId, name: &str, description: &str) -> Result<(), AppError> {
        let lab = labs::update(&self.db, id.as_str(), name, description)?;
        record_event(
            &mut self.db,
            self.run_id,
            EventKind::LabUpdated,
            EntityType::Laboratory,
            lab.id.as_str(),
        );
        self.reload();
        self.status = Some(format!("Laboratory {} updated.", lab.id));
        Ok(())
    }

    fn open_projects_for_selected_lab(&mut self) -> Result<(), AppError> {
        let id = self
            .selected_lab()
            .map(|lab| lab.lab.id.clone())
            .ok_or_else(|| AppError::Precondition {
                problem: "no laboratory is selected.".to_owned(),
                hint: "Move the selection or create a laboratory.".to_owned(),
            })?;
        let summary = labs::open(&self.db, &self.db, id.as_str())?;
        context::clear_active_project(&self.db)?;
        record_event(
            &mut self.db,
            self.run_id,
            EventKind::LabOpened,
            EntityType::Laboratory,
            summary.lab.id.as_str(),
        );
        self.screen = Screen::Group(DashboardGroup::Projects);
        self.selected = 0;
        self.page = 0;
        self.filter.clear();
        self.status = Some(format!(
            "Active laboratory: {} — {}",
            summary.lab.id, summary.lab.name
        ));
        self.reload();
        Ok(())
    }

    fn create_project(&mut self, name: &str, source: &str) -> Result<(), AppError> {
        let github_source = if source.trim().is_empty() {
            None
        } else {
            Some(
                GithubSource::parse(source).map_err(|error| AppError::Usage {
                    message: error.to_string(),
                })?,
            )
        };
        let mut token = None;
        if github_source.is_some() {
            let settings = settings::show(&self.settings_file())?.settings;
            if settings.source_control.provider
                != crate::domain::settings::SourceControlProvider::Github
            {
                return Err(AppError::Configuration {
                    problem: "the configured source-control provider is not supported.".to_owned(),
                    hint: "Set `source_control.provider` to `github` with `vlab settings`."
                        .to_owned(),
                });
            }
            if !settings.credentials.github_present() {
                return Err(AppError::Configuration {
                    problem: "`credentials.github_api_key` is not set.".to_owned(),
                    hint: "Edit settings.yaml and add credentials.github_api_key.".to_owned(),
                });
            }
            token = settings.credentials.github_api_key.clone();
        }
        let workspace = GitWorkspace::new(self.paths.workspace_dir(), token, DEFAULT_CLONE_TIMEOUT);
        let created: CreatedProject =
            projects::create(&self.db, &self.db, name, github_source, &workspace)?;
        context::save_active_project(&self.db, &created.project.id)?;
        if let Some(memory) = memories::remember(
            &self.db,
            &self.db,
            MemoryScope::Project(created.project.id.clone()),
            &memories::project_created_content(&created.project),
            self.secret().as_deref(),
        ) {
            record_event(
                &mut self.db,
                self.run_id,
                EventKind::MemoryCreated,
                EntityType::Memory,
                memory.id.as_str(),
            );
        }
        record_event(
            &mut self.db,
            self.run_id,
            EventKind::ProjectCreated,
            EntityType::Project,
            created.project.id.as_str(),
        );
        self.reload();
        self.status = Some(format!("Project {} created.", created.project.id));
        Ok(())
    }

    fn assign_agent(&mut self, agent_id: &str) -> Result<(), AppError> {
        let project = projects::assign_agent(&self.db, &self.db, agent_id)?;
        self.reload();
        self.status = Some(format!("Agent assigned to project {}.", project.id));
        Ok(())
    }

    fn show_kanban(&mut self) -> Result<(), AppError> {
        let project = self
            .selected_project()
            .ok_or_else(|| AppError::Precondition {
                problem: "no project is selected.".to_owned(),
                hint: "Move the selection.".to_owned(),
            })?;
        let board = kanban::show(&self.db, &self.db, project.id.as_str())?;
        self.mode = Mode::Message {
            title: format!("Kanban {}", board.project_id),
            lines: kanban_lines(&board),
        };
        Ok(())
    }

    fn execute_deferred(&mut self, deferred: Deferred) -> Result<String, AppError> {
        match deferred {
            Deferred::EditSettings => {
                let editor = TerminalEditor::from_environment()?;
                settings_edit::edit(&self.settings_file(), &editor)?;
                self.reload();
                Ok("Settings edited.".to_owned())
            }
            Deferred::NewAgent => {
                let settings = settings::show(&self.settings_file())?.settings;
                let mut prompter = TerminalPrompter;
                let profile = agents::wizard::run(&mut prompter, &self.db, &self.db, &settings)?;
                let memberships: Vec<LabId> = self
                    .db
                    .agent_memberships(&profile.id)?
                    .into_iter()
                    .map(|lab| lab.id)
                    .collect();
                if let Some(memory) = memories::remember(
                    &self.db,
                    &self.db,
                    MemoryScope::Agent(profile.id.clone()),
                    &memories::agent_created_content(&profile, &memberships),
                    settings.credentials.deepseek_api_key.as_deref(),
                ) {
                    record_event(
                        &mut self.db,
                        self.run_id,
                        EventKind::MemoryCreated,
                        EntityType::Memory,
                        memory.id.as_str(),
                    );
                }
                record_event(
                    &mut self.db,
                    self.run_id,
                    EventKind::AgentCreated,
                    EntityType::Agent,
                    profile.id.as_str(),
                );
                self.reload();
                Ok(format!("Agent {} created.", profile.id))
            }
        }
    }

    fn apply_result(&mut self, result: Result<(), AppError>) {
        match result {
            Ok(()) => {
                // Actions that open a report or detail modal set the mode
                // themselves; only a completed form/confirm returns to browse.
                if !matches!(
                    self.mode,
                    Mode::Message { .. } | Mode::Confirm(_) | Mode::Picker { .. } | Mode::Help
                ) {
                    self.mode = Mode::Browse;
                }
            }
            Err(error) => self.show_error(error),
        }
    }

    fn show_error(&mut self, error: AppError) {
        self.mode = Mode::Message {
            title: "Operation failed".to_owned(),
            lines: vec![clean(&error.to_string()), clean(&error.hint())],
        };
    }

    fn secret(&self) -> Option<String> {
        self.settings
            .as_ref()
            .and_then(|report| report.settings.credentials.deepseek_api_key.clone())
            .or_else(|| {
                settings::show(&self.settings_file())
                    .ok()
                    .and_then(|report| report.settings.credentials.deepseek_api_key)
            })
    }

    fn settings_lines(&self) -> Vec<String> {
        match (&self.settings, &self.settings_error) {
            (Some(report), _) => vec![
                format!("Path: {}", clean(&report.path.display().to_string())),
                format!("Provider: {}", report.settings.provider),
                format!("Model: {}", report.settings.model),
                format!(
                    "Source control: {}",
                    report.settings.source_control.provider
                ),
                format!(
                    "DeepSeek API key: {}",
                    if report.settings.credentials.deepseek_present() {
                        "set"
                    } else {
                        "unset"
                    }
                ),
                format!(
                    "GitHub API key: {}",
                    if report.settings.credentials.github_present() {
                        "set"
                    } else {
                        "unset"
                    }
                ),
            ],
            (None, Some(error)) => vec![error.clone()],
            (None, None) => vec!["Settings are not loaded.".to_owned()],
        }
    }

    fn credential_summary(&self) -> String {
        self.settings
            .as_ref()
            .map(|report| {
                format!(
                    "DeepSeek: {} · GitHub: {}",
                    if report.settings.credentials.deepseek_present() {
                        "set"
                    } else {
                        "unset"
                    },
                    if report.settings.credentials.github_present() {
                        "set"
                    } else {
                        "unset"
                    }
                )
            })
            .unwrap_or_else(|| "Settings are not loaded.".to_owned())
    }

    fn render(&mut self, frame: &mut Frame<'_>) {
        let area = frame.area();
        if area.width < 60 || area.height < 20 {
            frame.render_widget(
                Paragraph::new("Terminal too small. Minimum: 60x20.")
                    .alignment(Alignment::Center)
                    .block(Block::bordered().title("VLab")),
                area,
            );
            return;
        }
        match self.screen {
            Screen::Home => self.render_home(frame, area),
            Screen::Group(group) => self.render_group(frame, area, group),
        }
        if !matches!(self.mode, Mode::Browse) {
            self.render_modal(frame, area);
        }
    }

    fn render_home(&self, frame: &mut Frame<'_>, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(1),
                Constraint::Length(5),
            ])
            .split(area);
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled("VLab", self.theme.title()),
                Span::raw("  virtual laboratories"),
            ]))
            .block(self.block("Home")),
            chunks[0],
        );

        let label_width = 16usize;
        let separator = if self.theme.unicode {
            "  │  "
        } else {
            "  |  "
        };
        let mut lines = vec![home_header(self.theme, label_width, separator)];
        for (index, item) in HOME_ITEMS.iter().enumerate() {
            lines.push(home_line(
                *item,
                index == self.home_selected,
                self.theme,
                label_width,
                separator,
            ));
        }
        frame.render_widget(Paragraph::new(lines).block(self.block("Groups")), chunks[1]);

        let mut actions = action_grid(
            &[
                ("[↑/↓]", "Select"),
                ("[Enter]", "Open"),
                ("[1-4]", "Direct"),
                ("[q]", "Quit"),
            ],
            self.theme,
            usize::from(area.width.saturating_sub(2)),
        );
        actions.push(Line::from(Span::styled(
            "Choose a group to manage; Esc in a dashboard returns here.",
            self.theme.muted_style(),
        )));
        frame.render_widget(
            Paragraph::new(actions).block(self.block("Actions")),
            chunks[2],
        );
    }

    fn render_group(&mut self, frame: &mut Frame<'_>, area: Rect, group: DashboardGroup) {
        let mut footer = action_lines(group, self.theme, usize::from(area.width.saturating_sub(2)));
        if !self.filter.is_empty() {
            footer.push(Line::from(vec![
                Span::styled("Filter: ", self.theme.key()),
                Span::raw(clean(&self.filter)),
            ]));
        }
        if let Some(status) = &self.status {
            footer.push(Line::from(vec![
                Span::styled("Status: ", self.theme.key()),
                Span::styled(clean(status), self.theme.success_style()),
            ]));
        }
        let footer_height = u16::try_from(footer.len() + 2).unwrap_or(6).max(4);
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(1),
                Constraint::Length(footer_height),
            ])
            .split(area);
        let context = self.context_line(group);
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(group_title(group), self.theme.title()),
                Span::raw("  "),
                Span::styled(context, self.theme.muted_style()),
            ]))
            .block(self.block(group_title(group))),
            chunks[0],
        );
        self.page_size = usize::from(chunks[1].height.saturating_sub(2)).max(1);
        self.clamp_selection();
        self.render_body(frame, chunks[1], group);
        frame.render_widget(
            Paragraph::new(footer).block(self.block("Actions")),
            chunks[2],
        );
    }

    fn render_body(&mut self, frame: &mut Frame<'_>, area: Rect, group: DashboardGroup) {
        let start = self.page * self.page_size;
        let end = (start + self.page_size).min(self.visible_len());
        let widths = table_widths(group, usize::from(area.width.saturating_sub(2)));
        let mut lines = Vec::new();
        match group {
            DashboardGroup::Labs => {
                lines.push(table_header(
                    &["ID", "NAME", "DESCRIPTION", "AGENTS", "PROJECTS", "ACTIVE"],
                    &widths,
                    self.theme,
                ));
                for visible in start..end {
                    if let Some(index) = self.filtered_index(visible) {
                        let item = &self.labs[index];
                        let active = self
                            .active_lab
                            .as_ref()
                            .is_some_and(|id| id == &item.lab.id);
                        lines.push(table_row(
                            &[
                                item.lab.id.to_string(),
                                if item.check.valid() {
                                    item.lab.name.clone()
                                } else {
                                    format!("! {}", item.lab.name)
                                },
                                if item.lab.description.is_empty() {
                                    "—".to_owned()
                                } else {
                                    item.lab.description.clone()
                                },
                                item.lab.agent_count.to_string(),
                                item.lab.project_count.to_string(),
                                if active { "*" } else { "" }.to_owned(),
                            ],
                            &widths,
                            visible == self.selected,
                            self.theme,
                        ));
                    }
                }
            }
            DashboardGroup::Projects => {
                lines.push(table_header(
                    &["ID", "NAME", "REQ", "STORY", "OPEN", "CURRENT"],
                    &widths,
                    self.theme,
                ));
                for visible in start..end {
                    if let Some(index) = self.filtered_index(visible) {
                        let item = &self.projects[index];
                        let current = self
                            .active_project
                            .as_ref()
                            .is_some_and(|id| id == &item.id);
                        lines.push(table_row(
                            &[
                                item.id.to_string(),
                                item.name.clone(),
                                item.request_count.to_string(),
                                item.story_count.to_string(),
                                (item.todo + item.in_progress + item.in_review + item.blocked)
                                    .to_string(),
                                if current { "*" } else { "" }.to_owned(),
                            ],
                            &widths,
                            visible == self.selected,
                            self.theme,
                        ));
                    }
                }
            }
            DashboardGroup::Agents => {
                lines.push(table_header(
                    &["ID", "NAME", "ROLE", "PROJECTS", "OPEN"],
                    &widths,
                    self.theme,
                ));
                for visible in start..end {
                    if let Some(index) = self.filtered_index(visible) {
                        let item = &self.agents[index];
                        lines.push(table_row(
                            &[
                                item.record.id.to_string(),
                                item.record.name.clone(),
                                item.record.role.clone(),
                                item.projects.to_string(),
                                item.open_stories.to_string(),
                            ],
                            &widths,
                            visible == self.selected,
                            self.theme,
                        ));
                    }
                }
            }
            DashboardGroup::Settings => {
                for line in self.settings_lines() {
                    lines.push(Line::from(line));
                }
            }
        }
        if self.visible_len() == 0 {
            lines.push(Line::from(Span::styled(
                empty_state(group),
                self.theme.muted_style(),
            )));
        }
        let page_count = self.visible_len().saturating_sub(1) / self.page_size.max(1) + 1;
        if self.visible_len() > self.page_size {
            lines.push(Line::from(Span::styled(
                format!(
                    "Page {}/{} · {} rows",
                    self.page + 1,
                    page_count,
                    self.visible_len()
                ),
                self.theme.muted_style(),
            )));
        }
        frame.render_widget(
            Paragraph::new(lines).block(self.block(group_title(group))),
            area,
        );
    }

    fn render_modal(&self, frame: &mut Frame<'_>, area: Rect) {
        let (title, lines) = match &self.mode {
            Mode::Filter(text) => (
                "Filter".to_owned(),
                vec![format!(
                    "Type to filter; Enter apply; Esc clear.\n{}",
                    clean(text)
                )],
            ),
            Mode::LabForm {
                id,
                name,
                description,
                description_focus,
            } => (
                if id.is_some() {
                    "Edit laboratory".to_owned()
                } else {
                    "New laboratory".to_owned()
                },
                vec![format!(
                    "Name: {}\nDescription (optional): {}\n\nTab switch · Enter save · Esc cancel{}",
                    clean(name),
                    clean(description),
                    if *description_focus {
                        "  [description]"
                    } else {
                        "  [name]"
                    }
                )],
            ),
            Mode::ProjectForm {
                name,
                source,
                source_focus,
            } => (
                "New project".to_owned(),
                vec![format!(
                    "Name: {}\nGitHub source (optional): {}\n\nTab switch · Enter create · Esc cancel{}",
                    clean(name),
                    clean(source),
                    if *source_focus {
                        "  [source]"
                    } else {
                        "  [name]"
                    }
                )],
            ),
            Mode::Confirm(action) => (
                "Confirm deletion".to_owned(),
                vec![format!(
                    "{}\n\nThis cannot be undone.  y confirm · n/Esc cancel",
                    confirm_label(action)
                )],
            ),
            Mode::Message { title, lines } => {
                (clean(title), lines.iter().map(|line| clean(line)).collect())
            }
            Mode::Help => ("Help".to_owned(), help_lines()),
            Mode::Picker { agents, selected } => {
                let mut lines: Vec<String> = agents
                    .iter()
                    .enumerate()
                    .map(|(index, (id, name))| {
                        format!(
                            "{} {id}  {name}",
                            if index == *selected { ">" } else { " " }
                        )
                    })
                    .collect();
                lines.push("\nEnter assign · Esc cancel".to_owned());
                ("Assign agent".to_owned(), lines)
            }
            Mode::Browse => return,
        };
        let rect = centered_rect(78, 60, area);
        frame.render_widget(Clear, rect);
        frame.render_widget(
            Paragraph::new(lines.join("\n"))
                .wrap(Wrap { trim: false })
                .block(self.block(&title)),
            rect,
        );
    }

    fn block(&self, title: &str) -> Block<'static> {
        Block::bordered()
            .border_type(if self.theme.unicode {
                BorderType::Rounded
            } else {
                BorderType::Plain
            })
            .title(title.to_owned())
            .border_style(self.theme.border_style())
    }

    fn context_line(&self, group: DashboardGroup) -> String {
        let lab = self
            .active_lab
            .as_ref()
            .map(|id| id.to_string())
            .unwrap_or_else(|| "no active lab".to_owned());
        let project = self
            .active_project
            .as_ref()
            .map(|id| id.to_string())
            .unwrap_or_else(|| "no current project".to_owned());
        match group {
            DashboardGroup::Labs => format!("{lab} · page {}", self.page + 1),
            DashboardGroup::Projects => format!("lab {lab} · current {project}"),
            DashboardGroup::Agents => format!("lab {lab} · current {project}"),
            DashboardGroup::Settings => self
                .settings
                .as_ref()
                .map(|report| report.path.display().to_string())
                .unwrap_or_else(|| "missing or invalid settings".to_owned()),
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct Theme {
    color: bool,
    unicode: bool,
}

impl Theme {
    fn title(self) -> Style {
        if self.color {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().add_modifier(Modifier::BOLD)
        }
    }

    fn muted_style(self) -> Style {
        if self.color {
            Style::default().fg(Color::DarkGray)
        } else {
            Style::default()
        }
    }

    fn key(self) -> Style {
        if self.color {
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().add_modifier(Modifier::BOLD)
        }
    }

    fn success_style(self) -> Style {
        if self.color {
            Style::default().fg(Color::Green)
        } else {
            Style::default()
        }
    }

    fn border_style(self) -> Style {
        if self.color {
            Style::default().fg(Color::DarkGray)
        } else {
            Style::default()
        }
    }

    fn selected(self) -> Style {
        if self.color {
            Style::default()
                .bg(Color::DarkGray)
                .fg(Color::White)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().add_modifier(Modifier::REVERSED)
        }
    }
}

fn home_header(theme: Theme, label_width: usize, separator: &str) -> Line<'static> {
    Line::from(vec![
        Span::raw("   "),
        Span::styled(
            format!("{:<label_width$}", "NAME"),
            theme.muted_style().add_modifier(Modifier::BOLD),
        ),
        Span::styled(separator.to_owned(), theme.border_style()),
        Span::styled(
            "DESCRIPTION",
            theme.muted_style().add_modifier(Modifier::BOLD),
        ),
    ])
}

fn home_line(
    item: ScreenItem,
    selected: bool,
    theme: Theme,
    label_width: usize,
    separator: &str,
) -> Line<'static> {
    let base = if selected {
        theme.selected()
    } else {
        Style::default()
    };
    let marker = if selected {
        if theme.unicode { "▸" } else { ">" }
    } else {
        " "
    };
    let description = if selected { base } else { theme.muted_style() };
    Line::from(vec![
        Span::styled(format!(" {marker} "), base),
        Span::styled(
            format!("{:<label_width$}", item.label()),
            base.add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            separator.to_owned(),
            if selected { base } else { theme.border_style() },
        ),
        Span::styled(item.home_description(), description),
    ])
}

fn supports_unicode() -> bool {
    let locale = std::env::var("LC_ALL")
        .or_else(|_| std::env::var("LC_CTYPE"))
        .or_else(|_| std::env::var("LANG"))
        .unwrap_or_default()
        .to_ascii_lowercase();
    locale.contains("utf-8") || locale.contains("utf8")
}

fn group_title(group: DashboardGroup) -> &'static str {
    match group {
        DashboardGroup::Labs => "Laboratories",
        DashboardGroup::Projects => "Projects",
        DashboardGroup::Agents => "Agents",
        DashboardGroup::Settings => "Settings",
    }
}

fn action_items(group: DashboardGroup) -> Vec<(&'static str, &'static str)> {
    match group {
        DashboardGroup::Labs => vec![
            ("[n]", "New"),
            ("[c]", "Check"),
            ("[e]", "Edit"),
            ("[a]", "Set active"),
            ("[p]", "Projects"),
            ("[d]", "Delete"),
            ("[/]", "Filter"),
            ("[?]", "Help"),
            ("[Esc]", "Back"),
            ("[q]", "Quit"),
        ],
        DashboardGroup::Projects => vec![
            ("[n]", "New"),
            ("[Enter]", "Open"),
            ("[a]", "Assign"),
            ("[v]", "Kanban"),
            ("[d]", "Delete"),
            ("[/]", "Filter"),
            ("[?]", "Help"),
            ("[Esc]", "Back"),
            ("[q]", "Quit"),
        ],
        DashboardGroup::Agents => vec![
            ("[n]", "New"),
            ("[Enter]", "Detail"),
            ("[a]", "Assign"),
            ("[d]", "Archive"),
            ("[/]", "Filter"),
            ("[?]", "Help"),
            ("[Esc]", "Back"),
            ("[q]", "Quit"),
        ],
        DashboardGroup::Settings => vec![
            ("[e]", "Edit"),
            ("[r]", "Reload"),
            ("[p]", "Path"),
            ("[c]", "Credentials"),
            ("[/]", "Filter"),
            ("[?]", "Help"),
            ("[Esc]", "Back"),
            ("[q]", "Quit"),
        ],
    }
}

fn action_lines(group: DashboardGroup, theme: Theme, width: usize) -> Vec<Line<'static>> {
    action_grid(&action_items(group), theme, width)
}

fn action_grid(
    items: &[(&'static str, &'static str)],
    theme: Theme,
    width: usize,
) -> Vec<Line<'static>> {
    if items.is_empty() {
        return Vec::new();
    }
    let key_width = items.iter().map(|(key, _)| key.len()).max().unwrap_or(0);
    let label_width = items
        .iter()
        .map(|(_, label)| label.len())
        .max()
        .unwrap_or(0);
    let cell_width = key_width + 1 + label_width + 2;
    let columns = (width / cell_width.max(1)).clamp(1, 4);
    items
        .chunks(columns)
        .map(|chunk| {
            let mut spans = Vec::new();
            for (index, (key, label)) in chunk.iter().enumerate() {
                if index > 0 {
                    spans.push(Span::raw("  "));
                }
                spans.push(Span::styled(*key, theme.key()));
                spans.push(Span::raw(" ".repeat(key_width - key.len() + 1)));
                spans.push(Span::styled(
                    format!("{label:<label_width$}"),
                    theme.muted_style(),
                ));
            }
            Line::from(spans)
        })
        .collect()
}

fn table_widths(group: DashboardGroup, available: usize) -> Vec<usize> {
    const GAP: usize = 2;
    match group {
        DashboardGroup::Labs => {
            let fixed = 8 + 5 + 6 + 6 + GAP * 5;
            let text = available.saturating_sub(fixed);
            let name = (text * 2 / 5).clamp(10, 30);
            let description = text.saturating_sub(name).max(10);
            vec![8, name, description, 5, 6, 6]
        }
        DashboardGroup::Projects => {
            let fixed = 8 + 5 + 5 + 5 + 8 + GAP * 5;
            vec![8, available.saturating_sub(fixed).clamp(12, 45), 5, 5, 5, 8]
        }
        DashboardGroup::Agents => {
            let fixed = 8 + 10 + 8 + 5 + GAP * 4;
            vec![8, available.saturating_sub(fixed).clamp(12, 45), 10, 8, 5]
        }
        DashboardGroup::Settings => Vec::new(),
    }
}

fn empty_state(group: DashboardGroup) -> &'static str {
    match group {
        DashboardGroup::Labs => "No laboratories. Press n to create one.",
        DashboardGroup::Projects => "No projects in the active laboratory. Press n to create one.",
        DashboardGroup::Agents => "No agents in the active laboratory.",
        DashboardGroup::Settings => "Settings are not loaded.",
    }
}

fn confirm_label(action: &ConfirmAction) -> String {
    match action {
        ConfirmAction::Lab(id, name) => format!("Delete laboratory {id} — {}?", clean(name)),
        ConfirmAction::Project(id, name) => format!("Delete project {id} — {}?", clean(name)),
        ConfirmAction::Agent(id, name) => {
            format!(
                "Archive agent {id} — {}? Its history is preserved.",
                clean(name)
            )
        }
    }
}

fn table_header(labels: &[&str], widths: &[usize], theme: Theme) -> Line<'static> {
    let mut spans = Vec::new();
    for (index, (label, width)) in labels.iter().zip(widths).enumerate() {
        if index > 0 {
            spans.push(Span::raw("  "));
        }
        spans.push(Span::styled(
            format!("{:<width$}", label, width = width),
            theme.muted_style().add_modifier(Modifier::BOLD),
        ));
    }
    Line::from(spans)
}

fn table_row(values: &[String], widths: &[usize], selected: bool, theme: Theme) -> Line<'static> {
    let style = if selected {
        theme.selected()
    } else {
        Style::default()
    };
    let mut spans = Vec::new();
    for (index, (value, width)) in values.iter().zip(widths).enumerate() {
        if index > 0 {
            spans.push(Span::styled("  ", style));
        }
        let clipped: String = clean(value).chars().take(*width).collect();
        spans.push(Span::styled(
            format!("{clipped:<width$}", width = width),
            style,
        ));
    }
    Line::from(spans)
}

fn clean(value: &str) -> String {
    crate::cli::render::sanitize(value)
}

fn agent_detail_lines(detail: &agents::AgentDetail) -> Vec<String> {
    let mut lines = vec![
        format!("Name: {}", clean(detail.profile.name.as_str())),
        format!("Role: {}", clean(detail.profile.role.as_str())),
        format!(
            "Description: {}",
            clean(detail.profile.description.as_str())
        ),
        format!(
            "Skills: {}",
            detail
                .profile
                .skills
                .iter()
                .map(|skill| clean(skill.as_str()))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        format!("Archived: {}", if detail.archived { "yes" } else { "no" }),
        format!("Memories: {}", detail.memories),
    ];
    lines.push("Projects:".to_owned());
    for (id, name) in &detail.projects {
        lines.push(format!("  {id} {}", clean(name)));
    }
    lines
}

fn kanban_lines(board: &kanban::Board) -> Vec<String> {
    let mut lines = vec![
        format!("Project: {} — {}", board.project_id, board.project_name),
        format!("Laboratory: {}", board.lab_name),
        format!(
            "Coordinator: {}",
            board.lead_name.as_deref().unwrap_or("missing")
        ),
        format!(
            "Stories: {} · TODO {} · IN PROGRESS {} · IN REVIEW {} · BLOCKED {} · DONE {}",
            board.counts.total(),
            board.counts.todo,
            board.counts.in_progress,
            board.counts.in_review,
            board.counts.blocked,
            board.counts.done
        ),
        format!("Pending intake: {}", board.pending_intake),
    ];
    for state in kanban::StoryState::ALL {
        lines.push(format!("{}:", state.label()));
        for card in board.cards_in(state) {
            lines.push(format!(
                "  {} {} [{}] due {}",
                card.id,
                clean(&card.title),
                clean(&card.priority),
                clean(&card.due_date)
            ));
        }
    }
    lines
}

fn help_lines() -> Vec<String> {
    vec![
        "j/k or ↑/↓  move selection".to_owned(),
        "h/l or ←/→  previous/next page".to_owned(),
        "g/G         first/last page".to_owned(),
        "Enter       open/confirm selection".to_owned(),
        "n           new item in this group".to_owned(),
        "a           set active or assign".to_owned(),
        "d           delete with confirmation".to_owned(),
        "/           filter".to_owned(),
        "?           this help".to_owned(),
        "Esc         back to home".to_owned(),
        "q           quit".to_owned(),
    ]
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(vertical[1])[1]
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
        crate::obs::events::EventSink::record(db, &event);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::directory::LabOverview;
    use crate::app::lab_validation::LabCheck;
    use crate::domain::settings::Settings;
    use crate::infra::paths::PlatformDirs;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use std::path::PathBuf;

    fn fixture(screen: Screen) -> (App, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
        }
        let database = dir.path().join("vlab.db");
        let db = Database::open(&database).unwrap();
        db.migrate(RunId::generate()).unwrap();
        let paths = Paths::resolve(
            |_| None,
            Some(PlatformDirs {
                config: dir.path().join("config"),
                data: dir.path().join("data"),
            }),
        )
        .unwrap();
        let app = App {
            screen,
            mode: Mode::Browse,
            quit: false,
            theme: Theme {
                color: false,
                unicode: true,
            },
            run_id: RunId::generate(),
            db,
            paths,
            labs: Vec::new(),
            projects: Vec::new(),
            agents: Vec::new(),
            settings: None,
            settings_error: None,
            active_lab: None,
            active_project: None,
            selected: 0,
            page: 0,
            page_size: 8,
            filter: String::new(),
            status: None,
            home_selected: 0,
        };
        (app, dir)
    }

    fn rendered(app: &mut App) -> String {
        rendered_at(app, 100, 30)
    }

    fn rendered_at(app: &mut App, width: u16, height: u16) -> String {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| app.render(frame)).unwrap();
        terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect()
    }

    #[test]
    fn the_labs_dashboard_renders_actions_and_empty_state() {
        let (mut app, _dir) = fixture(Screen::Group(DashboardGroup::Labs));
        let screen = rendered(&mut app);
        assert!(screen.contains("Laboratories"), "{screen}");
        assert!(screen.contains("[n]"), "{screen}");
        assert!(screen.contains("New"), "{screen}");
        assert!(screen.contains("No laboratories"), "{screen}");
    }

    #[test]
    fn the_home_dashboard_lists_every_group() {
        let (mut app, _dir) = fixture(Screen::Home);
        let screen = rendered(&mut app);
        for label in ["Laboratories", "Projects", "Agents", "Settings"] {
            assert!(screen.contains(label), "{label}: {screen}");
        }
    }

    #[test]
    fn home_rows_align_labels_and_descriptions_in_columns() {
        let line = home_line(
            ScreenItem::Group(DashboardGroup::Labs),
            false,
            Theme {
                color: false,
                unicode: true,
            },
            16,
            "  │  ",
        );
        assert_eq!(line.spans[1].content.chars().count(), 16);
        assert_eq!(line.spans[2].content.as_ref(), "  │  ");
        assert_eq!(
            line.spans[3].content.as_ref(),
            "create, check, activate, delete"
        );
        let header = home_header(
            Theme {
                color: false,
                unicode: true,
            },
            16,
            "  │  ",
        );
        assert_eq!(header.spans[1].content.as_ref(), "NAME            ");
        assert_eq!(header.spans[3].content.as_ref(), "DESCRIPTION");
    }

    #[test]
    fn monochrome_selection_never_relies_on_color() {
        let style = Theme {
            color: false,
            unicode: true,
        }
        .selected();
        assert!(style.add_modifier.contains(Modifier::REVERSED));
        assert_eq!(style.fg, None);
    }

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, ratatui::crossterm::event::KeyModifiers::NONE)
    }

    #[test]
    fn keyboard_state_machine_opens_forms_and_quits() {
        let (mut app, _dir) = fixture(Screen::Group(DashboardGroup::Labs));
        app.handle_key(key(KeyCode::Char('n')));
        assert!(matches!(app.mode, Mode::LabForm { .. }));
        app.mode = Mode::Browse;
        app.handle_key(key(KeyCode::Char('?')));
        assert!(matches!(app.mode, Mode::Help));
        app.mode = Mode::Browse;
        app.handle_key(key(KeyCode::Char('q')));
        assert!(app.quit);
    }

    #[test]
    fn help_overlay_never_leaks_colors_in_plain_theme() {
        let (mut app, _dir) = fixture(Screen::Group(DashboardGroup::Labs));
        app.mode = Mode::Help;
        let screen = rendered(&mut app);
        assert!(screen.contains("j/k"), "{screen}");
        assert!(screen.contains("Esc"), "{screen}");
        assert!(screen.contains("q           quit"), "{screen}");
    }

    #[test]
    fn rendered_cells_strip_terminal_control_sequences() {
        let line = table_row(
            &["evil\u{1b}[31mname".to_owned()],
            &[40],
            false,
            Theme {
                color: false,
                unicode: true,
            },
        );
        let rendered: String = line
            .spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect();
        assert!(!rendered.contains('\u{1b}'), "{rendered}");
        assert!(rendered.contains("evilname"), "{rendered}");
    }

    #[test]
    fn supported_terminal_sizes_render_without_panicking() {
        let (mut app, _dir) = fixture(Screen::Group(DashboardGroup::Projects));
        for (width, height) in [(80, 24), (120, 40), (60, 20)] {
            let screen = rendered_at(&mut app, width, height);
            assert!(screen.contains("Projects"), "{width}x{height}: {screen}");
        }
        let small = rendered_at(&mut app, 40, 10);
        assert!(small.contains("Terminal too small"), "{small}");
    }

    fn lab_row(index: usize) -> LabListItem {
        let id = LabId::parse(&format!("{:08x}", index + 1)).unwrap();
        let lab = LabOverview {
            id,
            name: format!("Lab {index}"),
            description: String::new(),
            agent_count: 0,
            project_count: 0,
        };
        LabListItem {
            check: LabCheck {
                lab: lab.clone(),
                rules: Vec::new(),
            },
            lab,
        }
    }

    #[test]
    fn pagination_moves_by_page_and_clamps_the_selection() {
        let (mut app, _dir) = fixture(Screen::Group(DashboardGroup::Labs));
        app.labs = (0..10).map(lab_row).collect();
        app.page_size = 3;
        app.change_page(1);
        assert_eq!((app.page, app.selected), (1, 3));
        app.move_selection(1);
        assert_eq!(app.selected, 4);
        for _ in 0..10 {
            app.change_page(1);
        }
        assert_eq!(app.page, 3);
        assert_eq!(app.selected, 9);
    }

    #[test]
    fn a_large_laboratory_list_renders_only_one_page() {
        let (mut app, _dir) = fixture(Screen::Group(DashboardGroup::Labs));
        app.labs = (0..10_000).map(lab_row).collect();
        let screen = rendered(&mut app);
        assert!(screen.contains("Lab 0"));
        assert!(!screen.contains("Lab 9999"));
    }

    #[test]
    fn settings_dashboard_never_renders_a_credential_value() {
        let (mut app, _dir) = fixture(Screen::Group(DashboardGroup::Settings));
        app.settings = Some(SettingsReport {
            path: PathBuf::from("/private/settings.yaml"),
            settings: Settings::with_credentials(
                "deepseek",
                "deepseek-chat",
                crate::domain::settings::SourceControl::default(),
                crate::domain::settings::Credentials::parse(Some("sk-secret".to_owned()), None)
                    .unwrap(),
            )
            .unwrap(),
        });
        let screen = rendered(&mut app);
        assert!(screen.contains("DeepSeek API key: set"), "{screen}");
        assert!(!screen.contains("sk-secret"), "{screen}");
    }

    #[test]
    fn settings_dashboard_shows_a_missing_or_invalid_file() {
        let (mut app, _dir) = fixture(Screen::Group(DashboardGroup::Settings));
        app.settings = None;
        app.settings_error = Some("invalid settings".to_owned());
        let screen = rendered(&mut app);
        assert!(screen.contains("invalid settings"), "{screen}");
    }

    #[test]
    fn help_lists_navigation_and_safety_keys() {
        let help = help_lines().join("\n");
        for key in ["j/k", "Enter", "d", "Esc", "q"] {
            assert!(help.contains(key), "{key}: {help}");
        }
    }
}
