//! Project schedule screen (ADR-0057).
//!
//! Schedules are created, paused, resumed, and deleted here. Due schedules are
//! executed by the application on dashboard startup or with the hidden
//! `--run-schedules` command; this screen never calls the provider.

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Clear, Paragraph};

use crate::app::AppError;
use crate::app::directory::Directory;
use crate::app::schedules::ScheduleStore;
use crate::app::storage::AppStateStore;
use crate::domain::ids::{AgentId, ProjectId};
use crate::domain::schedule::Schedule;

use super::{Theme, action_grid, centered_rect, clean, table_header, table_row, wrap_text};

const SCHEDULE_ACTIONS: [(&str, &str); 8] = [
    ("[n]", "New"),
    ("[p]", "Pause/Resume"),
    ("[d]", "Delete"),
    ("[Enter]", "Detail"),
    ("[r]", "Reload"),
    ("[?]", "Help"),
    ("[Esc]", "Back"),
    ("[q]", "Quit"),
];

#[derive(Debug, Clone)]
pub(super) struct ScheduleView {
    pub(super) project_id: ProjectId,
    pub(super) project_name: String,
    pub(super) members: Vec<(AgentId, String)>,
    pub(super) schedules: Vec<Schedule>,
    pub(super) selected: usize,
    pub(super) offset: usize,
    pub(super) page_size: usize,
}

impl ScheduleView {
    pub(super) fn load(
        store: &(impl ScheduleStore + Directory + AppStateStore),
        project: &ProjectId,
    ) -> Result<Self, AppError> {
        let lab = crate::app::context::active_lab(store)?.require()?;
        let project_schedules =
            store
                .project_schedules(project, &lab)?
                .ok_or_else(|| AppError::NotFound {
                    entity: crate::app::EntityKind::Project,
                    id: project.to_string(),
                })?;
        let members = store
            .lab_agents(&lab)?
            .into_iter()
            .filter(|member| !member.archived)
            .map(|member| {
                let label = format!(
                    "{} ({})",
                    member.name,
                    member.role().map(|role| role.as_str()).unwrap_or("?")
                );
                (member.id, label)
            })
            .collect();
        Ok(Self {
            project_id: project.clone(),
            project_name: project_schedules.project_name,
            members,
            schedules: project_schedules.schedules,
            selected: 0,
            offset: 0,
            page_size: 8,
        })
    }

    pub(super) fn reload(&mut self, store: &(impl ScheduleStore + Directory + AppStateStore)) {
        if let Ok(lab) = crate::app::context::active_lab(store).and_then(|active| active.require())
            && let Ok(Some(project)) = store.project_schedules(&self.project_id, &lab)
        {
            self.project_name = project.project_name;
            self.schedules = project.schedules;
            if let Ok(members) = store.lab_agents(&lab) {
                self.members = members
                    .into_iter()
                    .filter(|member| !member.archived)
                    .map(|member| {
                        let label = format!(
                            "{} ({})",
                            member.name,
                            member.role().map(|role| role.as_str()).unwrap_or("?")
                        );
                        (member.id, label)
                    })
                    .collect();
            }
            self.clamp();
        }
    }

    pub(super) fn selected_schedule(&self) -> Option<&Schedule> {
        self.schedules.get(self.selected)
    }

    pub(super) fn owner_label(&self, id: &AgentId) -> String {
        self.members
            .iter()
            .find(|(agent, _)| agent == id)
            .map(|(_, label)| label.clone())
            .unwrap_or_else(|| id.to_string())
    }

    pub(super) fn move_selection(&mut self, delta: isize) {
        let len = self.schedules.len();
        if len == 0 {
            self.selected = 0;
            self.offset = 0;
            return;
        }
        self.selected = self.selected.saturating_add_signed(delta).min(len - 1);
        self.fit();
    }

    pub(super) fn page(&mut self, direction: isize) {
        let rows = isize::try_from(self.page_size.max(1)).unwrap_or(1);
        self.move_selection(direction * rows);
    }

    pub(super) fn first(&mut self) {
        self.selected = 0;
        self.fit();
    }

    pub(super) fn last(&mut self) {
        self.selected = self.schedules.len().saturating_sub(1);
        self.fit();
    }

    fn fit(&mut self) {
        let page = self.page_size.max(1);
        if self.selected < self.offset {
            self.offset = self.selected;
        } else if self.selected >= self.offset + page {
            self.offset = self.selected + 1 - page;
        }
    }

