//! Project interaction history screen (ADR-0056).
//!
//! The screen is read-only over persisted project data. It never calls the
//! provider or changes a story.

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Clear, Paragraph};

use crate::app::AppError;
use crate::app::interactions::{InteractionEntry, InteractionStore, ProjectInteractionHistory};
use crate::app::storage::AppStateStore;
use crate::domain::ids::ProjectId;

use super::{Theme, action_grid, centered_rect, clean, table_header, table_row, wrap_text};

const INTERACTION_ACTIONS: [(&str, &str); 5] = [
    ("[Enter]", "Detail"),
    ("[r]", "Reload"),
    ("[?]", "Help"),
    ("[Esc]", "Back"),
    ("[q]", "Quit"),
];

#[derive(Debug, Clone)]
pub(super) struct InteractionView {
    pub(super) history: ProjectInteractionHistory,
    pub(super) selected: usize,
    pub(super) offset: usize,
    pub(super) page_size: usize,
}

impl InteractionView {
    pub(super) fn load(
        store: &(impl InteractionStore + AppStateStore),
        project: &ProjectId,
    ) -> Result<Self, AppError> {
        Ok(Self {
            history: crate::app::interactions::show(store, store, project)?,
            selected: 0,
            offset: 0,
            page_size: 8,
        })
    }

    pub(super) fn reload(&mut self, store: &(impl InteractionStore + AppStateStore)) {
        if let Ok(history) = crate::app::interactions::show(store, store, &self.history.project_id)
        {
            self.history = history;
            self.clamp();
        }
    }

    pub(super) fn selected_entry(&self) -> Option<&InteractionEntry> {
        self.history.entries.get(self.selected)
    }

    pub(super) fn move_selection(&mut self, delta: isize) {
        let len = self.history.entries.len();
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
        self.selected = self.history.entries.len().saturating_sub(1);
        self.fit();
    }

    fn fit(&mut self) {
        let selected = self.selected;
        let page = self.page_size.max(1);
        if selected < self.offset {
            self.offset = selected;
        } else if selected >= self.offset + page {
            self.offset = selected + 1 - page;
        }
    }

    fn clamp(&mut self) {
        let len = self.history.entries.len();
        self.selected = self.selected.min(len.saturating_sub(1));
        self.offset = self.offset.min(self.selected);
        self.fit();
    }
}

pub(super) fn render(
    frame: &mut Frame<'_>,
    area: Rect,
    theme: Theme,
    view: &mut InteractionView,
    status: Option<&str>,
) {
    let width = usize::from(area.width.saturating_sub(2));
    let mut footer = action_grid(&INTERACTION_ACTIONS, theme, width);
    if let Some(status) = status {
        footer.push(Line::from(vec![
            Span::styled("Status: ", theme.key()),
            Span::styled(clean(status), theme.success_style()),
        ]));
    } else if let Some(entry) = view.selected_entry() {
        footer.push(Line::from(vec![
            Span::styled("Selected: ", theme.key()),
            Span::styled(
                format!("{} — {}", entry.kind.label(), clean(&entry.reference)),
                theme.muted_style(),
            ),
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
            Span::styled("Project interactions", theme.title()),
            Span::raw("  "),
            Span::styled(
                format!(
                    "{} — {} · {} entries",
                    view.history.project_id,
                    clean(&view.history.project_name),
                    view.history.entries.len()
                ),
                theme.muted_style(),
            ),
        ]))
        .block(super::block(theme, "Project interactions")),
        chunks[0],
    );

    view.page_size = usize::from(chunks[1].height.saturating_sub(3)).max(1);
    view.clamp();
    let inner_width = usize::from(chunks[1].width.saturating_sub(2));
    let widths = table_widths(inner_width);
    let mut lines = vec![table_header(
        &[
            "TIME",
            "KIND",
            "PARTICIPANTS",
            "REFERENCE",
            "OUTCOME",
            "RECAP",
        ],
        &widths,
        theme,
    )];
    if view.history.entries.is_empty() {
        lines.push(Line::from(Span::styled(
            "No interactions yet. Plan a request to start.",
            theme.muted_style(),
        )));
    } else {
        let start = view.offset;
        let end = (start + view.page_size).min(view.history.entries.len());
        for (row, entry) in view
            .history
            .entries
            .iter()
            .enumerate()
            .take(end)
            .skip(start)
        {
            lines.push(table_row(
                &[
                    entry.at.clone(),
                    entry.kind.label().to_owned(),
                    entry
                        .participants
                        .iter()
                        .map(|participant| clean(&participant.label()))
                        .collect::<Vec<_>>()
                        .join(", "),
                    entry.reference.clone(),
                    entry.outcome.label().to_owned(),
                    entry.recap.replace('\n', " "),
                ],
                &widths,
                row == view.selected,
                theme,
            ));
        }
    }
    frame.render_widget(
        Paragraph::new(lines).block(super::block(theme, "Interactions")),
        chunks[1],
    );
    frame.render_widget(
        Paragraph::new(footer).block(super::block(theme, "Actions")),
        chunks[2],
    );
}

