//! Two-pane laboratory populate screen (ADR-0053).
//!
//! Changes are staged in memory and written only with `s`. The screen reads
//! and writes local data; it never calls the provider.

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Paragraph};

use crate::app::AppError;
use crate::app::directory::{Directory, LabOverview};
use crate::app::populate::{PopulateAgent, PopulateSnapshot};
use crate::domain::ids::{AgentId, LabId};

use super::{Theme, action_grid, clean, table_header, table_row};

const POPULATE_ACTIONS: [(&str, &str); 8] = [
    ("[Tab]", "Side"),
    ("[↑/↓]", "Agent"),
    ("[Enter]", "Move"),
    ("[v]", "Details"),
    ("[s]", "Save"),
    ("[?]", "Help"),
    ("[Esc]", "Cancel"),
    ("[q]", "Quit"),
];

/// Which pane has focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Side {
    Available,
    Members,
}

impl Side {
    fn index(self) -> usize {
        match self {
            Self::Available => 0,
            Self::Members => 1,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Available => "Available agents",
            Self::Members => "In this laboratory",
        }
    }

    fn other(self) -> Self {
        match self {
            Self::Available => Self::Members,
            Self::Members => Self::Available,
        }
    }
}

/// Navigation and staged membership state for one laboratory.
#[derive(Debug, Clone)]
pub(super) struct PopulateView {
    pub(super) lab: LabOverview,
    pub(super) available: Vec<PopulateAgent>,
    pub(super) members: Vec<PopulateAgent>,
    pub(super) archived_members: usize,
    side: Side,
    selected: [usize; 2],
    offsets: [usize; 2],
    page_size: usize,
    baseline: Vec<AgentId>,
}

impl PopulateView {
    pub(super) fn load(store: &impl Directory, lab_id: &LabId) -> Result<Self, AppError> {
        Ok(Self::from_snapshot(crate::app::populate::show(
            store, lab_id,
        )?))
    }

    pub(super) fn from_snapshot(snapshot: PopulateSnapshot) -> Self {
        let baseline = snapshot
            .members
            .iter()
            .map(|agent| agent.id.clone())
            .collect();
        Self {
            lab: snapshot.lab,
            available: snapshot.available,
            members: snapshot.members,
            archived_members: snapshot.archived_members,
            side: Side::Available,
            selected: [0; 2],
            offsets: [0; 2],
            page_size: 8,
            baseline,
        }
    }

    pub(super) fn has_changes(&self) -> bool {
        let mut current: Vec<&str> = self.members.iter().map(|agent| agent.id.as_str()).collect();
        let mut baseline: Vec<&str> = self.baseline.iter().map(AgentId::as_str).collect();
        current.sort_unstable();
        baseline.sort_unstable();
        current != baseline
    }

    /// Additions and removals relative to the loaded membership set.
    pub(super) fn changes(&self) -> (usize, usize) {
        let current: std::collections::HashSet<&AgentId> =
            self.members.iter().map(|agent| &agent.id).collect();
        let baseline: std::collections::HashSet<&AgentId> = self.baseline.iter().collect();
        let added = current.difference(&baseline).count();
        let removed = baseline.difference(&current).count();
        (added, removed)
    }

    pub(super) fn desired_ids(&self) -> Vec<AgentId> {
        self.members.iter().map(|agent| agent.id.clone()).collect()
    }

    pub(super) fn mark_saved(&mut self) {
        self.baseline = self.desired_ids();
    }

    pub(super) fn lead_count(&self) -> usize {
        self.members.iter().filter(|agent| agent.is_lead()).count()
    }

    pub(super) fn lead_name(&self) -> Option<&str> {
        self.members
            .iter()
            .find(|agent| agent.is_lead())
            .map(|agent| agent.name.as_str())
    }

    pub(super) fn selected_agent(&self) -> Option<&PopulateAgent> {
        self.list().get(self.selected[self.side.index()])
    }

