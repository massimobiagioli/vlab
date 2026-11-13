//! Laboratory and project memory screen (ADR-0046).
//!
//! Memories are untrusted prompt context. This screen only edits the active
//! laboratory and its current project, and never calls the provider.

use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, Paragraph, Wrap};
use unicode_width::UnicodeWidthChar;

use crate::app::AppError;
use crate::app::directory::Directory;
use crate::app::memories::{MemoryRecord, MemoryStore};
use crate::app::projects::ProjectStore;
use crate::domain::ids::{LabId, MemoryId, ProjectId};
use crate::domain::memory::MemoryScope;

use super::{
    FormField, Theme, action_grid, block, clean, fitting, fixed_rect, prompt_lines, table_header,
    table_row, wrap_text,
};

const MEMORY_ACTIONS: [(&str, &str); 9] = [
    ("[Tab]", "Scope"),
    ("[n]", "New"),
    ("[Enter]", "View"),
    ("[e]", "Edit"),
    ("[d]", "Delete"),
    ("[r]", "Reload"),
    ("[?]", "Help"),
    ("[Esc]", "Back"),
    ("[q]", "Quit"),
];

/// Two tabs: laboratory and current project.
#[derive(Debug, Clone)]
pub(super) struct MemoryView {
    pub(super) lab: LabId,
    pub(super) lab_name: String,
    pub(super) project: Option<(ProjectId, String)>,
    pub(super) lists: [Vec<MemoryRecord>; 2],
    pub(super) selected: [usize; 2],
    pub(super) offsets: [usize; 2],
    pub(super) tab: usize,
    pub(super) page_size: usize,
    pub(super) error: Option<String>,
}

impl MemoryView {
    pub(super) fn load(
        store: &(impl MemoryStore + Directory + ProjectStore),
        lab: &LabId,
        project: Option<&ProjectId>,
    ) -> Result<Self, AppError> {
        let lab_name = store
            .lab(lab)?
            .map(|overview| overview.name)
            .ok_or_else(|| AppError::NotFound {
                entity: crate::app::EntityKind::Laboratory,
                id: lab.to_string(),
            })?;
        let project_info = match project {
            Some(id) => store
                .project_in_lab(id, lab)?
                .map(|overview| (overview.id, overview.name)),
            None => None,
        };
        let lab_memories = store.memories(&MemoryScope::Lab(lab.clone()))?;
        let project_memories = match &project_info {
            Some((id, _)) => store.memories(&MemoryScope::Project(id.clone()))?,
            None => Vec::new(),
        };
        Ok(Self {
            lab: lab.clone(),
            lab_name,
            project: project_info,
            lists: [lab_memories, project_memories],
            selected: [0; 2],
            offsets: [0; 2],
            tab: 0,
            page_size: 8,
            error: None,
        })
    }

    pub(super) fn reload(&mut self, store: &impl MemoryStore) {
        let lab = store.memories(&MemoryScope::Lab(self.lab.clone()));
        let project = match &self.project {
            Some((id, _)) => store.memories(&MemoryScope::Project(id.clone())),
            None => Ok(Vec::new()),
        };
        match (lab, project) {
            (Ok(lab), Ok(project)) => {
                self.lists = [lab, project];
                self.error = None;
            }
            (Err(error), _) | (_, Err(error)) => {
                self.error = Some(format!("{} {}", error, error.hint()));
            }
        }
        self.clamp();
        self.fit();
    }

    pub(super) fn tab(&self) -> usize {
        self.tab
    }

    pub(super) fn switch_tab(&mut self, delta: isize) {
        self.tab = if delta < 0 {
            self.tab.saturating_sub(1)
        } else {
            (self.tab + 1).min(1)
        };
    }

    pub(super) fn list(&self) -> &[MemoryRecord] {
        &self.lists[self.tab]
    }

    pub(super) fn scope(&self) -> Option<MemoryScope> {
        match self.tab {
            0 => Some(MemoryScope::Lab(self.lab.clone())),
            _ => self
                .project
                .as_ref()
                .map(|(id, _)| MemoryScope::Project(id.clone())),
        }
    }