fn table_widths(available: usize) -> Vec<usize> {
    const GAP: usize = 2;
    let fixed = 20 + 8 + 20 + 18 + 7 + GAP * 5;
    vec![20, 8, 20, 18, 7, available.saturating_sub(fixed).max(10)]
}

/// Scrollable full-recap dialog.
pub(super) fn render_detail(
    frame: &mut Frame<'_>,
    area: Rect,
    theme: Theme,
    entry: &InteractionEntry,
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
            format!(" {} interaction ", entry.kind.label()),
            theme.key(),
        )));
    let inner = block.inner(rect);
    let lines = detail_lines(entry, theme, usize::from(inner.width));
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

fn detail_lines(entry: &InteractionEntry, theme: Theme, width: usize) -> Vec<Line<'static>> {
    let label_style = theme
        .muted_style()
        .add_modifier(ratatui::style::Modifier::BOLD);
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
        "Kind",
        entry.kind.label().to_owned(),
        Style::default(),
    );
    field(
        &mut lines,
        "Outcome",
        entry.outcome.label().to_owned(),
        match entry.outcome {
            crate::app::interactions::InteractionOutcome::Success => theme.success_style(),
            crate::app::interactions::InteractionOutcome::Failure => theme.warning_style(),
            crate::app::interactions::InteractionOutcome::Pending => theme.muted_style(),
        },
    );
    field(&mut lines, "Time", clean(&entry.at), Style::default());
    field(
        &mut lines,
        "Participants",
        if entry.participants.is_empty() {
            "—".to_owned()
        } else {
            entry
                .participants
                .iter()
                .map(|participant| clean(&participant.label()))
                .collect::<Vec<_>>()
                .join(", ")
        },
        Style::default(),
    );
    field(
        &mut lines,
        "Reference",
        clean(&entry.reference),
        Style::default(),
    );
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled("Recap", label_style)));
    for line in wrap_text(&clean(&entry.recap), width) {
        lines.push(Line::from(line));
    }
    lines
}

#[cfg(test)]
mod tests {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    use super::*;
    use crate::app::interactions::{
        InteractionKind, InteractionOutcome, Participant, ProjectInteractionHistory,
    };
    use crate::domain::ids::{AgentId, ProjectId};

    fn agent(id: &str, name: &str, role: &str) -> Participant {
        Participant {
            id: AgentId::parse(id).unwrap(),
            name: name.to_owned(),
            role: role.to_owned(),
        }
    }

    fn entry(kind: InteractionKind, reference: &str, recap: &str) -> InteractionEntry {
        InteractionEntry {
            kind,
            outcome: InteractionOutcome::Success,
            participants: vec![
                agent("10000001", "Klaus", "lead"),
                agent("10000002", "Bram", "dev"),
            ],
            at: "2026-01-01T10:05:00Z".to_owned(),
            reference: reference.to_owned(),
            recap: recap.to_owned(),
        }
    }

    fn view() -> InteractionView {
        InteractionView {
            history: ProjectInteractionHistory {
                project_id: ProjectId::parse("0b1c2d3e").unwrap(),
                project_name: "Portal".to_owned(),
                entries: vec![
                    entry(
                        InteractionKind::Review,
                        "Story 2d3e4f50 — Reset link",
                        "The link expires and tests pass. Decision: accepted.",
                    ),
                    entry(
                        InteractionKind::Work,
                        "Story 2d3e4f50 — Reset link",
                        "Added the reset link and tests.",
                    ),
                    entry(
                        InteractionKind::Planning,
                        "Request 1c2d3e4f",
                        "Request: Add a reset flow. Planned stories: Reset link → Bram.",
                    ),
                ],
            },
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

    fn scene(view: &mut InteractionView, width: u16, height: u16) -> String {
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
    fn the_screen_lists_interactions_and_recaps() {
        let mut view = view();
        let screen = scene(&mut view, 120, 30);
        for text in [
            "Project interactions",
            "Portal",
            "3 entries",
            "TIME",
            "KIND",
            "PARTICIPANTS",
            "RECAP",
            "REVIEW",
            "WORK",
            "PLANNING",
            "Klaus (lead)",
            "Bram",
            "The link expires",
            "[Enter]",
        ] {
            assert!(screen.contains(text), "{text}: {screen}");
        }
    }

    #[test]
    fn navigation_and_empty_state_work() {
        let mut view = view();
        view.move_selection(1);
        assert_eq!(view.selected_entry().unwrap().kind, InteractionKind::Work);
        view.last();
        assert_eq!(
            view.selected_entry().unwrap().kind,
            InteractionKind::Planning
        );
        view.first();
        assert_eq!(view.selected_entry().unwrap().kind, InteractionKind::Review);

        let mut empty = view.clone();
        empty.history.entries.clear();
        let screen = scene(&mut empty, 100, 24);
        assert!(screen.contains("No interactions yet"), "{screen}");
    }

    #[test]
    fn the_detail_contains_the_full_recap() {
        let entry = &view().history.entries[0];
        let theme = theme();
        let lines = detail_lines(entry, theme, 70);
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
            "REVIEW",
            "Outcome",
            "Participants",
            "Story 2d3e4f50",
            "The link expires and tests pass.",
        ] {
            assert!(text.contains(expected), "{expected}: {text}");
        }
    }
}
