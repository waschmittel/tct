use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use unicode_width::UnicodeWidthStr;

pub fn highlight_lines(text: &str) -> Vec<Line<'static>> {
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

        lines.push(Line::from(highlight_line(line)));
    }

    lines
}

pub fn highlight_line(line: &str) -> Vec<Span<'static>> {
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
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        ));
        return spans;
    }
    for prefix in &["## ", "### ", "#### ", "##### ", "###### "] {
        if let Some(stripped) = owned.strip_prefix(prefix) {
            spans.push(Span::styled(prefix.to_string(), Style::default().fg(Color::DarkGray)));
            spans.push(Span::styled(
                stripped.to_string(),
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
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
        spans.push(Span::styled(trimmed[..2].to_string(), Style::default().fg(Color::Cyan)));
        highlight_inline(&trimmed[2..], &mut spans);
        return spans;
    }

    if let Some(dot_pos) = trimmed.find(". ") {
        let num_part = &trimmed[..dot_pos];
        if num_part.parse::<u64>().is_ok() {
            spans.push(Span::raw(indent_str));
            spans.push(Span::styled(
                trimmed[..dot_pos + 2].to_string(),
                Style::default().fg(Color::Cyan),
            ));
            highlight_inline(&trimmed[dot_pos + 2..], &mut spans);
            return spans;
        }
    }

    if trimmed.starts_with('|') && trimmed.ends_with('|') {
        if trimmed.chars().all(|c| c == '|' || c == '-' || c == ':' || c == ' ') {
            spans.push(Span::styled(owned.clone(), Style::default().fg(Color::DarkGray)));
        } else {
            for part in owned.split('|') {
                if part.is_empty() {
                    spans.push(Span::styled("|".to_string(), Style::default().fg(Color::DarkGray)));
                } else {
                    spans.push(Span::raw(part.to_string()));
                }
            }
            spans.push(Span::styled("|".to_string(), Style::default().fg(Color::DarkGray)));
        }
        return spans;
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

pub fn format_tables(text: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let mut result = Vec::new();
    let mut i = 0;

    while i < lines.len() {
        if lines[i].contains('|') && lines[i].trim().starts_with('|') {
            let start = i;
            while i < lines.len() && lines[i].contains('|') && lines[i].trim().starts_with('|') {
                i += 1;
            }
            let table_lines = &lines[start..i];
            let formatted = format_table_block(table_lines);
            result.extend(formatted);
        } else {
            result.push(lines[i].to_string());
            i += 1;
        }
    }

    result.join("\n")
}

fn format_table_block(lines: &[&str]) -> Vec<String> {
    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut separator_idx = None;

    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim().trim_matches('|');
        let cells: Vec<String> = trimmed.split('|').map(|c| c.trim().to_string()).collect();
        if cells.iter().all(|c| c.chars().all(|ch| ch == '-' || ch == ':' || ch == ' ')) && !cells.is_empty() {
            separator_idx = Some(i);
            rows.push(cells);
        } else {
            rows.push(cells);
        }
    }

    if rows.is_empty() {
        return lines.iter().map(|l| l.to_string()).collect();
    }

    let num_cols = rows.iter().map(|r| r.len()).max().unwrap_or(0);
    let mut col_widths = vec![3usize; num_cols];
    for (ri, row) in rows.iter().enumerate() {
        if Some(ri) == separator_idx {
            continue;
        }
        for (ci, cell) in row.iter().enumerate() {
            col_widths[ci] = col_widths[ci].max(UnicodeWidthStr::width(cell.as_str()));
        }
    }

    let mut out = Vec::new();
    for (ri, row) in rows.iter().enumerate() {
        if Some(ri) == separator_idx {
            let sep: Vec<String> = col_widths.iter().map(|&w| "-".repeat(w)).collect();
            out.push(format!("| {} |", sep.join(" | ")));
        } else {
            let cells: Vec<String> = (0..num_cols)
                .map(|ci| {
                    let cell = row.get(ci).map(|s| s.as_str()).unwrap_or("");
                    format!("{:<width$}", cell, width = col_widths[ci])
                })
                .collect();
            out.push(format!("| {} |", cells.join(" | ")));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_highlight_lines_code_block() {
        let lines = highlight_lines("```\nfn main() {}\n```");
        assert_eq!(lines.len(), 3);
    }

    #[test]
    fn test_highlight_lines_heading() {
        let lines = highlight_lines("# Hello");
        assert!(!lines.is_empty());
    }

    #[test]
    fn test_highlight_line_bold() {
        let spans = highlight_line("this is **bold** text");
        assert!(!spans.is_empty());
    }

    #[test]
    fn test_format_tables() {
        let input = "| a | b |\n| --- | --- |\n| 1 | 2 |";
        let output = format_tables(input);
        assert!(output.contains('|'));
    }
}
