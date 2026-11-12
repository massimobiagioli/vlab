//! Rendering primitives shared by every command (ADR-0005, ADR-0021).
//!
//! Output is deterministic and width-aware. Text is sanitized before layout,
//! identifiers are never truncated, long text is cut with an explicit `…`,
//! and no ANSI color is ever emitted.

use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

/// Width used when stdout is not a terminal or its size is unusable.
pub const DEFAULT_WIDTH: usize = 100;

/// The terminal's column count, or [`DEFAULT_WIDTH`].
pub fn terminal_width() -> usize {
    terminal_size::terminal_size()
        .map(|(width, _)| usize::from(width.0))
        .filter(|width| *width >= 20)
        .unwrap_or(DEFAULT_WIDTH)
}

/// Makes untrusted text inert for a terminal.
///
/// Escape sequences (CSI, OSC, and two-byte escapes) are removed, tabs become
/// spaces, newlines are kept, and every other C0/C1 control character or DEL
/// is dropped.
pub fn sanitize(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\u{1b}' => match chars.next() {
                // CSI: parameters and intermediates up to a final byte.
                Some('[') => {
                    for c in chars.by_ref() {
                        if ('\u{40}'..='\u{7e}').contains(&c) {
                            break;
                        }
                    }
                }
                // OSC: up to BEL or the string terminator ESC \.
                Some(']') => {
                    while let Some(c) = chars.next() {
                        if c == '\u{7}' {
                            break;
                        }
                        if c == '\u{1b}' && chars.peek() == Some(&'\\') {
                            chars.next();
                            break;
                        }
                    }
                }
                // Two-byte escape, or a trailing ESC.
                _ => {}
            },
            '\n' => out.push('\n'),
            '\t' => out.push(' '),
            c if c.is_control() => {}
            c => out.push(c),
        }
    }
    out
}

/// Whether a cell may be shortened during layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColumnKind {
    /// An identifier: always rendered in full, even on narrow terminals.
    Identifier,
    /// Free text: may be shortened with `…`.
    Text,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Column {
    pub header: &'static str,
    pub kind: ColumnKind,
}

impl Column {
    pub const fn identifier(header: &'static str) -> Self {
        Self {
            header,
            kind: ColumnKind::Identifier,
        }
    }

    pub const fn text(header: &'static str) -> Self {
        Self {
            header,
            kind: ColumnKind::Text,
        }
    }
}

const GAP: usize = 2;
const MIN_TEXT_WIDTH: usize = 8;

/// Renders rows as an aligned table when they fit, or as stacked
/// `label: value` blocks otherwise. Cells are sanitized and single-line.
pub fn render_table(width: usize, columns: &[Column], rows: &[Vec<String>]) -> String {
    let cells: Vec<Vec<String>> = rows
        .iter()
        .map(|row| {
            row.iter()
                .map(|cell| sanitize(cell).replace('\n', " "))
                .collect()
        })
        .collect();

    match layout(width, columns, &cells) {
        Some(widths) => table(columns, &cells, &widths),
        None => stacked(columns, &cells),
    }
}

fn layout(width: usize, columns: &[Column], rows: &[Vec<String>]) -> Option<Vec<usize>> {
    let mut widths: Vec<usize> = columns
        .iter()
        .enumerate()
        .map(|(index, column)| {
            let header = display_width(column.header);
            rows.iter()
                .filter_map(|row| row.get(index))
                .map(|cell| display_width(cell))
                .chain(std::iter::once(header))
                .max()
                .unwrap_or(header)
        })
        .collect();

    let gaps = GAP * columns.len().saturating_sub(1);
    if total_width(&widths, gaps) <= width {
        return Some(widths);
    }

    // Shrink text columns from widest to narrowest, never below a readable
    // minimum. Identifier columns keep their natural width.
    let mut text_columns: Vec<usize> = columns
        .iter()
        .enumerate()
        .filter(|(_, column)| column.kind == ColumnKind::Text)
        .map(|(index, _)| index)
        .collect();
    text_columns.sort_by_key(|index| std::cmp::Reverse(widths[*index]));

    for index in text_columns {
        let total = total_width(&widths, gaps);
        if total <= width {
            break;
        }
        let excess = total - width;
        let floor = MIN_TEXT_WIDTH.max(display_width(columns[index].header));
        let reducible = widths[index].saturating_sub(floor);
        let reduction = reducible.min(excess);
        widths[index] -= reduction;
    }

    (total_width(&widths, gaps) <= width).then_some(widths)
}

fn total_width(widths: &[usize], gaps: usize) -> usize {
    widths.iter().sum::<usize>() + gaps
}

fn table(columns: &[Column], rows: &[Vec<String>], widths: &[usize]) -> String {
    let mut out = String::new();
    push_row(
        &mut out,
        widths,
        &columns
            .iter()
            .map(|c| c.header.to_owned())
            .collect::<Vec<_>>(),
    );
    for row in rows {
        push_row(&mut out, widths, row);
    }
    out
}

fn push_row(out: &mut String, widths: &[usize], row: &[String]) {
    let mut line = String::new();
    for (index, width) in widths.iter().enumerate() {
        if index > 0 {
            line.push_str(&" ".repeat(GAP));
        }
        let cell = row.get(index).map(String::as_str).unwrap_or("");
        let cell = truncate_to(cell, *width);
        line.push_str(&cell);
        line.push_str(&" ".repeat(width.saturating_sub(display_width(&cell))));
    }
    out.push_str(line.trim_end());
    out.push('\n');
}