    pub(super) fn scope_label(&self) -> String {
        match self.tab {
            0 => format!("Laboratory {}", clean(&self.lab_name)),
            _ => match &self.project {
                Some((_, name)) => format!("Project {}", clean(name)),
                None => "No current project".to_owned(),
            },
        }
    }

    pub(super) fn selected_record(&self) -> Option<&MemoryRecord> {
        self.list().get(self.selected[self.tab])
    }

    pub(super) fn select_first(&mut self) {
        self.selected[self.tab] = 0;
        self.offsets[self.tab] = 0;
    }

    pub(super) fn move_selection(&mut self, delta: isize) {
        let len = self.list().len();
        if len == 0 {
            self.selected[self.tab] = 0;
            self.offsets[self.tab] = 0;
            return;
        }
        let selected = self.selected[self.tab];
        self.selected[self.tab] = if delta < 0 {
            selected.saturating_sub(delta.unsigned_abs())
        } else {
            selected
                .saturating_add(usize::try_from(delta).unwrap_or(usize::MAX))
                .min(len - 1)
        };
        self.fit();
    }

    pub(super) fn change_page(&mut self, delta: isize) {
        let rows = isize::try_from(self.page_size.max(1)).unwrap_or(1);
        self.move_selection(delta * rows);
    }

    pub(super) fn first(&mut self) {
        self.selected[self.tab] = 0;
        self.offsets[self.tab] = 0;
    }

    pub(super) fn last(&mut self) {
        self.selected[self.tab] = self.list().len().saturating_sub(1);
        self.fit();
    }

    pub(super) fn page_count(&self) -> usize {
        let len = self.list().len();
        if len == 0 {
            return 0;
        }
        len.saturating_sub(1) / self.page_size.max(1) + 1
    }

    fn fit(&mut self) {
        let selected = self.selected[self.tab];
        let page = self.page_size.max(1);
        if selected < self.offsets[self.tab] {
            self.offsets[self.tab] = selected;
        } else if selected >= self.offsets[self.tab] + page {
            self.offsets[self.tab] = selected + 1 - page;
        }
    }

    fn clamp(&mut self) {
        for tab in 0..2 {
            let len = self.lists[tab].len();
            self.selected[tab] = self.selected[tab].min(len.saturating_sub(1));
            self.offsets[tab] = self.offsets[tab].min(self.selected[tab]);
        }
    }
}

