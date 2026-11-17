//! Fixed three-room MSX laboratory floor (ADR-0062).
//!
//! This module is presentation only. It reads an `app::floor::Floor` snapshot
//! and never writes, calls the provider, or renders model content. The tick is
//! deterministic and never touches wall-clock time.

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::app::directory::AgentTask;
use crate::app::floor::{Floor, FloorAgent, FloorRoom};
use crate::app::kanban::StoryState;

use super::{Theme, action_grid, clean};

const LEG_TICKS: u64 = 18;
const WALK_TICKS: u64 = 6;
const MAX_TOUR: usize = 4;

const FLOOR_ACTIONS: [(&str, &str); 11] = [
    ("[↑/↓]", "Agent"),
    ("[←/→]", "Room"),
    ("[Enter]", "Detail"),
    ("[t]", "Task"),
    ("[f]", "Kanban"),
    ("[o]", "Populate"),
    ("[m]", "Memories"),
    ("[r]", "Reload"),
    ("[?]", "Help"),
    ("[Esc]", "Back"),
    ("[q]", "Quit"),
];

/// Navigation and animation state for the floor. Moving never writes.
#[derive(Debug, Clone)]
pub(super) struct FloorView {
    pub(super) floor: Floor,
    pub(super) selected: usize,
    pub(super) tick: u64,
}

impl FloorView {
    pub(super) fn new(floor: Floor) -> Self {
        let mut view = Self {
            floor,
            selected: 0,
            tick: 0,
        };
        view.clamp();
        view
    }

    pub(super) fn selected_agent(&self) -> Option<&FloorAgent> {
        self.floor.selected(self.selected)
    }

    pub(super) fn selected_task(&self) -> Option<AgentTask> {
        self.selected_agent()
            .and_then(FloorAgent::primary_task)
            .cloned()
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
    }

    pub(super) fn first_agent(&mut self) {
        self.selected = 0;
    }

    pub(super) fn last_agent(&mut self) {
        self.selected = self.floor.agents.len().saturating_sub(1);
    }

    /// Moves the selection to the next/previous occupied room.
    pub(super) fn change_page(&mut self, delta: isize) {
        let Some(current) = self.selected_agent() else {
            return;
        };
        let current = room_index(self.floor.room_for(current));
        let order = [0usize, 1, 2];
        let mut index = current as isize;
        for _ in 0..order.len() {
            index = (index + delta.signum()).rem_euclid(order.len() as isize);
            let room = order[index as usize];
            if let Some(position) = self
                .floor
                .agents
                .iter()
                .position(|agent| room_index(self.floor.room_for(agent)) == room)
            {
                self.selected = position;
                return;
            }
        }
    }

    /// Advances one presentation tick. No database or provider access.
    pub(super) fn advance(&mut self) {
        self.tick = self.tick.wrapping_add(1);
    }

    /// Keeps the selected agent by identifier after a reload.
    pub(super) fn replace(&mut self, floor: Floor) {
        let selected_id = self.selected_agent().map(|agent| agent.id.clone());
        self.floor = floor;
        self.selected = selected_id
            .and_then(|id| self.floor.agents.iter().position(|agent| agent.id == id))
            .unwrap_or(0);
        self.clamp();
    }

    fn clamp(&mut self) {
        let agents = self.floor.agents.len();
        self.selected = self.selected.min(agents.saturating_sub(1));
    }
}

/// Renders the fixed floor and updates no state outside `FloorView`.
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
    let footer_height = u16::try_from(footer.len() + 2).unwrap_or(7).max(5);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(1),
            Constraint::Length(footer_height),
        ])
        .split(area);

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
        format!(" · {} archived", view.floor.archived)
    };
    let project = view
        .floor
        .active_pod()
        .map(|pod| clean(&pod.name))
        .unwrap_or_else(|| "no active project".to_owned());
    let tasks = view
        .floor
        .active_pod()
        .map(|pod| pod.tasks.len())
        .unwrap_or(0);
    format!(
        "{} · {} · {} open · {} agents{} · read-only",
        clean(&view.floor.lab.name),
        project,
        tasks,
        view.floor.agents.len(),
        archived
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
    let room = room_name(view.floor.room_for(agent));
    let task = agent
        .primary_task()
        .map(|task| format!("[{}] {}", task.state.label(), clean(&task.title)))
        .unwrap_or_else(|| "no open task".to_owned());
    let details = format!(" · {} · {room} · {task}", clean(&agent.role));
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
        "(◕‿◕) agent · ★ lead · (zzz) idle · ⇅ exchange · ▸ task · [f] board · read-only"
    } else {
        "(o_o) agent · * lead · (zzz) idle · : exchange · > task · [f] board · read-only"
    };
    Line::from(Span::styled(
        truncate_width(legend, width),
        theme.muted_style(),
    ))
}

