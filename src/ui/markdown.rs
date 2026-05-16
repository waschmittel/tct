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

        let trimmed = line.trim_start();
        let base_indent = line.len() - trimmed.len();
        let mut list_indent = 0;

        if trimmed.starts_with("- ") || trimmed.starts_with("* ") {
            list_indent = base_indent + 2;
        } else if let Some(dot_pos) = trimmed.find(". ") {
            let num_part = &trimmed[..dot_pos];
            if num_part.parse::<u64>().is_ok() {
                list_indent = base_indent + dot_pos + 2;
            }
        }

        let highlighted = highlight_line(line, accent);
        let wrapped = if list_indent > 0 {
            wrap_spans_with_indent(highlighted, WRAP_WIDTH, list_indent)
        } else {
            wrap_spans(highlighted, WRAP_WIDTH)
        };
        lines.extend(wrapped);
    }

    lines
}

pub fn wrap_spans(spans: Vec<Span<'static>>, max_width: usize) -> Vec<Line<'static>> {
    wrap_spans_with_indent(spans, max_width, 0)
}

pub fn wrap_spans_with_indent(
    spans: Vec<Span<'static>>,
    max_width: usize,
    indent: usize,
) -> Vec<Line<'static>> {
    if max_width == 0 {
        return vec![Line::from(spans)];
    }

    let mut result: Vec<Line<'static>> = Vec::new();
    let mut current_spans: Vec<Span<'static>> = Vec::new();
    let mut current_width: usize = 0;
    let mut is_first_line = true;

    for span in spans {
        let style = span.style;
        let text = span.content.to_string();
        let span_width = text.chars().count();

        let effective_width = if is_first_line {
            max_width
        } else {
            max_width.saturating_sub(indent)
        };

        if current_width + span_width <= effective_width {
            current_width += span_width;
            current_spans.push(Span::styled(text, style));
            continue;
        }

        let mut remaining = text.as_str();
        while !remaining.is_empty() {
            let budget = effective_width.saturating_sub(current_width);
            if budget == 0 {
                result.push(Line::from(std::mem::take(&mut current_spans)));
                is_first_line = false;
                current_width = 0;
                // Re-calculate effective_width for the new line
                let new_effective_width = max_width.saturating_sub(indent);
                if new_effective_width == 0 {
                    // Cannot wrap further
                    current_spans.push(Span::styled(remaining.to_string(), style));
                    break;
                }
                continue;
            }

            let rem_width = remaining.chars().count();
            if rem_width <= budget {
                current_width += rem_width;
                current_spans.push(Span::styled(remaining.to_string(), style));
                break;
            }

            // Find the character boundary for the budget
            let mut split_idx = 0;
            let mut char_count = 0;
            for (idx, c) in remaining.char_indices() {
                if char_count >= budget {
                    break;
                }
                split_idx = idx + c.len_utf8();
                char_count += 1;
            }

            let slice = &remaining[..split_idx];
            let break_at = match slice.rfind(' ') {
                Some(pos) if pos > 0 => pos,
                _ => split_idx,
            };

            let (chunk, rest) = remaining.split_at(break_at);
            let rest = rest.strip_prefix(' ').unwrap_or(rest);

            if !chunk.is_empty() {
                current_spans.push(Span::styled(chunk.to_string(), style));
            }
            result.push(Line::from(std::mem::take(&mut current_spans)));
            is_first_line = false;
            current_width = 0;
            remaining = rest;
        }
    }

    if !current_spans.is_empty() {
        result.push(Line::from(current_spans));
    }

    if result.is_empty() {
        result.push(Line::from(Vec::<Span<'static>>::new()));
    }

    // Apply indentation to all lines except the first one
    if indent > 0 && result.len() > 1 {
        let indent_str = " ".repeat(indent);
        for line in result.iter_mut().skip(1) {
            line.spans.insert(0, Span::raw(indent_str.clone()));
        }
    }

    result
}

/// Visual line map entry: (source_row, source_col_offset, visual_line_len, visual_indent).
pub fn build_visual_map(lines: &[String], accent: Color, wrap_width: usize) -> Vec<(usize, usize, usize, usize)> {
    let mut map = Vec::new();
    for (li, line_text) in lines.iter().enumerate() {
        let trimmed = line_text.trim_start();
        let base_indent = line_text.len() - trimmed.len();
        let mut list_indent = 0;

        if trimmed.starts_with("- ") || trimmed.starts_with("* ") {
            list_indent = base_indent + 2;
        } else if let Some(dot_pos) = trimmed.find(". ") {
            let num_part = &trimmed[..dot_pos];
            if num_part.parse::<u64>().is_ok() {
                list_indent = base_indent + dot_pos + 2;
            }
        }

        let highlighted = highlight_line(line_text, accent);
        let wrapped = if list_indent > 0 {
            wrap_spans_with_indent(highlighted, wrap_width, list_indent)
        } else {
            wrap_spans(highlighted, wrap_width)
        };
        
        let line_chars: Vec<char> = line_text.chars().collect();
        let mut source_char_offset = 0;
        
        for (wi, wl) in wrapped.iter().enumerate() {
            let display_v_char_len: usize = wl.spans.iter().map(|s| s.content.chars().count()).sum();
            
            let mut actual_source_len = display_v_char_len;
            let mut current_v_indent = 0;
            
            if wi > 0 && list_indent > 0 {
                actual_source_len = display_v_char_len.saturating_sub(list_indent);
                current_v_indent = list_indent;
            }

            map.push((li, source_char_offset, display_v_char_len, current_v_indent));
            
            let gap = if source_char_offset + actual_source_len < line_chars.len()
                && line_chars[source_char_offset + actual_source_len] == ' '
            {
                1
            } else {
                0
            };
            source_char_offset += actual_source_len + gap;
        }
    }
    map
}

/// Find visual row and column for a source cursor position.
pub fn source_to_visual(
    visual_map: &[(usize, usize, usize, usize)],
    cursor_row: usize,
    cursor_col: usize,
) -> (usize, usize) {
    for (vi, &(src_row, src_offset, vlen, vindent)) in visual_map.iter().enumerate() {
        if src_row == cursor_row {
            let actual_src_len = vlen.saturating_sub(vindent);
            let col_in_segment = cursor_col.saturating_sub(src_offset);
            
            if col_in_segment <= actual_src_len
                || vi + 1 >= visual_map.len()
                || visual_map[vi + 1].0 != cursor_row
            {
                return (vi, col_in_segment.min(actual_src_len) + vindent);
            }
        }
    }
    (0, cursor_col)
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

    #[test]
    fn test_visual_map_single_line_no_wrap() {
        let lines = vec!["short line".to_string()];
        let map = build_visual_map(&lines, Color::Cyan, 80);
        assert_eq!(map.len(), 1);
        assert_eq!(map[0], (0, 0, 10, 0));
    }

    #[test]
    fn test_visual_map_wrapped_line() {
        // 100 chars, wraps at 80
        let text = "word ".repeat(20);
        let lines = vec![text.trim_end().to_string()];
        let map = build_visual_map(&lines, Color::Cyan, 20);
        assert!(map.len() >= 2, "expected wrapping, got {} entries", map.len());
        // All entries should reference source row 0
        for entry in &map {
            assert_eq!(entry.0, 0);
        }
        // Source offsets should be strictly increasing
        for w in map.windows(2) {
            assert!(w[1].1 > w[0].1);
        }
    }

    #[test]
    fn test_source_to_visual_no_wrap() {
        let lines = vec!["hello world".to_string()];
        let map = build_visual_map(&lines, Color::Cyan, 80);
        let (vrow, vcol) = source_to_visual(&map, 0, 5);
        assert_eq!(vrow, 0);
        assert_eq!(vcol, 5);
    }

    #[test]
    fn test_source_to_visual_after_wrap() {
        // "aaaa bbbb cccc dddd" wraps at 10 → "aaaa bbbb" (9) + "cccc dddd" (9)
        let lines = vec!["aaaa bbbb cccc dddd".to_string()];
        let map = build_visual_map(&lines, Color::Cyan, 10);
        assert_eq!(map.len(), 2);
        // Source col 0 → visual row 0, col 0
        assert_eq!(source_to_visual(&map, 0, 0), (0, 0));
        // Source col 10 ('c') → visual row 1, col 0
        let (vrow, vcol) = source_to_visual(&map, 0, 10);
        assert_eq!(vrow, 1);
        assert_eq!(vcol, 0);
        // Source col 14 ('d') → visual row 1, col 4
        let (vrow, vcol) = source_to_visual(&map, 0, 14);
        assert_eq!(vrow, 1);
        assert_eq!(vcol, 4);
    }

    #[test]
    fn test_source_to_visual_multiline() {
        let lines = vec!["short".to_string(), "also short".to_string()];
        let map = build_visual_map(&lines, Color::Cyan, 80);
        assert_eq!(map.len(), 2);
        assert_eq!(source_to_visual(&map, 1, 5), (1, 5));
    }

    #[test]
    fn test_wrap_spans_content_matches_source() {
        let source = "hello world foo bar baz qux quux corge grault";
        let spans = vec![Span::raw(source.to_string())];
        let lines = wrap_spans(spans, 20);
        // Concatenating all visual line content + one space per break should reconstruct source
        let mut reconstructed = String::new();
        for (i, line) in lines.iter().enumerate() {
            if i > 0 {
                reconstructed.push(' ');
            }
            for span in &line.spans {
                reconstructed.push_str(&span.content);
            }
        }
        assert_eq!(reconstructed, source);
    }

    #[test]
    fn test_wrap_spans_umlaut_panic() {
        let span = Span::raw("äöü"); // 6 bytes
        let spans = vec![span];
        // If we wrap at 1, it might try to slice at byte 1, which is middle of 'ä'.
        wrap_spans(spans, 1);
    }

    #[test]
    fn test_list_indentation_wrapping() {
        let text = "- This is a very long list item that should be wrapped and indented correctly.";
        let accent = Color::Cyan;
        
        // Test unsorted list
        let highlighted = highlight_line(text, accent);
        let wrapped = wrap_spans_with_indent(highlighted, 20, 2);
        
        assert!(wrapped.len() > 1);
        // Second line should start with 2 spaces
        assert!(wrapped[1].spans[0].content.starts_with("  "));

        // Test numbered list
        let text2 = "123. This is another long list item.";
        let highlighted2 = highlight_line(text2, accent);
        let wrapped2 = wrap_spans_with_indent(highlighted2, 10, 5);
        assert!(wrapped2.len() > 1);
        assert!(wrapped2[1].spans[0].content.starts_with("     "));
    }
}