pub(super) fn render(
    frame: &mut Frame<'_>,
    area: Rect,
    theme: Theme,
    view: &mut MemoryView,
    status: Option<&str>,
) {
    let footer_width = usize::from(area.width.saturating_sub(2));
    let mut footer = action_grid(&MEMORY_ACTIONS, theme, footer_width);
    if let Some(status) = status {
        footer.push(Line::from(vec![
            Span::styled("Status: ", theme.key()),
            Span::styled(
                truncate(&clean(status), footer_width.saturating_sub(8)),
                theme.success_style(),
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

    let project = view
        .project
        .as_ref()
        .map(|(id, name)| format!("{id} {}", clean(name)))
        .unwrap_or_else(|| "no current project".to_owned());
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("Memories", theme.title()),
            Span::raw("  "),
            Span::styled(
                truncate(
                    &format!(
                        "lab {} {} · project {project}",
                        view.lab,
                        clean(&view.lab_name)
                    ),
                    footer_width,
                ),
                theme.muted_style(),
            ),
        ]))
        .block(block(theme, "Memories")),
        chunks[0],
    );

    let available = usize::from(chunks[1].height.saturating_sub(2));
    view.page_size = available.saturating_sub(3).max(1);
    view.fit();
    let mut lines = vec![tabs_line(theme, view)];
    if let Some(error) = &view.error {
        lines.push(Line::from(Span::styled(
            truncate(error, footer_width),
            theme.warning_style(),
        )));
        lines.push(Line::from(Span::styled(
            "Press r to retry or Esc to go back.",
            theme.muted_style(),
        )));
    } else {
        let widths = memory_widths(usize::from(chunks[1].width.saturating_sub(2)));
        lines.push(table_header(&["ID", "CREATED", "CONTENT"], &widths, theme));
        let rows = view.list().len();
        if rows == 0 {
            lines.push(Line::from(Span::styled(
                if view.tab() == 0 {
                    "No laboratory memories. Press n to add one."
                } else if view.project.is_some() {
                    "No project memories. Press n to add one."
                } else {
                    "Open a project to see its memories."
                },
                theme.muted_style(),
            )));
        } else {
            let start = view.offsets[view.tab()];
            let end = (start + view.page_size).min(rows);
            for index in start..end {
                let record = &view.list()[index];
                let summary = clean(&record.memory.content).replace('\n', " ");
                lines.push(table_row(
                    &[
                        record.memory.id.to_string(),
                        record.created_at.clone(),
                        truncate(&summary, widths[2]),
                    ],
                    &widths,
                    index == view.selected[view.tab()],
                    theme,
                ));
            }
            if rows > view.page_size {
                lines.push(Line::from(Span::styled(
                    format!(
                        "Page {}/{} · {} memories",
                        view.offsets[view.tab()] / view.page_size.max(1) + 1,
                        view.page_count(),
                        rows
                    ),
                    theme.muted_style(),
                )));
            }
        }
    }
    frame.render_widget(
        Paragraph::new(lines).block(block(theme, "Memory list")),
        chunks[1],
    );
    frame.render_widget(
        Paragraph::new(footer).block(block(theme, "Actions")),
        chunks[2],
    );
}

fn tabs_line(theme: Theme, view: &MemoryView) -> Line<'static> {
    let lab = format!("Lab ({})", view.lists[0].len());
    let project = format!("Project ({})", view.lists[1].len());
    let active = |selected: bool| {
        if selected {
            theme.selected()
        } else {
            theme.muted_style()
        }
    };
    Line::from(vec![
        Span::styled(format!(" {lab} "), active(view.tab() == 0)),
        Span::raw("  "),
        Span::styled(format!(" {project} "), active(view.tab() == 1)),
        Span::raw("   Tab switches scope"),
    ])
}

fn memory_widths(available: usize) -> Vec<usize> {
    let fixed = 8 + 24 + 4;
    vec![8, 24, available.saturating_sub(fixed).max(12)]
}

fn truncate(value: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    let mut out = String::new();
    let mut used = 0;
    for ch in value.chars() {
        let next = ch.width().unwrap_or(1);
        if used + next > width {
            if width > used {
                out.push('…');
            }
            return out;
        }
        out.push(ch);
        used += next;
    }
    out
}

#[derive(Debug, Clone)]
pub(super) struct MemoryEditor {
    pub(super) scope: MemoryScope,
    pub(super) scope_label: String,
    pub(super) id: Option<MemoryId>,
    pub(super) field: FormField,
    pub(super) notice: Option<String>,
}

impl MemoryEditor {
    pub(super) fn new(
        scope: MemoryScope,
        scope_label: String,
        existing: Option<&MemoryRecord>,
    ) -> Self {
        Self {
            scope,
            scope_label,
            id: existing.map(|record| record.memory.id.clone()),
            field: FormField::new(
                "Memory",
                existing
                    .map(|record| clean(&record.memory.content))
                    .unwrap_or_default(),
            ),
            notice: None,
        }
    }

    fn title(&self) -> String {
        match &self.id {
            Some(id) => format!("Edit memory {id} · {}", self.scope_label),
            None => format!("New memory · {}", self.scope_label),
        }
    }
}