fn render_scene(frame: &mut Frame<'_>, area: Rect, theme: Theme, view: &mut FloorView) {
    let mut canvas = Canvas::new(usize::from(area.width), usize::from(area.height));
    if canvas.width < 4 || canvas.height < 4 {
        frame.render_widget(Paragraph::new("Terminal too small."), area);
        return;
    }
    let (tl, tr, bl, br, h, v) = if theme.unicode {
        ('╔', '╗', '╚', '╝', '═', '║')
    } else {
        ('+', '+', '+', '+', '-', '|')
    };
    canvas.frame(tl, tr, bl, br, h, v);
    let lab = clean(&view.floor.lab.name);
    canvas.title(2, 0, &format!("▌ LAB FLOOR · MSX ▌ {lab}"), theme.title());

    let Some((rooms, corridor_y)) = layout(&canvas) else {
        canvas.centered(
            1,
            1,
            canvas.width - 2,
            canvas.height - 2,
            "Terminal too small.",
            theme.muted_style(),
        );
        finish(frame, area, canvas);
        return;
    };

    if view.floor.agents.is_empty() {
        for (index, room) in rooms.iter().enumerate() {
            draw_room(&mut canvas, *room, index, &view.floor, theme);
        }
        canvas.centered(
            1,
            1,
            canvas.width - 2,
            canvas.height.saturating_sub(2),
            "No active agents in this laboratory.",
            theme.muted_style(),
        );
        finish(frame, area, canvas);
        return;
    }

    draw_corridor(&mut canvas, &rooms, corridor_y, theme);
    for (index, room) in rooms.iter().enumerate() {
        draw_room(&mut canvas, *room, index, &view.floor, theme);
    }
    draw_agents(&mut canvas, &view.floor, view, &rooms, corridor_y, theme);
    finish(frame, area, canvas);
}

fn finish(frame: &mut Frame<'_>, area: Rect, canvas: Canvas) {
    frame.render_widget(Paragraph::new(canvas.into_lines()), area);
}

// ---------------------------------------------------------------------------
// Geometry
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
struct RoomRect {
    x: usize,
    y: usize,
    w: usize,
    h: usize,
}

impl RoomRect {
    fn inner_x(self) -> usize {
        self.x + 1
    }

    fn inner_y(self) -> usize {
        self.y + 1
    }

    fn inner_w(self) -> usize {
        self.w.saturating_sub(2)
    }

    fn inner_h(self) -> usize {
        self.h.saturating_sub(2)
    }

    fn door_x(self) -> usize {
        self.x + self.w / 2
    }

    fn floor_y(self) -> usize {
        self.y + self.h.saturating_sub(2)
    }
}

fn layout(canvas: &Canvas) -> Option<([RoomRect; 3], usize)> {
    let inner_w = canvas.width.saturating_sub(2);
    let inner_h = canvas.height.saturating_sub(2);
    if inner_w < 30 || inner_h < 6 {
        return None;
    }
    let gap = if inner_w >= 66 { 1 } else { 0 };
    let total_gap = gap * 2;
    let room_w = (inner_w.saturating_sub(total_gap)) / 3;
    if room_w < 9 {
        return None;
    }
    let corridor_h = if inner_h >= 15 { 2 } else { 1 };
    let room_h = inner_h.saturating_sub(corridor_h);
    if room_h < 4 {
        return None;
    }
    let y = 1;
    let first_x = 1;
    let second_x = first_x + room_w + gap;
    let third_x = second_x + room_w + gap;
    let third_w = 1 + inner_w - third_x;
    let rooms = [
        RoomRect {
            x: first_x,
            y,
            w: room_w,
            h: room_h,
        },
        RoomRect {
            x: second_x,
            y,
            w: room_w,
            h: room_h,
        },
        RoomRect {
            x: third_x,
            y,
            w: third_w.max(room_w),
            h: room_h,
        },
    ];
    Some((rooms, 1 + room_h))
}

fn room_index(room: FloorRoom) -> usize {
    match room {
        FloorRoom::ProjectLab => 0,
        FloorRoom::WarRoom => 1,
        FloorRoom::Lounge => 2,
    }
}

fn room_name(room: FloorRoom) -> &'static str {
    match room {
        FloorRoom::ProjectLab => "Project Lab",
        FloorRoom::WarRoom => "War Room",
        FloorRoom::Lounge => "Lounge",
    }
}

// ---------------------------------------------------------------------------
// Rooms and furniture
// ---------------------------------------------------------------------------

fn draw_room(canvas: &mut Canvas, room: RoomRect, index: usize, floor: &Floor, theme: Theme) {
    draw_room_shell(canvas, room, index, theme);
    match index {
        0 => draw_project_lab(canvas, room, floor, theme),
        1 => draw_war_room(canvas, room, floor, theme),
        _ => draw_lounge(canvas, room, floor, theme),
    }
}

