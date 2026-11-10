use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use unicode_width::UnicodeWidthStr;

use crate::alert::Alert;
use crate::insert::table::{Table, delimiter_aligns, is_table_line};

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
fn render_table(block: &[&str], accent: Color) -> Vec<Line<'static>> {
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

fn border_style() -> Style {
    Style::default().fg(Color::DarkGray)
}

/// One table row. In a matrix (empty top-left cell) the first column is bold
/// like the header, and the header row has no border left of the open corner.
fn row_line(line: &str, header: bool, matrix: bool, accent: Color) -> Line<'static> {
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
fn rule_line(line: &str, matrix: bool) -> Line<'static> {
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

impl Rendered {
    pub fn lines(&self) -> &[Line<'static>] {
        &self.lines
    }
}

/// Detect leading list-marker (bullet or numbered) or citation indent so wrapped
/// continuation lines line up under the first character of the item text.
pub(crate) fn detect_list_indent(line: &str) -> usize {
    let trimmed = line.trim_start();
    let base_indent = line.len() - trimmed.len();
    if trimmed.starts_with("- ") || trimmed.starts_with("* ") {
        return base_indent + 2;
    }
    if let Some(dot_pos) = trimmed.find(". ") {
        let num_part = &trimmed[..dot_pos];
        if num_part.parse::<u64>().is_ok() {
            return base_indent + dot_pos + 2;
        }
    }
    if let Some(marker_len) = citation_marker_len(trimmed) {
        return base_indent + marker_len;
    }
    0
}

/// A line that is just `>>>` closes a multiline blockquote (GitLab style).
pub(crate) fn closes_quote(line: &str) -> bool {
    line.trim() == ">>>"
}

/// A line that opens or closes a multiline blockquote: `>>>`, or `>>> [!kind]`
/// (which opens an alert).
pub(crate) fn is_quote_fence(line: &str) -> bool {
    closes_quote(line) || fence_alert(line).is_some()
}

/// The alert a `>>> [!kind]` line opens.
pub(crate) fn fence_alert(line: &str) -> Option<Alert> {
    Alert::parse(line.trim().strip_prefix(">>> ")?)
}

/// The alert a `> [!kind]` line opens (single `>`; the caller checks that it
/// starts a quote).
pub(crate) fn quote_alert(line: &str) -> Option<Alert> {
    let trimmed = line.trim_start();
    let marker = citation_marker_len(trimmed)?;
    if trimmed[..marker].matches('>').count() != 1 {
        return None;
    }
    Alert::parse(&trimmed[marker..])
}

/// The tag line of an alert, shown as the kind in capitals, in the width of the tag.
fn alert_header(alert: Alert, tag: &str, color: Color) -> Span<'static> {
    Span::styled(
        alert.header(tag.trim_end().width()),
        Style::default().fg(color).add_modifier(Modifier::BOLD),
    )
}

/// Parse citation/blockquote marker length in `trimmed` (e.g. `> `, `>> `, `> > `).
pub(crate) fn citation_marker_len(trimmed: &str) -> Option<usize> {
    if !trimmed.starts_with('>') || is_quote_fence(trimmed) {
        return None;
    }
    let mut len = 0;
    let mut chars = trimmed.char_indices().peekable();
    while let Some(&(i, c)) = chars.peek() {
        if c == '>' {
            chars.next();
            len = i + 1;
            while let Some(&(sp_i, ' ')) = chars.peek() {
                chars.next();
                len = sp_i + 1;
            }
        } else {
            break;
        }
    }
    if len > 0 && (len == trimmed.len() || trimmed[..len].ends_with(' ')) {
        Some(len)
    } else {
        None
    }
}