pub(super) fn render_editor(
    frame: &mut Frame<'_>,
    area: Rect,
    theme: Theme,
    editor: &MemoryEditor,
) {
    let width = 76.min(area.width.saturating_sub(4)).max(40);
    let height = 15.min(area.height.saturating_sub(2));
    let rect = fixed_rect(width, height, area);
    frame.render_widget(Clear, rect);
    let dialog = block(theme, &editor.title());
    let inner = dialog.inner(rect);
    frame.render_widget(dialog, rect);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(3),
            Constraint::Length(1),
        ])
        .split(inner);
    frame.render_widget(
        Paragraph::new(Span::styled(
            fitting(
                &[
                    "Memory text becomes bounded prompt context, never instructions.",
                    "Memory text is prompt context, not instructions.",
                ],
                usize::from(inner.width),
            ),
            theme.muted_style(),
        )),
        chunks[0],
    );
    let area_block = ratatui::widgets::Block::bordered()
        .border_type(if theme.unicode {
            ratatui::widgets::BorderType::Rounded
        } else {
            ratatui::widgets::BorderType::Plain
        })
        .border_style(theme.key())
        .title(Line::from(Span::styled(" Memory ", theme.key())));
    let text_area = area_block.inner(chunks[1]);
    frame.render_widget(area_block, chunks[1]);
    frame.render_widget(
        Paragraph::new(prompt_lines(
            &editor.field,
            usize::from(text_area.width),
            usize::from(text_area.height),
            theme,
        )),
        text_area,
    );
    let count = format!("{} chars", editor.field.value.chars().count());
    let room = usize::from(inner.width).saturating_sub(count.chars().count() + 2);
    let footer = match &editor.notice {
        Some(notice) => Span::styled(truncate(notice, room), theme.warning_style()),
        None => Span::styled(
            fitting(
                &[
                    "Enter save · Alt+Enter new line · Esc cancel",
                    "Enter save · Alt+Enter line · Esc",
                    "Enter save · Esc",
                ],
                room,
            ),
            theme.muted_style(),
        ),
    };
    frame.render_widget(Paragraph::new(Line::from(footer)), chunks[2]);
    frame.render_widget(
        Paragraph::new(Span::styled(count, theme.muted_style())).alignment(Alignment::Right),
        chunks[2],
    );
}

pub(super) fn render_detail(
    frame: &mut Frame<'_>,
    area: Rect,
    theme: Theme,
    record: &MemoryRecord,
    scroll: &mut u16,
) {
    let width = 78.min(area.width.saturating_sub(4)).max(40);
    let height = 18.min(area.height.saturating_sub(2));
    let rect = fixed_rect(width, height, area);
    frame.render_widget(Clear, rect);
    let lines = detail_lines(record, usize::from(rect.width.saturating_sub(4)));
    let view_height = usize::from(rect.height.saturating_sub(2));
    let max = lines.len().saturating_sub(view_height);
    *scroll = (*scroll).min(u16::try_from(max).unwrap_or(u16::MAX));
    frame.render_widget(
        Paragraph::new(lines.join("\n"))
            .wrap(Wrap { trim: false })
            .scroll((*scroll, 0))
            .block(block(
                theme,
                &format!("Memory {} · Esc close", record.memory.id),
            )),
        rect,
    );
}

fn detail_lines(record: &MemoryRecord, width: usize) -> Vec<String> {
    let mut lines = vec![
        format!(
            "Scope: {} {}",
            record.memory.scope.kind(),
            record.memory.scope.owner_id()
        ),
        format!("Created: {}", clean(&record.created_at)),
        String::new(),
        "Content:".to_owned(),
    ];
    for line in clean(&record.memory.content).split('\n') {
        let wrapped = wrap_text(line, width.max(20));
        if wrapped.is_empty() {
            lines.push(String::new());
        } else {
            lines.extend(wrapped);
        }
    }
    lines.push(String::new());
    lines.push("Untrusted prompt context; the model never executes it.".to_owned());
    lines.push("↑/↓ scroll · Esc close".to_owned());
    lines
}

