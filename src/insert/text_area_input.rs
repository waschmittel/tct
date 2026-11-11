//! Multi-line text-area primitive backing the description editor.
//!
//! Wraps `tui_textarea::TextArea` and tracks the original text so we
//! can detect whether the user has unsaved changes. Soft wrap and wrapped
//! cursor navigation come from the textarea itself (`WrapMode::WordOrGlyph`);
//! the widget is rendered directly (frameless, inside the card detail
//! popup), so it also owns scrolling and the current-line highlight.

use ratatui::style::{Color, Style};
use tui_textarea::{TextArea, WrapMode};

/// Undo depth (steps are bursts, not characters, so this reaches far back).
const MAX_UNDO_STEPS: usize = 1000;

pub struct TextAreaInput {
    pub textarea: TextArea<'static>,
    pub original: String,
}

impl TextAreaInput {
    pub fn from_initial(initial: &str) -> Self {
        let lines: Vec<String> = initial.split('\n').map(|s| s.to_string()).collect();
        let mut textarea = TextArea::new(lines);
        textarea.set_wrap_mode(WrapMode::WordOrGlyph);
        // A typing burst is one undo step; structural edits group themselves.
        textarea.set_undo_coalescing(true);
        textarea.set_max_histories(MAX_UNDO_STEPS);
        // Same hanging indent as the markdown preview.
        textarea.set_wrap_continuation_indent(Some(crate::ui::markdown::detect_list_indent));
        textarea.set_cursor_line_style(Style::default());
        textarea.set_style(Style::default().fg(Color::White));
        Self {
            textarea,
            original: initial.to_string(),
        }
    }

    pub fn current_text(&self) -> String {
        self.textarea.lines().join("\n")
    }

