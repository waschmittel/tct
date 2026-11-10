use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

use crate::alert::Alert;
use crate::insert::table::is_table_line;

mod inline;
mod quote;
mod table;
#[cfg(test)]
mod tests;
mod wrap;

use inline::*;
pub(crate) use quote::*;
pub use table::widest_table_line;
pub(crate) use table::*;
pub(super) use wrap::wrap_spans_with_indent;
use wrap::*;

pub const WRAP_WIDTH: usize = 80;

/// Renders markdown text into styled, word-wrapped visual lines.
///
/// `MarkdownRenderer` is the single public entry point for turning raw
/// markdown source into ratatui `Line<'static>` values. List indentation
/// is detected automatically per source line; callers don't pass it in.
pub struct MarkdownRenderer<'a> {
    text: &'a str,
    width: usize,
    accent: Color,
}

/// Result of rendering markdown: the styled, word-wrapped visual lines.
pub struct Rendered {
    pub lines: Vec<Line<'static>>,
}

impl<'a> MarkdownRenderer<'a> {
    /// Build a renderer over raw markdown text. Source lines are split with
    /// `str::lines()`, which drops a trailing empty entry for a trailing '\n'
    /// — matching the existing description-text behavior.
    pub fn new(text: &'a str, width: usize, accent: Color) -> Self {
        Self {
            text,
            width,
            accent,
        }
    }

    pub fn render(&self) -> Rendered {
        let refs: Vec<&str> = self.text.lines().collect();
        render_source_lines(&refs, self.width, self.accent)
    }
}

fn render_source_lines(source_lines: &[&str], width: usize, accent: Color) -> Rendered {
    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut in_code_block = false;
    let mut in_quote = false;
    let mut alert: Option<Alert> = None;
    let mut prev_cite = false;

    let mut idx = 0;
    while idx < source_lines.len() {
        let line_text = source_lines[idx];
        idx += 1;
        // Code fence / code block lines: no inline highlighting, no wrapping.
        if line_text.starts_with("```") {
            in_code_block = !in_code_block;
            let span = Span::styled(line_text.to_string(), Style::default().fg(Color::DarkGray));
            lines.push(Line::from(vec![span]));
            continue;
        }
        if in_code_block {
            lines.push(Line::from(vec![Span::styled(
                format!("  {line_text}"),
                Style::default().fg(Color::Green),
            )]));
            continue;
        }

        if is_quote_fence(line_text) && (!in_quote || closes_quote(line_text)) {
            let opening = !in_quote;
            if opening {
                alert = fence_alert(line_text);
            }
            let color = alert.map_or(Color::Yellow, Alert::tui_color);
            let trimmed = line_text.trim_start();
            match alert.filter(|_| opening) {
                Some(a) => {
                    let tag = trimmed[3..].trim_start();
                    let lead = line_text.len() - tag.len();
                    lines.push(Line::from(vec![
                        Span::styled(line_text[..lead].to_string(), Style::default().fg(color)),
                        alert_header(a, tag, color),
                    ]));
                }
                None => lines.push(Line::from(Span::styled(
                    line_text.to_string(),
                    Style::default().fg(color),
                ))),
            }
            in_quote = opening;
            if !opening {
                alert = None;
            }
            prev_cite = false;
            continue;
        }
        if in_quote {
            let color = alert.map_or(Color::Yellow, Alert::tui_color);
            let style = Style::default().fg(color).add_modifier(Modifier::ITALIC);
            lines.extend(wrap_spans(
                vec![Span::styled(line_text.to_string(), style)],
                width,
            ));
            continue;
        }

        let trimmed = line_text.trim_start();
        if let Some(marker_len) = citation_marker_len(trimmed) {
            let first = !prev_cite;
            if first {
                alert = quote_alert(line_text);
            }
            prev_cite = true;
            if let Some(a) = alert {
                let color = a.tui_color();
                let indent = line_text.len() - trimmed.len();
                let (marker, rest) = trimmed.split_at(marker_len);
                let mut spans = vec![Span::raw(" ".repeat(indent))];
                spans.push(Span::styled(marker.to_string(), Style::default().fg(color)));
                spans.push(if first {
                    alert_header(a, rest, color)
                } else {
                    Span::styled(
                        rest.to_string(),
                        Style::default().fg(color).add_modifier(Modifier::ITALIC),
                    )
                });
                lines.extend(wrap_spans_with_indent(spans, width, indent + marker_len));
                continue;
            }
        } else {
            prev_cite = false;
            alert = None;
        }

        if is_table_line(line_text)
            && let Some(len) = table_block_len(&source_lines[idx - 1..])
        {
            lines.extend(render_table(&source_lines[idx - 1..idx - 1 + len], accent));
            idx += len - 1;
            continue;
        }

        let list_indent = detect_list_indent(line_text);
        let highlighted = highlight_line(line_text, accent);
        let wrapped = if list_indent > 0 {
            wrap_spans_with_indent(highlighted, width, list_indent)
        } else {
            wrap_spans(highlighted, width)
        };

        lines.extend(wrapped);
    }

    Rendered { lines }
}

impl Rendered {
    pub fn lines(&self) -> &[Line<'static>] {
        &self.lines
    }
}