    fn clamp(&mut self) {
        let len = self.schedules.len();
        self.selected = self.selected.min(len.saturating_sub(1));
        self.offset = self.offset.min(self.selected);
        self.fit();
    }
}

pub(super) fn render(
    frame: &mut Frame<'_>,
    area: Rect,
    theme: Theme,
    view: &mut ScheduleView,
    status: Option<&str>,
) {
    let width = usize::from(area.width.saturating_sub(2));
    let mut footer = action_grid(&SCHEDULE_ACTIONS, theme, width);
    if let Some(status) = status {
        footer.push(Line::from(vec![
            Span::styled("Status: ", theme.key()),
            Span::styled(clean(status), theme.success_style()),
        ]));
    } else if let Some(schedule) = view.selected_schedule() {
        footer.push(Line::from(vec![
            Span::styled("Selected: ", theme.key()),
            Span::styled(clean(schedule.title.as_str()), theme.muted_style()),
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
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("Scheduled tasks", theme.title()),
            Span::raw("  "),
            Span::styled(
                format!(
                    "{} — {} · {} schedules",
                    view.project_id,
                    clean(&view.project_name),
                    view.schedules.len()
                ),
                theme.muted_style(),
            ),
        ]))
        .block(super::block(theme, "Scheduled tasks")),
        chunks[0],
    );

    view.page_size = usize::from(chunks[1].height.saturating_sub(3)).max(1);
    view.clamp();
    let widths = table_widths(usize::from(chunks[1].width.saturating_sub(2)));
    let mut lines = vec![table_header(
        &[
            "TITLE",
            "OWNER",
            "EVERY",
            "NEXT RUN",
            "STATUS",
            "LAST RESULT",
        ],
        &widths,
        theme,
    )];
    if view.schedules.is_empty() {
        lines.push(Line::from(Span::styled(
            "No scheduled tasks. Press n to create one.",
            theme.muted_style(),
        )));
    } else {
        let start = view.offset;
        let end = (start + view.page_size).min(view.schedules.len());
        for (row, schedule) in view.schedules.iter().enumerate().take(end).skip(start) {
            let status = if schedule.enabled { "active" } else { "paused" };
            let last = schedule
                .last_error_code
                .as_deref()
                .or(schedule.last_request_status.as_deref())
                .unwrap_or("—");
            lines.push(table_row(
                &[
                    schedule.title.as_str().to_owned(),
                    view.owner_label(&schedule.agent_id),
                    schedule.interval.label().to_owned(),
                    schedule.next_run_at.clone(),
                    status.to_owned(),
                    last.to_owned(),
                ],
                &widths,
                row == view.selected,
                theme,
            ));
        }
    }
    frame.render_widget(
        Paragraph::new(lines).block(super::block(theme, "Schedules")),
        chunks[1],
    );
    frame.render_widget(
        Paragraph::new(footer).block(super::block(theme, "Actions")),
        chunks[2],
    );
}

fn table_widths(available: usize) -> Vec<usize> {
    const GAP: usize = 2;
    let fixed = 20 + 18 + 8 + 20 + 8 + GAP * 5;
    vec![20, 18, 8, 20, 8, available.saturating_sub(fixed).max(10)]
}

/// Scrollable detail dialog.
pub(super) fn render_detail(
    frame: &mut Frame<'_>,
    area: Rect,
    theme: Theme,
    schedule: &Schedule,
    owner: &str,
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
            format!(" Schedule {} ", schedule.id),
            theme.key(),
        )));
    let inner = block.inner(rect);
    let lines = detail_lines(schedule, owner, theme, usize::from(inner.width));
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

const LABEL_WIDTH: usize = 13;

