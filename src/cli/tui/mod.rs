//! Interactive group dashboards (ADR-0038).
//!
//! The dashboards are a presentation layer over the existing application use
//! cases. They never add provider calls or bypass confirmation rules.

use ratatui::crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
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
use crate::app::requests::{self, PlanningOutcome};
use crate::app::settings::{self, SettingsReport, SettingsStore};
use crate::app::settings_edit;
use crate::cli::args::DashboardGroup;
use crate::cli::prompt::{TerminalEditor, TerminalPrompter};
use crate::domain::budget::{CancelToken, ExecutionLimits};
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
    let mut terminal = start_terminal("starting the terminal dashboard")?;
    let result = App::new(screen, no_color, run_id).and_then(|mut app| app.run(&mut terminal));
    stop_terminal();
    result
}

/// Enters the dashboard terminal mode with bracketed paste (ADR-0044), so a
/// pasted request arrives as one event instead of keystrokes.
fn start_terminal(context: &'static str) -> Result<DefaultTerminal, AppError> {
    let terminal = ratatui::try_init().map_err(|error| AppError::Io {
        context,
        kind: error.kind(),
    })?;
    let _ = ratatui::crossterm::execute!(
        std::io::stdout(),
        ratatui::crossterm::event::EnableBracketedPaste
    );
    Ok(terminal)
}

