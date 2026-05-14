use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

pub const WRAP_WIDTH: usize = 80;

pub fn highlight_lines(text: &str, accent: Color) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    let mut in_code_block = false;

    for line in text.lines() {
        if line.starts_with("```") {
            in_code_block = !in_code_block;
            lines.push(Line::from(vec![Span::styled(
                line.to_string(),
                Style::default().fg(Color::DarkGray),
            )]));
            continue;
        }

        if in_code_block {
            lines.push(Line::from(vec![Span::styled(
                format!("  {line}"),
                Style::default().fg(Color::Green),
            )]));
            continue;
        }

        let highlighted = highlight_line(line, accent);
        let wrapped = wrap_spans(highlighted, WRAP_WIDTH);
        lines.extend(wrapped);
    }

    lines
}

pub fn wrap_spans(spans: Vec<Span<'static>>, max_width: usize) -> Vec<Line<'static>> {
    if max_width == 0 {
        return vec![Line::from(spans)];
    }

    let total_len: usize = spans.iter().map(|s| s.content.len()).sum();
    if total_len <= max_width {
        return vec![Line::from(spans)];
    }

    let mut result: Vec<Line<'static>> = Vec::new();
    let mut current_spans: Vec<Span<'static>> = Vec::new();
    let mut current_len: usize = 0;

    for span in spans {
        let style = span.style;
        let text = span.content.to_string();

        if current_len + text.len() <= max_width {
            current_len += text.len();
            current_spans.push(Span::styled(text, style));
            continue;
        }

        let mut remaining = text.as_str();
        while !remaining.is_empty() {
            let budget = max_width.saturating_sub(current_len);
            if budget == 0 {
                result.push(Line::from(std::mem::take(&mut current_spans)));
                current_len = 0;
                continue;
            }

            if remaining.len() <= budget {
                current_len += remaining.len();
                current_spans.push(Span::styled(remaining.to_string(), style));
                break;
            }

            let slice = &remaining[..budget];
            let break_at = match slice.rfind(' ') {
                Some(pos) if pos > 0 => pos,
                _ => budget,
            };

            let (chunk, rest) = remaining.split_at(break_at);
            let rest = rest.strip_prefix(' ').unwrap_or(rest);

            if !chunk.is_empty() {
                current_spans.push(Span::styled(chunk.to_string(), style));
            }
            result.push(Line::from(std::mem::take(&mut current_spans)));
            current_len = 0;
            remaining = rest;
        }
    }

    if !current_spans.is_empty() {
        result.push(Line::from(current_spans));
    }

    if result.is_empty() {
        result.push(Line::from(Vec::<Span<'static>>::new()));
    }

    result
}

pub fn highlight_line(line: &str, accent: Color) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let owned = line.to_string();

    if owned.starts_with("```") {
        spans.push(Span::styled(owned, Style::default().fg(Color::DarkGray)));
        return spans;
    }

    if let Some(stripped) = owned.strip_prefix("# ") {
        spans.push(Span::styled("# ".to_string(), Style::default().fg(Color::DarkGray)));
        spans.push(Span::styled(
            stripped.to_string(),
            Style::default().fg(accent).add_modifier(Modifier::BOLD),
        ));
        return spans;
    }
    for prefix in &["## ", "### ", "#### ", "##### ", "###### "] {
        if let Some(stripped) = owned.strip_prefix(prefix) {
            spans.push(Span::styled(prefix.to_string(), Style::default().fg(Color::DarkGray)));
            spans.push(Span::styled(
                stripped.to_string(),
                Style::default().fg(accent).add_modifier(Modifier::BOLD),
            ));
            return spans;
        }
    }

    if owned.starts_with("> ") {
        spans.push(Span::styled("> ".to_string(), Style::default().fg(Color::Yellow)));
        spans.push(Span::styled(
            owned[2..].to_string(),
            Style::default().fg(Color::Yellow).add_modifier(Modifier::ITALIC),
        ));
        return spans;
    }

    let trimmed = owned.trim_start();
    let indent = owned.len() - trimmed.len();
    let indent_str = " ".repeat(indent);

    if trimmed.starts_with("- ") || trimmed.starts_with("* ") {
        spans.push(Span::raw(indent_str));
        spans.push(Span::styled(trimmed[..2].to_string(), Style::default().fg(accent)));
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

fn highlight_inline(text: &str, spans: &mut Vec<Span<'static>>) {
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    let mut buf = String::new();

    while i < chars.len() {
        if i + 1 < chars.len() && chars[i] == '*' && chars[i + 1] == '*' {
            if let Some(end) = find_closing(&chars, i + 2, &['*', '*']) {
                if !buf.is_empty() {
                    spans.push(Span::raw(std::mem::take(&mut buf)));
                }
                spans.push(Span::styled("**".to_string(), Style::default().fg(Color::DarkGray)));
                let content: String = chars[i + 2..end].iter().collect();
                spans.push(Span::styled(content, Style::default().add_modifier(Modifier::BOLD)));
                spans.push(Span::styled("**".to_string(), Style::default().fg(Color::DarkGray)));
                i = end + 2;
                continue;
            }
        }

        if i + 1 < chars.len() && chars[i] == '~' && chars[i + 1] == '~' {
            if let Some(end) = find_closing(&chars, i + 2, &['~', '~']) {
                if !buf.is_empty() {
                    spans.push(Span::raw(std::mem::take(&mut buf)));
                }
                spans.push(Span::styled("~~".to_string(), Style::default().fg(Color::DarkGray)));
                let content: String = chars[i + 2..end].iter().collect();
                spans.push(Span::styled(
                    content,
                    Style::default().add_modifier(Modifier::CROSSED_OUT),
                ));
                spans.push(Span::styled("~~".to_string(), Style::default().fg(Color::DarkGray)));
                i = end + 2;
                continue;
            }
        }

        if chars[i] == '*' && (i + 1 >= chars.len() || chars[i + 1] != '*') {
            if let Some(end) = find_closing_single(&chars, i + 1, '*') {
                if !buf.is_empty() {
                    spans.push(Span::raw(std::mem::take(&mut buf)));
                }
                spans.push(Span::styled("*".to_string(), Style::default().fg(Color::DarkGray)));
                let content: String = chars[i + 1..end].iter().collect();
                spans.push(Span::styled(
                    content,
                    Style::default().add_modifier(Modifier::ITALIC),
                ));
                spans.push(Span::styled("*".to_string(), Style::default().fg(Color::DarkGray)));
                i = end + 1;
                continue;
            }
        }

        if chars[i] == '`' {
            if let Some(end) = find_closing_single(&chars, i + 1, '`') {
                if !buf.is_empty() {
                    spans.push(Span::raw(std::mem::take(&mut buf)));
                }
                spans.push(Span::styled("`".to_string(), Style::default().fg(Color::DarkGray)));
                let content: String = chars[i + 1..end].iter().collect();
                spans.push(Span::styled(
                    content,
                    Style::default().fg(Color::Green).bg(Color::Rgb(40, 40, 40)),
                ));
                spans.push(Span::styled("`".to_string(), Style::default().fg(Color::DarkGray)));
                i = end + 1;
                continue;
            }
        }

        buf.push(chars[i]);
        i += 1;
    }

    if !buf.is_empty() {
        spans.push(Span::raw(buf));
    }
}

fn find_closing(chars: &[char], start: usize, pattern: &[char]) -> Option<usize> {
    if pattern.len() != 2 {
        return None;
    }
    for i in start..chars.len().saturating_sub(1) {
        if chars[i] == pattern[0] && chars[i + 1] == pattern[1] {
            return Some(i);
        }
    }
    None
}

fn find_closing_single(chars: &[char], start: usize, ch: char) -> Option<usize> {
    for i in start..chars.len() {
        if chars[i] == ch {
            return Some(i);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_highlight_lines_code_block() {
        let lines = highlight_lines("```\nfn main() {}\n```", Color::Cyan);
        assert_eq!(lines.len(), 3);
    }

    #[test]
    fn test_highlight_lines_heading() {
        let lines = highlight_lines("# Hello", Color::Cyan);
        assert!(!lines.is_empty());
    }

    #[test]
    fn test_highlight_line_bold() {
        let spans = highlight_line("this is **bold** text", Color::Cyan);
        assert!(!spans.is_empty());
    }

    #[test]
    fn test_wrap_spans_no_wrap_needed() {
        let spans = vec![Span::raw("short text")];
        let lines = wrap_spans(spans, 80);
        assert_eq!(lines.len(), 1);
    }

    #[test]
    fn test_wrap_spans_wraps_at_boundary() {
        let text = "word ".repeat(20); // 100 chars
        let spans = vec![Span::raw(text)];
        let lines = wrap_spans(spans, 80);
        assert!(lines.len() >= 2);
        for line in &lines {
            let len: usize = line.spans.iter().map(|s| s.content.len()).sum();
            assert!(len <= 80, "line length {len} exceeds 80");
        }
    }

    #[test]
    fn test_wrap_spans_preserves_style() {
        let text = "word ".repeat(20);
        let style = Style::default().fg(Color::Red);
        let spans = vec![Span::styled(text, style)];
        let lines = wrap_spans(spans, 80);
        assert!(lines.len() >= 2);
        for line in &lines {
            for span in &line.spans {
                assert_eq!(span.style.fg, Some(Color::Red));
            }
        }
    }

    #[test]
    fn test_wrap_spans_zero_width() {
        let spans = vec![Span::raw("hello")];
        let lines = wrap_spans(spans, 0);
        assert_eq!(lines.len(), 1);
    }

    #[test]
    fn test_highlight_lines_wraps_long_paragraph() {
        let text = "word ".repeat(30); // 150 chars paragraph
        let lines = highlight_lines(&text, Color::Cyan);
        assert!(lines.len() >= 2);
    }

}