fn wrap_spans(spans: Vec<Span<'static>>, max_width: usize) -> Vec<Line<'static>> {
    wrap_spans_with_indent(spans, max_width, 0)
}

pub(super) fn wrap_spans_with_indent(
    spans: Vec<Span<'static>>,
    max_width: usize,
    indent: usize,
) -> Vec<Line<'static>> {
    if max_width == 0 {
        return vec![Line::from(spans)];
    }

    // Cap the hanging indent so continuation lines always keep room for text;
    // an indent >= the width would otherwise never fit and overflow the view.
    let indent = indent.min(max_width / 2);
    let width_for = |first_line: bool| {
        if first_line {
            max_width
        } else {
            max_width - indent
        }
    };

    let mut result: Vec<Line<'static>> = Vec::new();
    let mut current_spans: Vec<Span<'static>> = Vec::new();
    let mut current_width: usize = 0;
    let mut is_first_line = true;

    for span in spans {
        let style = span.style;
        let text = span.content.to_string();
        let span_width = text.chars().count();

        if current_width + span_width <= width_for(is_first_line) {
            current_width += span_width;
            current_spans.push(Span::styled(text, style));
            continue;
        }

        let mut remaining = text.as_str();
        while !remaining.is_empty() {
            // Recomputed every iteration: the first break inside a span moves
            // us onto an indented continuation line with a narrower budget.
            let budget = width_for(is_first_line).saturating_sub(current_width);
            if budget == 0 {
                result.push(Line::from(std::mem::take(&mut current_spans)));
                is_first_line = false;
                current_width = 0;
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
            for (count, (idx, c)) in remaining.char_indices().enumerate() {
                if count >= budget {
                    break;
                }
                split_idx = idx + c.len_utf8();
            }

            let slice = &remaining[..split_idx];
            let break_at = match slice.rfind(' ') {
                Some(pos) if pos > 0 => pos,
                _ => {
                    // No space to break at. If the line already has text and the
                    // next word fits on a fresh line, move it there whole instead
                    // of splitting it mid-word.
                    let next_word = remaining.split(' ').next().unwrap_or("");
                    if current_width > 0
                        && !remaining.starts_with(' ')
                        && next_word.chars().count() <= width_for(false)
                    {
                        result.push(Line::from(std::mem::take(&mut current_spans)));
                        is_first_line = false;
                        current_width = 0;
                        continue;
                    }
                    split_idx
                }
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

fn highlight_line(line: &str, accent: Color) -> Vec<Span<'static>> {
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
fn footnote_style() -> Style {
    Style::default().fg(Color::Magenta)
}

fn highlight_inline(text: &str, spans: &mut Vec<Span<'static>>) {
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

fn find_closing(chars: &[char], start: usize, pattern: &[char]) -> Option<usize> {
    if pattern.len() != 2 {
        return None;
    }
    (start..chars.len().saturating_sub(1))
        .find(|&i| chars[i] == pattern[0] && chars[i + 1] == pattern[1])
}

fn find_closing_triple(chars: &[char], start: usize) -> Option<usize> {
    (start + 1..chars.len().saturating_sub(2)).find(|&i| chars[i..i + 3] == ['*', '*', '*'])
}

fn find_closing_single(chars: &[char], start: usize, ch: char) -> Option<usize> {
    chars
        .iter()
        .enumerate()
        .skip(start)
        .find(|(_, c)| **c == ch)
        .map(|(i, _)| i)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render(text: &str, accent: Color, width: usize) -> Rendered {
        MarkdownRenderer::new(text, width, accent).render()
    }

    fn text(line: &Line) -> String {
        line.spans.iter().map(|s| s.content.as_ref()).collect()
    }

    #[test]
    fn tables_are_aligned_and_drawn_with_box_pipes_without_extra_rows() {
        let r = render("| a | bb |\n|:-|-:|\n| ccc | d |", Color::Cyan, 10);
        let rows: Vec<String> = r.lines().iter().map(text).collect();
        assert_eq!(rows, ["│  a  │ bb  │", "├─────┼─────┤", "│ ccc │   d │"]);
    }

    fn colored(r: &Rendered, color: Color) -> Vec<String> {
        r.lines()
            .iter()
            .flat_map(|l| l.spans.iter())
            .filter(|s| s.style.fg == Some(color))
            .map(|s| s.content.to_string())
            .collect()
    }

    #[test]
    fn footnotes_are_colored_but_the_text_is_unchanged() {
        let src = "A[^1] b `c[^2]` d[^my-note].\n\n[^1]: Note *text*[^2]\n  [^my-note]:x";
        let r = render(src, Color::Cyan, WRAP_WIDTH);
        let text: Vec<String> = r.lines().iter().map(text).collect();
        assert_eq!(text.join("\n"), src);
        assert_eq!(
            colored(&r, Color::Magenta),
            ["[^1]", "[^my-note]", "[^1]:", "[^2]", "  [^my-note]:"]
        );
    }

    #[test]
    fn footnotes_in_code_blocks_and_malformed_ones_stay_plain() {
        let r = render("```\n[^1]: x\n```\n[^] [^a b]", Color::Cyan, WRAP_WIDTH);
        assert!(colored(&r, Color::Magenta).is_empty());
    }

    #[test]
    fn matrix_corner_is_open_and_first_column_bold() {
        let r = render("|  | x |\n|---|---|\n| row | 1 |", Color::Cyan, 10);
        let rows: Vec<String> = r.lines().iter().map(text).collect();
        assert!(
            rows[0].starts_with(' ') && !rows[0].starts_with('│'),
            "no border left of the corner: {:?}",
            rows[0]
        );
        assert!(
            rows[1].starts_with('┌'),
            "the open corner gets a corner glyph, not a tee: {:?}",
            rows[1]
        );
        assert!(rows[2].starts_with('│'));
        let bold = |l: &Line, t: &str| {
            l.spans
                .iter()
                .any(|s| s.content.contains(t) && s.style.add_modifier.contains(Modifier::BOLD))
        };
        assert!(bold(&r.lines()[2], "row") && !bold(&r.lines()[2], "1"));
        // A normal table keeps its left border and plain first column.
        let n = render("| h | x |\n|---|---|\n| row | 1 |", Color::Cyan, 10);
        assert!(text(&n.lines()[0]).starts_with('│') && text(&n.lines()[1]).starts_with('├'));
        assert!(!bold(&n.lines()[2], "row"));
    }

    #[test]
    fn table_rows_are_never_wrapped() {
        let r = render(
            "| aaaaaaaaaa | bbbbbbbbbb |\n| - | - |\n| c | d |",
            Color::Cyan,
            8,
        );
        assert_eq!(r.lines().len(), 3);
        assert_eq!(text(&r.lines()[0]), "│ aaaaaaaaaa │ bbbbbbbbbb │");
    }

    #[test]
    fn pipes_without_a_delimiter_row_stay_plain_text() {
        let r = render("| a | b |\n| c | d |", Color::Cyan, 40);
        assert_eq!(text(&r.lines()[0]), "| a | b |");
    }

    #[test]
    fn pipes_in_code_blocks_are_left_alone() {
        let r = render("```\n| a |\n| - |\n```", Color::Cyan, 40);
        assert_eq!(text(&r.lines()[1]), "  | a |");
    }

    #[test]
    fn widest_table_line_measures_display_width() {
        assert_eq!(widest_table_line(["text", "| 日本 | b |", "| a |"]), 12);
        assert_eq!(widest_table_line(["no table"]), 0);
    }

    #[test]
    fn test_highlight_lines_code_block() {
        let r = render("```\nfn main() {}\n```", Color::Cyan, WRAP_WIDTH);
        assert_eq!(r.lines().len(), 3);
    }

    #[test]
    fn test_highlight_lines_heading() {
        let r = render("# Hello", Color::Cyan, WRAP_WIDTH);
        assert!(!r.lines().is_empty());
    }

    #[test]
    fn highlight_line_bold_italic() {
        let spans = highlight_line("a ***both*** b", Color::Cyan);
        let m = spans.iter().find(|s| s.content == "both").expect("span");
        assert!(
            m.style
                .add_modifier
                .contains(Modifier::BOLD | Modifier::ITALIC)
        );
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
        let r = render(&text, Color::Cyan, WRAP_WIDTH);
        assert!(r.lines().len() >= 2);
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
    fn test_highlight_lines_empty_input() {
        let r = render("", Color::Cyan, WRAP_WIDTH);
        assert!(r.lines().is_empty());
    }

    #[test]
    fn test_highlight_line_empty_string() {
        let spans = highlight_line("", Color::Cyan);
        assert!(spans.is_empty());
    }

    #[test]
    fn test_wrap_spans_empty() {
        let lines = wrap_spans(vec![], 80);
        assert_eq!(lines.len(), 1);
        assert!(lines[0].spans.is_empty());
    }

    #[test]
    fn test_wrap_long_no_space_word() {
        // 200-char word, no spaces — must hard-wrap
        let text: String = std::iter::repeat_n('x', 200).collect();
        let lines = wrap_spans(vec![Span::raw(text)], 80);
        assert!(lines.len() >= 3);
        for line in &lines {
            let len: usize = line.spans.iter().map(|s| s.content.chars().count()).sum();
            assert!(len <= 80, "line length {len} exceeds 80");
        }
    }

    #[test]
    fn test_wrap_spans_emoji_no_panic() {
        // Multi-byte emoji at wrap boundary
        let text = "😀 ".repeat(40);
        wrap_spans(vec![Span::raw(text)], 10);
    }

    #[test]
    fn test_wrap_spans_combining_chars_no_panic() {
        // Combining diacritic
        let text = "é\u{0301}".repeat(20); // many combining marks
        wrap_spans(vec![Span::raw(text)], 5);
    }

    #[test]
    fn test_highlight_lines_nested_backtick_in_text() {
        // ` inside text, not a fence — should not crash
        let r = render("inline `code` and **bold**", Color::Cyan, WRAP_WIDTH);
        assert!(!r.lines().is_empty());
    }

    #[test]
    fn test_highlight_lines_code_block_with_inline_markers() {
        // Inside a code block, ** should not be interpreted as bold
        let r = render("```\n**not bold**\n```", Color::Cyan, WRAP_WIDTH);
        let lines = r.lines();
        assert_eq!(lines.len(), 3);
        // Middle line should be styled as code (green)
        let middle_style = lines[1].spans[0].style;
        assert_eq!(middle_style.fg, Some(Color::Green));
    }

    #[test]
    fn test_highlight_lines_unterminated_code_block() {
        // Opening ``` with no close — all subsequent lines styled as code
        let r = render("```\ncode line\nstill code", Color::Cyan, WRAP_WIDTH);
        assert_eq!(r.lines().len(), 3);
    }

    #[test]
    fn test_highlight_line_unclosed_bold() {
        // ** with no closing — should not crash, render as literal
        let spans = highlight_line("this is **unclosed", Color::Cyan);
        assert!(!spans.is_empty());
    }

    #[test]
    fn test_wrap_spans_unicode_text_within_budget() {
        // CJK chars count as 1 char each; should not overflow
        let text = "你好世界".repeat(5); // 20 chars
        let lines = wrap_spans(vec![Span::raw(text)], 10);
        for line in &lines {
            let len: usize = line.spans.iter().map(|s| s.content.chars().count()).sum();
            assert!(len <= 10);
        }
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

    #[test]
    fn test_detect_list_indent_citations() {
        assert_eq!(detect_list_indent("> a citation"), 2);
        assert_eq!(detect_list_indent("  > an indented citation"), 4);
        assert_eq!(detect_list_indent(">> a nested citation"), 3);
        assert_eq!(detect_list_indent("> > spaced nested citation"), 4);
        assert_eq!(detect_list_indent("> "), 2);
        assert_eq!(detect_list_indent(">"), 1);
        assert_eq!(detect_list_indent(">= 5"), 0);
        assert_eq!(detect_list_indent("plain text"), 0);
    }

    #[test]
    fn quote_fence_is_not_a_citation_marker() {
        assert_eq!(citation_marker_len(">>>"), None);
        assert_eq!(citation_marker_len(">>> x"), Some(4));
        let text = ">>>\nquoted line\n>>>\nplain";
        let r = render(text, Color::Cyan, 80);
        let l: Vec<String> = r.lines().iter().map(|l| l.to_string()).collect();
        assert_eq!(l, vec![">>>", "quoted line", ">>>", "plain"]);
    }

    #[test]
    fn alerts_keep_their_width_and_show_the_kind_in_capitals_and_color() {
        let text =
            "> [!note]\n> body\n> more\nplain\n>>> [!WARNING]\nx\n\ny\n>>>\n>>> [!tip]\nz\n>>>";
        let r = render(text, Color::Cyan, WRAP_WIDTH);
        let rows: Vec<String> = r.lines().iter().map(line_text).collect();
        assert_eq!(rows[0], "> NOTE   ");
        assert_eq!(rows[1], "> body");
        assert_eq!(rows[3], "plain");
        assert_eq!(rows[4], ">>> WARNING   ");
        assert_eq!(rows[9], ">>> TIP   ");
        let fg = |i: usize| r.lines()[i].spans.last().unwrap().style.fg;
        assert_eq!(fg(0), Some(Alert::Note.tui_color()));
        assert_eq!(fg(1), Some(Alert::Note.tui_color()));
        assert_eq!(fg(3), None, "ends with the quote");
        assert_eq!(fg(5), Some(Alert::Warning.tui_color()));
        assert_eq!(fg(8), Some(Alert::Warning.tui_color()), "closing fence");
        assert_eq!(rows[0].width(), "> [!note]".width());
    }

    #[test]
    fn tag_not_at_the_start_of_a_quote_or_unknown_stays_literal() {
        let rows = render_texts(
            "> a\n> [!note]\n> [!bogus]\n>>> [!bogus]\nq\n>>>",
            WRAP_WIDTH,
        );
        assert_eq!(rows[1], "> [!note]");
        assert_eq!(rows[2], "> [!bogus]");
        assert_eq!(rows[3], ">>> [!bogus]");
    }

    #[test]
    fn test_citation_indentation_wrapping() {
        let text = "> This is a very long citation that should be wrapped and indented correctly.";
        let lines = render_texts(text, 25);
        assert!(lines.len() > 1);
        assert!(lines[0].starts_with("> "));
        for l in &lines[1..] {
            assert!(
                l.starts_with("  "),
                "citation continuation not indented: {l:?}"
            );
        }
        assert_fits(&lines, 25);
    }

    fn line_text(line: &Line<'_>) -> String {
        line.spans.iter().map(|s| s.content.as_ref()).collect()
    }

    fn render_texts(text: &str, width: usize) -> Vec<String> {
        MarkdownRenderer::new(text, width, Color::Cyan)
            .render()
            .lines
            .iter()
            .map(line_text)
            .collect()
    }

    /// No rendered line may be wider than the wrap width, or the view clips it.
    fn assert_fits(lines: &[String], width: usize) {
        for l in lines {
            assert!(l.chars().count() <= width, "line wider than {width}: {l:?}");
        }
    }

    #[test]
    fn deeply_indented_list_item_never_overflows_and_keeps_all_text() {
        let words = "alpha bravo charlie delta echo foxtrot golf hotel india juliet kilo lima";
        for indent in [0usize, 2, 8, 20, 30, 38, 39, 40, 41, 60] {
            for width in [20usize, 30, 40, 80] {
                let src = format!("{}- {words}", " ".repeat(indent));
                let lines = render_texts(&src, width);
                assert_fits(&lines, width);
                let joined: String = lines.join(" ");
                let got: Vec<&str> = joined.split_whitespace().collect();
                let want: Vec<&str> = src.split_whitespace().collect();
                assert_eq!(
                    got, want,
                    "text lost/changed (indent {indent}, width {width})"
                );
            }
        }
    }

    #[test]
    fn continuation_lines_after_first_break_in_a_span_are_indented_within_width() {
        // A single long span breaks several times; every continuation line
        // must use the reduced (width - indent) budget, not the full width.
        let lines = render_texts("    - aaaa bbbb cccc dddd eeee ffff gggg hhhh", 20);
        assert!(lines.len() > 2);
        assert_fits(&lines, 20);
        for l in &lines[1..] {
            assert!(l.starts_with("      "), "continuation not indented: {l:?}");
        }
    }

    #[test]
    fn indent_wider_than_width_still_shows_the_last_words() {
        let lines = render_texts("          - one two three four five six", 12);
        assert_fits(&lines, 12);
        assert!(lines.last().unwrap().contains("six"));
    }
}