    pub(super) fn switch_side(&mut self, delta: isize) {
        self.side = if delta < 0 {
            Side::Available
        } else {
            Side::Members
        };
        self.focus_selected();
    }

    pub(super) fn move_selection(&mut self, delta: isize) {
        let len = self.list().len();
        if len == 0 {
            self.selected[self.side.index()] = 0;
            self.offsets[self.side.index()] = 0;
            return;
        }
        let selected = &mut self.selected[self.side.index()];
        *selected = selected.saturating_add_signed(delta).min(len - 1);
        self.fit();
    }

    pub(super) fn first(&mut self) {
        self.selected[self.side.index()] = 0;
        self.fit();
    }

    pub(super) fn last(&mut self) {
        self.selected[self.side.index()] = self.list().len().saturating_sub(1);
        self.fit();
    }

    pub(super) fn page(&mut self, direction: isize) {
        let rows = isize::try_from(self.page_size.max(1)).unwrap_or(1);
        self.move_selection(direction * rows);
    }

    /// Moves the selected agent to the other pane. The source pane keeps the
    /// nearest selection.
    pub(super) fn move_selected(&mut self) {
        let source = self.side;
        let index = self.selected[source.index()];
        let Some(agent) = self.list().get(index).cloned() else {
            return;
        };
        self.list_mut(source).remove(index);
        let target = source.other();
        self.list_mut(target).push(agent);
        sort_agents(self.list_mut(target));
        let len = self.list().len();
        self.selected[source.index()] = index.min(len.saturating_sub(1));
        self.offsets[source.index()] =
            self.offsets[source.index()].min(self.selected[source.index()]);
        self.fit();
    }

    pub(super) fn empty_hint(&self) -> Option<&'static str> {
        if self.available.is_empty() && self.members.is_empty() {
            Some("No agents available. Run `make load-preset`, then reopen this screen.")
        } else if self.available.is_empty() {
            Some("Every active agent is already in this laboratory.")
        } else {
            None
        }
    }

    fn list(&self) -> &[PopulateAgent] {
        match self.side {
            Side::Available => &self.available,
            Side::Members => &self.members,
        }
    }

    fn list_mut(&mut self, side: Side) -> &mut Vec<PopulateAgent> {
        match side {
            Side::Available => &mut self.available,
            Side::Members => &mut self.members,
        }
    }

    fn focus_selected(&mut self) {
        self.fit();
    }

    fn fit(&mut self) {
        let index = self.side.index();
        let selected = self.selected[index];
        let page = self.page_size.max(1);
        if selected < self.offsets[index] {
            self.offsets[index] = selected;
        } else if selected >= self.offsets[index] + page {
            self.offsets[index] = selected + 1 - page;
        }
    }
}

fn sort_agents(agents: &mut [PopulateAgent]) {
    agents.sort_by(|left, right| {
        left.name
            .to_lowercase()
            .cmp(&right.name.to_lowercase())
            .then_with(|| left.id.as_str().cmp(right.id.as_str()))
    });
}

/// Renders the screen and updates the visible page size.
pub(super) fn render(
    frame: &mut Frame<'_>,
    area: Rect,
    theme: Theme,
    view: &mut PopulateView,
    status: Option<&str>,
) {
    let width = usize::from(area.width.saturating_sub(2));
    let mut footer = action_grid(&POPULATE_ACTIONS, theme, width);
    if let Some(status) = status {
        footer.push(Line::from(vec![
            Span::styled("Status: ", theme.key()),
            Span::styled(clean(status), theme.success_style()),
        ]));
    } else {
        footer.push(lead_line(theme, view));
        footer.push(changes_line(theme, view));
    }
    if let Some(hint) = view.empty_hint() {
        footer.push(Line::from(Span::styled(
            hint.to_owned(),
            theme.warning_style(),
        )));
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

    let archived = if view.archived_members == 0 {
        String::new()
    } else {
        format!(" · {} archived hidden", view.archived_members)
    };
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("Populate laboratory", theme.title()),
            Span::raw("  "),
            Span::styled(
                format!(
                    "{} — {} · available {} · in lab {}{archived}",
                    view.lab.id,
                    clean(&view.lab.name),
                    view.available.len(),
                    view.members.len(),
                ),
                theme.muted_style(),
            ),
        ]))
        .block(super::block(theme, "Populate laboratory")),
        chunks[0],
    );

    view.page_size = usize::from(chunks[1].height.saturating_sub(3)).max(1);
    view.fit();
    let panes = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(chunks[1]);
    render_pane(frame, panes[0], theme, view, Side::Available);
    render_pane(frame, panes[1], theme, view, Side::Members);

    frame.render_widget(
        Paragraph::new(footer).block(super::block(theme, "Actions")),
        chunks[2],
    );
}