fn stacked(columns: &[Column], rows: &[Vec<String>]) -> String {
    let label_width = columns
        .iter()
        .map(|column| display_width(column.header))
        .max()
        .unwrap_or(0);
    let mut out = String::new();
    for (index, row) in rows.iter().enumerate() {
        if index > 0 {
            out.push('\n');
        }
        for (column, cell) in columns.iter().zip(row) {
            let padding = " ".repeat(label_width.saturating_sub(display_width(column.header)));
            out.push_str(column.header);
            out.push(':');
            out.push_str(&padding);
            out.push(' ');
            out.push_str(cell);
            out.push('\n');
        }
    }
    out
}

/// Sanitizes and shortens one cell to `width` display columns, with `…`.
pub fn truncate_cell(value: &str, width: usize) -> String {
    truncate_to(&sanitize(value).replace('\n', " "), width)
}

fn truncate_to(value: &str, width: usize) -> String {
    if display_width(value) <= width {
        return value.to_owned();
    }
    if width == 0 {
        return String::new();
    }
    let mut out = String::new();
    let mut used = 0;
    for c in value.chars() {
        let char_width = UnicodeWidthChar::width(c).unwrap_or(0);
        if used + char_width + 1 > width {
            break;
        }
        out.push(c);
        used += char_width;
    }
    out.push('…');
    out
}

fn display_width(value: &str) -> usize {
    UnicodeWidthStr::width(value)
}

/// A helpful empty state for a list command.
pub fn empty_state(message: &str, hint: &str) -> String {
    format!("{message}\nHint: {hint}\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lab_columns() -> Vec<Column> {
        vec![
            Column::identifier("ID"),
            Column::text("Name"),
            Column::text("Agents"),
            Column::text("Valid"),
            Column::text("Active"),
        ]
    }

    fn lab_rows() -> Vec<Vec<String>> {
        vec![vec![
            "a3f91c07".to_owned(),
            "Payments Platform".to_owned(),
            "3".to_owned(),
            "valid".to_owned(),
            "*".to_owned(),
        ]]
    }

    #[test]
    fn plain_text_is_unchanged() {
        assert_eq!(sanitize("Payments Platform — ü"), "Payments Platform — ü");
    }

    #[test]
    fn escape_sequences_and_controls_are_removed() {
        let cases = [
            ("a\u{1b}[31mred\u{1b}[0m", "ared"),
            ("x\u{1b}[2Jy", "xy"),
            ("t\u{1b}]0;title\u{7}z", "tz"),
            ("t\u{1b}]8;;http://e\u{1b}\\link", "tlink"),
            ("bell\u{7}\r\u{0}\u{7f}\u{9b}", "bell"),
            ("tab\there", "tab here"),
            ("line\nnext", "line\nnext"),
            ("trailing\u{1b}", "trailing"),
            ("\u{1b}c", ""),
        ];
        for (input, expected) in cases {
            assert_eq!(sanitize(input), expected, "{input:?}");
        }
    }

    #[test]
    fn narrow_output_stacks_blocks_and_keeps_identifiers_whole() {
        let out = render_table(40, &lab_columns(), &lab_rows());
        assert!(out.contains("ID:"), "{out}");
        assert!(out.contains("a3f91c07"), "{out}");
        assert!(out.contains("Payments Platform"), "{out}");
        assert!(out.contains("Valid:  valid"), "{out}");
        assert!(!out.contains('\u{1b}'));
    }

    #[test]
    fn wide_output_aligns_columns() {
        for width in [80, 120] {
            let out = render_table(width, &lab_columns(), &lab_rows());
            assert!(out.starts_with("ID        Name"), "{width}: {out}");
            assert!(
                out.contains("a3f91c07  Payments Platform"),
                "{width}: {out}"
            );
            assert!(!out.contains('\u{1b}'));
        }
    }

    #[test]
    fn long_cells_are_truncated_with_an_explicit_ellipsis() {
        let columns = vec![Column::identifier("ID"), Column::text("Name")];
        let rows = vec![vec![
            "a3f91c07".to_owned(),
            "A very long laboratory name that cannot fit".to_owned(),
        ]];
        let out = render_table(30, &columns, &rows);
        assert!(out.contains("a3f91c07"), "{out}");
        assert!(out.contains('…'), "{out}");
        assert!(out.lines().all(|line| display_width(line) <= 30), "{out}");
    }

    #[test]
    fn control_characters_in_cells_are_neutralized() {
        let columns = vec![Column::identifier("ID"), Column::text("Name")];
        let rows = vec![vec!["a3f91c07".to_owned(), "Pay\u{1b}[31mments".to_owned()]];
        let out = render_table(80, &columns, &rows);
        assert!(out.contains("Payments"), "{out}");
        assert!(!out.contains('\u{1b}'));
    }

    #[test]
    fn an_empty_state_has_a_next_action() {
        let out = empty_state(
            "No laboratories yet.",
            "Run `vlab --new-lab \"Name\"` to create one.",
        );
        assert!(out.contains("No laboratories"));
        assert!(out.contains("Hint: Run `vlab --new-lab"));
    }
}