fn detail_lines(
    schedule: &Schedule,
    owner: &str,
    theme: Theme,
    width: usize,
) -> Vec<Line<'static>> {
    let label_style = theme.muted_style().add_modifier(Modifier::BOLD);
    let value_width = width.saturating_sub(LABEL_WIDTH).max(1);
    let mut lines = Vec::new();
    let field = |lines: &mut Vec<Line<'static>>, label: &str, value: String, style: Style| {
        for (index, part) in wrap_text(&value, value_width).into_iter().enumerate() {
            let label = if index == 0 { label } else { "" };
            lines.push(Line::from(vec![
                Span::styled(format!("{label:<LABEL_WIDTH$}"), label_style),
                Span::styled(part, style),
            ]));
        }
    };
    field(
        &mut lines,
        "Title",
        clean(schedule.title.as_str()),
        Style::default(),
    );
    field(&mut lines, "Owner", owner.to_owned(), Style::default());
    field(
        &mut lines,
        "Every",
        schedule.interval.label().to_owned(),
        Style::default(),
    );
    field(
        &mut lines,
        "Status",
        if schedule.enabled { "active" } else { "paused" }.to_owned(),
        if schedule.enabled {
            theme.success_style()
        } else {
            theme.muted_style()
        },
    );
    field(
        &mut lines,
        "Next run",
        clean(&schedule.next_run_at),
        Style::default(),
    );
    field(
        &mut lines,
        "Last run",
        schedule
            .last_run_at
            .as_deref()
            .map(clean)
            .unwrap_or_else(|| "—".to_owned()),
        Style::default(),
    );
    field(
        &mut lines,
        "Last request",
        schedule
            .last_request_id
            .as_ref()
            .map(|id| id.to_string())
            .unwrap_or_else(|| "—".to_owned()),
        Style::default(),
    );
    field(
        &mut lines,
        "Last result",
        schedule
            .last_error_code
            .as_deref()
            .or(schedule.last_request_status.as_deref())
            .unwrap_or("—")
            .to_owned(),
        Style::default(),
    );
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled("Task request", label_style)));
    for line in wrap_text(&clean(schedule.request_text.as_str()), width) {
        lines.push(Line::from(line));
    }
    lines
}

#[cfg(test)]
mod tests {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    use super::*;
    use crate::domain::agent::Description;
    use crate::domain::ids::{ProjectId, ScheduleId};
    use crate::domain::name::Name;
    use crate::domain::schedule::ScheduleInterval;

    fn schedule(enabled: bool) -> Schedule {
        Schedule {
            id: ScheduleId::parse("30000001").unwrap(),
            project_id: ProjectId::parse("0b1c2d3e").unwrap(),
            agent_id: AgentId::parse("10000001").unwrap(),
            title: Name::parse("Weekly security review").unwrap(),
            request_text: Description::parse("Review dependencies and auth boundaries.").unwrap(),
            interval: ScheduleInterval::Weekly,
            enabled,
            next_run_at: "2026-01-08T10:00:00Z".to_owned(),
            last_run_at: Some("2026-01-01T10:00:00Z".to_owned()),
            last_request_id: None,
            last_request_status: Some("PLANNED".to_owned()),
            last_error_code: None,
        }
    }

    fn view() -> ScheduleView {
        ScheduleView {
            project_id: ProjectId::parse("0b1c2d3e").unwrap(),
            project_name: "Portal".to_owned(),
            members: vec![(
                AgentId::parse("10000001").unwrap(),
                "Klaus (lead)".to_owned(),
            )],
            schedules: vec![schedule(true), schedule(false)],
            selected: 0,
            offset: 0,
            page_size: 8,
        }
    }

    fn theme() -> Theme {
        Theme {
            color: false,
            unicode: true,
        }
    }

    fn scene(view: &mut ScheduleView, width: u16, height: u16) -> String {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| render(frame, frame.area(), theme(), view, None))
            .unwrap();
        terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect()
    }

    #[test]
    fn the_screen_lists_schedules_and_status() {
        let mut view = view();
        let screen = scene(&mut view, 120, 30);
        for text in [
            "Scheduled tasks",
            "Portal",
            "Weekly security review",
            "Klaus (lead)",
            "Weekly",
            "2026-01-08",
            "active",
            "paused",
            "PLANNED",
            "[n]",
            "[p]",
        ] {
            assert!(screen.contains(text), "{text}: {screen}");
        }
    }

    #[test]
    fn navigation_and_detail_lines_work() {
        let mut view = view();
        view.move_selection(1);
        assert!(!view.selected_schedule().unwrap().enabled);
        view.first();
        assert!(view.selected_schedule().unwrap().enabled);
        let schedule = view.selected_schedule().unwrap();
        let lines = detail_lines(schedule, "Klaus (lead)", theme(), 70);
        let text = lines
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.as_ref())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");
        for expected in [
            "Weekly security review",
            "Klaus (lead)",
            "Weekly",
            "Next run",
            "Task request",
            "Review dependencies",
        ] {
            assert!(text.contains(expected), "{expected}: {text}");
        }
    }
}
