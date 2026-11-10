use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;

use super::*;

pub(super) fn highlight_line(line: &str, accent: Color) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let owned = line.to_string();

    if owned.starts_with("```") {
        spans.push(Span::styled(owned, Style::default().fg(Color::DarkGray)));
        return spans;
    }

    if let Some((_, marker, _)) = crate::footnote::def_start(&owned) {
        let head: String = owned.chars().take(marker).collect();
        let rest: String = owned.chars().skip(marker).collect();
        spans.push(Span::styled(head, footnote_style()));
        highlight_inline(&rest, &mut spans);
        return spans;
    }

    if let Some(stripped) = owned.strip_prefix("# ") {
        spans.push(Span::styled(
            "# ".to_string(),
            Style::default().fg(Color::DarkGray),
        ));
        spans.push(Span::styled(
            stripped.to_string(),
            Style::default().fg(accent).add_modifier(Modifier::BOLD),
        ));
        return spans;
    }
    for prefix in &["## ", "### ", "#### ", "##### ", "###### "] {
        if let Some(stripped) = owned.strip_prefix(prefix) {
            spans.push(Span::styled(
                prefix.to_string(),
                Style::default().fg(Color::DarkGray),
            ));
            spans.push(Span::styled(
                stripped.to_string(),
                Style::default().fg(accent).add_modifier(Modifier::BOLD),
            ));
            return spans;
        }
    }

    let trimmed = owned.trim_start();
    let indent = owned.len() - trimmed.len();
    let indent_str = " ".repeat(indent);

    if let Some(marker_len) = citation_marker_len(trimmed) {
        if indent > 0 {
            spans.push(Span::raw(indent_str));
        }
        spans.push(Span::styled(
            trimmed[..marker_len].to_string(),
            Style::default().fg(Color::Yellow),
        ));
        spans.push(Span::styled(
            trimmed[marker_len..].to_string(),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::ITALIC),
        ));
        return spans;
    }

    if trimmed.starts_with("- ") || trimmed.starts_with("* ") {
        spans.push(Span::raw(indent_str));
        spans.push(Span::styled(
            trimmed[..2].to_string(),
            Style::default().fg(accent),
        ));
        highlight_inline(&trimmed[2..], &mut spans);
        return spans;
    }

    if let Some(dot_pos) = trimmed.find(". ") {
        let num_part = &trimmed[..dot_pos];
        if num_part.parse::<u64>().is_ok() {
            spans.push(Span::raw(indent_str));
            spans.push(Span::styled(
                trimmed[..dot_pos + 2].to_string(),
                Style::default().fg(accent),
            ));
            highlight_inline(&trimmed[dot_pos + 2..], &mut spans);
            return spans;
        }
    }

    highlight_inline(&owned, &mut spans);
    spans
}

/// Footnote references and definition labels; the text itself stays as typed.
pub(super) fn footnote_style() -> Style {
    Style::default().fg(Color::Magenta)
}