fn stop_terminal() {
    let _ = ratatui::crossterm::execute!(
        std::io::stdout(),
        ratatui::crossterm::event::DisableBracketedPaste
    );
    let _ = ratatui::try_restore();
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Screen {
    Home,
    Group(DashboardGroup),
    /// Full-screen board of one project (ADR-0043); state lives in
    /// `App::kanban`.
    Kanban,
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
            Self::Group(DashboardGroup::Projects) => "create, edit, request, board",
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
struct Form {
    kind: FormKind,
    fields: Vec<FormField>,
    focus: usize,
}

#[derive(Debug, Clone)]
enum FormKind {
    LabNew,
    LabEdit(LabId),
    ProjectNew,
    ProjectEdit(ProjectId),
}

#[derive(Debug, Clone)]
struct FormField {
    label: &'static str,
    value: String,
    cursor: usize,
}

impl FormField {
    fn new(label: &'static str, value: impl Into<String>) -> Self {
        let value = value.into();
        let cursor = value.chars().count();
        Self {
            label,
            value,
            cursor,
        }
    }

    fn insert(&mut self, ch: char) {
        if ch.is_control() {
            return;
        }
        let byte = self
            .value
            .char_indices()
            .nth(self.cursor)
            .map(|(index, _)| index)
            .unwrap_or(self.value.len());
        self.value.insert(byte, ch);
        self.cursor += 1;
    }

    /// Inserts typed or pasted text, keeping line breaks and dropping every
    /// other control character.
    fn insert_text(&mut self, text: &str) {
        for ch in text.replace("\r\n", "\n").replace('\r', "\n").chars() {
            match ch {
                '\n' => self.insert_raw('\n'),
                '\t' => self.insert_raw(' '),
                ch => self.insert(ch),
            }
        }
    }

    fn insert_raw(&mut self, ch: char) {
        let byte = self
            .value
            .char_indices()
            .nth(self.cursor)
            .map(|(index, _)| index)
            .unwrap_or(self.value.len());
        self.value.insert(byte, ch);
        self.cursor += 1;
    }

    fn backspace(&mut self) {
        if self.cursor == 0 {
            return;
        }
        let byte = self
            .value
            .char_indices()
            .nth(self.cursor - 1)
            .map(|(index, _)| index)
            .unwrap_or(self.value.len());
        self.value.remove(byte);
        self.cursor -= 1;
    }

    fn delete(&mut self) {
        if self.cursor >= self.value.chars().count() {
            return;
        }
        let byte = self
            .value
            .char_indices()
            .nth(self.cursor)
            .map(|(index, _)| index)
            .unwrap_or(self.value.len());
        self.value.remove(byte);
    }

    fn left(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
    }

    fn right(&mut self) {
        self.cursor = (self.cursor + 1).min(self.value.chars().count());
    }

    fn home(&mut self) {
        self.cursor = 0;
    }

    fn end(&mut self) {
        self.cursor = self.value.chars().count();
    }
}

impl Form {
    fn lab_new() -> Self {
        Self {
            kind: FormKind::LabNew,
            fields: vec![
                FormField::new("Name", ""),
                FormField::new("Description (optional)", ""),
            ],
            focus: 0,
        }
    }

    fn lab_edit(id: LabId, name: String, description: String) -> Self {
        Self {
            kind: FormKind::LabEdit(id),
            fields: vec![
                FormField::new("Name", name),
                FormField::new("Description (optional)", description),
            ],
            focus: 0,
        }
    }

    fn project_new() -> Self {
        Self {
            kind: FormKind::ProjectNew,
            fields: vec![
                FormField::new("Name", ""),
                FormField::new("GitHub source (optional)", ""),
            ],
            focus: 0,
        }
    }

    fn project_edit(id: ProjectId, name: String) -> Self {
        Self {
            kind: FormKind::ProjectEdit(id),
            fields: vec![FormField::new("Name", name)],
            focus: 0,
        }
    }

    fn next(&mut self) {
        self.focus = (self.focus + 1) % self.fields.len();
    }

    fn previous_wrapped(&mut self) {
        self.focus = if self.focus == 0 {
            self.fields.len() - 1
        } else {
            self.focus - 1
        };
    }

    fn current(&mut self) -> &mut FormField {
        &mut self.fields[self.focus]
    }

    fn title(&self) -> String {
        match &self.kind {
            FormKind::LabNew => "New laboratory".to_owned(),
            FormKind::LabEdit(id) => format!("Edit laboratory {id}"),
            FormKind::ProjectNew => "New project".to_owned(),
            FormKind::ProjectEdit(id) => format!("Edit project {id}"),
        }
    }

    fn values(&self) -> (String, String) {
        (
            self.fields[0].value.clone(),
            self.fields
                .get(1)
                .map(|field| field.value.clone())
                .unwrap_or_default(),
        )
    }
}

/// Request prompt dialog state (ADR-0044).
#[derive(Debug, Clone)]
struct Prompt {
    project_id: ProjectId,
    project_name: String,
    field: FormField,
    notice: Option<&'static str>,
}

impl Prompt {
    fn new(project_id: ProjectId, project_name: String) -> Self {
        Self {
            project_id,
            project_name,
            field: FormField::new("Request", ""),
            notice: None,
        }
    }
}

#[derive(Debug, Clone)]
enum Mode {
    Browse,
    Filter(String),
    Form(Form),
    Confirm(ConfirmAction),
    Message {
        title: String,
        lines: Vec<String>,
    },
    Help,
    Prompt(Prompt),
    /// Shown while the provider call blocks the dashboard.
    Planning {
        project_name: String,
    },
    /// Card detail dialog over the Kanban screen; `scroll` is clamped when
    /// rendered.
    Story {
        detail: Box<kanban::StoryDetail>,
        scroll: u16,
    },
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

/// Navigation state of the Kanban screen. Moving never writes.
///
/// Each list remembers its selected card and its own scroll offset, so
/// switching lists returns to the same place.
#[derive(Debug, Clone)]
struct KanbanView {
    board: kanban::Board,
    column: usize,
    cards: [usize; 5],
    offsets: [usize; 5],
    /// Cards fully visible per list in the last render; paging uses it.
    visible: [usize; 5],
}

impl KanbanView {
    fn new(board: kanban::Board) -> Self {
        let mut view = Self {
            board,
            column: 0,
            cards: [0; 5],
            offsets: [0; 5],
            visible: [1; 5],
        };
        view.clamp();
        view
    }

    fn state(&self) -> kanban::StoryState {
        kanban::StoryState::ALL[self.column]
    }

    fn column_len(&self, column: usize) -> usize {
        self.board.cards_in(kanban::StoryState::ALL[column]).count()
    }

    fn move_column(&mut self, delta: isize) {
        self.column = if delta < 0 {
            self.column.saturating_sub(1)
        } else {
            (self.column + 1).min(kanban::StoryState::ALL.len() - 1)
        };
    }

    /// Moves the selection by `delta` cards in the focused list. The next
    /// render scrolls the list to keep it visible ([`KanbanView::fit`]).
    fn move_card(&mut self, delta: isize) {
        let last = self.column_len(self.column).saturating_sub(1);
        let card = &mut self.cards[self.column];
        *card = card.saturating_add_signed(delta).min(last);
    }

    fn page(&mut self, direction: isize) {
        let rows = isize::try_from(self.visible[self.column].max(1)).unwrap_or(1);
        self.move_card(direction * rows);
    }

    fn first_card(&mut self) {
        self.cards[self.column] = 0;
    }

    fn last_card(&mut self) {
        self.cards[self.column] = self.column_len(self.column).saturating_sub(1);
    }

    /// Scrolls `column` by the minimum needed so the selected card fits in
    /// `available` rows, given each card's height; pulls back after a resize
    /// when earlier cards fit again. Returns how many cards are visible.
    fn fit(&mut self, column: usize, heights: &[u16], available: u16) -> usize {
        let selected = self.cards[column].min(heights.len().saturating_sub(1));
        let span = |from: usize, to: usize| -> u32 {
            heights[from..to]
                .iter()
                .map(|height| u32::from(*height))
                .sum()
        };
        let available = u32::from(available);
        let mut offset = self.offsets[column].min(selected);
        while offset < selected && span(offset, selected + 1) > available {
            offset += 1;
        }
        while offset > 0 && span(offset - 1, heights.len()) <= available {
            offset -= 1;
        }
        self.offsets[column] = offset;
        let mut used = 0;
        let mut visible = 0;
        for height in &heights[offset.min(heights.len())..] {
            used += u32::from(*height);
            if used > available {
                break;
            }
            visible += 1;
        }
        self.visible[column] = visible.max(1);
        visible
    }

    fn selected_card(&self) -> Option<&kanban::Card> {
        self.board
            .cards_in(self.state())
            .nth(self.cards[self.column])
    }

    /// Keeps every remembered position valid after a reload.
    fn replace(&mut self, board: kanban::Board) {
        self.board = board;
        self.clamp();
    }

    fn clamp(&mut self) {
        for column in 0..kanban::StoryState::ALL.len() {
            let len = self.column_len(column);
            self.cards[column] = self.cards[column].min(len.saturating_sub(1));
            self.offsets[column] = self.offsets[column].min(self.cards[column]);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Deferred {
    EditSettings,
    NewAgent,
    /// Plans `App::pending_request` inside the dashboard.
    Plan,
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
    kanban: Option<KanbanView>,
    pending_request: Option<(ProjectId, String)>,
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
            kanban: None,
            pending_request: None,
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
                Event::Paste(text) => {
                    if let Mode::Prompt(prompt) = &mut self.mode {
                        prompt.field.insert_text(&text);
                        prompt.notice = None;
                    }
                }
                Event::Key(key) if key.kind == KeyEventKind::Press => {
                    if let Some(deferred) = self.handle_key(key) {
                        if deferred == Deferred::Plan {
                            self.run_planning(terminal)?;
                            continue;
                        }
                        stop_terminal();
                        let result = self.execute_deferred(deferred);
                        *terminal = start_terminal("restarting the terminal dashboard")?;
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
            Screen::Kanban => {
                // The Projects selection is kept for Esc; only the board is
                // refreshed here.
                if let Err(error) = self.reload_kanban() {
                    self.show_error(error);
                }
                return;
            }
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
            Screen::Kanban => 0,
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
            Mode::Form(mut form) => {
                match key.code {
                    KeyCode::Esc => {}
                    KeyCode::Tab | KeyCode::Down => {
                        form.next();
                        self.mode = Mode::Form(form);
                    }
                    KeyCode::BackTab | KeyCode::Up => {
                        form.previous_wrapped();
                        self.mode = Mode::Form(form);
                    }
                    KeyCode::Enter => {
                        let result = self.submit_form(&form);
                        self.apply_result(result);
                    }
                    KeyCode::Backspace => {
                        form.current().backspace();
                        self.mode = Mode::Form(form);
                    }
                    KeyCode::Delete => {
                        form.current().delete();
                        self.mode = Mode::Form(form);
                    }
                    KeyCode::Left => {
                        form.current().left();
                        self.mode = Mode::Form(form);
                    }
                    KeyCode::Right => {
                        form.current().right();
                        self.mode = Mode::Form(form);
                    }
                    KeyCode::Home => {
                        form.current().home();
                        self.mode = Mode::Form(form);
                    }
                    KeyCode::End => {
                        form.current().end();
                        self.mode = Mode::Form(form);
                    }
                    KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                        form.current().insert(ch);
                        self.mode = Mode::Form(form);
                    }
                    _ => self.mode = Mode::Form(form),
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
            // Input is read by the cancellation watcher while planning.
            Mode::Planning { .. } => None,
            Mode::Prompt(mut prompt) => {
                prompt.notice = None;
                match key.code {
                    KeyCode::Esc => return None,
                    KeyCode::Enter if key.modifiers.contains(KeyModifiers::ALT) => {
                        prompt.field.insert_text("\n");
                    }
                    KeyCode::Enter => {
                        if prompt.field.value.trim().is_empty() {
                            prompt.notice = Some("Write a request first.");
                        } else {
                            self.pending_request = Some((prompt.project_id, prompt.field.value));
                            return Some(Deferred::Plan);
                        }
                    }
                    KeyCode::Backspace => prompt.field.backspace(),
                    KeyCode::Delete => prompt.field.delete(),
                    KeyCode::Left => prompt.field.left(),
                    KeyCode::Right => prompt.field.right(),
                    KeyCode::Home => prompt.field.home(),
                    KeyCode::End => prompt.field.end(),
                    KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                        prompt.field.insert(ch);
                    }
                    _ => {}
                }
                self.mode = Mode::Prompt(prompt);
                None
            }
            Mode::Story { detail, mut scroll } => {
                match key.code {
                    KeyCode::Esc | KeyCode::Enter => return None,
                    KeyCode::Up | KeyCode::Char('k') => scroll = scroll.saturating_sub(1),
                    KeyCode::Down | KeyCode::Char('j') => scroll = scroll.saturating_add(1),
                    KeyCode::PageUp => scroll = scroll.saturating_sub(10),
                    KeyCode::PageDown | KeyCode::Char(' ') => scroll = scroll.saturating_add(10),
                    KeyCode::Home | KeyCode::Char('g') => scroll = 0,
                    KeyCode::End | KeyCode::Char('G') => scroll = u16::MAX,
                    _ => {}
                }
                self.mode = Mode::Story { detail, scroll };
                None
            }
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
            Screen::Kanban => {
                self.handle_kanban(key);
                None
            }
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
                        DashboardGroup::Labs => self.mode = Mode::Form(Form::lab_new()),
                        DashboardGroup::Projects => self.mode = Mode::Form(Form::project_new()),
                        DashboardGroup::Agents => return Some(Deferred::NewAgent),
                        DashboardGroup::Settings => return Some(Deferred::EditSettings),
                    },
                    KeyCode::Char('c') if group == DashboardGroup::Labs => {
                        let result = self.check_selected_lab();
                        self.apply_result(result);
                    }
                    KeyCode::Char('e') if group == DashboardGroup::Labs => {
                        if let Some(lab) = self.selected_lab() {
                            self.mode = Mode::Form(Form::lab_edit(
                                lab.lab.id.clone(),
                                lab.lab.name.clone(),
                                lab.lab.description.clone(),
                            ));
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
                        let result = self.open_kanban();
                        self.apply_result(result);
                    }
                    KeyCode::Char('r') if group == DashboardGroup::Projects => {
                        if let Some(project) = self.selected_project() {
                            self.mode =
                                Mode::Prompt(Prompt::new(project.id.clone(), project.name.clone()));
                        } else {
                            self.status = Some("Select a project for the request.".to_owned());
                        }
                    }
                    KeyCode::Char('e') if group == DashboardGroup::Projects => {
                        if let Some(project) = self.selected_project() {
                            self.mode = Mode::Form(Form::project_edit(
                                project.id.clone(),
                                project.name.clone(),
                            ));
                        } else {
                            self.status = Some("Select a project to edit.".to_owned());
                        }
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

    fn handle_kanban(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('q') => self.quit = true,
            KeyCode::Esc => self.close_kanban(),
            KeyCode::Char('?') => self.mode = Mode::Help,
            KeyCode::Char('n') => {
                if let Some(view) = self.kanban.as_ref() {
                    self.mode = Mode::Prompt(Prompt::new(
                        view.board.project_id.clone(),
                        view.board.project_name.clone(),
                    ));
                }
            }
            KeyCode::Left | KeyCode::Char('h') => {
                if let Some(view) = self.kanban.as_mut() {
                    view.move_column(-1);
                }
            }
            KeyCode::Right | KeyCode::Char('l') => {
                if let Some(view) = self.kanban.as_mut() {
                    view.move_column(1);
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if let Some(view) = self.kanban.as_mut() {
                    view.move_card(-1);
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if let Some(view) = self.kanban.as_mut() {
                    view.move_card(1);
                }
            }
            KeyCode::PageUp => {
                if let Some(view) = self.kanban.as_mut() {
                    view.page(-1);
                }
            }
            KeyCode::PageDown => {
                if let Some(view) = self.kanban.as_mut() {
                    view.page(1);
                }
            }
            KeyCode::Home | KeyCode::Char('g') => {
                if let Some(view) = self.kanban.as_mut() {
                    view.first_card();
                }
            }
            KeyCode::End | KeyCode::Char('G') => {
                if let Some(view) = self.kanban.as_mut() {
                    view.last_card();
                }
            }
            KeyCode::Enter => {
                let result = self.show_story();
                self.apply_result(result);
            }
            KeyCode::Char('r') => match self.reload_kanban() {
                Ok(()) => self.status = Some("Board reloaded.".to_owned()),
                Err(error) => self.show_error(error),
            },
            _ => {}
        }
    }

    fn open_kanban(&mut self) -> Result<(), AppError> {
        let project = self
            .selected_project()
            .ok_or_else(|| AppError::Precondition {
                problem: "no project is selected.".to_owned(),
                hint: "Move the selection.".to_owned(),
            })?;
        let board = kanban::show(&self.db, &self.db, project.id.as_str())?;
        self.kanban = Some(KanbanView::new(board));
        self.screen = Screen::Kanban;
        self.status = None;
        Ok(())
    }

    fn reload_kanban(&mut self) -> Result<(), AppError> {
        let Some(view) = self.kanban.as_mut() else {
            return Ok(());
        };
        let board = kanban::show(&self.db, &self.db, view.board.project_id.as_str())?;
        view.replace(board);
        Ok(())
    }

    fn close_kanban(&mut self) {
        self.kanban = None;
        self.screen = Screen::Group(DashboardGroup::Projects);
        self.status = None;
        self.reload();
    }

    fn show_story(&mut self) -> Result<(), AppError> {
        let Some(view) = self.kanban.as_ref() else {
            return Ok(());
        };
        let Some(card) = view.selected_card() else {
            self.status = Some(format!("No stories in {}.", view.state().label()));
            return Ok(());
        };
        let detail = kanban::story(&self.db, &self.db, &view.board.project_id, &card.id)?;
        self.mode = Mode::Story {
            detail: Box::new(detail),
            scroll: 0,
        };
        Ok(())
    }

    /// Submits the pending request through the same use case as the flag
    /// path, then shows the outcome (ADR-0044).
    fn run_planning(&mut self, terminal: &mut DefaultTerminal) -> Result<(), AppError> {
        let Some((project_id, text)) = self.pending_request.take() else {
            return Ok(());
        };
        match self.plan_request(terminal, &project_id, &text) {
            Ok((title, lines)) => {
                self.reload();
                self.mode = Mode::Message { title, lines };
            }
            Err(error) => {
                self.reload();
                self.show_error(error);
            }
        }
        Ok(())
    }

    fn plan_request(
        &mut self,
        terminal: &mut DefaultTerminal,
        project_id: &ProjectId,
        text: &str,
    ) -> Result<(String, Vec<String>), AppError> {
        let project = projects::open(&self.db, &self.db, project_id.as_str())?;
        let settings = settings::show(&self.settings_file())?.settings;
        let limits = ExecutionLimits::default();
        let provider = crate::cli::dispatch::provider_from_settings(&settings, limits)?;
        self.mode = Mode::Planning {
            project_name: project.name.clone(),
        };
        terminal
            .draw(|frame| self.render(frame))
            .map_err(|error| AppError::Io {
                context: "drawing the terminal dashboard",
                kind: error.kind(),
            })?;
        let cancel = CancelToken::default();
        let watcher = CancelWatcher::start(cancel.clone());
        let outcome = requests::plan(
            &self.db,
            &self.db,
            project_id,
            text,
            &settings,
            &provider,
            limits,
            &crate::domain::clock::SystemClock,
            cancel,
            self.run_id,
        );
        watcher.stop();
        Ok(self.outcome_lines(&outcome?))
    }

    fn outcome_lines(&self, outcome: &PlanningOutcome) -> (String, Vec<String>) {
        let on_board = self.screen == Screen::Kanban;
        let retry_key = if on_board { "n" } else { "r" };
        match outcome {
            PlanningOutcome::Planned {
                request_id,
                stories,
            } => {
                let names: std::collections::HashMap<String, String> = self
                    .active_lab
                    .as_ref()
                    .and_then(|lab| self.db.lab_agents(lab).ok())
                    .unwrap_or_default()
                    .into_iter()
                    .map(|member| (member.id.to_string(), member.name))
                    .collect();
                let mut lines = vec![
                    format!("{} stories added to TODO.", stories.len()),
                    String::new(),
                ];
                for story in stories {
                    let assignee = story
                        .assigned_agent_id
                        .as_deref()
                        .and_then(|id| names.get(id).cloned())
                        .unwrap_or_else(|| "Unassigned".to_owned());
                    lines.push(format!("• {} — {assignee}", story.title));
                }
                lines.push(String::new());
                lines.push(if on_board {
                    "The board has been refreshed. Press any key to close.".to_owned()
                } else {
                    "Press v to open the board.".to_owned()
                });
                (format!("Request {request_id} planned"), lines)
            }
            PlanningOutcome::NeedsClarification {
                request_id,
                questions,
            } => {
                let mut lines = vec!["The Tech Lead needs more information:".to_owned()];
                for (index, question) in questions.iter().enumerate() {
                    lines.push(format!("{}. {question}", index + 1));
                }
                lines.push(String::new());
                lines.push(format!("Press {retry_key} to submit a revised request."));
                (format!("Request {request_id} needs clarification"), lines)
            }
            PlanningOutcome::Failed {
                request_id,
                code,
                hint,
                ..
            } => (
                format!("Request {request_id} failed"),
                vec![
                    format!("Code: {code}"),
                    format!("Hint: {hint}"),
                    String::new(),
                    "The request is kept for traceability.".to_owned(),
                ],
            ),
            PlanningOutcome::Cancelled { request_id } => (
                format!("Request {request_id} cancelled"),
                vec![
                    "Planning was cancelled; no stories were added.".to_owned(),
                    "Provider-side processing may have continued after local cancellation."
                        .to_owned(),
                ],
            ),
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
            Screen::Group(DashboardGroup::Settings) | Screen::Home | Screen::Kanban => {
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

    fn rename_project(&mut self, id: &ProjectId, name: &str) -> Result<(), AppError> {
        let project = projects::rename(&self.db, &self.db, id.as_str(), name)?;
        record_event(
            &mut self.db,
            self.run_id,
            EventKind::ProjectUpdated,
            EntityType::Project,
            project.id.as_str(),
        );
        self.reload();
        self.status = Some(format!("Project {} updated.", project.id));
        Ok(())
    }

    fn assign_agent(&mut self, agent_id: &str) -> Result<(), AppError> {
        let project = projects::assign_agent(&self.db, &self.db, agent_id)?;
        self.reload();
        self.status = Some(format!("Agent assigned to project {}.", project.id));
        Ok(())
    }

    fn execute_deferred(&mut self, deferred: Deferred) -> Result<String, AppError> {
        match deferred {
            // Planning stays inside the dashboard; see `run_planning`.
            Deferred::Plan => Ok(String::new()),
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

    fn submit_form(&mut self, form: &Form) -> Result<(), AppError> {
        let (name, value) = form.values();
        match &form.kind {
            FormKind::LabNew => self.create_lab(&name, &value),
            FormKind::LabEdit(id) => self.update_lab(id, &name, &value),
            FormKind::ProjectNew => self.create_project(&name, &value),
            FormKind::ProjectEdit(id) => self.rename_project(id, &name),
        }
    }

    fn apply_result(&mut self, result: Result<(), AppError>) {
        match result {
            Ok(()) => {
                // Actions that open a report or detail modal set the mode
                // themselves; only a completed form/confirm returns to browse.
                if !matches!(
                    self.mode,
                    Mode::Message { .. }
                        | Mode::Confirm(_)
                        | Mode::Picker { .. }
                        | Mode::Help
                        | Mode::Story { .. }
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
            Screen::Kanban => self.render_kanban(frame, area),
        }
        match &self.mode {
            Mode::Browse => {}
            Mode::Form(form) => self.render_form(frame, area, form),
            Mode::Prompt(prompt) => self.render_prompt(frame, area, prompt),
            Mode::Story { .. } => {
                let theme = self.theme;
                if let Mode::Story { detail, scroll } = &mut self.mode {
                    render_story(frame, area, theme, detail, scroll);
                }
            }
            _ => self.render_modal(frame, area),
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

    fn render_kanban(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let Some(mut view) = self.kanban.take() else {
            return;
        };
        self.draw_kanban(frame, area, &mut view);
        self.kanban = Some(view);
    }

    fn draw_kanban(&self, frame: &mut Frame<'_>, area: Rect, view: &mut KanbanView) {
        let board = &view.board;
        let mut footer = action_grid(
            &KANBAN_ACTIONS,
            self.theme,
            usize::from(area.width.saturating_sub(2)),
        );
        footer.push(Line::from(Span::styled(
            "Stories are read-only; Enter shows the full story.",
            self.theme.muted_style(),
        )));
        if let Some(status) = &self.status {
            footer.push(Line::from(vec![
                Span::styled("Status: ", self.theme.key()),
                Span::styled(clean(status), self.theme.success_style()),
            ]));
        }
        let footer_height = u16::try_from(footer.len() + 2).unwrap_or(6);
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(4),
                Constraint::Min(6),
                Constraint::Length(footer_height),
            ])
            .split(area);

        let separator = if self.theme.unicode { " · " } else { " - " };
        let mut context = vec![
            board.project_id.to_string(),
            format!("lab {}", clean(&board.lab_name)),
            format!(
                "lead {}",
                board
                    .lead_name
                    .as_deref()
                    .map(clean)
                    .unwrap_or_else(|| "missing".to_owned())
            ),
        ];
        if let Some(source) = &board.github_source {
            context.push(clean(source));
        }
        let summary = format!(
            "Stories {}{separator}Done {}{separator}Pending intake {}",
            board.counts.total(),
            board.counts.done,
            board.pending_intake
        );
        frame.render_widget(
            Paragraph::new(vec![
                Line::from(vec![
                    Span::styled(clean(&board.project_name), self.theme.title()),
                    Span::raw("  "),
                    Span::styled(context.join(separator), self.theme.muted_style()),
                ]),
                Line::from(Span::styled(summary, self.theme.muted_style())),
            ])
            .block(self.block("Kanban")),
            chunks[0],
        );

        let body = chunks[1];
        let inner = Rect {
            x: body.x.saturating_add(1),
            y: body.y.saturating_add(1),
            width: body.width.saturating_sub(2),
            height: body.height.saturating_sub(2),
        };
        let total = kanban::StoryState::ALL.len();
        let shown = kanban_window(usize::from(inner.width));
        let first = if view.column < shown {
            0
        } else {
            view.column + 1 - shown
        };
        let last = (first + shown).min(total);
        let mut title = "Board".to_owned();
        if first > 0 {
            title.push_str(&format!(
                "  {} {first} more",
                if self.theme.unicode { "‹" } else { "<" }
            ));
        }
        if last < total {
            title.push_str(&format!(
                "  {} more {}",
                total - last,
                if self.theme.unicode { "›" } else { ">" }
            ));
        }
        frame.render_widget(self.block(&title), body);

        let constraints = vec![Constraint::Ratio(1, u32::try_from(shown).unwrap_or(1)); shown];
        let columns = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(constraints)
            .split(inner);
        for (slot, column) in (first..last).enumerate() {
            self.render_kanban_list(frame, columns[slot], view, column);
        }
        frame.render_widget(
            Paragraph::new(footer).block(self.block("Actions")),
            chunks[2],
        );
    }

    fn render_kanban_list(
        &self,
        frame: &mut Frame<'_>,
        area: Rect,
        view: &mut KanbanView,
        column: usize,
    ) {
        let state = kanban::StoryState::ALL[column];
        let focused = column == view.column;
        // List border plus card border on each side.
        let text_width = usize::from(area.width.saturating_sub(4));
        let titles: Vec<Vec<String>> = view
            .board
            .cards_in(state)
            .map(|card| wrap_clipped(&clean(&card.title), text_width, KANBAN_TITLE_LINES))
            .collect();
        let heights: Vec<u16> = titles
            .iter()
            .map(|lines| u16::try_from(lines.len().max(1)).unwrap_or(1) + 2)
            .collect();
        let total = titles.len();
        let visible = if total == 0 {
            0
        } else {
            view.fit(column, &heights, area.height.saturating_sub(2))
        };
        let selected = view.cards[column];
        let offset = view.offsets[column];

        let border = if focused {
            self.theme.key()
        } else {
            self.theme.border_style()
        };
        let title_style = if focused {
            self.theme.key()
        } else {
            self.theme.title()
        };
        let (up, down) = if self.theme.unicode {
            ("↑", "↓")
        } else {
            ("^", "v")
        };
        let mut block = Block::bordered()
            .border_type(self.border_type())
            .title(Line::from(Span::styled(
                format!(" {} {total} ", state.label()),
                title_style,
            )))
            .border_style(border);
        // Scroll indicators live in the list borders so cards keep every row.
        if offset > 0 {
            block = block.title(
                Line::from(Span::styled(
                    format!(" {up}{offset} "),
                    self.theme.muted_style(),
                ))
                .right_aligned(),
            );
        }
        let below = total.saturating_sub(offset + visible);
        if below > 0 {
            block = block.title_bottom(Line::from(Span::styled(
                format!(" {down}{below} more "),
                self.theme.muted_style(),
            )));
        }
        if visible < total {
            block = block.title_bottom(
                Line::from(Span::styled(
                    format!(" {}/{total} ", selected + 1),
                    self.theme.muted_style(),
                ))
                .right_aligned(),
            );
        }
        let inner = block.inner(area);
        frame.render_widget(block, area);

        if total == 0 {
            let mut lines = vec![Line::from(Span::styled(
                "No stories",
                self.theme.muted_style(),
            ))];
            if state == kanban::StoryState::Todo && view.board.cards.is_empty() {
                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled(
                    "Press n to submit a request.",
                    self.theme.muted_style(),
                )));
            }
            frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), inner);
            return;
        }

        let mut y = inner.y;
        for index in offset..offset + visible {
            let rect = Rect {
                x: inner.x,
                y,
                width: inner.width,
                height: heights[index],
            };
            self.render_kanban_card(frame, rect, &titles[index], focused && index == selected);
            y += heights[index];
        }
    }

    fn render_kanban_card(
        &self,
        frame: &mut Frame<'_>,
        area: Rect,
        title: &[String],
        selected: bool,
    ) {
        // A card shows only the short description; Enter opens the rest.
        let width = usize::from(area.width.saturating_sub(2));
        let style = if selected {
            self.theme.selected()
        } else {
            Style::default()
        };
        let lines: Vec<Line<'static>> = title
            .iter()
            .map(|line| Line::from(Span::styled(format!("{line:<width$}"), style)))
            .collect();
        let block = Block::bordered()
            .border_type(self.border_type())
            .border_style(if selected {
                self.theme.key()
            } else {
                self.theme.border_style()
            });
        frame.render_widget(Paragraph::new(lines).block(block), area);
    }

    fn border_type(&self) -> BorderType {
        if self.theme.unicode {
            BorderType::Rounded
        } else {
            BorderType::Plain
        }
    }

    fn render_prompt(&self, frame: &mut Frame<'_>, area: Rect, prompt: &Prompt) {
        let width = 76.min(area.width.saturating_sub(4)).max(40);
        let height = 14.min(area.height.saturating_sub(2));
        let rect = fixed_rect(width, height, area);
        frame.render_widget(Clear, rect);
        let block = self.block(&format!("New request · {}", clean(&prompt.project_name)));
        let inner = block.inner(rect);
        frame.render_widget(block, rect);
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Min(3),
                Constraint::Length(1),
            ])
            .split(inner);
        let intro = fitting(
            &[
                "Describe what you need; the Tech Lead plans it into stories.",
                "Describe what you need.",
            ],
            usize::from(inner.width),
        );
        frame.render_widget(
            Paragraph::new(Span::styled(intro, self.theme.muted_style())),
            chunks[0],
        );
        let area_block = Block::bordered()
            .border_type(self.border_type())
            .border_style(self.theme.key())
            .title(Line::from(Span::styled(" Request ", self.theme.key())));
        let text_area = area_block.inner(chunks[1]);
        frame.render_widget(area_block, chunks[1]);
        frame.render_widget(
            Paragraph::new(prompt_lines(
                &prompt.field,
                usize::from(text_area.width),
                usize::from(text_area.height),
                self.theme,
            )),
            text_area,
        );
        let count = format!("{} chars", prompt.field.value.chars().count());
        // Leave room for the right-aligned count.
        let room = usize::from(inner.width).saturating_sub(count.chars().count() + 2);
        let footer = match prompt.notice {
            Some(notice) => Span::styled(notice, self.theme.warning_style()),
            None => Span::styled(
                fitting(
                    &[
                        "Enter submit · Alt+Enter new line · Esc cancel",
                        "Enter submit · Alt+Enter line · Esc",
                        "Enter submit · Esc cancel",
                    ],
                    room,
                ),
                self.theme.muted_style(),
            ),
        };
        frame.render_widget(Paragraph::new(Line::from(footer)), chunks[2]);
        frame.render_widget(
            Paragraph::new(Span::styled(count, self.theme.muted_style()))
                .alignment(Alignment::Right),
            chunks[2],
        );
    }

    fn render_form(&self, frame: &mut Frame<'_>, area: Rect, form: &Form) {
        let width = u16::try_from(76)
            .unwrap_or(76)
            .min(area.width.saturating_sub(4))
            .max(40);
        let height = u16::try_from(form.fields.len() * 3 + 4)
            .unwrap_or(10)
            .min(area.height.saturating_sub(2));
        let rect = fixed_rect(width, height, area);
        frame.render_widget(Clear, rect);
        let block = self.block(&form.title());
        let inner = Rect {
            x: rect.x.saturating_add(1),
            y: rect.y.saturating_add(1),
            width: rect.width.saturating_sub(2),
            height: rect.height.saturating_sub(2),
        };
        frame.render_widget(block, rect);

        let mut constraints = vec![Constraint::Length(3); form.fields.len()];
        constraints.push(Constraint::Min(0));
        constraints.push(Constraint::Length(1));
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints(constraints)
            .split(inner);
        for (index, field) in form.fields.iter().enumerate() {
            let focused = index == form.focus;
            let border = if focused {
                self.theme.key()
            } else {
                self.theme.border_style()
            };
            let title_style = if focused {
                self.theme.key()
            } else {
                self.theme.muted_style()
            };
            let block = Block::bordered()
                .border_type(if self.theme.unicode {
                    BorderType::Rounded
                } else {
                    BorderType::Plain
                })
                .title(Line::from(Span::styled(
                    format!(" {} ", field.label),
                    title_style,
                )))
                .border_style(border);
            frame.render_widget(
                Paragraph::new(form_field_line(field, focused, self.theme)).block(block),
                chunks[index],
            );
        }
        if let Some(hint) = chunks.get(form.fields.len() + 1) {
            frame.render_widget(
                Paragraph::new(Line::from(Span::styled(
                    "Tab/↑↓ switch · Enter save · Esc cancel",
                    self.theme.muted_style(),
                ))),
                *hint,
            );
        }
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
            Mode::Help => (
                "Help".to_owned(),
                if self.screen == Screen::Kanban {
                    kanban_help_lines()
                } else {
                    help_lines()
                },
            ),
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
            Mode::Planning { project_name } => (
                "Planning".to_owned(),
                vec![
                    format!("Planning a request for {}…", clean(project_name)),
                    String::new(),
                    "The Tech Lead is drafting stories; this can take up to 4 minutes.".to_owned(),
                    String::new(),
                    "Esc cancel".to_owned(),
                ],
            ),
            Mode::Form(_) | Mode::Browse | Mode::Story { .. } | Mode::Prompt(_) => return,
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

    fn warning_style(self) -> Style {
        if self.color {
            Style::default()
                .fg(Color::Yellow)
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
            ("[e]", "Edit"),
            ("[r]", "Request"),
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

const KANBAN_ACTIONS: [(&str, &str); 9] = [
    ("[n]", "Request"),
    ("[←/→]", "List"),
    ("[↑/↓]", "Card"),
    ("[PgUp/Dn]", "Scroll"),
    ("[Enter]", "Detail"),
    ("[r]", "Reload"),
    ("[?]", "Help"),
    ("[Esc]", "Back"),
    ("[q]", "Quit"),
];

/// Most short-description lines on a card; shorter titles make shorter cards.
const KANBAN_TITLE_LINES: usize = 3;

/// Narrowest readable list; fewer lists are shown below it.
const KANBAN_MIN_LIST_WIDTH: usize = 18;

/// How many lists fit side by side in `width` columns.
fn kanban_window(width: usize) -> usize {
    (width / KANBAN_MIN_LIST_WIDTH).clamp(1, kanban::StoryState::ALL.len())
}

/// Word-wraps `value` to `width` characters; longer words are split.
fn wrap_text(value: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return Vec::new();
    }
    let mut lines = Vec::new();
    let mut current = String::new();
    for word in value.split_whitespace() {
        let mut word: Vec<char> = word.chars().collect();
        loop {
            let used = current.chars().count();
            let gap = usize::from(used > 0);
            if used + gap + word.len() <= width {
                if gap == 1 {
                    current.push(' ');
                }
                current.extend(word);
                break;
            }
            if used == 0 {
                current = word.drain(..width).collect();
            }
            lines.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    lines
}

/// Like [`wrap_text`] but keeps at most `max_lines`; a cut ends with `…`.
fn wrap_clipped(value: &str, width: usize, max_lines: usize) -> Vec<String> {
    let mut lines = wrap_text(value, width);
    if lines.len() > max_lines {
        lines.truncate(max_lines);
        if let Some(last) = lines.last_mut() {
            let kept: String = last.chars().take(width.saturating_sub(1)).collect();
            *last = format!("{}…", kept.trim_end());
        }
    }
    lines
}

/// Card detail dialog: every story field, word-wrapped and scrollable.
fn render_story(
    frame: &mut Frame<'_>,
    area: Rect,
    theme: Theme,
    detail: &kanban::StoryDetail,
    scroll: &mut u16,
) {
    let rect = centered_rect(84, 84, area);
    let block = Block::bordered()
        .border_type(if theme.unicode {
            BorderType::Rounded
        } else {
            BorderType::Plain
        })
        .border_style(theme.key())
        .title(Line::from(Span::styled(
            format!(" Story {} ", detail.id),
            theme.key(),
        )));
    let inner = block.inner(rect);
    let lines = story_lines(detail, theme, usize::from(inner.width));
    let total = u16::try_from(lines.len()).unwrap_or(u16::MAX);
    let max_scroll = total.saturating_sub(inner.height);
    *scroll = (*scroll).min(max_scroll);
    let hint = if max_scroll > 0 {
        format!(
            " ↑/↓ scroll {}/{} · Esc close ",
            *scroll + 1,
            max_scroll + 1
        )
    } else {
        " Esc close ".to_owned()
    };
    let block = block.title_bottom(Line::from(Span::styled(hint, theme.muted_style())));
    frame.render_widget(Clear, rect);
    frame.render_widget(
        Paragraph::new(lines).scroll((*scroll, 0)).block(block),
        rect,
    );
}

const STORY_LABEL_WIDTH: usize = 12;

fn story_lines(story: &kanban::StoryDetail, theme: Theme, width: usize) -> Vec<Line<'static>> {
    let label_style = theme.muted_style().add_modifier(Modifier::BOLD);
    let value_width = width.saturating_sub(STORY_LABEL_WIDTH).max(1);
    let mut lines: Vec<Line<'static>> = wrap_text(&clean(&story.title), width)
        .into_iter()
        .map(|line| Line::from(Span::styled(line, theme.title())))
        .collect();
    lines.push(Line::from(""));

    let field = |lines: &mut Vec<Line<'static>>, label: &str, value: String, style: Style| {
        for (index, part) in wrap_text(&value, value_width).into_iter().enumerate() {
            let label = if index == 0 { label } else { "" };
            lines.push(Line::from(vec![
                Span::styled(format!("{label:<STORY_LABEL_WIDTH$}"), label_style),
                Span::styled(part, style),
            ]));
        }
    };
    let state_style = if story.state == kanban::StoryState::Blocked {
        theme.warning_style()
    } else {
        Style::default()
    };
    field(
        &mut lines,
        "State",
        story.state.label().to_owned(),
        state_style,
    );
    field(
        &mut lines,
        "Assignee",
        story
            .assignee
            .as_deref()
            .map(clean)
            .unwrap_or_else(|| "Unassigned".to_owned()),
        Style::default(),
    );
    field(
        &mut lines,
        "Priority",
        clean(&story.priority),
        Style::default(),
    );
    field(
        &mut lines,
        "Complexity",
        clean(&story.complexity),
        Style::default(),
    );
    let overdue = story.overdue && story.state != kanban::StoryState::Done;
    field(
        &mut lines,
        "Due",
        format!(
            "{}{}",
            clean(&story.due_date),
            if overdue { "  ! OVERDUE" } else { "" }
        ),
        if overdue {
            theme.warning_style()
        } else {
            Style::default()
        },
    );
    field(
        &mut lines,
        "Skills",
        if story.required_skills.is_empty() {
            "—".to_owned()
        } else {
            story
                .required_skills
                .iter()
                .map(|skill| clean(skill))
                .collect::<Vec<_>>()
                .join(", ")
        },
        Style::default(),
    );
    field(
        &mut lines,
        "Request",
        story.request_id.to_string(),
        Style::default(),
    );
    field(&mut lines, "Story", story.id.to_string(), Style::default());

    let section =
        |lines: &mut Vec<Line<'static>>, heading: &str, items: Vec<String>, bullet: &str| {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(heading.to_owned(), label_style)));
            if items.is_empty() {
                lines.push(Line::from(Span::styled("—", theme.muted_style())));
            }
            let indent = bullet.chars().count();
            for item in items {
                for (index, part) in wrap_text(&item, width.saturating_sub(indent).max(1))
                    .into_iter()
                    .enumerate()
                {
                    let prefix = if index == 0 {
                        bullet.to_owned()
                    } else {
                        " ".repeat(indent)
                    };
                    lines.push(Line::from(format!("{prefix}{part}")));
                }
            }
        };
    section(
        &mut lines,
        "Description",
        vec![clean(&story.description)],
        "",
    );
    section(
        &mut lines,
        "Acceptance criteria",
        story
            .acceptance_criteria
            .iter()
            .map(|criterion| clean(criterion))
            .collect(),
        if theme.unicode { "• " } else { "- " },
    );
    section(
        &mut lines,
        "Assignment rationale",
        story
            .rationale
            .iter()
            .map(|rationale| clean(rationale))
            .collect(),
        "",
    );
    lines
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
    let key_width = items
        .iter()
        .map(|(key, _)| key.chars().count())
        .max()
        .unwrap_or(0);
    let label_width = items
        .iter()
        .map(|(_, label)| label.chars().count())
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
                spans.push(Span::raw(" ".repeat(key_width - key.chars().count() + 1)));
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

fn kanban_help_lines() -> Vec<String> {
    vec![
        "h/l or ←/→  previous/next list".to_owned(),
        "j/k or ↑/↓  previous/next card; the list scrolls".to_owned(),
        "PgUp/PgDn   scroll the list by one page".to_owned(),
        "g/G         first/last card in the list".to_owned(),
        "Enter       card detail (↑/↓ scroll, Esc close)".to_owned(),
        "n           new request for this project".to_owned(),
        "r           reload the board".to_owned(),
        "?           this help".to_owned(),
        "Esc         back to projects".to_owned(),
        "q           quit".to_owned(),
        String::new(),
        "The board is read-only; it never changes story state.".to_owned(),
    ]
}

fn help_lines() -> Vec<String> {
    vec![
        "j/k or ↑/↓  move selection".to_owned(),
        "h/l or ←/→  previous/next page".to_owned(),
        "g/G         first/last page".to_owned(),
        "Enter       open/confirm selection".to_owned(),
        "n           new item in this group".to_owned(),
        "e           edit the selected item".to_owned(),
        "r           new request for the selected project".to_owned(),
        "v           project Kanban board".to_owned(),
        "a           set active or assign".to_owned(),
        "d           delete with confirmation".to_owned(),
        "/           filter".to_owned(),
        "?           this help".to_owned(),
        "Esc         back to home".to_owned(),
        "q           quit".to_owned(),
    ]
}

fn fixed_rect(width: u16, height: u16, area: Rect) -> Rect {
    let width = width.min(area.width);
    let height = height.min(area.height);
    Rect {
        x: area.x + (area.width - width) / 2,
        y: area.y + (area.height - height) / 2,
        width,
        height,
    }
}

fn form_field_line(field: &FormField, focused: bool, theme: Theme) -> Line<'static> {
    if !focused {
        return Line::from(Span::raw(clean(&field.value)));
    }
    let chars: Vec<char> = field.value.chars().collect();
    let before: String = chars.iter().take(field.cursor).collect();
    let at = chars.get(field.cursor).copied();
    let after: String = chars
        .iter()
        .skip(field.cursor + usize::from(at.is_some()))
        .collect();
    let cursor = if theme.unicode { "▏" } else { "|" };
    let mut spans = vec![Span::raw(before)];
    spans.push(Span::styled(
        at.map(|ch| ch.to_string())
            .unwrap_or_else(|| cursor.to_owned()),
        theme.selected(),
    ));
    spans.push(Span::raw(after));
    Line::from(spans)
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

/// Lays out the request text word-wrapped at `width` (long words split),
/// keeps the cursor row visible in `height` rows, and marks the cursor.
fn prompt_lines(
    field: &FormField,
    width: usize,
    height: usize,
    theme: Theme,
) -> Vec<Line<'static>> {
    if width == 0 || height == 0 {
        return Vec::new();
    }
    let chars: Vec<char> = field.value.chars().collect();
    // Each row holds character indices, so the cursor maps back exactly.
    let mut rows: Vec<Vec<usize>> = vec![Vec::new()];
    for (index, ch) in chars.iter().enumerate() {
        if *ch == '\n' {
            if let Some(row) = rows.last_mut() {
                row.push(index);
            }
            rows.push(Vec::new());
            continue;
        }
        if let Some(row) = rows.last_mut().filter(|row| row.len() == width) {
            let tail = match row.iter().rposition(|&at| chars[at] == ' ') {
                Some(space) if space + 1 < row.len() => row.split_off(space + 1),
                _ => Vec::new(),
            };
            rows.push(tail);
        }
        if let Some(row) = rows.last_mut() {
            row.push(index);
        }
    }
    let cursor = if field.cursor >= chars.len() {
        let last = rows.len() - 1;
        if rows[last].len() >= width {
            rows.push(Vec::new());
        }
        (rows.len() - 1, rows[rows.len() - 1].len())
    } else {
        rows.iter()
            .enumerate()
            .find_map(|(row, indices)| {
                indices
                    .iter()
                    .position(|&at| at == field.cursor)
                    .map(|column| (row, column))
            })
            .unwrap_or((0, 0))
    };
    let first = (cursor.0 + 1).saturating_sub(height);
    let mark = if theme.unicode { "▏" } else { "|" };
    let text = |indices: &[usize]| -> String {
        indices
            .iter()
            .map(|&at| if chars[at] == '\n' { ' ' } else { chars[at] })
            .collect()
    };
    rows.iter()
        .enumerate()
        .skip(first)
        .take(height)
        .map(|(index, row)| {
            if field.value.is_empty() && index == 0 {
                return Line::from(vec![
                    Span::styled(mark.to_owned(), theme.selected()),
                    Span::styled(" e.g. Add password reset by email", theme.muted_style()),
                ]);
            }
            if index != cursor.0 {
                return Line::from(text(row).trim_end().to_owned());
            }
            let before = text(&row[..cursor.1.min(row.len())]);
            let at = row.get(cursor.1).map(|&at| match chars[at] {
                '\n' => ' ',
                ch => ch,
            });
            let after = text(row.get(cursor.1 + 1..).unwrap_or_default());
            Line::from(vec![
                Span::raw(before),
                Span::styled(
                    at.map(String::from).unwrap_or_else(|| mark.to_owned()),
                    theme.selected(),
                ),
                Span::raw(after.trim_end().to_owned()),
            ])
        })
        .collect()
}

/// The first candidate that fits `width`, or the last one.
fn fitting<'a>(candidates: &[&'a str], width: usize) -> &'a str {
    candidates
        .iter()
        .copied()
        .find(|text| text.chars().count() <= width)
        .unwrap_or(candidates[candidates.len() - 1])
}

/// Reads terminal input while planning blocks the dashboard; `Esc` or
/// `Ctrl-C` cancels. It only touches the cancel token, never the database.
struct CancelWatcher {
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
    handle: std::thread::JoinHandle<()>,
}

impl CancelWatcher {
    fn start(cancel: CancelToken) -> Self {
        let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let flag = std::sync::Arc::clone(&stop);
        let handle = std::thread::spawn(move || {
            while !flag.load(std::sync::atomic::Ordering::SeqCst) {
                if !event::poll(std::time::Duration::from_millis(50)).unwrap_or(false) {
                    continue;
                }
                if let Ok(Event::Key(key)) = event::read()
                    && key.kind == KeyEventKind::Press
                    && is_cancel_key(key)
                {
                    cancel.cancel();
                }
            }
        });
        Self { stop, handle }
    }

    fn stop(self) {
        self.stop.store(true, std::sync::atomic::Ordering::SeqCst);
        let _ = self.handle.join();
    }
}

fn is_cancel_key(key: KeyEvent) -> bool {
    key.code == KeyCode::Esc
        || (key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL))
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
            kanban: None,
            pending_request: None,
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
    fn form_fields_support_cursor_editing_and_focus() {
        let mut form = Form::lab_new();
        for ch in "Demo".chars() {
            form.current().insert(ch);
        }
        assert_eq!(form.fields[0].value, "Demo");
        form.current().left();
        form.current().insert('X');
        assert_eq!(form.fields[0].value, "DemXo");
        form.current().backspace();
        assert_eq!(form.fields[0].value, "Demo");
        form.current().home();
        form.current().delete();
        assert_eq!(form.fields[0].value, "emo");
        form.next();
        assert_eq!(form.focus, 1);
        form.previous_wrapped();
        assert_eq!(form.focus, 0);
    }

    #[test]
    fn the_form_modal_renders_field_boxes_and_hints() {
        let (mut app, _dir) = fixture(Screen::Group(DashboardGroup::Labs));
        app.mode = Mode::Form(Form::lab_edit(
            LabId::parse("a3f91c07").unwrap(),
            "Demo Lab".to_owned(),
            "Platform team".to_owned(),
        ));
        let screen = rendered(&mut app);
        assert!(screen.contains("Edit laboratory"), "{screen}");
        assert!(screen.contains("Name"), "{screen}");
        assert!(screen.contains("Description (optional)"), "{screen}");
        assert!(screen.contains("Enter save"), "{screen}");
        assert!(screen.contains("Demo Lab"), "{screen}");
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
        assert!(matches!(app.mode, Mode::Form(_)));
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

    fn card(id: &str, title: &str, state: kanban::StoryState) -> kanban::Card {
        kanban::Card {
            id: crate::domain::ids::StoryId::parse(id).unwrap(),
            title: title.to_owned(),
            state,
            assignee: None,
            priority: "high".to_owned(),
            due_date: "2026-01-03".to_owned(),
            overdue: true,
        }
    }

    fn board(cards: Vec<kanban::Card>) -> kanban::Board {
        kanban::Board {
            project_id: ProjectId::parse("0b1c2d3e").unwrap(),
            project_name: "Portal".to_owned(),
            github_source: None,
            lab_name: "Payments".to_owned(),
            lead_name: Some("Grace".to_owned()),
            counts: kanban::BoardCounts::default(),
            pending_intake: 1,
            cards,
        }
    }

    fn kanban_fixture(cards: Vec<kanban::Card>) -> (App, tempfile::TempDir) {
        let (mut app, dir) = fixture(Screen::Kanban);
        app.kanban = Some(KanbanView::new(board(cards)));
        (app, dir)
    }

    #[test]
    fn the_kanban_is_a_full_screen_board_of_lists_and_cards() {
        let (mut app, _dir) = kanban_fixture(vec![
            card("2d3e4f50", "Reset password", kanban::StoryState::Todo),
            card("3e4f5061", "Audit log", kanban::StoryState::Todo),
            card("4f506172", "Rate limit", kanban::StoryState::Blocked),
        ]);
        let screen = rendered(&mut app);
        for text in [
            "Kanban",
            "Portal",
            "lead Grace",
            "Pending intake 1",
            "TODO 2",
            "IN PROGRESS 0",
            "IN REVIEW 0",
            "BLOCKED 1",
            "DONE 0",
            "Reset password",
            "Rate limit",
            "[Enter]",
            "Detail",
            "Stories are read-only",
        ] {
            assert!(screen.contains(text), "{text}: {screen}");
        }
        // Cards show only the short description; the rest is in the detail.
        for hidden in ["2d3e4f50", "Unassigned", "OVERDUE", "high", "2026-01-03"] {
            assert!(!screen.contains(hidden), "{hidden}: {screen}");
        }
        // The project table is not underneath: this is a screen, not a modal.
        assert!(!screen.contains("REQ"), "{screen}");
    }

    fn many(count: usize) -> Vec<kanban::Card> {
        (0..count)
            .map(|index| {
                card(
                    &format!("{:08x}", index + 1),
                    &format!("Story {index}"),
                    kanban::StoryState::Todo,
                )
            })
            .collect()
    }

    #[test]
    fn each_list_scrolls_to_keep_the_selected_card_visible() {
        let (mut app, _dir) = kanban_fixture(many(12));
        let screen = rendered_at(&mut app, 100, 30);
        let rows = app.kanban.as_ref().unwrap().visible[0];
        assert!((2..12).contains(&rows), "{rows}");
        assert!(screen.contains("Story 0"), "{screen}");
        assert!(!screen.contains("Story 11"), "{screen}");
        assert!(screen.contains(&format!("↓{} more", 12 - rows)), "{screen}");
        assert!(screen.contains("1/12"), "{screen}");

        // Moving below the last visible card scrolls by one card.
        for _ in 0..rows {
            app.handle_key(key(KeyCode::Down));
        }
        let screen = rendered_at(&mut app, 100, 30);
        let view = app.kanban.as_ref().unwrap();
        assert_eq!((view.cards[0], view.offsets[0]), (rows, 1));
        assert!(screen.contains("↑1"), "{screen}");
        assert!(!screen.contains("Story 0 "), "{screen}");

        // Moving back up inside the window does not scroll.
        app.handle_key(key(KeyCode::Up));
        rendered_at(&mut app, 100, 30);
        assert_eq!(app.kanban.as_ref().unwrap().offsets[0], 1);

        app.handle_key(key(KeyCode::End));
        let screen = rendered_at(&mut app, 100, 30);
        let view = app.kanban.as_ref().unwrap();
        assert_eq!((view.cards[0], view.offsets[0]), (11, 12 - rows));
        assert!(screen.contains("Story 11"), "{screen}");
        assert!(!screen.contains("more"), "{screen}");

        app.handle_key(key(KeyCode::PageUp));
        assert_eq!(app.kanban.as_ref().unwrap().cards[0], 11 - rows);
        app.handle_key(key(KeyCode::Char('g')));
        rendered_at(&mut app, 100, 30);
        let view = app.kanban.as_ref().unwrap();
        assert_eq!((view.cards[0], view.offsets[0]), (0, 0));
        app.handle_key(key(KeyCode::PageDown));
        assert_eq!(app.kanban.as_ref().unwrap().cards[0], rows);
    }

    #[test]
    fn card_height_follows_the_short_description() {
        let mut cards = many(3);
        cards[1].title = "A much longer short description that wraps over lines".to_owned();
        let (mut app, _dir) = kanban_fixture(cards);
        let mut view = app.kanban.take().unwrap();
        // One-line cards take 3 rows, a three-line card 5.
        assert_eq!(view.fit(0, &[3, 5, 3], 11), 3);
        assert_eq!(view.fit(0, &[3, 5, 3], 8), 2);
        view.cards[0] = 2;
        assert_eq!(view.fit(0, &[3, 5, 3], 8), 2);
        assert_eq!(view.offsets[0], 1);
        // Growing the space pulls earlier cards back into view.
        assert_eq!(view.fit(0, &[3, 5, 3], 20), 3);
        assert_eq!(view.offsets[0], 0);
        app.kanban = Some(view);
        let screen = rendered(&mut app);
        assert!(screen.contains("A much longer"), "{screen}");
    }

    #[test]
    fn lists_keep_their_own_scroll_and_follow_a_resize() {
        let mut cards = many(12);
        cards.push(card("aaaaaaaa", "Review me", kanban::StoryState::InReview));
        let (mut app, _dir) = kanban_fixture(cards);
        rendered_at(&mut app, 100, 30);
        app.handle_key(key(KeyCode::End));
        rendered_at(&mut app, 100, 30);
        let offset = app.kanban.as_ref().unwrap().offsets[0];
        assert!(offset > 0);
        app.handle_key(key(KeyCode::Right));
        app.handle_key(key(KeyCode::Right));
        app.handle_key(key(KeyCode::Down));
        rendered_at(&mut app, 100, 30);
        let view = app.kanban.as_ref().unwrap();
        assert_eq!((view.offsets[0], view.offsets[2]), (offset, 0));

        // A taller terminal shows more cards; the offset shrinks to fit.
        rendered_at(&mut app, 100, 80);
        let view = app.kanban.as_ref().unwrap();
        assert_eq!(view.visible[0], 12);
        assert_eq!(view.offsets[0], 0);
        assert_eq!(view.cards[0], 11);
    }

    #[test]
    fn narrow_terminals_show_a_window_of_lists_around_the_focus() {
        let (mut app, _dir) =
            kanban_fixture(vec![card("2d3e4f50", "Reset", kanban::StoryState::Done)]);
        let screen = rendered_at(&mut app, 60, 20);
        assert!(screen.contains("TODO"), "{screen}");
        assert!(!screen.contains("DONE 1"), "{screen}");
        assert!(screen.contains("more ›"), "{screen}");
        for _ in 0..4 {
            app.handle_key(key(KeyCode::Right));
        }
        let screen = rendered_at(&mut app, 60, 20);
        assert!(screen.contains("DONE 1"), "{screen}");
        assert!(screen.contains("‹"), "{screen}");
        for (width, height) in [(80, 24), (100, 30), (160, 50)] {
            let screen = rendered_at(&mut app, width, height);
            assert!(screen.contains("Kanban"), "{width}x{height}: {screen}");
        }
    }

    #[test]
    fn board_navigation_clamps_and_esc_returns_to_projects() {
        let (mut app, _dir) = kanban_fixture(vec![
            card("2d3e4f50", "One", kanban::StoryState::Todo),
            card("3e4f5061", "Two", kanban::StoryState::Todo),
        ]);
        app.handle_key(key(KeyCode::Left));
        app.handle_key(key(KeyCode::Up));
        let view = app.kanban.as_ref().unwrap();
        assert_eq!((view.column, view.cards[0]), (0, 0));
        for _ in 0..5 {
            app.handle_key(key(KeyCode::Char('j')));
        }
        let view = app.kanban.as_ref().unwrap();
        assert_eq!(view.cards[0], 1);
        assert_eq!(view.selected_card().unwrap().title, "Two");
        for _ in 0..9 {
            app.handle_key(key(KeyCode::Char('l')));
        }
        let view = app.kanban.as_ref().unwrap();
        assert_eq!(view.column, 4);
        assert!(view.selected_card().is_none());
        app.handle_key(key(KeyCode::Char('?')));
        assert!(matches!(app.mode, Mode::Help));
        assert!(rendered(&mut app).contains("previous/next list"));
        app.handle_key(key(KeyCode::Esc));
        app.handle_key(key(KeyCode::Esc));
        assert_eq!(app.screen, Screen::Group(DashboardGroup::Projects));
        assert!(app.kanban.is_none());
    }

    #[test]
    fn card_titles_are_sanitized_and_wrapped() {
        let (mut app, _dir) = kanban_fixture(vec![card(
            "2d3e4f50",
            "evil\u{1b}[31mtitle that keeps going for a long while and still does not end",
            kanban::StoryState::Todo,
        )]);
        let screen = rendered(&mut app);
        assert!(!screen.contains('\u{1b}'), "{screen}");
        assert!(screen.contains("eviltitle"), "{screen}");
        assert!(screen.contains('…'), "{screen}");
    }

    #[test]
    fn a_long_story_detail_scrolls_and_clamps() {
        let detail = kanban::StoryDetail {
            id: crate::domain::ids::StoryId::parse("2d3e4f50").unwrap(),
            request_id: crate::domain::ids::RequestId::parse("1c2d3e4f").unwrap(),
            title: "Reset".to_owned(),
            description: "word ".repeat(400),
            state: kanban::StoryState::Todo,
            assignee: Some("Ada".to_owned()),
            priority: "high".to_owned(),
            complexity: "small".to_owned(),
            due_date: "2026-01-03".to_owned(),
            overdue: false,
            acceptance_criteria: (0..30).map(|index| format!("Criterion {index}")).collect(),
            required_skills: Vec::new(),
            rationale: Some("Last line of the dialog".to_owned()),
        };
        let (mut app, _dir) = kanban_fixture(Vec::new());
        app.mode = Mode::Story {
            detail: Box::new(detail),
            scroll: 0,
        };
        let screen = rendered(&mut app);
        assert!(screen.contains("↑/↓ scroll 1/"), "{screen}");
        assert!(!screen.contains("Last line of the dialog"), "{screen}");
        app.handle_key(key(KeyCode::End));
        let screen = rendered(&mut app);
        assert!(screen.contains("Last line of the dialog"), "{screen}");
        let Mode::Story { scroll, .. } = app.mode else {
            panic!("dialog closed");
        };
        assert!(scroll > 0 && scroll < u16::MAX, "{scroll}");
        app.handle_key(key(KeyCode::Home));
        assert!(rendered(&mut app).contains("Reset"));
    }

    #[test]
    fn action_columns_align_with_multibyte_keys() {
        let lines = action_grid(
            &[("[←/→]", "List"), ("[Enter]", "Detail")],
            Theme {
                color: false,
                unicode: true,
            },
            80,
        );
        let text: String = lines[0]
            .spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect();
        assert_eq!(text, "[←/→]   List    [Enter] Detail");
    }

    #[test]
    fn titles_wrap_on_words_and_mark_cuts() {
        assert_eq!(
            wrap_clipped("Reset password", 20, 2),
            vec!["Reset password"]
        );
        assert_eq!(
            wrap_clipped("Reset the password flow", 10, 2),
            vec!["Reset the", "password…"]
        );
        assert_eq!(wrap_clipped("abcdefghij", 4, 2), vec!["abcd", "efg…"]);
        assert_eq!(wrap_clipped("a b", 0, 2), Vec::<String>::new());
        assert_eq!(
            wrap_text("Reset the password flow", 10),
            vec!["Reset the", "password", "flow"]
        );
    }

    fn seeded() -> (App, tempfile::TempDir) {
        let (mut app, dir) = fixture(Screen::Group(DashboardGroup::Projects));
        for sql in [
            "INSERT INTO labs (id, name, created_at, updated_at) VALUES ('a3f91c07', 'Payments', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
            "INSERT INTO projects (id, lab_id, name, created_at, updated_at) VALUES ('0b1c2d3e', 'a3f91c07', 'Portal', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
            "INSERT INTO requests (id, project_id, content, status, created_at, updated_at) VALUES ('1c2d3e4f', '0b1c2d3e', 'Add reset', 'PLANNED', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
            "INSERT INTO stories (id, project_id, request_id, title, description, acceptance_criteria_json, priority, complexity, required_skills_json, state, due_date, created_at, updated_at) VALUES ('2d3e4f50', '0b1c2d3e', '1c2d3e4f', 'Reset', 'Send a link', '[\"Link expires\"]', 'high', 'small', '[\"rust\"]', 'TODO', '2026-01-03', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        ] {
            app.db.execute(sql).unwrap();
        }
        context::save_active_lab(&app.db, &LabId::parse("a3f91c07").unwrap()).unwrap();
        app.reload();
        (app, dir)
    }

    #[test]
    fn the_projects_dashboard_opens_the_board_and_card_detail() {
        let (mut app, _dir) = seeded();
        app.handle_key(key(KeyCode::Char('v')));
        assert_eq!(app.screen, Screen::Kanban);
        app.handle_key(key(KeyCode::Enter));
        assert!(matches!(app.mode, Mode::Story { .. }), "{:?}", app.mode);
        let screen = rendered(&mut app);
        for expected in [
            "Story 2d3e4f50",
            "State",
            "TODO",
            "Assignee",
            "Unassigned",
            "Priority",
            "high",
            "Complexity",
            "small",
            "2026-01-03",
            "! OVERDUE",
            "rust",
            "1c2d3e4f",
            "Description",
            "Send a link",
            "Acceptance criteria",
            "• Link expires",
            "Esc close",
        ] {
            assert!(screen.contains(expected), "{expected}: {screen}");
        }
        // Keys other than Esc/Enter keep the dialog open.
        app.handle_key(key(KeyCode::Char('x')));
        assert!(matches!(app.mode, Mode::Story { .. }));
        app.handle_key(key(KeyCode::Esc));
        assert!(matches!(app.mode, Mode::Browse));
        assert_eq!(app.screen, Screen::Kanban);
        app.handle_key(key(KeyCode::Char('r')));
        assert_eq!(app.status.as_deref(), Some("Board reloaded."));
    }

    #[test]
    fn the_projects_dashboard_renames_the_selected_project() {
        let (mut app, _dir) = seeded();
        app.handle_key(key(KeyCode::Char('e')));
        let Mode::Form(form) = &app.mode else {
            panic!("expected the edit form");
        };
        assert_eq!(form.title(), "Edit project 0b1c2d3e");
        assert_eq!(form.fields.len(), 1);
        assert!(rendered(&mut app).contains("Edit project"));
        app.handle_key(key(KeyCode::End));
        for ch in " Web".chars() {
            app.handle_key(key(KeyCode::Char(ch)));
        }
        app.handle_key(key(KeyCode::Enter));
        assert!(matches!(app.mode, Mode::Browse), "{:?}", app.mode);
        assert_eq!(app.projects[0].name, "Portal Web");

        app.handle_key(key(KeyCode::Char('e')));
        for _ in 0.."Portal Web".len() {
            app.handle_key(key(KeyCode::Backspace));
        }
        app.handle_key(key(KeyCode::Enter));
        let Mode::Message { title, lines } = &app.mode else {
            panic!("expected a validation error");
        };
        assert_eq!(title, "Operation failed");
        assert!(!lines.join(" ").contains("Portal"), "{lines:?}");
        assert_eq!(app.projects[0].name, "Portal Web");
    }

    #[test]
    fn pasted_text_keeps_line_breaks_and_drops_controls() {
        let mut field = FormField::new("Request", "");
        field.insert_text("Add reset\r\nby email\u{1b}[31m\tnow\r");
        assert_eq!(field.value, "Add reset\nby email[31m now\n");
        assert_eq!(field.cursor, field.value.chars().count());
    }

    fn prompt_text(lines: &[Line<'_>]) -> Vec<String> {
        lines
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.as_ref())
                    .collect()
            })
            .collect()
    }

    #[test]
    fn the_prompt_wraps_and_scrolls_to_the_cursor() {
        let theme = Theme {
            color: false,
            unicode: true,
        };
        let mut field = FormField::new("Request", "");
        assert_eq!(
            prompt_text(&prompt_lines(&field, 20, 3, theme)),
            vec!["▏ e.g. Add password reset by email"]
        );
        field.insert_text("abcdefghij\nxy");
        // A word longer than the row is split.
        assert_eq!(
            prompt_text(&prompt_lines(&field, 4, 5, theme)),
            vec!["abcd", "efgh", "ij", "xy▏"]
        );
        // Only the last rows are visible when the cursor is at the end.
        assert_eq!(
            prompt_text(&prompt_lines(&field, 4, 2, theme)),
            vec!["ij", "xy▏"]
        );
        field.home();
        assert_eq!(
            prompt_text(&prompt_lines(&field, 4, 2, theme)),
            vec!["abcd", "efgh"]
        );
        // A full row moves the end cursor to a new row.
        let mut full = FormField::new("Request", "");
        full.insert_text("abcd");
        assert_eq!(
            prompt_text(&prompt_lines(&full, 4, 3, theme)),
            vec!["abcd", "▏"]
        );
        // Words move whole to the next row; the cursor follows its character.
        let mut words = FormField::new("Request", "");
        words.insert_text("reset my password");
        assert_eq!(
            prompt_text(&prompt_lines(&words, 10, 3, theme)),
            vec!["reset my", "password▏"]
        );
        words.home();
        for _ in 0..9 {
            words.right();
        }
        assert_eq!(
            prompt_text(&prompt_lines(&words, 10, 3, theme)),
            vec!["reset my", "password"]
        );
        let lines = prompt_lines(&words, 10, 3, theme);
        assert_eq!(lines[1].spans[1].content.as_ref(), "p");
    }

    #[test]
    fn narrow_footers_pick_a_shorter_hint() {
        assert_eq!(fitting(&["long hint", "short"], 20), "long hint");
        assert_eq!(fitting(&["long hint", "short"], 6), "short");
        assert_eq!(fitting(&["long hint", "short"], 2), "short");
    }

    #[test]
    fn the_projects_dashboard_opens_a_clean_request_prompt() {
        let (mut app, _dir) = seeded();
        app.handle_key(key(KeyCode::Char('r')));
        let Mode::Prompt(prompt) = &app.mode else {
            panic!("expected the prompt, got {:?}", app.mode);
        };
        assert_eq!(prompt.project_id.as_str(), "0b1c2d3e");
        let screen = rendered(&mut app);
        for text in [
            "New request · Portal",
            "Request",
            "e.g. Add password reset by email",
            "Enter submit",
            "Esc cancel",
            "0 chars",
        ] {
            assert!(screen.contains(text), "{text}: {screen}");
        }

        // An empty request is not submitted.
        app.handle_key(key(KeyCode::Enter));
        let Mode::Prompt(prompt) = &app.mode else {
            panic!("the prompt closed");
        };
        assert_eq!(prompt.notice, Some("Write a request first."));
        assert!(rendered(&mut app).contains("Write a request first."));

        for ch in "Add reset".chars() {
            app.handle_key(key(KeyCode::Char(ch)));
        }
        app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::ALT));
        for ch in "by email".chars() {
            app.handle_key(key(KeyCode::Char(ch)));
        }
        assert!(rendered(&mut app).contains("18 chars"));
        let deferred = app.handle_key(key(KeyCode::Enter));
        assert_eq!(deferred, Some(Deferred::Plan));
        let (project, text) = app.pending_request.clone().unwrap();
        assert_eq!(project.as_str(), "0b1c2d3e");
        assert_eq!(text, "Add reset\nby email");

        app.pending_request = None;
        app.handle_key(key(KeyCode::Char('r')));
        app.handle_key(key(KeyCode::Char('x')));
        assert_eq!(app.handle_key(key(KeyCode::Esc)), None);
        assert!(matches!(app.mode, Mode::Browse));
        assert!(app.pending_request.is_none());
    }

    #[test]
    fn the_kanban_offers_a_request_from_the_empty_state() {
        let (mut app, _dir) = kanban_fixture(Vec::new());
        let screen = rendered(&mut app);
        assert!(screen.contains("Press n to submit"), "{screen}");
        assert!(!screen.contains("--new-request"), "{screen}");
        assert!(screen.contains("[n]"), "{screen}");
        app.handle_key(key(KeyCode::Char('n')));
        let Mode::Prompt(prompt) = &app.mode else {
            panic!("expected the prompt");
        };
        assert_eq!(prompt.project_name, "Portal");
    }

    #[test]
    fn planning_outcomes_are_reported_and_sanitized() {
        let (mut app, _dir) = kanban_fixture(Vec::new());
        let request_id = crate::domain::ids::RequestId::parse("1c2d3e4f").unwrap();
        let (title, lines) = app.outcome_lines(&PlanningOutcome::NeedsClarification {
            request_id: request_id.clone(),
            questions: vec!["Which \u{1b}[31memail provider?".to_owned()],
        });
        assert_eq!(title, "Request 1c2d3e4f needs clarification");
        assert!(
            lines
                .join("\n")
                .contains("Press n to submit a revised request.")
        );
        app.mode = Mode::Message { title, lines };
        let screen = rendered(&mut app);
        assert!(screen.contains("1. Which email provider?"), "{screen}");
        assert!(!screen.contains('\u{1b}'), "{screen}");

        let (title, lines) = app.outcome_lines(&PlanningOutcome::Cancelled {
            request_id: request_id.clone(),
        });
        assert_eq!(title, "Request 1c2d3e4f cancelled");
        assert!(lines.join(" ").contains("Provider-side processing"));

        let (title, lines) = app.outcome_lines(&PlanningOutcome::Failed {
            request_id,
            code: "provider_timeout",
            hint: "Try again later.",
            validation: false,
        });
        assert_eq!(title, "Request 1c2d3e4f failed");
        assert!(lines.contains(&"Code: provider_timeout".to_owned()));

        app.mode = Mode::Planning {
            project_name: "Portal".to_owned(),
        };
        let screen = rendered(&mut app);
        assert!(screen.contains("Planning a request for Portal"), "{screen}");
        assert!(screen.contains("Esc cancel"), "{screen}");
    }

    #[test]
    fn only_esc_and_ctrl_c_cancel_planning() {
        assert!(is_cancel_key(key(KeyCode::Esc)));
        assert!(is_cancel_key(KeyEvent::new(
            KeyCode::Char('c'),
            KeyModifiers::CONTROL
        )));
        assert!(!is_cancel_key(key(KeyCode::Char('c'))));
        assert!(!is_cancel_key(key(KeyCode::Enter)));
    }
}
