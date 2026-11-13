//! Laboratory floor scene and navigation state (ADR-0045).
//!
//! This module is presentation only. It reads an `app::floor::Floor` snapshot
//! and never writes, calls the provider, or renders model content.

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::app::floor::{Floor, FloorAgent, FloorPod};
use crate::domain::ids::AgentId;

use super::{Theme, action_grid, clean};

const POD_MIN_WIDTH: usize = 22;
const POD_GAP: usize = 2;
const CONNECTOR_HEIGHT: usize = 3;
const LEAD_HEIGHT: usize = 3;

const FLOOR_ACTIONS: [(&str, &str); 8] = [
    ("[↑/↓]", "Agent"),
    ("[←/→]", "Pods"),
    ("[Enter]", "Detail"),
    ("[m]", "Memories"),
    ("[r]", "Reload"),
    ("[?]", "Help"),
    ("[Esc]", "Back"),
    ("[q]", "Quit"),
];

/// Navigation state for the floor. Moving never writes.
#[derive(Debug, Clone)]
pub(super) struct FloorView {
    pub(super) floor: Floor,
    pub(super) selected: usize,
    pub(super) pod_page: usize,
    /// Pods fully visible in the last render; paging uses it.
    pub(super) visible_pods: usize,
}

impl FloorView {
    pub(super) fn new(floor: Floor) -> Self {
        let mut view = Self {
            floor,
            selected: 0,
            pod_page: 0,
            visible_pods: 1,
        };
        view.clamp();
        view
    }

    pub(super) fn selected_agent(&self) -> Option<&FloorAgent> {
        self.floor.selected(self.selected)
    }

    pub(super) fn move_selection(&mut self, delta: isize) {
        let len = self.floor.agents.len();
        if len == 0 {
            self.selected = 0;
            return;
        }
        if delta < 0 {
            self.selected = self.selected.saturating_sub(1);
        } else {
            self.selected = (self.selected + 1).min(len - 1);
        }
        self.follow_selected();
    }

    pub(super) fn first_agent(&mut self) {
        self.selected = 0;
        self.follow_selected();
    }

    pub(super) fn last_agent(&mut self) {
        self.selected = self.floor.agents.len().saturating_sub(1);
        self.follow_selected();
    }

    pub(super) fn page_count(&self) -> usize {
        if self.floor.pods.is_empty() {
            return 0;
        }
        let per = self.visible_pods.max(1);
        self.floor.pods.len().saturating_sub(1) / per + 1
    }

    pub(super) fn change_page(&mut self, delta: isize) {
        let pages = self.page_count();
        if pages == 0 {
            return;
        }
        let next = if delta < 0 {
            self.pod_page.saturating_sub(1)
        } else {
            (self.pod_page + 1).min(pages - 1)
        };
        if next == self.pod_page {
            return;
        }
        self.pod_page = next;
        self.select_first_in_page();
    }

    fn select_first_in_page(&mut self) {
        let per = self.visible_pods.max(1);
        let start = self.pod_page * per;
        let end = (start + per).min(self.floor.pods.len());
        let ids: Vec<&AgentId> = self.floor.pods[start..end]
            .iter()
            .flat_map(|pod| pod.members.iter())
            .collect();
        if let Some(index) = self
            .floor
            .agents
            .iter()
            .position(|agent| ids.contains(&&agent.id))
        {
            self.selected = index;
        }
    }

    /// Keeps the selected agent's pod on the visible page.
    fn follow_selected(&mut self) {
        let Some(agent) = self.selected_agent() else {
            return;
        };
        if self.floor.lead.as_ref() == Some(&agent.id) {
            return;
        }
        let per = self.visible_pods.max(1);
        if let Some(index) = self
            .floor
            .pods
            .iter()
            .position(|pod| pod.members.contains(&agent.id))
        {
            self.pod_page = index / per;
        }
    }

    /// Keeps every remembered position valid after a reload and preserves the
    /// selected agent by identifier.
    pub(super) fn replace(&mut self, floor: Floor) {
        let selected_id = self.selected_agent().map(|agent| agent.id.clone());
        self.floor = floor;
        self.selected = selected_id
            .and_then(|id| self.floor.agents.iter().position(|agent| agent.id == id))
            .unwrap_or(0);
        self.pod_page = 0;
        self.follow_selected();
        self.clamp();
    }

    fn clamp(&mut self) {
        let agents = self.floor.agents.len();
        self.selected = self.selected.min(agents.saturating_sub(1));
        self.pod_page = self.pod_page.min(self.page_count().saturating_sub(1));
    }
}