    pub fn is_modified(&self) -> bool {
        self.current_text() != self.original
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{Terminal, backend::TestBackend};

    /// `width` is the wrap width: the widget keeps one more column for the caret.
    fn render_rows(text: &str, width: u16, cursor_end: bool) -> Vec<String> {
        let mut input = TextAreaInput::from_initial(text);
        if cursor_end {
            input.textarea.move_cursor(tui_textarea::CursorMove::Bottom);
            input.textarea.move_cursor(tui_textarea::CursorMove::End);
        }
        let mut terminal = Terminal::new(TestBackend::new(width + 1, 4)).unwrap();
        terminal
            .draw(|f| f.render_widget(&input.textarea, f.area()))
            .unwrap();
        let buf = terminal.backend().buffer().clone();
        (0..4)
            .map(|y| {
                (0..width + 1)
                    .map(|x| buf[(x, y)].symbol().to_string())
                    .collect::<String>()
            })
            .collect()
    }

    #[test]
    fn space_after_a_word_ending_at_the_edge_does_not_start_the_next_row() {
        let rows = render_rows("0123456789 next words", 10, false);
        assert_eq!(rows[0].trim_end(), "0123456789");
        assert_eq!(rows[1].trim_end(), "next words");
    }

    #[test]
    fn spaces_inside_a_row_are_kept() {
        let rows = render_rows("ab cd ef", 20, false);
        assert_eq!(rows[0].trim_end(), "ab cd ef");
    }

    #[test]
    fn a_word_wider_than_the_row_still_splits() {
        let rows = render_rows("abcdefghijkl", 10, false);
        assert_eq!(rows[0].trim_end(), "abcdefghij");
        assert_eq!(rows[1].trim_end(), "kl");
    }

    #[test]
    fn cursor_after_the_hanging_space_does_not_panic() {
        let rows = render_rows("0123456789 ", 10, true);
        assert_eq!(rows[0].trim_end(), "0123456789");
    }

    #[test]
    fn bullet_continuation_rows_are_indented_under_the_text() {
        let rows = render_rows("- aaa bbb ccc ddd", 10, false);
        assert_eq!(rows[0].trim_end(), "- aaa bbb");
        assert_eq!(rows[1].trim_end(), "  ccc ddd");
    }

    #[test]
    fn citation_continuation_rows_are_indented_under_the_text() {
        let rows = render_rows("> aaa bbb ccc ddd", 10, false);
        assert_eq!(rows[0].trim_end(), "> aaa bbb");
        assert_eq!(rows[1].trim_end(), "  ccc ddd");
    }

    #[test]
    fn nested_and_indented_citation_continuation_rows_keep_the_hanging_indent() {
        let rows = render_rows("  > aaa bbb ccc ddd", 10, false);
        assert_eq!(rows[1].trim_end(), "    bbb");

        let rows = render_rows(">> aaa bbb ccc ddd", 10, false);
        assert_eq!(rows[1].trim_end(), "   ccc ddd");

        let rows = render_rows("> > aaa bbb ccc ddd", 10, false);
        assert_eq!(rows[1].trim_end(), "    bbb");
    }

    #[test]
    fn numbered_and_nested_continuation_rows_keep_the_hanging_indent() {
        let rows = render_rows("10. aaa bbb ccc ddd", 10, false);
        assert_eq!(rows[1].trim_end(), "    bbb");
        let rows = render_rows("  - aaa bbb ccc ddd", 10, false);
        assert_eq!(rows[1].trim_end(), "    bbb");
    }

    #[test]
    fn plain_paragraph_continuation_rows_stay_flush_left() {
        let rows = render_rows("aaa bbb ccc ddd", 8, false);
        assert_eq!(rows[1].trim_end(), "ccc ddd");
    }

    #[test]
    fn cursor_navigation_and_typing_work_on_indented_rows() {
        let mut input = TextAreaInput::from_initial("- aaa bbb ccc ddd");
        let mut terminal = Terminal::new(TestBackend::new(11, 4)).unwrap();
        terminal
            .draw(|f| f.render_widget(&input.textarea, f.area()))
            .unwrap();
        // Cursor starts at (0,0); move down one visual row, then to its end.
        input.textarea.move_cursor(tui_textarea::CursorMove::Down);
        input.textarea.move_cursor(tui_textarea::CursorMove::End);
        assert_eq!(input.textarea.cursor(), (0, 17));
        input.textarea.move_cursor(tui_textarea::CursorMove::Up);
        input.textarea.insert_char('X');
        assert!(input.current_text().starts_with("- aaa bbb"));
        assert_eq!(
            input.current_text().chars().filter(|c| *c == 'X').count(),
            1
        );
    }

    #[test]
    fn cursor_navigation_and_typing_work_on_citation_indented_rows() {
        let mut input = TextAreaInput::from_initial("> aaa bbb ccc ddd");
        let mut terminal = Terminal::new(TestBackend::new(11, 4)).unwrap();
        terminal
            .draw(|f| f.render_widget(&input.textarea, f.area()))
            .unwrap();
        input.textarea.move_cursor(tui_textarea::CursorMove::Down);
        input.textarea.move_cursor(tui_textarea::CursorMove::End);
        assert_eq!(input.textarea.cursor(), (0, 17));
        input.textarea.move_cursor(tui_textarea::CursorMove::Up);
        input.textarea.insert_char('X');
        assert!(input.current_text().starts_with("> aaa bbb"));
        assert_eq!(
            input.current_text().chars().filter(|c| *c == 'X').count(),
            1
        );
    }

    #[test]
    fn current_line_has_no_highlight() {
        let input = TextAreaInput::from_initial("first\nsecond");
        assert_eq!(input.textarea.cursor_line_style(), Style::default());
    }

    #[test]
    fn trailing_punctuation_and_quotes_stay_attached_on_wrap() {
        // Trailing comma does not wrap independently onto next row
        let rows = render_rows("aaa bbbb, cc", 8, false);
        assert_eq!(rows[0].trim_end(), "aaa");
        assert_eq!(rows[1].trim_end(), "bbbb, cc");

        // Trailing period does not wrap independently onto next row
        let rows = render_rows("aaa bbbb. cc", 8, false);
        assert_eq!(rows[0].trim_end(), "aaa");
        assert_eq!(rows[1].trim_end(), "bbbb. cc");

        // Preceding quote does not stay trailing on current row
        let rows = render_rows("aaa \"bbbb cc", 6, false);
        assert_eq!(rows[0].trim_end(), "aaa");
        assert_eq!(rows[1].trim_end(), "\"bbbb");
        assert_eq!(rows[2].trim_end(), "cc");

        let rows = render_rows("aaa \"bbbb cc", 8, false);
        assert_eq!(rows[0].trim_end(), "aaa");
        assert_eq!(rows[1].trim_end(), "\"bbbb cc");

        // Quoted word stays together
        let rows = render_rows("aaa \"bbbb\" cc", 9, false);
        assert_eq!(rows[0].trim_end(), "aaa");
        assert_eq!(rows[1].trim_end(), "\"bbbb\" cc");

        // Parentheses stay together
        let rows = render_rows("aaa (bbbb) cc", 9, false);
        assert_eq!(rows[0].trim_end(), "aaa");
        assert_eq!(rows[1].trim_end(), "(bbbb) cc");

        // Brackets stay together
        let rows = render_rows("aaa [bbbb] cc", 9, false);
        assert_eq!(rows[0].trim_end(), "aaa");
        assert_eq!(rows[1].trim_end(), "[bbbb] cc");

        // Exclamation mark stays with word
        let rows = render_rows("aaa bbbb! cc", 8, false);
        assert_eq!(rows[0].trim_end(), "aaa");
        assert_eq!(rows[1].trim_end(), "bbbb! cc");

        // Question mark stays with word
        let rows = render_rows("aaa bbbb? cc", 8, false);
        assert_eq!(rows[0].trim_end(), "aaa");
        assert_eq!(rows[1].trim_end(), "bbbb? cc");

        // Colon stays with word
        let rows = render_rows("aaa bbbb: cc", 8, false);
        assert_eq!(rows[0].trim_end(), "aaa");
        assert_eq!(rows[1].trim_end(), "bbbb: cc");

        // Semicolon stays with word
        let rows = render_rows("aaa bbbb; cc", 8, false);
        assert_eq!(rows[0].trim_end(), "aaa");
        assert_eq!(rows[1].trim_end(), "bbbb; cc");

        // Markdown bold markup stays with word
        let rows = render_rows("aaa **bbbb** cc", 11, false);
        assert_eq!(rows[0].trim_end(), "aaa");
        assert_eq!(rows[1].trim_end(), "**bbbb** cc");
    }

    #[test]
    fn hyphen_in_middle_of_word_wraps_after_hyphen() {
        // "foo-bar" wraps as "foo-" and "bar" when width is 5
        let rows = render_rows("foo-bar", 5, false);
        assert_eq!(rows[0].trim_end(), "foo-");
        assert_eq!(rows[1].trim_end(), "bar");

        // "foo-bar" fits completely when width is 7
        let rows = render_rows("foo-bar", 7, false);
        assert_eq!(rows[0].trim_end(), "foo-bar");

        // "aaa foo-bar" with width 8 fits "aaa foo-" on line 1, wraps "bar" to line 2
        let rows = render_rows("aaa foo-bar", 8, false);
        assert_eq!(rows[0].trim_end(), "aaa foo-");
        assert_eq!(rows[1].trim_end(), "bar");

        // Multiple hyphens wrap at each hyphen boundary
        let rows = render_rows("foo-bar-baz", 5, false);
        assert_eq!(rows[0].trim_end(), "foo-");
        assert_eq!(rows[1].trim_end(), "bar-");
        assert_eq!(rows[2].trim_end(), "baz");

        // Trailing punctuation on hyphenated word stays attached
        let rows = render_rows("foo-bar,", 5, false);
        assert_eq!(rows[0].trim_end(), "foo-");
        assert_eq!(rows[1].trim_end(), "bar,");
    }
}
