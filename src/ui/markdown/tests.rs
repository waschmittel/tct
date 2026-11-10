use super::*;
use unicode_width::UnicodeWidthStr;

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
    let text = "> [!note]\n> body\n> more\nplain\n>>> [!WARNING]\nx\n\ny\n>>>\n>>> [!tip]\nz\n>>>";
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