/// Updates `view.visible_pods` while rendering.
pub(super) fn render(
    frame: &mut Frame<'_>,
    area: Rect,
    theme: Theme,
    view: &mut FloorView,
    status: Option<&str>,
) {
    let footer_width = usize::from(area.width.saturating_sub(2));
    let mut footer = action_grid(&FLOOR_ACTIONS, theme, footer_width);
    footer.push(status_line(theme, view, status, footer_width));
    footer.push(legend_line(theme, footer_width));
    let footer_height = u16::try_from(footer.len() + 2).unwrap_or(6).max(4);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(1),
            Constraint::Length(footer_height),
        ])
        .split(area);

    let scene_inner = usize::from(chunks[1].width.saturating_sub(2));
    view.visible_pods = pod_columns(scene_inner).0;
    let pages = view.page_count();
    view.pod_page = if pages == 0 {
        0
    } else {
        view.pod_page.min(pages - 1)
    };
    let context_width = usize::from(area.width.saturating_sub(21));
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("Laboratory floor", theme.title()),
            Span::raw("  "),
            Span::styled(
                truncate_width(&header_context(view), context_width),
                theme.muted_style(),
            ),
        ]))
        .block(super::block(theme, "Laboratory floor")),
        chunks[0],
    );
    render_scene(frame, chunks[1], theme, view);
    frame.render_widget(
        Paragraph::new(footer).block(super::block(theme, "Actions")),
        chunks[2],
    );
}

fn header_context(view: &FloorView) -> String {
    let archived = if view.floor.archived == 0 {
        String::new()
    } else {
        format!(" · {} archived hidden", view.floor.archived)
    };
    let pages = view.page_count();
    let page = if pages > 1 {
        format!(" · pods {}/{}", view.pod_page + 1, pages)
    } else {
        String::new()
    };
    format!(
        "{} · {} agents · {} projects{} · read-only{}",
        clean(&view.floor.lab.name),
        view.floor.agents.len(),
        view.floor.project_pods().count(),
        archived,
        page
    )
}

fn status_line(
    theme: Theme,
    view: &FloorView,
    status: Option<&str>,
    width: usize,
) -> Line<'static> {
    if let Some(status) = status {
        return Line::from(vec![
            Span::styled("Status: ", theme.key()),
            Span::styled(
                truncate_width(&clean(status), width.saturating_sub(8)),
                theme.success_style(),
            ),
        ]);
    }
    let Some(agent) = view.selected_agent() else {
        return Line::from(Span::styled(
            truncate_width("No active agents in this laboratory.", width),
            theme.muted_style(),
        ));
    };
    let state = if agent.is_idle() {
        "idle (zzz)"
    } else if agent.is_focused() {
        "focus desk"
    } else {
        "in the huddle"
    };
    let skills = if agent.skills.is_empty() {
        "none".to_owned()
    } else {
        agent.skills.join(", ")
    };
    let details = format!(
        " · {} · {} project(s) · {} open · {state} · skills: {}",
        clean(&agent.role),
        agent.projects.len(),
        agent.open_stories,
        clean(&skills)
    );
    Line::from(vec![
        Span::styled("Selected: ", theme.key()),
        Span::styled(
            truncate_width(&clean(&agent.name), width.saturating_sub(10)),
            Style::default().add_modifier(Modifier::BOLD),
        ),
        Span::raw(truncate_width(&details, width.saturating_sub(12))),
    ])
}

fn legend_line(theme: Theme, width: usize) -> Line<'static> {
    let legend = if theme.unicode {
        "(◕‿◕) agent · ★ Tech Lead · (zzz) idle · ( ? ) exchange · (///) focus · read-only"
    } else {
        "(o_o) agent · * Tech Lead · (zzz) idle · ( ? ) exchange · (///) focus · read-only"
    };
    Line::from(Span::styled(
        truncate_width(legend, width),
        theme.muted_style(),
    ))
}