fn render_pane(frame: &mut Frame<'_>, area: Rect, theme: Theme, view: &PopulateView, side: Side) {
    let focused = view.side == side;
    let list = match side {
        Side::Available => &view.available,
        Side::Members => &view.members,
    };
    let total = list.len();
    let index = side.index();
    let selected = view.selected[index].min(total.saturating_sub(1));
    let offset = view.offsets[index].min(selected);
    let border = if focused {
        theme.key()
    } else {
        theme.border_style()
    };
    let title_style = if focused { theme.key() } else { theme.title() };
    let mut block = Block::bordered()
        .border_type(if theme.unicode {
            BorderType::Rounded
        } else {
            BorderType::Plain
        })
        .title(Line::from(Span::styled(
            format!(" {} ({total}) ", side.label()),
            title_style,
        )))
        .border_style(border);
    if total > view.page_size {
        block = block.title_bottom(
            Line::from(Span::styled(
                format!(" {}/{} ", selected + 1, total),
                theme.muted_style(),
            ))
            .right_aligned(),
        );
    }
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if total == 0 {
        let text = match side {
            Side::Available => "No available agents.",
            Side::Members => "No agents in this laboratory yet.",
        };
        frame.render_widget(Paragraph::new(text), inner);
        return;
    }

    let widths = pane_widths(usize::from(inner.width));
    let mut lines = vec![table_header(&["NAME", "ROLE", "PROFILE"], &widths, theme)];
    let start = offset;
    let end = (start + view.page_size).min(total);
    for (row, agent) in list.iter().enumerate().take(end).skip(start) {
        lines.push(table_row(
            &[
                agent.name.clone(),
                agent.role.clone(),
                agent.description.clone(),
            ],
            &widths,
            focused && row == selected,
            theme,
        ));
    }
    frame.render_widget(Paragraph::new(lines), inner);
}

fn pane_widths(available: usize) -> Vec<usize> {
    let text = available.saturating_sub(4);
    let name = (text * 2 / 5).clamp(8, 24);
    let role = (text / 4).clamp(4, 10);
    let profile = text.saturating_sub(name + role).max(4);
    vec![name, role, profile]
}

fn lead_line(theme: Theme, view: &PopulateView) -> Line<'static> {
    let count = view.lead_count();
    if count == 1 {
        Line::from(vec![
            Span::styled("Tech Lead: ", theme.key()),
            Span::styled(
                view.lead_name().unwrap_or_default().to_owned(),
                Style::default(),
            ),
        ])
    } else if count == 0 {
        Line::from(Span::styled(
            "Warning: no Tech Lead selected.",
            theme.warning_style(),
        ))
    } else {
        Line::from(Span::styled(
            format!("Warning: {count} Tech Leads selected; keep exactly one."),
            theme.warning_style(),
        ))
    }
}

fn changes_line(theme: Theme, view: &PopulateView) -> Line<'static> {
    let (added, removed) = view.changes();
    if added == 0 && removed == 0 {
        Line::from(Span::styled("No unsaved changes.", theme.muted_style()))
    } else {
        Line::from(vec![
            Span::styled("Changes: ", theme.key()),
            Span::styled(
                format!("+{added} added · -{removed} removed · press s to save"),
                theme.success_style(),
            ),
        ])
    }
}