pub(super) fn highlight_inline(text: &str, spans: &mut Vec<Span<'static>>) {
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    let mut buf = String::new();

    while i < chars.len() {
        if chars[i] == '['
            && let Some((len, _)) = crate::footnote::ref_at(&chars[i..])
        {
            if !buf.is_empty() {
                spans.push(Span::raw(std::mem::take(&mut buf)));
            }
            spans.push(Span::styled(
                chars[i..i + len].iter().collect::<String>(),
                footnote_style(),
            ));
            i += len;
            continue;
        }

        if i + 2 < chars.len()
            && chars[i..i + 3] == ['*', '*', '*']
            && let Some(end) = find_closing_triple(&chars, i + 3)
        {
            if !buf.is_empty() {
                spans.push(Span::raw(std::mem::take(&mut buf)));
            }
            spans.push(Span::styled(
                "***".to_string(),
                Style::default().fg(Color::DarkGray),
            ));
            let content: String = chars[i + 3..end].iter().collect();
            spans.push(Span::styled(
                content,
                Style::default().add_modifier(Modifier::BOLD | Modifier::ITALIC),
            ));
            spans.push(Span::styled(
                "***".to_string(),
                Style::default().fg(Color::DarkGray),
            ));
            i = end + 3;
            continue;
        }

        if i + 1 < chars.len()
            && chars[i] == '*'
            && chars[i + 1] == '*'
            && let Some(end) = find_closing(&chars, i + 2, &['*', '*'])
        {
            if !buf.is_empty() {
                spans.push(Span::raw(std::mem::take(&mut buf)));
            }
            spans.push(Span::styled(
                "**".to_string(),
                Style::default().fg(Color::DarkGray),
            ));
            let content: String = chars[i + 2..end].iter().collect();
            spans.push(Span::styled(
                content,
                Style::default().add_modifier(Modifier::BOLD),
            ));
            spans.push(Span::styled(
                "**".to_string(),
                Style::default().fg(Color::DarkGray),
            ));
            i = end + 2;
            continue;
        }

        if i + 1 < chars.len()
            && chars[i] == '~'
            && chars[i + 1] == '~'
            && let Some(end) = find_closing(&chars, i + 2, &['~', '~'])
        {
            if !buf.is_empty() {
                spans.push(Span::raw(std::mem::take(&mut buf)));
            }
            spans.push(Span::styled(
                "~~".to_string(),
                Style::default().fg(Color::DarkGray),
            ));
            let content: String = chars[i + 2..end].iter().collect();
            spans.push(Span::styled(
                content,
                Style::default().add_modifier(Modifier::CROSSED_OUT),
            ));
            spans.push(Span::styled(
                "~~".to_string(),
                Style::default().fg(Color::DarkGray),
            ));
            i = end + 2;
            continue;
        }

        if chars[i] == '*'
            && (i + 1 >= chars.len() || chars[i + 1] != '*')
            && let Some(end) = find_closing_single(&chars, i + 1, '*')
        {
            if !buf.is_empty() {
                spans.push(Span::raw(std::mem::take(&mut buf)));
            }
            spans.push(Span::styled(
                "*".to_string(),
                Style::default().fg(Color::DarkGray),
            ));
            let content: String = chars[i + 1..end].iter().collect();
            spans.push(Span::styled(
                content,
                Style::default().add_modifier(Modifier::ITALIC),
            ));
            spans.push(Span::styled(
                "*".to_string(),
                Style::default().fg(Color::DarkGray),
            ));
            i = end + 1;
            continue;
        }

        if chars[i] == '`'
            && let Some(end) = find_closing_single(&chars, i + 1, '`')
        {
            if !buf.is_empty() {
                spans.push(Span::raw(std::mem::take(&mut buf)));
            }
            spans.push(Span::styled(
                "`".to_string(),
                Style::default().fg(Color::DarkGray),
            ));
            let content: String = chars[i + 1..end].iter().collect();
            spans.push(Span::styled(
                content,
                Style::default().fg(Color::Green).bg(Color::Rgb(40, 40, 40)),
            ));
            spans.push(Span::styled(
                "`".to_string(),
                Style::default().fg(Color::DarkGray),
            ));
            i = end + 1;
            continue;
        }

        buf.push(chars[i]);
        i += 1;
    }

    if !buf.is_empty() {
        spans.push(Span::raw(buf));
    }
}

pub(super) fn find_closing(chars: &[char], start: usize, pattern: &[char]) -> Option<usize> {
    if pattern.len() != 2 {
        return None;
    }
    (start..chars.len().saturating_sub(1))
        .find(|&i| chars[i] == pattern[0] && chars[i + 1] == pattern[1])
}

pub(super) fn find_closing_triple(chars: &[char], start: usize) -> Option<usize> {
    (start + 1..chars.len().saturating_sub(2)).find(|&i| chars[i..i + 3] == ['*', '*', '*'])
}

pub(super) fn find_closing_single(chars: &[char], start: usize, ch: char) -> Option<usize> {
    chars
        .iter()
        .enumerate()
        .skip(start)
        .find(|(_, c)| **c == ch)
        .map(|(i, _)| i)
}