fn render_scene(frame: &mut Frame<'_>, area: Rect, theme: Theme, view: &mut FloorView) {
    let mut canvas = Canvas::new(usize::from(area.width), usize::from(area.height));
    let (frame_tl, frame_tr, frame_bl, frame_br, frame_h, frame_v) = if theme.unicode {
        ('╔', '╗', '╚', '╝', '═', '║')
    } else {
        ('+', '+', '+', '+', '-', '|')
    };
    if canvas.width < 4 || canvas.height < 4 {
        frame.render_widget(Paragraph::new("Terminal too small."), area);
        return;
    }
    canvas.frame(frame_tl, frame_tr, frame_bl, frame_br, frame_h, frame_v);
    let lab = clean(&view.floor.lab.name);
    canvas.title(1, 0, &format!("LAB FLOOR · {lab}"), theme.title());

    let inner_w = canvas.width.saturating_sub(2);
    let inner_h = canvas.height.saturating_sub(2);
    if view.floor.agents.is_empty() {
        canvas.centered(
            1,
            1,
            inner_w,
            inner_h,
            "No active agents in this laboratory.",
            theme.muted_style(),
        );
        view.visible_pods = 0;
        finish(frame, area, canvas);
        return;
    }

    let pod_count = view.floor.pods.len();
    let (columns, pod_width) = pod_columns(inner_w);
    let pages = pod_count.saturating_sub(1) / columns.max(1) + 1;
    view.visible_pods = columns;
    view.pod_page = view.pod_page.min(pages.saturating_sub(1));
    let start = view.pod_page * columns;
    let visible = pod_count.saturating_sub(start).min(columns);
    if visible == 0 {
        view.visible_pods = 0;
        canvas.centered(
            1,
            1,
            inner_w,
            3,
            "No project pods yet.",
            theme.muted_style(),
        );
        draw_lead(&mut canvas, inner_w, inner_h, 1, 0, view, theme);
        finish(frame, area, canvas);
        return;
    }

    let lead_room = inner_h >= 9;
    let reserved = if lead_room {
        LEAD_HEIGHT + CONNECTOR_HEIGHT
    } else {
        0
    };
    let pod_area_height = inner_h.saturating_sub(reserved).max(3);
    let member_capacity = pod_area_height.saturating_sub(2).max(1);
    let plans: Vec<PodPlan> = view.floor.pods[start..start + visible]
        .iter()
        .map(|pod| PodPlan::new(&view.floor, pod, member_capacity, theme))
        .collect();
    let content_height = plans
        .iter()
        .map(|plan| plan.rows_height())
        .max()
        .unwrap_or(1)
        .min(member_capacity)
        .max(1);
    let pod_height = content_height + 2;
    let total_width = visible * pod_width + (visible.saturating_sub(1)) * POD_GAP;
    let start_x = 1 + inner_w.saturating_sub(total_width) / 2;
    let content_height = pod_height
        + if lead_room {
            CONNECTOR_HEIGHT + LEAD_HEIGHT
        } else {
            0
        };
    let pods_y = 1 + inner_h.saturating_sub(content_height) / 2;

    let mut centers = Vec::with_capacity(visible);
    for (index, pod) in view.floor.pods[start..start + visible].iter().enumerate() {
        let x = start_x + index * (pod_width + POD_GAP);
        centers.push(x + pod_width / 2);
        plans[index].draw(
            &mut canvas,
            &view.floor,
            x,
            pods_y,
            pod_width,
            view.selected,
            theme,
        );
        let title = if pod.whole_lab {
            format!("{} · whole lab", clean(&pod.name))
        } else {
            clean(&pod.name)
        };
        canvas.box_frame(
            x,
            pods_y,
            pod_width,
            pod_height,
            &title,
            theme.border_style(),
            theme.title(),
            theme.unicode,
        );
    }

    let lead_box = if lead_room {
        let lead_center = draw_lead(
            &mut canvas,
            inner_w,
            inner_h,
            pods_y,
            pod_height,
            view,
            theme,
        );
        Some(lead_center)
    } else {
        None
    };
    if let Some(lead_center) = lead_box {
        draw_connectors(
            &mut canvas,
            &centers,
            lead_center,
            pods_y + pod_height,
            theme,
        );
    } else {
        draw_compact_lead(
            &mut canvas,
            inner_w,
            inner_h,
            pods_y,
            pod_height,
            view,
            theme,
        );
    }
    finish(frame, area, canvas);
}

fn draw_compact_lead(
    canvas: &mut Canvas,
    inner_w: usize,
    inner_h: usize,
    pods_y: usize,
    pod_height: usize,
    view: &FloorView,
    theme: Theme,
) {
    let y = pods_y + pod_height + 1;
    if y > inner_h {
        return;
    }
    let lead = view.floor.lead.as_ref().and_then(|id| view.floor.agent(id));
    let text = match lead {
        Some(agent) => format!(
            "TECH LEAD: {} {}",
            clean(&agent.name),
            status_bubble(agent, theme)
        ),
        None => "TECH LEAD: missing".to_owned(),
    };
    let text = truncate_width(&text, inner_w);
    let x = 1 + inner_w.saturating_sub(text.width()) / 2;
    if lead.is_some_and(|agent| view.selected_agent().is_some_and(|sel| sel.id == agent.id)) {
        for column in 0..text.width().min(inner_w) {
            canvas.set(x + column, y, ' ', theme.selected());
        }
    }
    paint_text(canvas, x, y, inner_w, &text, theme.warning_style());
}

fn finish(frame: &mut Frame<'_>, area: Rect, canvas: Canvas) {
    frame.render_widget(Paragraph::new(canvas.into_lines()), area);
}

fn pod_columns(width: usize) -> (usize, usize) {
    let mut columns = ((width + POD_GAP) / (POD_MIN_WIDTH + POD_GAP)).clamp(1, 4);
    loop {
        let pod_width = width.saturating_sub((columns - 1) * POD_GAP) / columns.max(1);
        if columns == 1 || pod_width >= 16 {
            return (columns, pod_width.max(1));
        }
        columns -= 1;
    }
}