fn draw_room_shell(canvas: &mut Canvas, room: RoomRect, index: usize, theme: Theme) {
    let title = match index {
        0 => "PROJECT LAB",
        1 => "WAR ROOM",
        _ => "LOUNGE",
    };
    let accent = room_accent(index, theme);
    canvas.box_frame(
        room.x,
        room.y,
        room.w,
        room.h,
        title,
        accent,
        accent,
        theme.unicode,
    );
    fill_floor(canvas, room, theme);
    // The corners carry a tiny decor marker so even an empty room reads as a
    // room rather than a blank box.
    let star = if theme.unicode { '✦' } else { '*' };
    canvas.set(room.inner_x(), room.inner_y(), star, accent);
    canvas.set(
        room.x + room.w.saturating_sub(2),
        room.inner_y(),
        star,
        accent,
    );
}

fn fill_floor(canvas: &mut Canvas, room: RoomRect, theme: Theme) {
    let style = floor_style(theme);
    for y in room.inner_y()..room.y + room.h.saturating_sub(1) {
        for x in room.inner_x()..room.x + room.w.saturating_sub(1) {
            let tile = if (x + y) % 2 == 0 {
                if theme.unicode { '░' } else { '.' }
            } else if theme.unicode {
                '·'
            } else {
                ' '
            };
            canvas.set(x, y, tile, style);
        }
    }
}

fn draw_project_lab(canvas: &mut Canvas, room: RoomRect, floor: &Floor, theme: Theme) {
    let accent = room_accent(0, theme);
    let inner_x = room.inner_x();
    let inner_w = room.inner_w();
    let y = room.inner_y();
    let pod = floor.active_pod();
    let subtitle = pod
        .map(|pod| {
            let suffix = if pod.whole_lab { " · whole lab" } else { "" };
            format!("{}{}", clean(&pod.name), suffix)
        })
        .unwrap_or_else(|| "NO ACTIVE PROJECT".to_owned());
    paint_text(
        canvas,
        inner_x,
        y,
        inner_w,
        &subtitle,
        if pod.is_some() {
            accent
        } else {
            theme.warning_style()
        },
    );
    let board_y = y + 1;
    let task_rows = room.inner_h().saturating_sub(4).max(1);
    let Some(pod) = pod else {
        paint_text(
            canvas,
            inner_x,
            board_y,
            inner_w,
            "Open a project from Projects.",
            theme.muted_style(),
        );
        draw_desk(canvas, room, theme);
        return;
    };
    if pod.tasks.is_empty() {
        paint_text(
            canvas,
            inner_x,
            board_y,
            inner_w,
            "No open tasks.",
            theme.muted_style(),
        );
    } else {
        for (row, task) in pod.tasks.iter().take(task_rows).enumerate() {
            let line = format!("{} {}", state_glyph(task.state, theme), clean(&task.title));
            paint_text(
                canvas,
                inner_x,
                board_y + row,
                inner_w,
                &line,
                task_style(task.state, theme),
            );
        }
        if pod.tasks.len() > task_rows {
            paint_text(
                canvas,
                inner_x,
                board_y + task_rows,
                inner_w,
                &format!("+{} more", pod.tasks.len() - task_rows),
                theme.muted_style(),
            );
        }
    }
    draw_desk(canvas, room, theme);
}

fn draw_war_room(canvas: &mut Canvas, room: RoomRect, floor: &Floor, theme: Theme) {
    let accent = room_accent(1, theme);
    let inner_x = room.inner_x();
    let inner_w = room.inner_w();
    let y = room.inner_y();
    let lead = floor.lead.as_ref().and_then(|id| floor.agent(id));
    let lead_text = lead
        .map(|agent| format!("★ TECH LEAD: {}", clean(&agent.name)))
        .unwrap_or_else(|| "NO TECH LEAD · run check-lab".to_owned());
    paint_text(
        canvas,
        inner_x,
        y,
        inner_w,
        &lead_text,
        theme.warning_style(),
    );
    let focused = floor
        .focused_agents()
        .filter(|agent| floor.room_for(agent) == FloorRoom::WarRoom)
        .count();
    paint_text(
        canvas,
        inner_x,
        y + 1,
        inner_w,
        &format!("focus desks: {focused}"),
        theme.muted_style(),
    );
    let board = if theme.unicode {
        "[▤] WHITEBOARD"
    } else {
        "[#] WHITEBOARD"
    };
    paint_text(canvas, inner_x, y + 2, inner_w, board, accent);
    if room.inner_h() >= 7 {
        paint_text(
            canvas,
            inner_x,
            y + 3,
            inner_w,
            "coordinate · decide · unblock",
            theme.muted_style(),
        );
    }
    draw_desk(canvas, room, theme);
}