pub(super) fn help_lines() -> Vec<String> {
    vec![
        "Tab/Shift+Tab  laboratory/project memories".to_owned(),
        "j/k or ↑/↓     move selection".to_owned(),
        "PgUp/PgDn      scroll by one page".to_owned(),
        "g/G            first/last memory".to_owned(),
        "Enter          full memory".to_owned(),
        "n              new memory in this scope".to_owned(),
        "e              edit the selected memory".to_owned(),
        "d              delete with confirmation".to_owned(),
        "r              reload".to_owned(),
        "?              this help".to_owned(),
        "Esc            back to the floor".to_owned(),
        "q              quit".to_owned(),
        String::new(),
        "Memories are untrusted prompt context. Credentials are rejected.".to_owned(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::ids::{LabId, ProjectId};
    use crate::domain::memory::Memory;
    use crate::infra::db::Database;
    use crate::infra::paths::{Paths, PlatformDirs};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use std::cell::RefCell;
    use std::collections::HashMap;

    struct Fake {
        lab: LabId,
        project: ProjectId,
        memories: RefCell<Vec<MemoryRecord>>,
    }

    impl Fake {
        fn new() -> Self {
            Self {
                lab: LabId::parse("a3f91c07").unwrap(),
                project: ProjectId::parse("0b1c2d3e").unwrap(),
                memories: RefCell::new(Vec::new()),
            }
        }
    }

    impl MemoryStore for Fake {
        fn insert_memory(&self, memory: &Memory) -> Result<(), crate::app::storage::StorageError> {
            self.memories.borrow_mut().insert(
                0,
                MemoryRecord {
                    memory: memory.clone(),
                    created_at: "2026-01-01T00:00:00Z".to_owned(),
                },
            );
            Ok(())
        }
        fn memories(
            &self,
            scope: &MemoryScope,
        ) -> Result<Vec<MemoryRecord>, crate::app::storage::StorageError> {
            Ok(self
                .memories
                .borrow()
                .iter()
                .filter(|record| &record.memory.scope == scope)
                .cloned()
                .collect())
        }
        fn memory(
            &self,
            id: &MemoryId,
        ) -> Result<Option<MemoryRecord>, crate::app::storage::StorageError> {
            Ok(self
                .memories
                .borrow()
                .iter()
                .find(|record| &record.memory.id == id)
                .cloned())
        }
        fn memory_count(
            &self,
            scope: &MemoryScope,
        ) -> Result<u64, crate::app::storage::StorageError> {
            Ok(self
                .memories
                .borrow()
                .iter()
                .filter(|record| &record.memory.scope == scope)
                .count() as u64)
        }
        fn update_memory(&self, memory: &Memory) -> Result<(), crate::app::storage::StorageError> {
            let mut records = self.memories.borrow_mut();
            if let Some(record) = records
                .iter_mut()
                .find(|record| record.memory.id == memory.id)
            {
                record.memory.content = memory.content.clone();
                Ok(())
            } else {
                Err(crate::app::storage::StorageError::Operation)
            }
        }
        fn delete_memory(&self, id: &MemoryId) -> Result<(), crate::app::storage::StorageError> {
            self.memories
                .borrow_mut()
                .retain(|record| &record.memory.id != id);
            Ok(())
        }
    }

    impl Directory for Fake {
        fn insert_lab(
            &self,
            _: &crate::domain::lab::Lab,
        ) -> Result<(), crate::app::storage::StorageError> {
            unreachable!()
        }
        fn lab_exists(&self, id: &LabId) -> Result<bool, crate::app::storage::StorageError> {
            Ok(*id == self.lab)
        }
        fn lab(
            &self,
            id: &LabId,
        ) -> Result<Option<crate::app::directory::LabOverview>, crate::app::storage::StorageError>
        {
            Ok(
                (*id == self.lab).then(|| crate::app::directory::LabOverview {
                    id: id.clone(),
                    name: "Demo Lab".to_owned(),
                    description: String::new(),
                    agent_count: 3,
                    project_count: 1,
                }),
            )
        }
        fn labs(
            &self,
        ) -> Result<Vec<crate::app::directory::LabOverview>, crate::app::storage::StorageError>
        {
            Ok(Vec::new())
        }
        fn lab_agents(
            &self,
            _: &LabId,
        ) -> Result<Vec<crate::app::directory::AgentRecord>, crate::app::storage::StorageError>
        {
            Ok(Vec::new())
        }
        fn insert_agent(
            &self,
            _: &crate::domain::agent::AgentProfile,
            _: &[LabId],
        ) -> Result<(), crate::app::storage::StorageError> {
            unreachable!()
        }
        fn agent_exists(
            &self,
            _: &crate::domain::ids::AgentId,
        ) -> Result<bool, crate::app::storage::StorageError> {
            Ok(false)
        }
        fn agent(
            &self,
            _: &crate::domain::ids::AgentId,
        ) -> Result<Option<crate::app::directory::AgentRecord>, crate::app::storage::StorageError>
        {
            Ok(None)
        }
        fn agent_memberships(
            &self,
            _: &crate::domain::ids::AgentId,
        ) -> Result<Vec<crate::app::directory::LabOverview>, crate::app::storage::StorageError>
        {
            Ok(Vec::new())
        }
    }

    impl ProjectStore for Fake {
        fn insert_project(
            &self,
            _: &crate::domain::project::Project,
        ) -> Result<(), crate::app::storage::StorageError> {
            unreachable!()
        }
        fn project(
            &self,
            _: &ProjectId,
        ) -> Result<
            Option<(ProjectId, LabId, String, Option<String>)>,
            crate::app::storage::StorageError,
        > {
            Ok(None)
        }
        fn projects(
            &self,
            _: &LabId,
        ) -> Result<Vec<crate::app::projects::ProjectOverview>, crate::app::storage::StorageError>
        {
            Ok(Vec::new())
        }
        fn project_in_lab(
            &self,
            id: &ProjectId,
            lab: &LabId,
        ) -> Result<Option<crate::app::projects::ProjectOverview>, crate::app::storage::StorageError>
        {
            Ok((id == &self.project && lab == &self.lab).then(|| {
                crate::app::projects::ProjectOverview {
                    id: id.clone(),
                    name: "Portal".to_owned(),
                    github_source: None,
                    request_count: 0,
                    story_count: 0,
                    todo: 0,
                    in_progress: 0,
                    in_review: 0,
                    blocked: 0,
                    done: 0,
                }
            }))
        }
    }

    #[test]
    fn the_view_lists_both_scopes_and_switches_tabs() {
        let fake = Fake::new();
        fake.insert_memory(
            &Memory::new(
                MemoryId::parse("10000001").unwrap(),
                MemoryScope::Lab(fake.lab.clone()),
                "Lab note",
            )
            .unwrap(),
        )
        .unwrap();
        fake.insert_memory(
            &Memory::new(
                MemoryId::parse("20000002").unwrap(),
                MemoryScope::Project(fake.project.clone()),
                "Project note",
            )
            .unwrap(),
        )
        .unwrap();
        let mut view = MemoryView::load(&fake, &fake.lab, Some(&fake.project)).unwrap();
        assert_eq!(view.list().len(), 1);
        assert_eq!(view.selected_record().unwrap().memory.content, "Lab note");
        view.switch_tab(1);
        assert_eq!(view.list().len(), 1);
        assert_eq!(
            view.selected_record().unwrap().memory.content,
            "Project note"
        );
        assert!(view.scope().is_some());
    }

    #[test]
    fn the_view_keeps_selection_and_pages_in_bounds() {
        let fake = Fake::new();
        for index in 0..12 {
            fake.insert_memory(
                &Memory::new(
                    MemoryId::parse(&format!("{:08x}", 0x1000_0000 + index)).unwrap(),
                    MemoryScope::Lab(fake.lab.clone()),
                    &format!("Note {index}"),
                )
                .unwrap(),
            )
            .unwrap();
        }
        let mut view = MemoryView::load(&fake, &fake.lab, None).unwrap();
        view.page_size = 5;
        for _ in 0..7 {
            view.move_selection(1);
        }
        assert_eq!(view.selected[0], 7);
        assert_eq!(view.offsets[0], 3);
        view.change_page(1);
        assert_eq!(view.selected[0], 11);
        view.last();
        assert_eq!(view.selected[0], 11);
        view.first();
        assert_eq!((view.selected[0], view.offsets[0]), (0, 0));
    }

    #[test]
    fn the_editor_prefills_existing_content_and_the_list_renders() {
        let fake = Fake::new();
        fake.insert_memory(
            &Memory::new(
                MemoryId::parse("10000001").unwrap(),
                MemoryScope::Lab(fake.lab.clone()),
                "Keep tests green",
            )
            .unwrap(),
        )
        .unwrap();
        let view = MemoryView::load(&fake, &fake.lab, None).unwrap();
        let record = view.list()[0].clone();
        let editor = MemoryEditor::new(
            MemoryScope::Lab(fake.lab.clone()),
            "Laboratory Demo Lab".to_owned(),
            Some(&record),
        );
        assert_eq!(editor.field.value, "Keep tests green");
        assert!(editor.title().contains("Edit memory 10000001"));
        let theme = Theme {
            color: false,
            unicode: true,
        };
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut view = view;
        terminal
            .draw(|frame| render(frame, frame.area(), theme, &mut view, None))
            .unwrap();
        let screen: String = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        for text in [
            "Memories",
            "Lab (1)",
            "Project (0)",
            "Keep tests green",
            "[n]",
            "New",
        ] {
            assert!(screen.contains(text), "{text}: {screen}");
        }
    }

    #[test]
    fn the_editor_and_detail_render_at_the_minimum_size() {
        let theme = Theme {
            color: false,
            unicode: true,
        };
        let backend = TestBackend::new(60, 20);
        let mut terminal = Terminal::new(backend).unwrap();
        let editor = MemoryEditor::new(
            MemoryScope::Lab(LabId::parse("a3f91c07").unwrap()),
            "Laboratory Demo Lab".to_owned(),
            None,
        );
        terminal
            .draw(|frame| render_editor(frame, frame.area(), theme, &editor))
            .unwrap();
        let screen: String = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(screen.contains("New memory"), "{screen}");
        assert!(screen.contains("Enter save"), "{screen}");

        let record = MemoryRecord {
            memory: Memory::new(
                MemoryId::parse("10000001").unwrap(),
                MemoryScope::Lab(LabId::parse("a3f91c07").unwrap()),
                "Keep tests green",
            )
            .unwrap(),
            created_at: "2026-01-01T00:00:00Z".to_owned(),
        };
        let mut scroll = 0;
        terminal
            .draw(|frame| render_detail(frame, frame.area(), theme, &record, &mut scroll))
            .unwrap();
        let screen: String = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(screen.contains("Keep tests green"), "{screen}");
        assert!(screen.contains("Esc close"), "{screen}");
    }

    #[test]
    fn a_database_backed_view_loads_and_reloads() {
        let dir = tempfile::tempdir().unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
        }
        let db = Database::open(dir.path().join("vlab.db")).unwrap();
        db.migrate(crate::obs::RunId::generate()).unwrap();
        for sql in [
            "INSERT INTO labs (id, name, created_at, updated_at) VALUES ('a3f91c07', 'Demo Lab', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
            "INSERT INTO projects (id, lab_id, name, created_at, updated_at) VALUES ('0b1c2d3e', 'a3f91c07', 'Portal', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
            "INSERT INTO memories (id, scope_type, lab_id, project_id, agent_id, content, created_at) VALUES ('10000001', 'lab', 'a3f91c07', NULL, NULL, 'Lab note', '2026-01-01T00:00:00Z')",
            "INSERT INTO memories (id, scope_type, lab_id, project_id, agent_id, content, created_at) VALUES ('20000002', 'project', NULL, '0b1c2d3e', NULL, 'Project note', '2026-01-01T00:00:00Z')",
        ] {
            db.execute(sql).unwrap();
        }
        let mut view = MemoryView::load(
            &db,
            &LabId::parse("a3f91c07").unwrap(),
            Some(&ProjectId::parse("0b1c2d3e").unwrap()),
        )
        .unwrap();
        assert_eq!(view.lists[0].len(), 1);
        assert_eq!(view.lists[1].len(), 1);
        // The helper types used by the fixture remain constructible.
        let _ = (
            HashMap::<String, String>::new(),
            Paths::resolve(|_| None, Some(PlatformDirs::current().unwrap())).ok(),
        );
        view.reload(&db);
        assert_eq!(view.lists[0][0].memory.content, "Lab note");
    }
}