fn draw_lead(
    canvas: &mut Canvas,
    inner_w: usize,
    inner_h: usize,
    pods_y: usize,
    pod_height: usize,
    view: &FloorView,
    theme: Theme,
) -> usize {
    let y = pods_y + pod_height + CONNECTOR_HEIGHT;
    if y + LEAD_HEIGHT > 1 + inner_h {
        return inner_w / 2 + 1;
    }
    let lead = view.floor.lead.as_ref().and_then(|id| view.floor.agent(id));
    let name = lead
        .map(|agent| clean(&agent.name))
        .unwrap_or_else(|| "no Tech Lead".to_owned());
    let bubble = lead.map(|agent| status_bubble(agent, theme)).unwrap_or("");
    let width = (name.width() + bubble.width() + 22).max(26).min(inner_w);
    let x = 1 + inner_w.saturating_sub(width) / 2;
    canvas.box_frame(
        x,
        y,
        width,
        LEAD_HEIGHT,
        "TECH LEAD",
        if lead.is_some() {
            theme.border_style()
        } else {
            theme.warning_style()
        },
        theme.warning_style(),
        theme.unicode,
    );
    if let Some(agent) = lead {
        paint_agent_row(
            canvas,
            x,
            y + 1,
            width,
            &view.floor,
            agent,
            bubble,
            view.selected_agent()
                .is_some_and(|selected| selected.id == agent.id),
            theme,
        );
        if bubble.is_empty() && !agent.projects.is_empty() {
            paint_text(
                canvas,
                x + 2,
                y + 1,
                width.saturating_sub(4),
                &format!("coordinating {} project(s)", agent.projects.len()),
                theme.muted_style(),
            );
        }
    } else {
        paint_text(
            canvas,
            x + 2,
            y + 1,
            width.saturating_sub(4),
            "Run check-lab and add a lead.",
            theme.warning_style(),
        );
    }
    x + width / 2
}

fn draw_connectors(
    canvas: &mut Canvas,
    centers: &[usize],
    lead_center: usize,
    start_y: usize,
    theme: Theme,
) {
    if centers.is_empty() || start_y + CONNECTOR_HEIGHT > canvas.height {
        return;
    }
    let (v, h, up, down, cross) = if theme.unicode {
        ('│', '─', '┴', '┬', '┼')
    } else {
        ('|', '-', '+', '+', '+')
    };
    for center in centers {
        canvas.set(*center, start_y, v, theme.border_style());
    }
    let min = centers
        .iter()
        .copied()
        .chain(std::iter::once(lead_center))
        .min()
        .unwrap_or(0);
    let max = centers
        .iter()
        .copied()
        .chain(std::iter::once(lead_center))
        .max()
        .unwrap_or(0);
    if min == max {
        for row in 0..CONNECTOR_HEIGHT {
            canvas.set(min, start_y + row, v, theme.border_style());
        }
        return;
    }
    for x in min..=max {
        canvas.set(x, start_y + 1, h, theme.border_style());
    }
    for center in centers {
        let symbol = if *center == lead_center { cross } else { up };
        canvas.set(*center, start_y + 1, symbol, theme.border_style());
    }
    canvas.set(
        lead_center,
        start_y + 1,
        if centers.contains(&lead_center) {
            cross
        } else {
            down
        },
        theme.border_style(),
    );
    for row in 2..CONNECTOR_HEIGHT {
        canvas.set(lead_center, start_y + row, v, theme.border_style());
    }
}

fn status_bubble(agent: &FloorAgent, _theme: Theme) -> &'static str {
    if agent.is_idle() {
        "(zzz)"
    } else if agent.is_focused() {
        "(///)"
    } else {
        "( ... )"
    }
}

fn sprite<'a>(agent: &'a FloorAgent, floor: &'a Floor, theme: Theme) -> &'a str {
    let lead = floor.lead.as_ref() == Some(&agent.id);
    match (theme.unicode, lead) {
        (true, true) => "★(◕‿◕)",
        (true, false) => "(◕‿◕)",
        (false, true) => "*(o_o)",
        (false, false) => "(o_o)",
    }
}