fn draw_lounge(canvas: &mut Canvas, room: RoomRect, floor: &Floor, theme: Theme) {
    let accent = room_accent(2, theme);
    let inner_x = room.inner_x();
    let inner_w = room.inner_w();
    let y = room.inner_y();
    let idle = floor
        .agents
        .iter()
        .filter(|agent| floor.room_for(agent) == FloorRoom::Lounge)
        .count();
    let couch = if theme.unicode {
        "╰──╯"
    } else {
        "[__]"
    };
    let coffee = if theme.unicode { "[☕]" } else { "[c]" };
    let plant = if theme.unicode { "♣" } else { "*" };
    paint_text(
        canvas,
        inner_x,
        y,
        inner_w,
        &format!("{couch}  {coffee}  {plant}"),
        accent,
    );
    paint_text(
        canvas,
        inner_x,
        y + 1,
        inner_w,
        &format!("{idle} idle agent(s)"),
        theme.muted_style(),
    );
    if room.inner_h() >= 7 {
        paint_text(
            canvas,
            inner_x,
            y + 2,
            inner_w,
            "coffee · rest · recharge",
            theme.muted_style(),
        );
    }
    if idle == 0 {
        paint_text(
            canvas,
            inner_x,
            room.y + room.h.saturating_sub(2),
            inner_w,
            "Everyone is busy.",
            theme.muted_style(),
        );
    }
    draw_couch(canvas, room, theme);
}

fn draw_desk(canvas: &mut Canvas, room: RoomRect, theme: Theme) {
    let y = room.floor_y();
    let desks = if room.inner_w() >= 24 { 2 } else { 1 };
    let desk = if theme.unicode {
        "▄▄▄▄▄"
    } else {
        "_____"
    };
    for index in 0..desks {
        let x = room.inner_x() + 1 + index * 7;
        paint_text(canvas, x, y, 5, desk, theme.border_style());
    }
    let terminal = if theme.unicode { "[▥]" } else { "[#]" };
    paint_text(
        canvas,
        room.x + room.w.saturating_sub(5),
        y,
        4,
        terminal,
        theme.title(),
    );
}

fn draw_couch(canvas: &mut Canvas, room: RoomRect, theme: Theme) {
    let y = room.floor_y();
    let couch = if theme.unicode {
        "╰──╯"
    } else {
        "[__]"
    };
    paint_text(
        canvas,
        room.inner_x() + 1,
        y,
        room.inner_w().saturating_sub(2),
        couch,
        theme.border_style(),
    );
}

fn draw_corridor(canvas: &mut Canvas, rooms: &[RoomRect; 3], corridor_y: usize, theme: Theme) {
    let y = corridor_y;
    if y >= canvas.height.saturating_sub(1) {
        return;
    }
    let (h, door) = if theme.unicode {
        ('─', '╥')
    } else {
        ('-', '+')
    };
    for x in 1..canvas.width.saturating_sub(1) {
        canvas.set(x, y, h, theme.border_style());
    }
    for room in rooms {
        let door_x = room.door_x();
        canvas.set(door_x, y, door, theme.title());
        canvas.set(door_x, y.saturating_sub(1), door, theme.title());
    }
    let label = "VLAB · MSX · CORRIDOR";
    let label = if theme.unicode {
        label
    } else {
        "VLAB - MSX - CORRIDOR"
    };
    let label_x = 1 + canvas.width.saturating_sub(2 + label.width()) / 2;
    paint_text(
        canvas,
        label_x,
        y,
        canvas.width.saturating_sub(label_x + 1),
        label,
        theme.muted_style(),
    );
    // A couple of direction markers keep the corridor from reading as a rule.
    canvas.set(
        2,
        y,
        if theme.unicode { '▶' } else { '>' },
        theme.muted_style(),
    );
    canvas.set(
        canvas.width.saturating_sub(3),
        y,
        if theme.unicode { '◀' } else { '<' },
        theme.muted_style(),
    );
    let link_width = rooms[0].door_x().saturating_sub(4);
    let link = if link_width >= "[f] KANBAN".width() {
        "[f] KANBAN"
    } else {
        "[f]"
    };
    paint_text(canvas, 3, y, link_width, link, theme.key());
}

fn room_accent(index: usize, theme: Theme) -> Style {
    if !theme.color {
        return theme.title();
    }
    let color = match index {
        0 => Color::Rgb(36, 219, 219),
        1 => Color::Rgb(255, 255, 36),
        _ => Color::Rgb(36, 219, 36),
    };
    Style::default().fg(color).add_modifier(Modifier::BOLD)
}