#[cfg(test)]
mod tests {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    use super::*;
    use crate::app::directory::LabOverview;

    fn agent(id: &str, name: &str, role: &str) -> PopulateAgent {
        PopulateAgent {
            id: AgentId::parse(id).unwrap(),
            name: name.to_owned(),
            role: role.to_owned(),
            description: format!("{role} profile"),
            skills: vec!["rust".to_owned()],
        }
    }

    fn view() -> PopulateView {
        PopulateView::from_snapshot(PopulateSnapshot {
            lab: LabOverview {
                id: LabId::parse("a3f91c07").unwrap(),
                name: "Payments".to_owned(),
                description: String::new(),
                agent_count: 1,
                project_count: 0,
            },
            available: vec![
                agent("10000002", "Bram", "dev"),
                agent("10000003", "Nigel", "qa"),
            ],
            members: vec![agent("10000001", "Klaus", "lead")],
            archived_members: 0,
        })
    }

    fn scene(view: &mut PopulateView, width: u16, height: u16) -> String {
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

    fn theme() -> Theme {
        Theme {
            color: false,
            unicode: true,
        }
    }

    #[test]
    fn the_screen_shows_both_panes_and_the_lead() {
        let mut view = view();
        let screen = scene(&mut view, 100, 30);
        for text in [
            "Populate laboratory",
            "Payments",
            "Available agents (2)",
            "In this laboratory (1)",
            "Bram",
            "Nigel",
            "Klaus",
            "dev profile",
            "Tech Lead: Klaus",
            "No unsaved changes.",
            "[Enter]",
            "[v]",
            "Details",
        ] {
            assert!(screen.contains(text), "{text}: {screen}");
        }
    }

    #[test]
    fn moving_an_agent_stages_the_change() {
        let mut view = view();
        assert!(!view.has_changes());
        view.move_selected();
        assert!(view.has_changes());
        assert_eq!(view.changes(), (1, 0));
        assert_eq!(view.members.len(), 2);
        assert_eq!(view.available.len(), 1);
        assert_eq!(view.members[0].name, "Bram");
        assert!(
            view.desired_ids()
                .contains(&AgentId::parse("10000002").unwrap())
        );
        let screen = scene(&mut view, 100, 30);
        assert!(screen.contains("+1 added"), "{screen}");
    }

    #[test]
    fn switching_side_and_moving_back_restores_the_baseline() {
        let mut view = view();
        view.move_selected();
        view.switch_side(1);
        assert_eq!(
            view.members[view.selected[Side::Members.index()]].name,
            "Bram"
        );
        view.move_selection(1);
        assert_eq!(
            view.members[view.selected[Side::Members.index()]].name,
            "Klaus"
        );
        view.move_selection(-1);
        view.move_selected();
        assert!(!view.has_changes());
        assert_eq!(view.available.len(), 2);
        assert_eq!(view.members.len(), 1);
    }

    #[test]
    fn the_lead_warning_is_visible_when_no_lead_is_selected() {
        let mut view = view();
        view.switch_side(1);
        view.move_selected();
        let screen = scene(&mut view, 100, 30);
        assert!(screen.contains("no Tech Lead selected"), "{screen}");
    }

    #[test]
    fn scroll_and_empty_states_do_not_panic() {
        let mut view = view();
        view.page(1);
        let screen = scene(&mut view, 60, 20);
        assert!(screen.contains("Available agents"), "{screen}");

        let mut empty = PopulateView::from_snapshot(PopulateSnapshot {
            lab: view.lab.clone(),
            available: Vec::new(),
            members: Vec::new(),
            archived_members: 0,
        });
        let screen = scene(&mut empty, 100, 30);
        assert!(
            screen.contains("No agents in this laboratory yet."),
            "{screen}"
        );
        assert!(screen.contains("make load-preset"), "{screen}");
    }
}