#[allow(clippy::too_many_arguments)]
fn paint_agent_row(
    canvas: &mut Canvas,
    x: usize,
    y: usize,
    width: usize,
    floor: &Floor,
    agent: &FloorAgent,
    bubble: &str,
    selected: bool,
    theme: Theme,
) {
    let base = if selected {
        theme.selected()
    } else {
        Style::default()
    };
    let prefix = if selected {
        if theme.unicode { "▶ " } else { "> " }
    } else {
        "  "
    };
    let sprite = sprite(agent, floor, theme);
    let inner_x = x + 1;
    let inner_w = width.saturating_sub(2);
    for column in 0..inner_w {
        canvas.set(inner_x + column, y, ' ', base);
    }
    paint_text(canvas, inner_x, y, inner_w, prefix, base);
    paint_text(
        canvas,
        inner_x + prefix.width(),
        y,
        inner_w.saturating_sub(prefix.width()),
        sprite,
        if selected {
            base
        } else if floor.lead.as_ref() == Some(&agent.id) {
            theme.warning_style()
        } else {
            Style::default()
        },
    );
    let name_x = inner_x + prefix.width() + sprite.width() + 1;
    let bubble_w = bubble.width();
    let bubble_x = (inner_x + inner_w).saturating_sub(bubble_w + 1);
    let available = bubble_x.saturating_sub(name_x);
    let name_width = if available > 3 {
        available - 1
    } else {
        available
    };
    paint_text(
        canvas,
        name_x,
        y,
        name_width,
        &clean(&agent.name),
        if selected { base } else { Style::default() },
    );
    if bubble_w > 0 && bubble_x >= inner_x {
        let bubble_style = if selected {
            base
        } else if bubble == "(zzz)" {
            theme.muted_style()
        } else if bubble.contains("///") {
            theme.warning_style()
        } else {
            theme.title()
        };
        paint_text(canvas, bubble_x, y, bubble_w, bubble, bubble_style);
    }
}

fn paint_text(canvas: &mut Canvas, x: usize, y: usize, max_width: usize, text: &str, style: Style) {
    let mut used = 0;
    if y >= canvas.height {
        return;
    }
    for ch in text.chars() {
        let width = ch.width().unwrap_or(1);
        if used + width > max_width {
            if max_width > used {
                canvas.set(x + used, y, '…', style);
            }
            return;
        }
        canvas.set(x + used, y, ch, style);
        used += width;
    }
}

#[derive(Debug, Clone)]
enum RowPlan {
    Pair { first: AgentId, second: AgentId },
    Agent(AgentId),
    Focus(Vec<AgentId>),
    More(usize),
}

impl RowPlan {
    fn height(&self) -> usize {
        match self {
            Self::Pair { .. } => 3,
            Self::Agent(_) | Self::More(_) => 1,
            Self::Focus(agents) => agents.len() + 2,
        }
    }
}

struct PodPlan {
    rows: Vec<RowPlan>,
    empty: bool,
}

impl PodPlan {
    fn new(floor: &Floor, pod: &FloorPod, capacity: usize, _theme: Theme) -> Self {
        let lead = floor.lead.as_ref();
        let pairs = floor.exchange_pairs(pod);
        let paired: Vec<&AgentId> = pairs
            .iter()
            .flat_map(|pair| [&pair.first, &pair.second])
            .collect();
        let singles: Vec<AgentId> = pod
            .members
            .iter()
            .filter(|id| Some(*id) != lead)
            .filter(|id| !paired.contains(id))
            .filter_map(|id| floor.agent(id))
            .filter(|agent| !agent.is_focused())
            .map(|agent| agent.id.clone())
            .collect();
        let focused: Vec<AgentId> = pod
            .members
            .iter()
            .filter(|id| Some(*id) != lead)
            .filter_map(|id| floor.agent(id))
            .filter(|agent| agent.is_focused())
            .map(|agent| agent.id.clone())
            .collect();

        let mut rows = Vec::new();
        let mut used = 0;
        let mut hidden = 0;
        for pair in pairs {
            if used + 3 <= capacity {
                rows.push(RowPlan::Pair {
                    first: pair.first,
                    second: pair.second,
                });
                used += 3;
            } else {
                hidden += 2;
            }
        }
        for id in singles {
            if used < capacity {
                rows.push(RowPlan::Agent(id));
                used += 1;
            } else {
                hidden += 1;
            }
        }
        if !focused.is_empty() {
            let available = capacity.saturating_sub(used);
            if available >= 3 {
                let fit = (available - 2).min(focused.len());
                rows.push(RowPlan::Focus(focused[..fit].to_vec()));
                used += 2 + fit;
                hidden += focused.len() - fit;
            } else {
                hidden += focused.len();
            }
        }
        if hidden > 0 {
            if used >= capacity {
                match rows.pop() {
                    Some(RowPlan::Agent(_)) => {
                        hidden += 1;
                    }
                    Some(RowPlan::Pair { .. }) => {
                        hidden += 2;
                    }
                    Some(RowPlan::Focus(agents)) => {
                        hidden += agents.len();
                    }
                    Some(RowPlan::More(_)) | None => {}
                }
            }
            rows.push(RowPlan::More(hidden));
        }
        let empty = rows.is_empty();
        if empty {
            rows.push(RowPlan::More(0));
        }
        Self { rows, empty }
    }

    fn rows_height(&self) -> usize {
        self.rows.iter().map(RowPlan::height).sum()
    }