fn floor_style(theme: Theme) -> Style {
    if theme.color {
        Style::default().fg(Color::Rgb(38, 38, 58))
    } else {
        Style::default()
    }
}

fn task_style(state: StoryState, theme: Theme) -> Style {
    if !theme.color {
        return Style::default().add_modifier(Modifier::BOLD);
    }
    let color = match state {
        StoryState::Todo => Color::Rgb(255, 255, 36),
        StoryState::InProgress => Color::Rgb(36, 219, 36),
        StoryState::InReview => Color::Rgb(36, 219, 219),
        StoryState::Blocked => Color::Rgb(219, 36, 36),
        StoryState::Done => Color::DarkGray,
    };
    Style::default().fg(color).add_modifier(Modifier::BOLD)
}

fn state_glyph(state: StoryState, theme: Theme) -> &'static str {
    match (state, theme.unicode) {
        (StoryState::Todo, true) => "◇",
        (StoryState::Todo, false) => "-",
        (StoryState::InProgress, true) => "▶",
        (StoryState::InProgress, false) => ">",
        (StoryState::InReview, true) => "◉",
        (StoryState::InReview, false) => "o",
        (StoryState::Blocked, true) => "!",
        (StoryState::Blocked, false) => "!",
        (StoryState::Done, true) => "✓",
        (StoryState::Done, false) => "x",
    }
}

// ---------------------------------------------------------------------------
// Agents and movement
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
struct Spot {
    room: usize,
    x: usize,
    y: usize,
}

fn draw_agents(
    canvas: &mut Canvas,
    floor: &Floor,
    view: &FloorView,
    rooms: &[RoomRect; 3],
    corridor_y: usize,
    theme: Theme,
) {
    let mut by_room: [Vec<&FloorAgent>; 3] = [Vec::new(), Vec::new(), Vec::new()];
    for agent in &floor.agents {
        let room = room_index(floor.room_for(agent));
        by_room[room].push(agent);
    }

    let pixel = canvas.width >= 76 && rooms[0].inner_w() >= 22;
    let mut drawn: Vec<DrawnAgent> = Vec::with_capacity(floor.agents.len());
    for (room_index_value, agents) in by_room.iter().enumerate() {
        let room = rooms[room_index_value];
        let count = agents.len().max(1);
        for (slot, agent) in agents.iter().enumerate() {
            let spots = tour_for(floor, agent, room_index_value, room, slot, count, rooms);
            let (x, y, walking) = position_at(agent, &spots, view.tick, rooms, corridor_y);
            let selected = view.selected_agent().is_some_and(|sel| sel.id == agent.id);
            draw_agent(canvas, floor, agent, x, y, walking, selected, pixel, theme);
            drawn.push(DrawnAgent {
                room: room_index_value,
                x,
                y,
                walking,
                role: agent.role.clone(),
            });
        }
    }
    draw_exchanges(canvas, &drawn, rooms, theme);
}

fn tour_for(
    floor: &Floor,
    agent: &FloorAgent,
    room: usize,
    rect: RoomRect,
    slot: usize,
    count: usize,
    rooms: &[RoomRect; 3],
) -> Vec<Spot> {
    let mut spots = room_spots(rect, room, slot, count);
    // A worker may walk to the War Room to coordinate with the lead. This is
    // the only cross-room tour and is grounded in stored assignments.
    if room == 0 && floor.lead.is_some() && agent.is_working() {
        let war = rooms[1];
        spots.push(Spot {
            room: 1,
            x: war.inner_x() + war.inner_w().saturating_sub(3),
            y: war.floor_y().saturating_sub(1),
        });
    }
    spots.truncate(MAX_TOUR);
    spots
}

fn room_spots(rect: RoomRect, room: usize, slot: usize, count: usize) -> Vec<Spot> {
    let usable = rect.inner_w().saturating_sub(2).max(1);
    let step = (usable / count.max(1)).max(3);
    let base = rect.inner_x() + 1 + (slot * step) % usable;
    let y = rect.floor_y();
    let second = (base + step.min(4)).min(rect.x + rect.w.saturating_sub(3));
    let third = (second + step.min(4)).min(rect.x + rect.w.saturating_sub(3));
    match room {
        0 | 1 => vec![Spot { room, x: base, y }, Spot { room, x: second, y }],
        _ => vec![
            Spot { room, x: base, y },
            Spot { room, x: second, y },
            Spot { room, x: third, y },
        ],
    }
}

