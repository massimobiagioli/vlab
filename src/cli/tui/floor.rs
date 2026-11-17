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
use crate::app::floor::{Floor, FloorAgent, FloorPod, FloorRoom};
use crate::app::kanban::StoryState;

use super::{Theme, action_grid, clean};

const LEG_TICKS: u64 = 60;
const WALK_TICKS: u64 = 24;
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
    if view.floor.active_project.is_none() {
        return Line::from(Span::styled(
            truncate_width(
                "No active project · open one from Projects · Enter shows this agent",
                width,
            ),
            theme.muted_style(),
        ));
    }
    let room = room_name(view.floor.room_for(agent));
    let details = format!(" · {} · {room} · Enter detail · t task", clean(&agent.role));
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
        "(•‿•) agent · ★ lead · ⇅ exchange · Enter detail · [f] board · read-only"
    } else {
        "(o_o) agent · * lead · : exchange · Enter detail · [f] board · read-only"
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
    canvas.title(2, 0, &format!("▌ LAB FLOOR ▌ {lab}"), theme.title());

    let Some(layout) = layout(&canvas) else {
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
        for (index, room) in layout.rooms.iter().enumerate() {
            draw_room(&mut canvas, *room, index, &view.floor, &layout, theme);
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

    draw_corridor(&mut canvas, &layout, theme);
    for (index, room) in layout.rooms.iter().enumerate() {
        draw_room(&mut canvas, *room, index, &view.floor, &layout, theme);
    }
    draw_agents(&mut canvas, &view.floor, view, &layout, theme);
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

#[derive(Debug, Clone, Copy)]
struct FloorPlan {
    rooms: [RoomRect; 3],
    corridor_y: usize,
    /// Top-down T plan on wide terminals; compact side-by-side fallback when
    /// the scene is too short for a readable center room.
    t_shape: bool,
}

fn layout(canvas: &Canvas) -> Option<FloorPlan> {
    let inner_w = canvas.width.saturating_sub(2);
    let inner_h = canvas.height.saturating_sub(2);
    if inner_w < 30 || inner_h < 5 {
        return None;
    }
    if inner_h >= 12 && inner_w >= 48 {
        return t_layout(inner_w, inner_h);
    }
    row_layout(inner_w, inner_h)
}

fn t_layout(inner_w: usize, inner_h: usize) -> Option<FloorPlan> {
    let top_h = (inner_h * 45 / 100).clamp(4, inner_h.saturating_sub(4));
    let side_w = inner_w.saturating_sub(4) / 2;
    if side_w < 12 {
        return row_layout(inner_w, inner_h);
    }
    let center_w = side_w
        .saturating_sub(2)
        .clamp(12, inner_w.saturating_sub(2));
    let center_x = 1 + (inner_w - center_w) / 2;
    let center_y = 1 + top_h + 1;
    let center_h = inner_h.saturating_sub(top_h + 2);
    if center_h < 3 {
        return row_layout(inner_w, inner_h);
    }
    let rooms = [
        RoomRect {
            x: 1,
            y: 1,
            w: side_w,
            h: top_h,
        },
        RoomRect {
            x: center_x,
            y: center_y,
            w: center_w,
            h: center_h,
        },
        RoomRect {
            x: 1 + inner_w - side_w,
            y: 1,
            w: side_w,
            h: top_h,
        },
    ];
    Some(FloorPlan {
        rooms,
        corridor_y: 1 + top_h / 2,
        t_shape: true,
    })
}

fn row_layout(inner_w: usize, inner_h: usize) -> Option<FloorPlan> {
    let gap = if inner_w >= 66 { 1 } else { 0 };
    let room_w = (inner_w.saturating_sub(gap * 2)) / 3;
    if room_w < 9 {
        return None;
    }
    let corridor_h = if inner_h >= 15 { 2 } else { 1 };
    let room_h = inner_h.saturating_sub(corridor_h);
    if room_h < 4 {
        return None;
    }
    let first_x = 1;
    let second_x = first_x + room_w + gap;
    let third_x = second_x + room_w + gap;
    let third_w = 1 + inner_w - third_x;
    let rooms = [
        RoomRect {
            x: first_x,
            y: 1,
            w: room_w,
            h: room_h,
        },
        RoomRect {
            x: second_x,
            y: 1,
            w: room_w,
            h: room_h,
        },
        RoomRect {
            x: third_x,
            y: 1,
            w: third_w.max(room_w),
            h: room_h,
        },
    ];
    Some(FloorPlan {
        rooms,
        corridor_y: 1 + room_h,
        t_shape: false,
    })
}

fn room_door(index: usize, layout: &FloorPlan) -> (usize, usize) {
    let room = layout.rooms[index];
    if !layout.t_shape {
        return (room.x + room.w / 2, room.y + room.h.saturating_sub(1));
    }
    match index {
        0 => (room.x + room.w.saturating_sub(1), layout.corridor_y),
        1 => (room.x + room.w / 2, room.y),
        _ => (room.x, layout.corridor_y),
    }
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

fn draw_room(
    canvas: &mut Canvas,
    room: RoomRect,
    index: usize,
    floor: &Floor,
    layout: &FloorPlan,
    theme: Theme,
) {
    draw_room_shell(canvas, room, index, layout, theme);
    match index {
        0 => draw_project_lab(canvas, room, floor, theme),
        1 => draw_war_room(canvas, room, floor, theme),
        _ => draw_lounge(canvas, room, floor, theme),
    }
}

fn draw_room_shell(
    canvas: &mut Canvas,
    room: RoomRect,
    index: usize,
    layout: &FloorPlan,
    theme: Theme,
) {
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
    let (door_x, door_y) = room_door(index, layout);
    let door = match (layout.t_shape, index) {
        (true, 0) => '┤',
        (true, 2) => '├',
        (true, 1) => '┬',
        _ => '╥',
    };
    canvas.set(door_x, door_y, door, accent);
    if layout.t_shape && index == 1 {
        canvas.set(
            room.x + room.w / 2,
            room.y + room.h.saturating_sub(1),
            '╨',
            accent,
        );
    }
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
    let y = room.inner_y();
    draw_task_board(canvas, room, floor.active_pod(), theme);
    let board = if theme.unicode { "[▤]" } else { "[#]" };
    paint_text(canvas, room.inner_x(), y + 1, 3, board, accent);
    draw_plant(canvas, room, theme);
    draw_desk(canvas, room, theme);
    draw_chair_row(canvas, room, theme);
}

fn draw_war_room(canvas: &mut Canvas, room: RoomRect, floor: &Floor, theme: Theme) {
    let accent = room_accent(1, theme);
    let y = room.inner_y();
    let star = if theme.unicode { '★' } else { '*' };
    canvas.set(room.inner_x(), y, star, theme.warning_style());
    let clock = if theme.unicode { '◷' } else { 'o' };
    canvas.set(
        room.x + room.w.saturating_sub(3),
        y,
        clock,
        theme.muted_style(),
    );
    let board = if theme.unicode { "[▤]" } else { "[#]" };
    paint_text(canvas, room.inner_x(), y + 1, 3, board, accent);
    if room.inner_h() >= 6 {
        paint_text(
            canvas,
            room.inner_x() + 5,
            y + 1,
            2,
            if theme.unicode { "▤▤" } else { "##" },
            theme.muted_style(),
        );
    }
    draw_focus_desks(canvas, room, floor, theme);
    draw_plant(canvas, room, theme);
    draw_desk(canvas, room, theme);
}

fn draw_lounge(canvas: &mut Canvas, room: RoomRect, floor: &Floor, theme: Theme) {
    let accent = room_accent(2, theme);
    let y = room.inner_y();
    let coffee = if theme.unicode { "[☕]" } else { "[c]" };
    paint_text(canvas, room.inner_x(), y, 4, coffee, accent);
    let tv = if theme.unicode { "[▓]" } else { "[#]" };
    paint_text(
        canvas,
        room.x + room.w.saturating_sub(5),
        y,
        3,
        tv,
        theme.muted_style(),
    );
    draw_plant(canvas, room, theme);
    draw_couch(canvas, room, theme);
    if room.inner_h() >= 7 {
        let rug = if theme.unicode {
            "≈≈≈≈"
        } else {
            "~~~~"
        };
        paint_text(
            canvas,
            room.inner_x() + 2,
            room.inner_y() + 2,
            room.inner_w().saturating_sub(3),
            rug,
            theme.muted_style(),
        );
    }
    let _ = floor;
}

fn draw_task_board(canvas: &mut Canvas, room: RoomRect, pod: Option<&FloorPod>, theme: Theme) {
    let Some(pod) = pod else {
        return;
    };
    let y = room.inner_y();
    let mut x = room.inner_x() + 5;
    for task in pod.tasks.iter().take(5) {
        if x + 1 >= room.x + room.w.saturating_sub(1) {
            break;
        }
        canvas.set(
            x,
            y + 1,
            if theme.unicode { '■' } else { '#' },
            task_style(task.state, theme),
        );
        x += 2;
    }
}

fn draw_focus_desks(canvas: &mut Canvas, room: RoomRect, floor: &Floor, theme: Theme) {
    let focused = floor
        .focused_agents()
        .filter(|agent| floor.room_for(agent) == FloorRoom::WarRoom)
        .count();
    let y = (room.inner_y() + 3).min(room.floor_y().saturating_sub(1));
    for index in 0..focused.min(3) {
        let x = room.inner_x() + 1 + index * 7;
        let desk = if theme.unicode { "▄▄▄" } else { "___" };
        paint_text(canvas, x, y, 3, desk, theme.border_style());
        let terminal = if theme.unicode { "[▥]" } else { "[#]" };
        paint_text(canvas, x + 3, y, 3, terminal, theme.title());
    }
}

fn draw_chair_row(canvas: &mut Canvas, room: RoomRect, theme: Theme) {
    let y = room.floor_y().saturating_sub(1);
    if y <= room.inner_y() {
        return;
    }
    let chair = if theme.unicode { '▣' } else { 'o' };
    for index in 0..2 {
        let x = room.inner_x() + 2 + index * 7;
        if x + 1 < room.x + room.w.saturating_sub(1) {
            canvas.set(x, y, chair, theme.muted_style());
        }
    }
}

fn draw_plant(canvas: &mut Canvas, room: RoomRect, theme: Theme) {
    let plant = if theme.unicode { '♣' } else { '*' };
    canvas.set(
        room.x + room.w.saturating_sub(3),
        room.inner_y() + 2,
        plant,
        theme.success_style(),
    );
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

fn draw_corridor(canvas: &mut Canvas, layout: &FloorPlan, theme: Theme) {
    let (h, v, cross, door) = if theme.unicode {
        ('▒', '▒', '┼', '╥')
    } else {
        ('.', '.', '+', '+')
    };
    let y = layout.corridor_y;
    if y >= canvas.height.saturating_sub(1) {
        return;
    }
    if layout.t_shape {
        let (left_x, _) = room_door(0, layout);
        let (right_x, _) = room_door(2, layout);
        for x in left_x.min(right_x)..=left_x.max(right_x) {
            canvas.set(x, y, h, theme.border_style());
        }
        let center = layout.rooms[1];
        let center_x = center.x + center.w / 2;
        for row in y..=center.y {
            canvas.set(center_x, row, v, theme.border_style());
        }
        canvas.set(left_x, y, door, theme.title());
        canvas.set(right_x, y, door, theme.title());
        canvas.set(center_x, y, cross, theme.border_style());
        canvas.set(center_x, center.y, door, theme.title());
        let arrow = if theme.unicode { '›' } else { '>' };
        let back = if theme.unicode { '‹' } else { '<' };
        canvas.set(center_x.saturating_sub(2), y, arrow, theme.muted_style());
        canvas.set(center_x + 2, y, back, theme.muted_style());
        canvas.set(
            center_x,
            center.y + center.h.saturating_sub(1),
            if theme.unicode { '╨' } else { '+' },
            theme.title(),
        );
    } else {
        for x in 1..canvas.width.saturating_sub(1) {
            canvas.set(x, y, h, theme.border_style());
        }
        for (index, _) in layout.rooms.iter().enumerate() {
            let (door_x, door_y) = room_door(index, layout);
            canvas.set(door_x, door_y, door, theme.title());
        }
    }
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
    layout: &FloorPlan,
    theme: Theme,
) {
    let mut by_room: [Vec<&FloorAgent>; 3] = [Vec::new(), Vec::new(), Vec::new()];
    for agent in &floor.agents {
        let room = room_index(floor.room_for(agent));
        by_room[room].push(agent);
    }

    let mut drawn: Vec<DrawnAgent> = Vec::with_capacity(floor.agents.len());
    for (room_index_value, agents) in by_room.iter().enumerate() {
        let room = layout.rooms[room_index_value];
        let count = agents.len().max(1);
        for (slot, agent) in agents.iter().enumerate() {
            let spots = tour_for(
                floor,
                agent,
                room_index_value,
                room,
                slot,
                count,
                &layout.rooms,
            );
            let (x, y, walking) = position_at(agent, &spots, view.tick, layout);
            let selected = view.selected_agent().is_some_and(|sel| sel.id == agent.id);
            let max_x = if walking {
                canvas.width.saturating_sub(2)
            } else {
                room.x + room.w.saturating_sub(2)
            };
            draw_agent(canvas, floor, agent, x, y, walking, selected, max_x, theme);
            drawn.push(DrawnAgent {
                room: room_index_value,
                x,
                y,
                walking,
                role: agent.role.clone(),
            });
        }
    }
    draw_exchanges(canvas, &drawn, &layout.rooms, theme);
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
    if room == 1 && floor.lead.as_ref() != Some(&agent.id) {
        let focus_y = (rect.inner_y() + 3).min(rect.floor_y().saturating_sub(1));
        for spot in &mut spots {
            spot.y = focus_y;
        }
    }
    // A worker may walk to the War Room to coordinate with the lead. This is
    // the only cross-room tour and is grounded in stored assignments.
    if (room == 0 && floor.lead.is_some() && agent.is_working())
        || (room == 2 && floor.lead.is_some())
    {
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
    let base = rect.inner_x() + 3 + (slot * step) % usable;
    let desk_y = rect.floor_y().saturating_sub(1);
    let mid_y = (rect.inner_y() + 2).min(desk_y);
    let upper_y = (rect.inner_y() + 1).min(desk_y.saturating_sub(1));
    let second = (base + step.min(4)).min(rect.x + rect.w.saturating_sub(3));
    let third = (second + step.min(4)).min(rect.x + rect.w.saturating_sub(3));
    let mut spots = vec![
        Spot {
            room,
            x: base,
            y: desk_y,
        },
        Spot {
            room,
            x: second,
            y: mid_y,
        },
    ];
    if rect.inner_h() >= 4 {
        spots.push(Spot {
            room,
            x: if room == 2 { third } else { base },
            y: upper_y,
        });
    }
    spots
}

fn position_at(
    agent: &FloorAgent,
    spots: &[Spot],
    tick: u64,
    layout: &FloorPlan,
) -> (usize, usize, bool) {
    if spots.is_empty() {
        return (1, 1, false);
    }
    let cycle = LEG_TICKS * spots.len() as u64;
    let offset = stable_hash(agent.id.as_str()) % cycle;
    let phase = tick.wrapping_add(offset) % cycle;
    let leg = (phase / LEG_TICKS) as usize;
    let within = phase % LEG_TICKS;
    let from = spots[leg];
    let to = spots[(leg + 1) % spots.len()];
    if within < WALK_TICKS && spots.len() > 1 {
        let (x, y) = path_point(from, to, within, WALK_TICKS, layout);
        (x, y, true)
    } else {
        (from.x, from.y, false)
    }
}

fn path_point(from: Spot, to: Spot, step: u64, steps: u64, layout: &FloorPlan) -> (usize, usize) {
    if from.room == to.room {
        return interpolate(from.x, from.y, to.x, to.y, step, steps);
    }
    let from_door = room_door(from.room, layout);
    let to_door = room_door(to.room, layout);
    let points: Vec<(usize, usize)> = if layout.t_shape {
        let center = layout.rooms[1];
        let hub = (center.x + center.w / 2, layout.corridor_y);
        vec![(from.x, from.y), from_door, hub, to_door, (to.x, to.y)]
    } else {
        let from_room = layout.rooms[from.room];
        let to_room = layout.rooms[to.room];
        vec![
            (from.x, from.y),
            (
                from_room.door_x(),
                from_room.y + from_room.h.saturating_sub(1),
            ),
            (from_room.door_x(), layout.corridor_y),
            (to_room.door_x(), layout.corridor_y),
            (to_room.door_x(), to_room.y + to_room.h.saturating_sub(1)),
            (to.x, to.y),
        ]
    };
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
    max_x: usize,
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
        " "
    };
    let sprite = if walking {
        "(•_•)"
    } else if lead {
        if theme.unicode {
            "★(•‿•)"
        } else {
            "*(o_o)"
        }
    } else if agent.is_idle() {
        "(-_-)"
    } else if agent.is_focused() {
        "(>_<)"
    } else if theme.unicode {
        "(•‿•)"
    } else {
        "(o_o)"
    };
    let text = format!("{prefix}{sprite}");
    let available = max_x.saturating_sub(x).max(1);
    paint_text(canvas, x, y, available, &text, base);

    let name_width = max_x.saturating_sub(x);
    if name_width > 0 && y + 1 < canvas.height {
        paint_text(
            canvas,
            x,
            y + 1,
            name_width,
            &clean(&agent.name),
            if selected { base } else { theme.muted_style() },
        );
    }

    let bubble = if lead {
        if theme.unicode { "(★)" } else { "(*)" }
    } else if agent.is_idle() {
        "(zzz)"
    } else if walking {
        if theme.unicode { "(→)" } else { "(>)" }
    } else if agent.is_focused() {
        "(///)"
    } else {
        "( ... )"
    };
    if y > 0 {
        let bubble_width = bubble.width();
        let bubble_x = x.min(max_x.saturating_sub(bubble_width));
        paint_text(
            canvas,
            bubble_x,
            y - 1,
            bubble_width,
            bubble,
            if selected { base } else { theme.muted_style() },
        );
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
        let y = first.y.saturating_sub(2).max(rect.inner_y());
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
        let mut cid = agent("10000003", "Cid", "qa", 0, &["20000001"]);
        cid.tasks.clear();
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
            "Portal",
            "★",
            "▤",
            "Selected:",
            "read-only",
        ] {
            assert!(screen.contains(text), "{text}: {screen}");
        }
        assert!(screen.contains("░"), "{screen}");
        assert!(screen.contains("☕"), "{screen}");
        assert!(screen.contains("♣"), "{screen}");
        assert!(!screen.contains("Reset password"), "{screen}");
        assert!(!screen.contains("Audit log"), "{screen}");
        assert!(!screen.contains("TECH LEAD"), "{screen}");
        assert!(screen.contains("Bea"), "{screen}");
        assert!(screen.contains("Cid"), "{screen}");
        assert!(screen.contains("(zzz)"), "{screen}");
    }

    #[test]
    fn the_floor_works_at_the_minimum_size_and_without_unicode() {
        for (width, height) in [(60, 20), (80, 24), (100, 30)] {
            let mut view = FloorView::new(floor());
            let screen = scene(&mut view, width, height, theme(true));
            for text in ["PROJECT LAB", "WAR ROOM", "LOUNGE", "Portal"] {
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
        let layout = layout(&canvas).expect("layout");
        let rooms = layout.rooms;
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
        let first = position_at(agent, &spots, 0, &layout);
        let same = position_at(agent, &spots, 0, &layout);
        let later = position_at(agent, &spots, 30, &layout);
        assert_eq!(first, same);
        assert_ne!(first, later);
        // The path through the corridor never leaves the canvas.
        for tick in 0..60 {
            let (x, y, _) = position_at(agent, &spots, tick, &layout);
            assert!(x < canvas.width && y < canvas.height, "{tick}: {x},{y}");
        }
    }

    #[test]
    fn exchanges_are_drawn_for_two_different_roles_in_a_room() {
        let mut canvas = Canvas::new(60, 24);
        let layout = layout(&canvas).expect("layout");
        let rooms = layout.rooms;
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
        assert!(screen.contains("no active project"), "{screen}");
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