    #[allow(clippy::too_many_arguments)]
    fn draw(
        &self,
        canvas: &mut Canvas,
        floor: &Floor,
        x: usize,
        y: usize,
        width: usize,
        selected: usize,
        theme: Theme,
    ) {
        let inner_x = x + 1;
        let inner_w = width.saturating_sub(2);
        let mut row_y = y + 1;
        let mut pair_index = 0;
        let mut focus_index = 0;
        for row in &self.rows {
            match row {
                RowPlan::Pair { first, second } => {
                    let (first_bubble, second_bubble) = if pair_index % 2 == 0 {
                        ("( ? )", "( ! )")
                    } else {
                        ("( ! )", "( ? )")
                    };
                    if let Some(agent) = floor.agent(first) {
                        paint_agent_row(
                            canvas,
                            x,
                            row_y,
                            width,
                            floor,
                            agent,
                            first_bubble,
                            selected == selected_index(floor, &agent.id),
                            theme,
                        );
                    }
                    if theme.unicode {
                        canvas.set(inner_x + 2, row_y + 1, '⇅', theme.muted_style());
                    } else {
                        canvas.set(inner_x + 2, row_y + 1, ':', theme.muted_style());
                    }
                    if let Some(agent) = floor.agent(second) {
                        paint_agent_row(
                            canvas,
                            x,
                            row_y + 2,
                            width,
                            floor,
                            agent,
                            second_bubble,
                            selected == selected_index(floor, &agent.id),
                            theme,
                        );
                    }
                    row_y += 3;
                    pair_index += 1;
                }
                RowPlan::Agent(id) => {
                    if let Some(agent) = floor.agent(id) {
                        paint_agent_row(
                            canvas,
                            x,
                            row_y,
                            width,
                            floor,
                            agent,
                            status_bubble(agent, theme),
                            selected == selected_index(floor, &agent.id),
                            theme,
                        );
                    }
                    row_y += 1;
                }
                RowPlan::Focus(agents) => {
                    let box_height = agents.len() + 2;
                    let (focus_x, focus_w) = if inner_w >= 16 {
                        (inner_x + 1, inner_w - 2)
                    } else {
                        (inner_x, inner_w)
                    };
                    canvas.box_frame(
                        focus_x,
                        row_y,
                        focus_w,
                        box_height,
                        "focus desk",
                        theme.border_style(),
                        theme.warning_style(),
                        theme.unicode,
                    );
                    for (index, id) in agents.iter().enumerate() {
                        if let Some(agent) = floor.agent(id) {
                            let bubble = if (focus_index + index) % 2 == 0 {
                                "(///)"
                            } else {
                                "(</>)"
                            };
                            paint_agent_row(
                                canvas,
                                focus_x,
                                row_y + 1 + index,
                                focus_w,
                                floor,
                                agent,
                                bubble,
                                selected == selected_index(floor, &agent.id),
                                theme,
                            );
                        }
                    }
                    focus_index += agents.len();
                    row_y += box_height;
                }
                RowPlan::More(hidden) => {
                    if *hidden > 0 {
                        paint_text(
                            canvas,
                            inner_x,
                            row_y,
                            inner_w,
                            &format!("+{hidden} more"),
                            theme.muted_style(),
                        );
                    } else if self.empty {
                        paint_text(
                            canvas,
                            inner_x,
                            row_y,
                            inner_w,
                            "(lead at desk)",
                            theme.muted_style(),
                        );
                    }
                    row_y += 1;
                }
            }
        }
    }
}

fn selected_index(floor: &Floor, id: &AgentId) -> usize {
    floor
        .agents
        .iter()
        .position(|agent| &agent.id == id)
        .unwrap_or(usize::MAX)
}

// ---------------------------------------------------------------------------
// Character canvas
// ---------------------------------------------------------------------------

#[derive(Clone, Copy)]
struct Cell {
    symbol: char,
    continuation: bool,
    style: Style,
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            symbol: ' ',
            continuation: false,
            style: Style::default(),
        }
    }
}

struct Canvas {
    width: usize,
    height: usize,
    cells: Vec<Cell>,
}