fn position_at(
    agent: &FloorAgent,
    spots: &[Spot],
    tick: u64,
    rooms: &[RoomRect; 3],
    corridor_y: usize,
) -> (usize, usize, bool) {
    if spots.is_empty() {
        return (1, 1, false);
    }
    let offset = stable_hash(agent.id.as_str()) % LEG_TICKS;
    let phase = tick.wrapping_add(offset) % (LEG_TICKS * spots.len() as u64);
    let leg = (phase / LEG_TICKS) as usize;
    let within = phase % LEG_TICKS;
    let from = spots[leg];
    let to = spots[(leg + 1) % spots.len()];
    if within < WALK_TICKS && spots.len() > 1 {
        let (x, y) = path_point(from, to, within, WALK_TICKS, rooms, corridor_y);
        (x, y, true)
    } else {
        (from.x, from.y, false)
    }
}

fn path_point(
    from: Spot,
    to: Spot,
    step: u64,
    steps: u64,
    rooms: &[RoomRect; 3],
    corridor_y: usize,
) -> (usize, usize) {
    if from.room == to.room {
        return interpolate(from.x, from.y, to.x, to.y, step, steps);
    }
    let from_room = rooms[from.room];
    let to_room = rooms[to.room];
    let points = [
        (from.x, from.y),
        (
            from_room.door_x(),
            from_room.y + from_room.h.saturating_sub(1),
        ),
        (from_room.door_x(), corridor_y),
        (to_room.door_x(), corridor_y),
        (to_room.door_x(), to_room.y + to_room.h.saturating_sub(1)),
        (to.x, to.y),
    ];
    let lengths: Vec<usize> = points
        .windows(2)
        .map(|pair| pair[0].0.abs_diff(pair[1].0) + pair[0].1.abs_diff(pair[1].1))
        .collect();
    let total: usize = lengths.iter().sum();
    if total == 0 {
        return (from.x, from.y);
    }
    let target = (total as u64 * step / steps.max(1)) as usize;
    let mut travelled = 0;
    for (index, length) in lengths.iter().enumerate() {
        if travelled + length >= target || index + 1 == lengths.len() {
            let local = target.saturating_sub(travelled);
            let (x1, y1) = points[index];
            let (x2, y2) = points[index + 1];
            let denominator = (*length).max(1) as u64;
            return interpolate(x1, y1, x2, y2, local as u64, denominator);
        }
        travelled += length;
    }
    (to.x, to.y)
}

fn interpolate(
    x1: usize,
    y1: usize,
    x2: usize,
    y2: usize,
    step: u64,
    steps: u64,
) -> (usize, usize) {
    let steps = steps.max(1) as i64;
    let step = step.min(steps as u64) as i64;
    let x = x1 as i64 + (x2 as i64 - x1 as i64) * step / steps;
    let y = y1 as i64 + (y2 as i64 - y1 as i64) * step / steps;
    (x.max(0) as usize, y.max(0) as usize)
}

fn stable_hash(value: &str) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for byte in value.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

#[derive(Debug)]
struct DrawnAgent {
    room: usize,
    x: usize,
    y: usize,
    walking: bool,
    role: String,
}

#[allow(clippy::too_many_arguments)]
fn draw_agent(
    canvas: &mut Canvas,
    floor: &Floor,
    agent: &FloorAgent,
    x: usize,
    y: usize,
    walking: bool,
    selected: bool,
    pixel: bool,
    theme: Theme,
) {
    let lead = floor.lead.as_ref() == Some(&agent.id);
    let base = if selected {
        theme.selected()
    } else {
        Style::default()
    };
    let prefix = if selected {
        if theme.unicode { "▶" } else { ">" }
    } else {
        ""
    };
    let bubble = if agent.is_idle() {
        "(zzz)"
    } else if walking {
        if theme.unicode { "(→)" } else { "(>)" }
    } else if agent.is_focused() {
        "(///)"
    } else if lead {
        "(★)"
    } else {
        "( ... )"
    };
    if pixel {
        let (top, bottom) = if walking {
            if theme.unicode {
                ("▝▄▘", "█>█")
            } else {
                (".-.", "|>|")
            }
        } else if lead {
            if theme.unicode {
                ("▗▄▖", "█★█")
            } else {
                (".-.", "|*|")
            }
        } else {
            if theme.unicode {
                ("▗▄▖", "█◕█")
            } else {
                (".-.", "|o|")
            }
        };
        paint_text(canvas, x, y.saturating_sub(1), 4, top, base);
        paint_text(canvas, x, y, 4, bottom, base);
        let name_x = x + 4;
        let name_w = canvas.width.saturating_sub(name_x + 1);
        paint_text(canvas, name_x, y, name_w, &clean(&agent.name), base);
        let bubble_x = name_x + name_w.saturating_sub(bubble.width() + 1);
        paint_text(
            canvas,
            bubble_x,
            y,
            bubble.width(),
            bubble,
            if selected { base } else { theme.muted_style() },
        );
        if !prefix.is_empty() {
            paint_text(canvas, x, y.saturating_sub(1), 1, prefix, base);
        }
    } else {
        let sprite = if walking {
            "(>_>)"
        } else if lead {
            if theme.unicode {
                "★(◕‿◕)"
            } else {
                "*(o_o)"
            }
        } else {
            if theme.unicode {
                "(◕‿◕)"
            } else {
                "(o_o)"
            }
        };
        let line = if prefix.is_empty() {
            format!("{sprite} {bubble}")
        } else {
            format!("{prefix}{sprite} {bubble}")
        };
        let available = canvas.width.saturating_sub(x + 1);
        paint_text(canvas, x, y, available, &line, base);
        let name_x = x + line.width() + 1;
        if name_x < canvas.width.saturating_sub(1) {
            paint_text(
                canvas,
                name_x,
                y,
                canvas.width.saturating_sub(name_x + 1),
                &clean(&agent.name),
                if selected { base } else { theme.muted_style() },
            );
        }
    }
}

