use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use unicode_width::UnicodeWidthStr;

use super::*;
use crate::insert::table::{Table, delimiter_aligns, is_table_line};

/// Number of lines of the pipe table starting at the first line (header,
/// delimiter row, body rows), or `None` when no table starts there.
pub(crate) fn table_block_len(lines: &[&str]) -> Option<usize> {
    lines.get(1).filter(|l| delimiter_aligns(l).is_some())?;
    Some(lines.iter().take_while(|l| is_table_line(l)).count())
}

/// Display width of the widest pipe-table line, so the detail popup can grow to fit it.
pub fn widest_table_line<'a>(lines: impl IntoIterator<Item = &'a str>) -> usize {
    lines
        .into_iter()
        .filter(|l| is_table_line(l))
        .map(UnicodeWidthStr::width)
        .max()
        .unwrap_or(0)
}

/// A pipe table drawn with box-drawing pipes: re-aligned to its cells, never
/// wrapped, and no extra rows — the delimiter row becomes the rule under the header.
pub(super) fn render_table(block: &[&str], accent: Color) -> Vec<Line<'static>> {
    let owned: Vec<String> = block.iter().map(|l| l.to_string()).collect();
    let Some(table) = Table::parse(&owned) else {
        return block
            .iter()
            .map(|l| Line::from(highlight_line(l, accent)))
            .collect();
    };
    let matrix = table.is_matrix();
    table
        .render()
        .lines
        .iter()
        .enumerate()
        .map(|(i, line)| match i {
            1 => rule_line(line, matrix),
            _ => row_line(line, i == 0, matrix, accent),
        })
        .collect()
}

pub(super) fn border_style() -> Style {
    Style::default().fg(Color::DarkGray)
}

/// One table row. In a matrix (empty top-left cell) the first column is bold
/// like the header, and the header row has no border left of the open corner.
pub(super) fn row_line(line: &str, header: bool, matrix: bool, accent: Color) -> Line<'static> {
    let mut spans = Vec::new();
    let mut pipes = 0; // unescaped pipes seen so far; the first cell sits after pipe 1
    for span in highlight_line(line, accent) {
        let mut text = String::new();
        let mut escaped = false;
        let bold = |pipes: usize| header || (matrix && pipes == 1);
        let styled = |pipes: usize| {
            if bold(pipes) {
                span.style.add_modifier(Modifier::BOLD)
            } else {
                span.style
            }
        };
        for c in span.content.chars() {
            if c == '|' && !escaped {
                if !text.is_empty() {
                    spans.push(Span::styled(std::mem::take(&mut text), styled(pipes)));
                }
                pipes += 1;
                if matrix && header && pipes == 1 {
                    spans.push(Span::raw(" "));
                } else {
                    spans.push(Span::styled("│", border_style()));
                }
            } else {
                text.push(c);
            }
            escaped = c == '\\' && !escaped;
        }
        if !text.is_empty() {
            spans.push(Span::styled(text, styled(pipes)));
        }
    }
    Line::from(spans)
}

/// The rule under the header. In a matrix the header row has no left border,
/// so the rule opens the frame and gets a corner (`┌`) instead of a `├`.
pub(super) fn rule_line(line: &str, matrix: bool) -> Line<'static> {
    let first = line.find('|');
    let last = line.rfind('|');
    let text: String = line
        .char_indices()
        .map(|(i, c)| match c {
            '|' if Some(i) == first => {
                if matrix {
                    '┌'
                } else {
                    '├'
                }
            }
            '|' if Some(i) == last => '┤',
            '|' => '┼',
            ' ' if first.is_none_or(|f| i < f) => ' ',
            _ => '─',
        })
        .collect();
    Line::from(Span::styled(text, border_style()))
}