impl Canvas {
    fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            cells: vec![Cell::default(); width * height],
        }
    }

    fn set(&mut self, x: usize, y: usize, symbol: char, style: Style) {
        if x >= self.width || y >= self.height {
            return;
        }
        let width = symbol.width().unwrap_or(1);
        if width == 0 {
            return;
        }
        if width > 1 && x + 1 < self.width {
            self.cells[y * self.width + x + 1] = Cell {
                symbol: ' ',
                continuation: true,
                style,
            };
        }
        self.cells[y * self.width + x] = Cell {
            symbol,
            continuation: false,
            style,
        };
    }

    fn frame(&mut self, tl: char, tr: char, bl: char, br: char, horizontal: char, vertical: char) {
        if self.width < 2 || self.height < 2 {
            return;
        }
        let style = Style::default();
        self.set(0, 0, tl, style);
        self.set(self.width - 1, 0, tr, style);
        self.set(0, self.height - 1, bl, style);
        self.set(self.width - 1, self.height - 1, br, style);
        for x in 1..self.width - 1 {
            self.set(x, 0, horizontal, style);
            self.set(x, self.height - 1, horizontal, style);
        }
        for y in 1..self.height - 1 {
            self.set(0, y, vertical, style);
            self.set(self.width - 1, y, vertical, style);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn box_frame(
        &mut self,
        x: usize,
        y: usize,
        width: usize,
        height: usize,
        title: &str,
        border_style: Style,
        title_style: Style,
        unicode: bool,
    ) {
        if width < 2 || height < 2 || x + width > self.width || y + height > self.height {
            return;
        }
        let (tl, tr, bl, br, h, v) = if unicode {
            ('┌', '┐', '└', '┘', '─', '│')
        } else {
            ('+', '+', '+', '+', '-', '|')
        };
        self.set(x, y, tl, border_style);
        self.set(x + width - 1, y, tr, border_style);
        self.set(x, y + height - 1, bl, border_style);
        self.set(x + width - 1, y + height - 1, br, border_style);
        for column in 1..width - 1 {
            self.set(x + column, y, h, border_style);
            self.set(x + column, y + height - 1, h, border_style);
        }
        for row in 1..height - 1 {
            self.set(x, y + row, v, border_style);
            self.set(x + width - 1, y + row, v, border_style);
        }
        if width >= 6 && !title.is_empty() {
            let label = format!(" {} ", truncate_width(title, width.saturating_sub(4)));
            paint_text(self, x + 2, y, width.saturating_sub(4), &label, title_style);
        }
    }

    fn title(&mut self, x: usize, y: usize, title: &str, style: Style) {
        let width = self.width.saturating_sub(x + 1);
        paint_text(self, x, y, width, title, style);
    }

    fn centered(
        &mut self,
        x: usize,
        y: usize,
        width: usize,
        height: usize,
        text: &str,
        style: Style,
    ) {
        let text_width = truncate_width(text, width).width();
        let text_x = x + width.saturating_sub(text_width) / 2;
        let text_y = y + height / 2;
        paint_text(self, text_x, text_y, width, text, style);
    }

    fn into_lines(self) -> Vec<Line<'static>> {
        let mut lines = Vec::with_capacity(self.height);
        for row in 0..self.height {
            let mut spans: Vec<Span<'static>> = Vec::new();
            let mut current = String::new();
            let mut current_style = Style::default();
            for column in 0..self.width {
                let cell = self.cells[row * self.width + column];
                if cell.continuation {
                    continue;
                }
                if cell.style != current_style && !current.is_empty() {
                    spans.push(Span::styled(std::mem::take(&mut current), current_style));
                }
                current_style = cell.style;
                current.push(cell.symbol);
            }
            if !current.is_empty() {
                spans.push(Span::styled(current, current_style));
            }
            lines.push(Line::from(spans));
        }
        lines
    }
}