fn draw_exchanges(canvas: &mut Canvas, drawn: &[DrawnAgent], rooms: &[RoomRect; 3], theme: Theme) {
    for (room, rect) in rooms.iter().enumerate() {
        let agents: Vec<&DrawnAgent> = drawn
            .iter()
            .filter(|agent| agent.room == room && !agent.walking)
            .collect();
        if room == 2 || agents.len() < 2 {
            continue;
        }
        let first = agents[0];
        let second = agents.iter().skip(1).find(|agent| agent.role != first.role);
        let Some(second) = second else {
            continue;
        };
        let x = (first.x + second.x) / 2;
        let y = first.y.saturating_sub(1).max(rect.inner_y());
        if x < canvas.width && y < canvas.height {
            canvas.set(
                x,
                y,
                if theme.unicode { '⇅' } else { ':' },
                theme.warning_style(),
            );
        }
    }
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
    use crate::domain::ids::{AgentId, LabId, ProjectId, StoryId};
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
            tasks: Vec::new(),
        }
    }

    fn task(id: &str, project: &str, title: &str, state: StoryState) -> AgentTask {
        AgentTask {
            id: StoryId::parse(id).unwrap(),
            project_id: ProjectId::parse(project).unwrap(),
            project_name: "Portal".to_owned(),
            title: title.to_owned(),
            state,
            priority: "high".to_owned(),
            due_date: "2026-01-03".to_owned(),
            assignee: None,
            assignee_name: Some("Bea".to_owned()),
        }
    }

    fn floor() -> Floor {
        let portal = ProjectId::parse("20000001").unwrap();
        let mobile = ProjectId::parse("20000002").unwrap();
        let reset = task(
            "30000001",
            "20000001",
            "Reset password",
            StoryState::InProgress,
        );
        let audit = task("30000002", "20000001", "Audit log", StoryState::Todo);
        let mut ada = agent("10000001", "Ada", "lead", 0, &["20000001"]);
        ada.tasks.clear();
        let mut bea = agent("10000002", "Bea", "dev", 1, &["20000001"]);
        bea.tasks.push(reset.clone());
        let mut cid = agent("10000003", "Cid", "qa", 1, &["20000001"]);
        cid.tasks.push(audit.clone());
        let mut dee = agent("10000004", "Dee", "analyst", 2, &["20000002"]);
        dee.tasks.push(task(
            "30000003",
            "20000002",
            "Dependency review",
            StoryState::InProgress,
        ));
        Floor {
            lab: LabOverview {
                id: LabId::parse("a3f91c07").unwrap(),
                name: "Demo Lab".to_owned(),
                description: String::new(),
                agent_count: 4,
                project_count: 2,
            },
            lead: Some(agent_id("10000001")),
            active_project: Some(portal.clone()),
            agents: vec![ada, bea, cid, dee],
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
                    tasks: vec![reset, audit],
                },
                FloorPod {
                    project_id: Some(mobile),
                    name: "Mobile".to_owned(),
                    members: vec![agent_id("10000004")],
                    whole_lab: false,
                    tasks: Vec::new(),
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
    fn the_fixed_floor_shows_three_rooms_tasks_and_decorations() {
        let mut view = FloorView::new(floor());
        let screen = scene(&mut view, 100, 30, theme(true));
        for text in [
            "LAB FLOOR",
            "PROJECT LAB",
            "WAR ROOM",
            "LOUNGE",
            "CORRIDOR",
            "Portal",
            "Reset password",
            "Audit log",
            "TECH LEAD",
            "focus desks",
            "WHITEBOARD",
            "idle agent",
            "[f] KANBAN",
            "Selected:",
            "read-only",
        ] {
            assert!(screen.contains(text), "{text}: {screen}");
        }
        assert!(screen.contains("▗▄▖"), "{screen}");
        assert!(screen.contains("░"), "{screen}");
        assert!(screen.contains("☕"), "{screen}");
        assert!(screen.contains("♣"), "{screen}");
    }

    #[test]
    fn the_floor_works_at_the_minimum_size_and_without_unicode() {
        for (width, height) in [(60, 20), (80, 24), (100, 30)] {
            let mut view = FloorView::new(floor());
            let screen = scene(&mut view, width, height, theme(true));
            for text in [
                "PROJECT LAB",
                "WAR ROOM",
                "LOUNGE",
                "Portal",
                "Reset password",
            ] {
                assert!(screen.contains(text), "{width}x{height} {text}: {screen}");
            }
        }
        let mut ascii = FloorView::new(floor());
        let screen = scene(&mut ascii, 60, 20, theme(false));
        assert!(screen.contains("PROJECT LAB"), "{screen}");
        assert!(screen.contains("[f]"), "{screen}");
        assert!(screen.contains("Kanban"), "{screen}");
        assert!(!screen.contains('╔'), "{screen}");
        assert!(!screen.contains("◕"), "{screen}");
    }

    #[test]
    fn selection_moves_between_agents_and_rooms() {
        let mut view = FloorView::new(floor());
        assert_eq!(view.selected_agent().unwrap().name, "Ada");
        view.move_selection(1);
        assert_eq!(view.selected_agent().unwrap().name, "Bea");
        view.change_page(1);
        assert_ne!(
            view.floor.room_for(view.selected_agent().unwrap()),
            FloorRoom::ProjectLab
        );
        view.first_agent();
        assert_eq!(view.selected_agent().unwrap().name, "Ada");
        view.last_agent();
        assert_eq!(view.selected_agent().unwrap().name, "Dee");
    }

    #[test]
    fn movement_is_deterministic_and_changes_position() {
        let model = floor();
        let agent = model.agent(&agent_id("10000002")).unwrap();
        let canvas = Canvas::new(100, 30);
        let (rooms, corridor_y) = layout(&canvas).expect("layout");
        let spots = vec![
            Spot {
                room: 0,
                x: 8,
                y: rooms[0].floor_y(),
            },
            Spot {
                room: 0,
                x: 18,
                y: rooms[0].floor_y(),
            },
            Spot {
                room: 1,
                x: rooms[1].inner_x() + 4,
                y: rooms[1].floor_y(),
            },
        ];
        let first = position_at(agent, &spots, 0, &rooms, corridor_y);
        let same = position_at(agent, &spots, 0, &rooms, corridor_y);
        let later = position_at(agent, &spots, 6, &rooms, corridor_y);
        assert_eq!(first, same);
        assert_ne!(first, later);
        // The path through the corridor never leaves the canvas.
        for tick in 0..60 {
            let (x, y, _) = position_at(agent, &spots, tick, &rooms, corridor_y);
            assert!(x < canvas.width && y < canvas.height, "{tick}: {x},{y}");
        }
    }

    #[test]
    fn exchanges_are_drawn_for_two_different_roles_in_a_room() {
        let mut canvas = Canvas::new(60, 24);
        let (rooms, _) = layout(&canvas).expect("layout");
        let drawn = vec![
            DrawnAgent {
                room: 0,
                x: 6,
                y: rooms[0].floor_y(),
                walking: false,
                role: "dev".to_owned(),
            },
            DrawnAgent {
                room: 0,
                x: 14,
                y: rooms[0].floor_y(),
                walking: false,
                role: "qa".to_owned(),
            },
        ];
        draw_exchanges(&mut canvas, &drawn, &rooms, theme(true));
        let screen: String = canvas
            .into_lines()
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.as_ref())
                    .collect::<String>()
            })
            .collect();
        assert!(screen.contains('⇅'), "{screen}");
    }

    #[test]
    fn the_selected_task_and_empty_states_are_explicit() {
        let mut view = FloorView::new(floor());
        view.move_selection(1);
        assert_eq!(view.selected_task().unwrap().title, "Reset password");
        let mut empty = floor();
        empty.agents.clear();
        empty.pods.clear();
        empty.lead = None;
        empty.active_project = None;
        let mut view = FloorView::new(empty);
        let screen = scene(&mut view, 80, 24, theme(true));
        assert!(screen.contains("No active agents"), "{screen}");
        assert!(screen.contains("PROJECT LAB"), "{screen}");
        assert!(screen.contains("NO ACTIVE PROJECT"), "{screen}");
    }

    #[test]
    fn replace_keeps_the_selected_agent_and_advance_changes_the_tick() {
        let mut view = FloorView::new(floor());
        view.move_selection(2);
        let id = view.selected_agent().unwrap().id.clone();
        view.replace(floor());
        assert_eq!(view.selected_agent().unwrap().id, id);
        let tick = view.tick;
        view.advance();
        assert_eq!(view.tick, tick + 1);
    }
}