fn truncate_width(value: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    if value.width() <= width {
        return value.to_owned();
    }
    let mut out = String::new();
    let mut used = 0;
    for ch in value.chars() {
        let w = ch.width().unwrap_or(1);
        if used + w > width.saturating_sub(1) {
            break;
        }
        out.push(ch);
        used += w;
    }
    out.push('…');
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::directory::LabOverview;
    use crate::app::floor::{Floor, FloorAgent, FloorPod};
    use crate::domain::agent::Role;
    use crate::domain::ids::{LabId, ProjectId};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn agent_id(value: &str) -> AgentId {
        AgentId::parse(value).unwrap()
    }

    fn agent(id: &str, name: &str, role: &str, open: u64, projects: &[&str]) -> FloorAgent {
        FloorAgent {
            id: agent_id(id),
            name: name.to_owned(),
            role: role.to_owned(),
            skills: vec!["rust".to_owned()],
            open_stories: open,
            projects: projects
                .iter()
                .map(|value| ProjectId::parse(value).unwrap())
                .collect(),
        }
    }

    fn floor() -> Floor {
        let portal = ProjectId::parse("20000001").unwrap();
        let mobile = ProjectId::parse("20000002").unwrap();
        Floor {
            lab: LabOverview {
                id: LabId::parse("a3f91c07").unwrap(),
                name: "Demo Lab".to_owned(),
                description: String::new(),
                agent_count: 4,
                project_count: 2,
            },
            lead: Some(agent_id("10000001")),
            agents: vec![
                agent("10000001", "Ada", "lead", 0, &["20000001"]),
                agent("10000002", "Bea", "dev", 1, &["20000001"]),
                agent("10000003", "Cid", "qa", 1, &["20000001"]),
                agent("10000004", "Dee", "analyst", 2, &["20000002"]),
            ],
            pods: vec![
                FloorPod {
                    project_id: Some(portal),
                    name: "Portal".to_owned(),
                    members: vec![
                        agent_id("10000001"),
                        agent_id("10000002"),
                        agent_id("10000003"),
                    ],
                    whole_lab: false,
                },
                FloorPod {
                    project_id: Some(mobile),
                    name: "Mobile".to_owned(),
                    members: vec![agent_id("10000004")],
                    whole_lab: false,
                },
            ],
            archived: 0,
        }
    }

    fn scene(view: &mut FloorView, width: u16, height: u16, theme: Theme) -> String {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| render(frame, frame.area(), theme, view, None))
            .unwrap();
        terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect()
    }

    fn theme(unicode: bool) -> Theme {
        Theme {
            color: false,
            unicode,
        }
    }

    #[test]
    fn the_scene_shows_pods_the_lead_and_the_idle_bubble() {
        let mut view = FloorView::new(floor());
        let screen = scene(&mut view, 100, 30, theme(true));
        for text in [
            "LAB FLOOR",
            "Demo Lab",
            "Portal",
            "Mobile",
            "TECH LEAD",
            "Ada",
            "(zzz)",
            "( ? )",
            "( ! )",
            "focus desk",
            "(///)",
            "Selected:",
            "[Enter]",
            "read-only",
        ] {
            assert!(screen.contains(text), "{text}: {screen}");
        }
        assert!(!screen.contains("10000001"), "{screen}");
    }

    #[test]
    fn the_scene_works_at_the_minimum_size() {
        let mut view = FloorView::new(floor());
        let screen = scene(&mut view, 60, 20, theme(true));
        assert!(screen.contains("LAB FLOOR"), "{screen}");
        assert!(screen.contains("Portal"), "{screen}");
        assert!(screen.contains("TECH LEAD"), "{screen}");
        assert!(screen.contains("(zzz)"), "{screen}");
    }

    #[test]
    fn ascii_fallback_uses_plain_sprite_and_borders() {
        let mut view = FloorView::new(floor());
        let screen = scene(&mut view, 80, 24, theme(false));
        assert!(screen.contains("(o_o)"), "{screen}");
        assert!(screen.contains("TECH LEAD"), "{screen}");
        assert!(!screen.contains("(◕‿◕)"), "{screen}");
        assert!(!screen.contains('╔'), "{screen}");
    }

    #[test]
    fn selection_moves_and_follows_the_agent_pod() {
        let mut view = FloorView::new(floor());
        scene(&mut view, 80, 24, theme(true));
        // Force one pod per page so the second pod is a different page.
        view.visible_pods = 1;
        view.move_selection(1);
        assert_eq!(view.selected_agent().unwrap().name, "Bea");
        assert_eq!(view.pod_page, 0);
        view.move_selection(1);
        assert_eq!(view.selected_agent().unwrap().name, "Cid");
        view.move_selection(1);
        assert_eq!(view.selected_agent().unwrap().name, "Dee");
        assert_eq!(view.pod_page, 1);
        view.first_agent();
        assert_eq!(view.selected_agent().unwrap().name, "Ada");
        // The lead desk is visible on every page, so its page is unchanged.
        assert_eq!(view.pod_page, 1);
    }

    #[test]
    fn pages_are_clamped_and_select_an_agent_in_the_page() {
        let mut view = FloorView::new(floor());
        scene(&mut view, 60, 20, theme(true));
        let per = view.visible_pods;
        assert!(per >= 1);
        if per < view.floor.pods.len() {
            view.change_page(1);
            assert_eq!(view.pod_page, 1);
            assert!(view.selected_agent().is_some());
            view.change_page(1);
            assert_eq!(view.pod_page, view.page_count() - 1);
        }
    }

    #[test]
    fn replace_keeps_the_selected_agent_by_id() {
        let mut view = FloorView::new(floor());
        view.move_selection(2);
        let id = view.selected_agent().unwrap().id.clone();
        view.replace(floor());
        assert_eq!(view.selected_agent().unwrap().id, id);
    }

    #[test]
    fn many_members_show_a_more_row() {
        let mut many = floor();
        let pod = &mut many.pods[0];
        for index in 0..20 {
            pod.members
                .push(agent_id(&format!("{:08x}", 0x3000_0000 + index)));
            many.agents.push(agent(
                &format!("{:08x}", 0x3000_0000 + index),
                &format!("Agent {index}"),
                "dev",
                1,
                &["20000001"],
            ));
        }
        let mut view = FloorView::new(many);
        let screen = scene(&mut view, 100, 30, theme(true));
        assert!(screen.contains("more"), "{screen}");
    }

    #[test]
    fn a_lead_only_laboratory_shows_an_empty_state_and_the_desk() {
        let mut model = floor();
        model.agents.truncate(1);
        model.pods.clear();
        let mut view = FloorView::new(model);
        let screen = scene(&mut view, 80, 24, theme(true));
        assert!(screen.contains("No project pods yet."), "{screen}");
        assert!(screen.contains("TECH LEAD"), "{screen}");
        assert!(screen.contains("Ada"), "{screen}");
    }

    #[test]
    fn role_parsing_matches_the_exchange_rule() {
        for role in ["lead", "dev", "analyst", "qa"] {
            assert!(Role::parse(role).is_ok());
        }
    }
}
