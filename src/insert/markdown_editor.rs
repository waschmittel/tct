//! Description editor — multi-line markdown editing with list
//! auto-continuation, renumbering, and markdown formatting shortcuts.
//! The textarea widget renders itself (soft wrap, scroll, selection);
//! the styled markdown preview appears in the card detail view.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use tui_textarea::CursorMove;

use super::alert_edit;
use super::emphasis;
use super::line_input::has_ctrl_or_cmd;
use super::table_edit::{self, grouped};
use super::text_area_input::TextAreaInput;
use super::{InsertHandler, InsertOutcome, InsertSurface};
use crate::app::LoadedBoard;
use crate::command::Command;
use crate::dialog::confirm_cancel_edit::ConfirmCancelEdit;
use crate::model::id::CardId;

const NEST_INDENT: usize = 3;

pub struct MarkdownEditor {
    pub input: TextAreaInput,
    pub card_id: CardId,
    /// `Ctrl+T` was pressed inside a table; the next key picks the table edit.
    table_chord: bool,
    /// `Ctrl+G` was pressed inside a quote; the next key picks the alert kind.
    alert_chord: bool,
}

impl MarkdownEditor {
    pub fn new(card_id: CardId, initial: &str) -> Self {
        let mut input = TextAreaInput::from_initial(initial);
        input.textarea.move_cursor(CursorMove::Bottom);
        input.textarea.move_cursor(CursorMove::End);
        Self {
            input,
            card_id,
            table_chord: false,
            alert_chord: false,
        }
    }
}

impl InsertHandler for MarkdownEditor {
    fn handle_key(&mut self, key: KeyEvent, _b: Option<&LoadedBoard>) -> InsertOutcome {
        self.dispatch_key(key)
    }

    fn handle_paste(&mut self, text: &str) -> InsertOutcome {
        self.paste_text(text);
        InsertOutcome::Stay
    }

    fn surface(&self) -> InsertSurface {
        InsertSurface::CardDetail
    }
    fn title(&self) -> &str {
        "Edit Description"
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl MarkdownEditor {
    pub fn copy_selection(&mut self) {
        if self.input.textarea.is_selecting() {
            self.input.textarea.copy();
            let text = self.input.textarea.yank_text();
            crate::app::copy_to_system_clipboard(&text);
        }
    }

    pub fn cut_selection(&mut self) {
        if self.input.textarea.is_selecting() {
            grouped(&mut self.input.textarea, None, |ta| {
                ta.cut();
                renumber_all(ta);
                table_edit::reformat_at_cursor(ta);
            });
            let text = self.input.textarea.yank_text();
            crate::app::copy_to_system_clipboard(&text);
        }
    }

    pub fn paste_text(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
        grouped(&mut self.input.textarea, None, |ta| {
            ta.set_yank_text(normalized);
            ta.paste();
            renumber_all(ta);
            table_edit::reformat_at_cursor(ta);
        });
    }

    pub fn paste_from_clipboard(&mut self) {
        if let Some(text) = crate::app::get_from_system_clipboard() {
            if !text.is_empty() {
                self.paste_text(&text);
                return;
            }
        }
        grouped(&mut self.input.textarea, None, |ta| {
            ta.paste();
            renumber_all(ta);
            table_edit::reformat_at_cursor(ta);
        });
    }

    fn table_chord_key(&mut self, key: KeyEvent) -> InsertOutcome {
        let refused = table_edit::chord_op(key)
            .and_then(|op| table_edit::run_op(&mut self.input.textarea, op));
        InsertOutcome::StayWithStatus(refused)
    }

    fn dispatch_key(&mut self, key: KeyEvent) -> InsertOutcome {
        if std::mem::take(&mut self.table_chord) {
            return self.table_chord_key(key);
        }
        if std::mem::take(&mut self.alert_chord) {
            let refused = alert_edit::chord_kind(key).and_then(|kind| {
                grouped(&mut self.input.textarea, None, |ta| {
                    alert_edit::set(ta, kind)
                })
            });
            return InsertOutcome::StayWithStatus(refused);
        }
        if let Some(op) = table_edit::alt_op(key)
            && table_edit::in_table(&self.input.textarea)
        {
            let refused = table_edit::run_op(&mut self.input.textarea, op);
            return InsertOutcome::StayWithStatus(refused);
        }
        match (key.code, key.modifiers) {
            (KeyCode::Char('s'), m) if has_ctrl_or_cmd(m) => {
                let body = self.input.current_text();
                self.input.original = body.clone();
                InsertOutcome::Save(Command::EditCardDescription {
                    card_id: self.card_id.clone(),
                    body,
                })
            }
            (KeyCode::Char('q'), m) if has_ctrl_or_cmd(m) => {
                let body = self.input.current_text();
                InsertOutcome::Confirm(Command::EditCardDescription {
                    card_id: self.card_id.clone(),
                    body,
                })
            }
            (KeyCode::Esc, _) => {
                if self.input.is_modified() {
                    InsertOutcome::OpenDialogOverInsert(Box::new(ConfirmCancelEdit))
                } else {
                    InsertOutcome::Cancel
                }
            }
            (KeyCode::F(1), _) => InsertOutcome::Help,
            (KeyCode::Char('h'), m) if has_ctrl_or_cmd(m) => InsertOutcome::Help,
            (KeyCode::Char('?' | '/'), m) if has_ctrl_or_cmd(m) => InsertOutcome::Help,
            (KeyCode::Char('z'), m) if has_ctrl_or_cmd(m) && m.contains(KeyModifiers::SHIFT) => {
                self.input.textarea.redo();
                InsertOutcome::Stay
            }
            (KeyCode::Char('z'), m) if has_ctrl_or_cmd(m) => {
                self.input.textarea.undo();
                InsertOutcome::Stay
            }
            (KeyCode::Char('y'), m) if has_ctrl_or_cmd(m) => {
                self.input.textarea.redo();
                InsertOutcome::Stay
            }
            (KeyCode::Char('c'), m) if has_ctrl_or_cmd(m) => {
                self.copy_selection();
                InsertOutcome::Stay
            }
            (KeyCode::Char('x'), m) if has_ctrl_or_cmd(m) => {
                self.cut_selection();
                InsertOutcome::Stay
            }
            (KeyCode::Char('v'), m) if has_ctrl_or_cmd(m) => {
                self.paste_from_clipboard();
                InsertOutcome::Stay
            }
            (KeyCode::Insert, m) if m.contains(KeyModifiers::SHIFT) => {
                self.paste_from_clipboard();
                InsertOutcome::Stay
            }
            (KeyCode::Insert, m) if has_ctrl_or_cmd(m) => {
                self.copy_selection();
                InsertOutcome::Stay
            }
            (KeyCode::Delete, m) if m.contains(KeyModifiers::SHIFT) => {
                self.cut_selection();
                InsertOutcome::Stay
            }
            (KeyCode::Char('a'), m) if m.contains(KeyModifiers::SUPER) => {
                self.input.textarea.select_all();
                InsertOutcome::Stay
            }
            (KeyCode::Char('g'), m)
                if m.contains(KeyModifiers::CONTROL)
                    && alert_edit::in_quote(&self.input.textarea) =>
            {
                self.alert_chord = true;
                InsertOutcome::StayWithStatus(Some(alert_edit::CHORD_HINT.into()))
            }
            (KeyCode::Char('b'), m) if has_ctrl_or_cmd(m) => {
                grouped(&mut self.input.textarea, None, |ta| {
                    emphasis::toggle(ta, emphasis::BOLD)
                });
                InsertOutcome::Stay
            }
            // Ctrl+I is Tab on a terminal, so italic lives on Ctrl+E (emphasis); Cmd+I
            // still reaches here as 'i' on terminals that report Super.
            (KeyCode::Char('e' | 'i'), m) if has_ctrl_or_cmd(m) => {
                grouped(&mut self.input.textarea, None, |ta| {
                    emphasis::toggle(ta, emphasis::ITALIC)
                });
                InsertOutcome::Stay
            }
            (KeyCode::Char('k'), m) if has_ctrl_or_cmd(m) => {
                grouped(&mut self.input.textarea, None, |ta| {
                    wrap_selection_or_insert(ta, "`", "`")
                });
                InsertOutcome::Stay
            }
            (KeyCode::Char('l'), m) if has_ctrl_or_cmd(m) => {
                insert_at_line_start(&mut self.input.textarea, "- ");
                InsertOutcome::Stay
            }
            (KeyCode::Char('t'), m) if has_ctrl_or_cmd(m) => {
                if table_edit::in_table(&self.input.textarea) {
                    self.table_chord = true;
                    return InsertOutcome::StayWithStatus(Some(table_edit::CHORD_HINT.into()));
                }
                table_edit::insert_skeleton(&mut self.input.textarea);
                InsertOutcome::Stay
            }
            (KeyCode::Enter, _) => {
                let ta = &mut self.input.textarea;
                if !table_edit::enter(ta) {
                    grouped(ta, None, handle_enter_in_list);
                }
                InsertOutcome::Stay
            }
            (KeyCode::Tab, m) if !m.contains(KeyModifiers::SHIFT) => {
                let ta = &mut self.input.textarea;
                if !table_edit::tab(ta, false) && !grouped(ta, None, handle_tab_nest) {
                    ta.input(key);
                }
                InsertOutcome::Stay
            }
            (KeyCode::BackTab, _) | (KeyCode::Tab, _) => {
                let ta = &mut self.input.textarea;
                if !table_edit::tab(ta, true) {
                    grouped(ta, None, handle_shift_tab_unnest);
                }
                InsertOutcome::Stay
            }
            _ => {
                self.type_key(key);
                InsertOutcome::Stay
            }
        }
    }
}

impl MarkdownEditor {
    /// An ordinary editing key. Edits inside a table reflow it; edits that add or
    /// remove lines renumber lists. Both join the key's edit into one undo step.
    fn type_key(&mut self, key: KeyEvent) {
        let ta = &mut self.input.textarea;
        if matches!(key.code, KeyCode::Char(_))
            && !key
                .modifiers
                .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
        {
            table_edit::snap_into_row(ta);
        }
        let cell_key = table_edit::cell_key(ta);
        if cell_key.is_some() {
            ta.break_undo_run();
        }
        let mark = ta.undo_mark();
        let lines_before = ta.lines().len();
        if !ta.input(key) {
            return;
        }
        let reformatted = table_edit::reformat_at_cursor(ta);
        let lines_changed = ta.lines().len() != lines_before;
        if lines_changed {
            renumber_all(ta);
        }
        if reformatted || lines_changed {
            ta.undo_group_since(mark, cell_key);
        }
    }
}

// ── List autocontinue + nest/unnest + renumber helpers ───────────────

pub(crate) fn handle_enter_in_list(textarea: &mut tui_textarea::TextArea<'static>) {
    let (row, col) = textarea.cursor();

    if row == 0 && col == 0 {
        textarea.insert_newline();
        textarea.move_cursor(CursorMove::Up);
        return;
    }

    let current_line = textarea.lines().get(row).cloned().unwrap_or_default();
    let trimmed = current_line.trim_start();

    if trimmed == "-" || trimmed == "*" || trimmed == "- " || trimmed == "* " {
        textarea.move_cursor(CursorMove::Head);
        textarea.delete_line_by_end();
        textarea.insert_newline();
        return;
    }
    if let Some(num_str) = trimmed
        .strip_suffix(". ")
        .or_else(|| trimmed.strip_suffix('.'))
        && num_str.parse::<u64>().is_ok()
    {
        textarea.move_cursor(CursorMove::Head);
        textarea.delete_line_by_end();
        textarea.insert_newline();
        return;
    }

    if let Some(marker_len) = crate::ui::markdown::citation_marker_len(trimmed) {
        if trimmed[marker_len..].trim().is_empty() {
            textarea.move_cursor(CursorMove::Head);
            textarea.delete_line_by_end();
            textarea.insert_newline();
            return;
        }
        let indent = current_line.chars().count() - trimmed.chars().count();
        let marker = &trimmed[..marker_len];
        let prefix = if marker.ends_with(' ') {
            format!("{}{marker}", " ".repeat(indent))
        } else {
            format!("{}{} ", " ".repeat(indent), marker)
        };
        if col < indent + marker_len {
            // Cursor sits inside the marker: just push the item down a line.
            textarea.insert_newline();
            return;
        }
        split_line_for_new_item(textarea, col, &prefix);
        return;
    }

    if trimmed.starts_with("- ") || trimmed.starts_with("* ") {
        let indent = current_line.chars().count() - trimmed.chars().count();
        let prefix_char = &trimmed[..2];
        let prefix = format!("{}{prefix_char}", " ".repeat(indent));
        if col < prefix.chars().count() {
            // Cursor sits inside the marker: just push the item down a line.
            textarea.insert_newline();
            return;
        }
        split_line_for_new_item(textarea, col, &prefix);
        return;
    }

    if let Some(dot_pos) = trimmed.find(". ") {
        let num_part = &trimmed[..dot_pos];
        if num_part.parse::<u64>().is_ok() {
            let indent = current_line.chars().count() - trimmed.chars().count();
            let marker_len = indent + dot_pos + 2;
            if col < marker_len {
                textarea.insert_newline();
                renumber_all(textarea);
                return;
            }
            split_line_for_new_item(textarea, col, &format!("{}1. ", " ".repeat(indent)));
            renumber_all(textarea);
            // Put the cursor right after the (possibly renumbered) marker of the new item.
            let new_row = row + 1;
            let new_col = textarea
                .lines()
                .get(new_row)
                .and_then(|l| l.find(". ").map(|p| l[..p].chars().count() + 2))
                .unwrap_or(0);
            textarea.move_cursor(CursorMove::Jump(new_row as u16, new_col as u16));
            return;
        }
    }

    textarea.insert_newline();
}

/// Break the current line at the cursor (`col`) and start the new line with
/// `prefix`, moving the text after the cursor into the new list item. Spaces
/// right after the cursor are dropped so the item text doesn't start with a gap.
fn split_line_for_new_item(
    textarea: &mut tui_textarea::TextArea<'static>,
    col: usize,
    prefix: &str,
) {
    let (row, _) = textarea.cursor();
    let leading_spaces = textarea
        .lines()
        .get(row)
        .map(|l| l.chars().skip(col).take_while(|c| *c == ' ').count())
        .unwrap_or(0);
    if leading_spaces > 0 {
        textarea.delete_str(leading_spaces);
    }
    textarea.insert_newline();
    textarea.insert_str(prefix);
}

/// Tab: nest the current list item or citation one level deeper. Returns `false` (and
/// does nothing) when the cursor line is not a list item or citation.
pub(crate) fn handle_tab_nest(textarea: &mut tui_textarea::TextArea<'static>) -> bool {
    shift_list_item(textarea, true)
}

/// Shift+Tab: move the current list item or citation one level up. Un-indenting a
/// top-level list item or citation converts it to a regular line.
pub(crate) fn handle_shift_tab_unnest(textarea: &mut tui_textarea::TextArea<'static>) {
    shift_list_item(textarea, false);
}

/// Shared nest/unnest logic. Re-indents the item to a proper multiple of
/// `NEST_INDENT` (never landing in between), then makes it continue the list it
/// lands in: if an item at the new indent exists (above, skipping deeper children),
/// the bullet character / numbered scheme is copied from it; numbers are fixed up
/// by `renumber_all`. Un-indenting a top-level list item or citation removes the
/// marker so the line is no longer a list item or citation.
/// The cursor stays on the same character. Returns whether the line was shiftable.
fn shift_list_item(textarea: &mut tui_textarea::TextArea<'static>, nest: bool) -> bool {
    let (row, col) = textarea.cursor();
    let Some(line) = textarea.lines().get(row).cloned() else {
        return false;
    };
    let trimmed = line.trim_start();
    let indent = line.len() - trimmed.len();
    let Some((current, old_marker_len)) = parse_marker(trimmed) else {
        return false;
    };

    if !nest && indent == 0 {
        if current == Marker::Citation {
            let marker = &trimmed[..old_marker_len];
            let num_arrows = marker.chars().filter(|&c| c == '>').count();
            if num_arrows > 1 {
                // Nested citation like ">> " or "> > ": drop one citation level.
                let chars_to_remove = if marker.starts_with("> ") { 2 } else { 1 };
                textarea.move_cursor(CursorMove::Jump(row as u16, 0));
                textarea.delete_str(chars_to_remove);
                let new_line_len = textarea.lines().get(row).map_or(0, |l| l.chars().count());
                let target_col = col.saturating_sub(chars_to_remove).min(new_line_len);
                textarea.move_cursor(CursorMove::Jump(row as u16, target_col as u16));
                return true;
            }
        }

        // Top-level item: remove the marker completely so it is no longer a list item or citation.
        textarea.move_cursor(CursorMove::Jump(row as u16, 0));
        textarea.delete_str(old_marker_len);
        renumber_all(textarea);
        let new_line_len = textarea.lines().get(row).map_or(0, |l| l.chars().count());
        let target_col = col.saturating_sub(old_marker_len).min(new_line_len);
        textarea.move_cursor(CursorMove::Jump(row as u16, target_col as u16));
        return true;
    }

    let new_indent = if nest {
        (indent / NEST_INDENT + 1) * NEST_INDENT
    } else {
        (indent.saturating_sub(1) / NEST_INDENT) * NEST_INDENT
    };

    textarea.move_cursor(CursorMove::Jump(row as u16, 0));
    if indent > 0 {
        textarea.delete_str(indent);
    }
    if new_indent > 0 {
        textarea.insert_str(" ".repeat(new_indent));
    }

    let sibling = sibling_marker(textarea.lines(), row, new_indent);
    let replacement = match (current, sibling) {
        (Marker::Bullet(c), Some(Marker::Bullet(p))) => (c != p).then(|| format!("{p} ")),
        (Marker::Numbered, Some(Marker::Bullet(p))) => Some(format!("{p} ")),
        (Marker::Bullet(_), Some(Marker::Numbered)) => Some("1. ".to_string()),
        // A nested numbered item starts at 1 unless renumbering finds a run to continue.
        (Marker::Numbered, _) => nest.then(|| "1. ".to_string()),
        _ => None,
    };
    let mut new_marker_len = old_marker_len;
    if let Some(marker) = replacement {
        textarea.move_cursor(CursorMove::Jump(row as u16, new_indent as u16));
        textarea.delete_str(old_marker_len);
        new_marker_len = marker.chars().count();
        textarea.insert_str(marker);
    }

    let line_len = |textarea: &tui_textarea::TextArea<'static>| {
        textarea.lines().get(row).map_or(0, |l| l.chars().count())
    };
    let len_before_renumber = line_len(textarea);
    renumber_all(textarea);
    let new_line_len = line_len(textarea);

    // Keep the cursor on the same character: shift for the indent change, the
    // marker rewrite (if the cursor was past the marker) and renumbering.
    let indent_delta = new_indent as isize - indent as isize;
    let target_col = if col >= indent + old_marker_len {
        col as isize
            + indent_delta
            + (new_marker_len as isize - old_marker_len as isize)
            + (new_line_len as isize - len_before_renumber as isize)
    } else if col >= indent {
        new_indent as isize + (col as isize - indent as isize)
    } else {
        (col as isize + indent_delta).max(0)
    };
    let target_col = (target_col.max(0) as usize).min(new_line_len);
    textarea.move_cursor(CursorMove::Jump(row as u16, target_col as u16));
    true
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Marker {
    Bullet(char),
    Numbered,
    Citation,
}

/// Parse a list or citation marker at the start of `trimmed`: its kind and length in chars.
fn parse_marker(trimmed: &str) -> Option<(Marker, usize)> {
    if trimmed.starts_with("- ") || trimmed.starts_with("* ") {
        return Some((Marker::Bullet(trimmed.chars().next()?), 2));
    }
    if trimmed == "-" || trimmed == "*" {
        return Some((Marker::Bullet(trimmed.chars().next()?), 1));
    }
    if let Some(dot) = trimmed.find(". ") {
        if trimmed[..dot].parse::<u64>().is_ok() {
            return Some((Marker::Numbered, dot + 2));
        }
    }
    if let Some(num_str) = trimmed
        .strip_suffix('.')
        .filter(|s| s.parse::<u64>().is_ok())
    {
        return Some((Marker::Numbered, num_str.chars().count() + 1));
    }
    if let Some(marker_len) = crate::ui::markdown::citation_marker_len(trimmed) {
        return Some((Marker::Citation, marker_len));
    }
    None
}

/// Marker of the nearest list item above `row` at exactly `indent` (the list
/// an item at that indent would continue), skipping deeper (child) lines and
/// stopping at a blank, shallower or unrelated line.
fn sibling_marker(lines: &[String], row: usize, indent: usize) -> Option<Marker> {
    for line in lines[..row].iter().rev() {
        if line.trim().is_empty() {
            return None;
        }
        let trimmed = line.trim_start();
        let line_indent = line.len() - trimmed.len();
        if line_indent > indent {
            continue;
        }
        if line_indent == indent {
            return parse_marker(trimmed).map(|(m, _)| m);
        }
        return None;
    }
    None
}

pub(crate) fn renumber_all(textarea: &mut tui_textarea::TextArea<'static>) {
    let lines = textarea.lines().to_vec();
    let saved = textarea.cursor();

    let mut stack: Vec<(usize, u64)> = Vec::new();

    for (r, line) in lines.iter().enumerate() {
        if line.is_empty() {
            stack.clear();
            continue;
        }
        let trimmed = line.trim_start();
        let indent = line.len() - trimmed.len();

        let numbered = trimmed.find(". ").and_then(|p| {
            let s = &trimmed[..p];
            s.parse::<u64>().ok().map(|n| (s.to_string(), n))
        });

        if let Some((num_str, n)) = numbered {
            while let Some(&(top_indent, _)) = stack.last() {
                if top_indent > indent {
                    stack.pop();
                } else {
                    break;
                }
            }

            let expected = match stack.last() {
                Some(&(top_indent, next)) if top_indent == indent => next,
                _ => n,
            };

            match stack.last_mut() {
                Some(top) if top.0 == indent => top.1 = expected + 1,
                _ => stack.push((indent, expected + 1)),
            }

            if expected != n {
                textarea.move_cursor(CursorMove::Jump(r as u16, indent as u16));
                textarea.delete_str(num_str.chars().count());
                textarea.insert_str(expected.to_string());
            }
        } else {
            while let Some(&(top_indent, _)) = stack.last() {
                if top_indent >= indent {
                    stack.pop();
                } else {
                    break;
                }
            }
        }
    }

    let (sr, sc) = saved;
    textarea.move_cursor(CursorMove::Jump(sr as u16, sc as u16));
}

fn wrap_selection_or_insert(
    textarea: &mut tui_textarea::TextArea<'static>,
    prefix: &str,
    suffix: &str,
) {
    if textarea.is_selecting() {
        textarea.cut();
        let selected = textarea.yank_text().to_string();
        textarea.insert_str(format!("{prefix}{selected}{suffix}"));
    } else {
        textarea.insert_str(format!("{prefix}{suffix}"));
        for _ in 0..suffix.len() {
            textarea.move_cursor(CursorMove::Back);
        }
    }
}

fn insert_at_line_start(textarea: &mut tui_textarea::TextArea<'static>, prefix: &str) {
    textarea.move_cursor(CursorMove::Head);
    textarea.insert_str(prefix);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    fn make_editor(initial: &str) -> MarkdownEditor {
        MarkdownEditor::new("card1".into(), initial)
    }

    fn editor_lines(e: &MarkdownEditor) -> Vec<String> {
        e.input
            .textarea
            .lines()
            .iter()
            .map(|s| s.to_string())
            .collect()
    }

    fn editor_cursor(e: &MarkdownEditor) -> (usize, usize) {
        let (r, c) = e.input.textarea.cursor();
        (r, c)
    }

    fn press(e: &mut MarkdownEditor, code: KeyCode) {
        e.handle_key(KeyEvent::new(code, KeyModifiers::empty()), None);
    }

    #[test]
    fn new_editor_places_cursor_at_end_of_content() {
        assert_eq!(editor_cursor(&make_editor("")), (0, 0));
        assert_eq!(editor_cursor(&make_editor("hello")), (0, 5));
        assert_eq!(editor_cursor(&make_editor("one\ntwo\nthree")), (2, 5));
        assert_eq!(editor_cursor(&make_editor("one\n")), (1, 0));
    }

    #[test]
    fn typing_after_entering_edit_mode_appends() {
        let mut e = make_editor("abc\ndef");
        press(&mut e, KeyCode::Char('X'));
        assert_eq!(
            editor_lines(&e),
            vec!["abc".to_string(), "defX".to_string()]
        );
    }

    #[test]
    fn help_keys_return_help_outcome() {
        let mut e = make_editor("hello");
        assert!(matches!(
            e.handle_key(KeyEvent::new(KeyCode::F(1), KeyModifiers::empty()), None),
            InsertOutcome::Help
        ));
        assert!(matches!(
            e.handle_key(
                KeyEvent::new(KeyCode::Char('h'), KeyModifiers::CONTROL),
                None
            ),
            InsertOutcome::Help
        ));
        assert!(matches!(
            e.handle_key(KeyEvent::new(KeyCode::Char('h'), KeyModifiers::SUPER), None),
            InsertOutcome::Help
        ));
        assert!(matches!(
            e.handle_key(
                KeyEvent::new(KeyCode::Char('?'), KeyModifiers::CONTROL),
                None
            ),
            InsertOutcome::Help
        ));
        // Typing bare 'h' or '?' appends text into buffer
        press(&mut e, KeyCode::Char('h'));
        press(&mut e, KeyCode::Char('?'));
        assert_eq!(editor_lines(&e), vec!["helloh?".to_string()]);
    }

    #[test]
    fn test_copy_and_paste_with_selection() {
        let mut e = make_editor("hello world");
        e.input.textarea.move_cursor(CursorMove::Jump(0, 6));
        for _ in 0..5 {
            e.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::SHIFT), None);
        }
        assert!(e.input.textarea.is_selecting());
        // Copy with Ctrl+C
        e.handle_key(
            KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
            None,
        );
        assert_eq!(e.input.textarea.yank_text(), "world");
        assert_eq!(editor_lines(&e), vec!["hello world".to_string()]);

        // Paste at end with Ctrl+V
        e.input.textarea.move_cursor(CursorMove::End);
        e.handle_key(
            KeyEvent::new(KeyCode::Char('v'), KeyModifiers::CONTROL),
            None,
        );
        assert_eq!(editor_lines(&e), vec!["hello worldworld".to_string()]);
    }

    #[test]
    fn test_cut_and_paste_with_selection() {
        let mut e = make_editor("hello world");
        e.input.textarea.move_cursor(CursorMove::Jump(0, 5));
        for _ in 0..6 {
            e.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::SHIFT), None);
        }
        assert!(e.input.textarea.is_selecting());
        // Cut with Ctrl+X
        e.handle_key(
            KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL),
            None,
        );
        assert_eq!(e.input.textarea.yank_text(), " world");
        assert_eq!(editor_lines(&e), vec!["hello".to_string()]);

        // Paste at end with Ctrl+V
        e.input.textarea.move_cursor(CursorMove::End);
        e.handle_key(
            KeyEvent::new(KeyCode::Char('v'), KeyModifiers::CONTROL),
            None,
        );
        assert_eq!(editor_lines(&e), vec!["hello world".to_string()]);
    }

    #[test]
    fn test_paste_over_selection_replaces_it() {
        let mut e = make_editor("hello world");
        crate::app::copy_to_system_clipboard("everyone");
        e.input.textarea.move_cursor(CursorMove::Jump(0, 6));
        for _ in 0..5 {
            e.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::SHIFT), None);
        }
        assert!(e.input.textarea.is_selecting());
        // Paste replaces selected "world"
        e.paste_from_clipboard();
        assert_eq!(editor_lines(&e), vec!["hello everyone".to_string()]);
        assert!(!e.input.textarea.is_selecting());
    }

    #[test]
    fn test_cut_copy_paste_keys_and_super_modifier() {
        let mut e = make_editor("abc def");
        // Cmd+C / Cmd+X without selection do not insert 'c' or 'x'
        e.handle_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::SUPER), None);
        e.handle_key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::SUPER), None);
        assert_eq!(editor_lines(&e), vec!["abc def".to_string()]);

        // Select "def"
        e.input.textarea.move_cursor(CursorMove::Jump(0, 4));
        for _ in 0..3 {
            e.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::SHIFT), None);
        }
        // Cmd+C copies
        e.handle_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::SUPER), None);
        assert_eq!(e.input.textarea.yank_text(), "def");

        // Cmd+V pastes
        e.input.textarea.move_cursor(CursorMove::End);
        e.handle_key(KeyEvent::new(KeyCode::Char('v'), KeyModifiers::SUPER), None);
        assert_eq!(editor_lines(&e), vec!["abc defdef".to_string()]);
    }

    #[test]
    fn test_internal_clipboard_fallback_across_instances() {
        let mut e1 = make_editor("source text");
        e1.input.textarea.move_cursor(CursorMove::Jump(0, 0));
        for _ in 0..6 {
            e1.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::SHIFT), None);
        }
        e1.handle_key(
            KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
            None,
        );

        let mut e2 = make_editor("dest: ");
        e2.paste_from_clipboard();
        assert_eq!(editor_lines(&e2), vec!["dest: source".to_string()]);
    }

    #[test]
    fn test_highlight_and_wrap_selection() {
        let mut e = make_editor("format me please");
        e.input.textarea.move_cursor(CursorMove::Jump(0, 7));
        for _ in 0..2 {
            e.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::SHIFT), None);
        }
        assert!(e.input.textarea.is_selecting());
        // Bold the selection
        e.handle_key(
            KeyEvent::new(KeyCode::Char('b'), KeyModifiers::CONTROL),
            None,
        );
        assert_eq!(editor_lines(&e), vec!["format **me** please".to_string()]);
    }

    fn ctrl(e: &mut MarkdownEditor, c: char) {
        e.handle_key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL), None);
    }

    #[test]
    fn bold_selection_skips_surrounding_whitespace() {
        let mut e = make_editor("one two three four");
        e.input.textarea.move_cursor(CursorMove::Jump(0, 3));
        e.input.textarea.start_selection();
        e.input.textarea.move_cursor(CursorMove::Jump(0, 14));
        ctrl(&mut e, 'b');
        assert_eq!(editor_lines(&e), vec!["one **two three** four".to_string()]);
    }

    #[test]
    fn bold_inside_bold_removes_it() {
        let mut e = make_editor("a **bold** b");
        e.input.textarea.move_cursor(CursorMove::Jump(0, 6));
        ctrl(&mut e, 'b');
        assert_eq!(editor_lines(&e), vec!["a bold b".to_string()]);
        assert_eq!(editor_cursor(&e), (0, 4));
    }

    #[test]
    fn emphasis_without_selection_in_table_cell_formats_cell_and_reformats() {
        let mut e = make_editor("| a | b |\n|---|---|\n| one | two |");
        e.input.textarea.move_cursor(CursorMove::Jump(2, 3));
        ctrl(&mut e, 'b');
        let lines = editor_lines(&e);
        assert!(lines[2].contains("**one**"), "{lines:?}");
        assert_eq!(lines[0].len(), lines[2].len(), "{lines:?}");
        ctrl(&mut e, 'b');
        assert!(editor_lines(&e)[2].contains("| one "));
        ctrl(&mut e, 'b');
        ctrl(&mut e, 'e');
        assert!(editor_lines(&e)[2].contains("***one***"));
    }

    #[test]
    fn test_bracketed_paste() {
        let mut e = make_editor("title\n");
        e.handle_paste("- item 1\n- item 2");
        assert_eq!(
            editor_lines(&e),
            vec![
                "title".to_string(),
                "- item 1".to_string(),
                "- item 2".to_string()
            ]
        );
    }

    #[test]
    fn enter_at_col0_of_first_list_line_inserts_blank_line_above() {
        let mut e = make_editor("- first\n- second");
        e.input.textarea.move_cursor(CursorMove::Top);
        e.input.textarea.move_cursor(CursorMove::Head);
        assert_eq!(editor_cursor(&e), (0, 0));

        press(&mut e, KeyCode::Enter);

        assert_eq!(
            editor_lines(&e),
            vec![
                "".to_string(),
                "- first".to_string(),
                "- second".to_string()
            ]
        );
        assert_eq!(editor_cursor(&e), (0, 0));
    }

    #[test]
    fn enter_at_col0_of_inner_list_line_pushes_the_item_down() {
        let mut e = make_editor("intro\n- first\n- second");
        e.input.textarea.move_cursor(CursorMove::Top);
        e.input.textarea.move_cursor(CursorMove::Down);
        e.input.textarea.move_cursor(CursorMove::Head);
        assert_eq!(editor_cursor(&e), (1, 0));

        press(&mut e, KeyCode::Enter);

        assert_eq!(
            editor_lines(&e),
            vec![
                "intro".to_string(),
                "".to_string(),
                "- first".to_string(),
                "- second".to_string()
            ]
        );
        assert_eq!(editor_cursor(&e), (2, 0));
    }

    fn cursor_at(e: &mut MarkdownEditor, row: usize, col: usize) {
        e.input
            .textarea
            .move_cursor(CursorMove::Jump(row as u16, col as u16));
    }

    #[test]
    fn enter_in_middle_of_bullet_item_splits_it() {
        let mut e = make_editor("- hello world\n- next");
        cursor_at(&mut e, 0, 7); // after "- hello"
        press(&mut e, KeyCode::Enter);
        assert_eq!(
            editor_lines(&e),
            vec![
                "- hello".to_string(),
                "- world".to_string(),
                "- next".to_string()
            ]
        );
        assert_eq!(editor_cursor(&e), (1, 2));
    }

    #[test]
    fn enter_mid_word_splits_the_word_and_keeps_text_after_cursor() {
        let mut e = make_editor("* abcdef");
        cursor_at(&mut e, 0, 5); // "* abc|def"
        press(&mut e, KeyCode::Enter);
        assert_eq!(
            editor_lines(&e),
            vec!["* abc".to_string(), "* def".to_string()]
        );
        assert_eq!(editor_cursor(&e), (1, 2));
    }

    #[test]
    fn enter_at_start_of_item_text_leaves_empty_item_above() {
        let mut e = make_editor("- text");
        cursor_at(&mut e, 0, 2);
        press(&mut e, KeyCode::Enter);
        assert_eq!(
            editor_lines(&e),
            vec!["- ".to_string(), "- text".to_string()]
        );
        assert_eq!(editor_cursor(&e), (1, 2));
    }

    #[test]
    fn enter_in_nested_bullet_keeps_indent_for_the_tail() {
        let mut e = make_editor("   - one two");
        cursor_at(&mut e, 0, 8); // "   - one| two"
        press(&mut e, KeyCode::Enter);
        assert_eq!(
            editor_lines(&e),
            vec!["   - one".to_string(), "   - two".to_string()]
        );
        assert_eq!(editor_cursor(&e), (1, 5));
    }

    #[test]
    fn enter_in_middle_of_numbered_item_splits_and_renumbers() {
        let mut e = make_editor("1. hello world\n2. next");
        cursor_at(&mut e, 0, 8); // "1. hello| world"
        press(&mut e, KeyCode::Enter);
        assert_eq!(
            editor_lines(&e),
            vec![
                "1. hello".to_string(),
                "2. world".to_string(),
                "3. next".to_string()
            ]
        );
        assert_eq!(editor_cursor(&e), (1, 3));
    }

    #[test]
    fn enter_at_col0_of_numbered_item_pushes_it_down_without_new_item() {
        let mut e = make_editor("intro\n1. a\n2. b");
        cursor_at(&mut e, 1, 0);
        press(&mut e, KeyCode::Enter);
        assert_eq!(
            editor_lines(&e),
            vec![
                "intro".to_string(),
                "".to_string(),
                "1. a".to_string(),
                "2. b".to_string()
            ]
        );
    }

    #[test]
    fn enter_at_end_of_list_item_continues_list() {
        let mut e = make_editor("- first");
        e.input.textarea.move_cursor(CursorMove::End);

        press(&mut e, KeyCode::Enter);

        assert_eq!(
            editor_lines(&e),
            vec!["- first".to_string(), "- ".to_string()]
        );
    }

    #[test]
    fn enter_at_end_of_citation_continues_citation() {
        let mut e = make_editor("> first quote");
        e.input.textarea.move_cursor(CursorMove::End);
        press(&mut e, KeyCode::Enter);
        assert_eq!(
            editor_lines(&e),
            vec!["> first quote".to_string(), "> ".to_string()]
        );
        assert_eq!(editor_cursor(&e), (1, 2));
    }

    #[test]
    fn enter_on_quote_fence_is_plain_newline() {
        let mut e = make_editor(">>>");
        e.input.textarea.move_cursor(CursorMove::End);
        press(&mut e, KeyCode::Enter);
        assert_eq!(editor_lines(&e), vec![">>>".to_string(), "".to_string()]);
    }

    #[test]
    fn enter_on_alert_fence_is_plain_newline() {
        let mut e = make_editor(">>> [!note]");
        e.input.textarea.move_cursor(CursorMove::End);
        press(&mut e, KeyCode::Enter);
        assert_eq!(
            editor_lines(&e),
            vec![">>> [!note]".to_string(), "".to_string()]
        );
    }

    fn ctrl_out(e: &mut MarkdownEditor, c: char) -> InsertOutcome {
        e.handle_key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL), None)
    }

    fn plain(e: &mut MarkdownEditor, c: char) -> InsertOutcome {
        e.handle_key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::empty()), None)
    }

    #[test]
    fn ctrl_g_chord_turns_a_quote_into_an_alert_in_one_undo_step() {
        let mut e = make_editor("> one\n> two");
        assert!(matches!(
            ctrl_out(&mut e, 'g'),
            InsertOutcome::StayWithStatus(Some(_))
        ));
        plain(&mut e, 'w');
        assert_eq!(editor_lines(&e), vec!["> [!warning]", "> one", "> two"]);
        e.input.textarea.undo();
        assert_eq!(editor_lines(&e), vec!["> one", "> two"]);
    }

    #[test]
    fn ctrl_g_chord_in_multiline_quote_and_removal() {
        let mut e = make_editor(">>>\ntext\n>>>");
        e.input.textarea.move_cursor(CursorMove::Jump(1, 0));
        ctrl(&mut e, 'g');
        plain(&mut e, 'c');
        assert_eq!(editor_lines(&e), vec![">>> [!caution]", "text", ">>>"]);
        ctrl(&mut e, 'g');
        plain(&mut e, 'x');
        assert_eq!(editor_lines(&e), vec![">>>", "text", ">>>"]);
    }

    #[test]
    fn ctrl_g_outside_a_quote_is_ignored_and_unknown_chord_key_is_ignored() {
        let mut e = make_editor("plain text");
        assert!(matches!(ctrl_out(&mut e, 'g'), InsertOutcome::Stay));
        plain(&mut e, 'x');
        assert_eq!(editor_lines(&e), vec!["plain textx"], "no chord armed");
        ctrl_out(&mut e, 'a');
        assert_eq!(editor_cursor(&e), (0, 0), "Ctrl+A is still line start");

        let mut e = make_editor("> q");
        ctrl(&mut e, 'g');
        plain(&mut e, 'z');
        assert_eq!(editor_lines(&e), vec!["> q"]);
    }

    #[test]
    fn enter_on_empty_citation_clears_marker_and_exits() {
        let mut e = make_editor("> ");
        press(&mut e, KeyCode::Enter);
        assert_eq!(editor_lines(&e), vec!["".to_string(), "".to_string()]);
        assert_eq!(editor_cursor(&e), (1, 0));

        let mut e2 = make_editor(">");
        press(&mut e2, KeyCode::Enter);
        assert_eq!(editor_lines(&e2), vec!["".to_string(), "".to_string()]);
        assert_eq!(editor_cursor(&e2), (1, 0));
    }

    #[test]
    fn enter_in_middle_of_citation_splits_it() {
        let mut e = make_editor("> hello world\n> next");
        cursor_at(&mut e, 0, 7); // after "> hello"
        press(&mut e, KeyCode::Enter);
        assert_eq!(
            editor_lines(&e),
            vec![
                "> hello".to_string(),
                "> world".to_string(),
                "> next".to_string()
            ]
        );
        assert_eq!(editor_cursor(&e), (1, 2));
    }

    #[test]
    fn enter_in_indented_citation_keeps_indent() {
        let mut e = make_editor("  > quote text");
        e.input.textarea.move_cursor(CursorMove::End);
        press(&mut e, KeyCode::Enter);
        assert_eq!(
            editor_lines(&e),
            vec!["  > quote text".to_string(), "  > ".to_string()]
        );
        assert_eq!(editor_cursor(&e), (1, 4));
    }

    #[test]
    fn enter_in_nested_citation_continues_nested_markers() {
        let mut e = make_editor(">> nested quote");
        e.input.textarea.move_cursor(CursorMove::End);
        press(&mut e, KeyCode::Enter);
        assert_eq!(
            editor_lines(&e),
            vec![">> nested quote".to_string(), ">> ".to_string()]
        );
        assert_eq!(editor_cursor(&e), (1, 3));

        let mut e2 = make_editor("> > spaced quote");
        e2.input.textarea.move_cursor(CursorMove::End);
        press(&mut e2, KeyCode::Enter);
        assert_eq!(
            editor_lines(&e2),
            vec!["> > spaced quote".to_string(), "> > ".to_string()]
        );
        assert_eq!(editor_cursor(&e2), (1, 4));
    }

    #[test]
    fn enter_inside_citation_marker_pushes_the_item_down() {
        let mut e = make_editor("intro\n> quote");
        cursor_at(&mut e, 1, 0);
        press(&mut e, KeyCode::Enter);
        assert_eq!(
            editor_lines(&e),
            vec!["intro".to_string(), "".to_string(), "> quote".to_string()]
        );

        let mut e2 = make_editor("> quote");
        cursor_at(&mut e2, 0, 0);
        press(&mut e2, KeyCode::Enter);
        assert_eq!(
            editor_lines(&e2),
            vec!["".to_string(), "> quote".to_string()]
        );
    }

    #[test]
    fn enter_at_end_of_numbered_item_continues_with_next_number() {
        let mut e = make_editor("1. first");
        e.input.textarea.move_cursor(CursorMove::End);
        press(&mut e, KeyCode::Enter);
        assert_eq!(
            editor_lines(&e),
            vec!["1. first".to_string(), "2. ".to_string()]
        );
    }

    #[test]
    fn enter_in_numbered_list_renumbers_following_items() {
        let mut e = make_editor("1. a\n2. b\n3. c");
        e.input.textarea.move_cursor(CursorMove::Top);
        e.input.textarea.move_cursor(CursorMove::End);
        press(&mut e, KeyCode::Enter);
        assert_eq!(
            editor_lines(&e),
            vec![
                "1. a".to_string(),
                "2. ".to_string(),
                "3. b".to_string(),
                "4. c".to_string(),
            ]
        );
        assert_eq!(editor_cursor(&e), (1, 3));
    }

    #[test]
    fn enter_at_last_numbered_item_does_not_renumber() {
        let mut e = make_editor("1. a\n2. b");
        e.input.textarea.move_cursor(CursorMove::Bottom);
        e.input.textarea.move_cursor(CursorMove::End);
        press(&mut e, KeyCode::Enter);
        assert_eq!(
            editor_lines(&e),
            vec!["1. a".to_string(), "2. b".to_string(), "3. ".to_string()]
        );
    }

    #[test]
    fn enter_in_parent_numbered_list_skips_nested_children() {
        let mut e = make_editor("1. parent\n   1. child\n   2. child\n2. parent2");
        e.input.textarea.move_cursor(CursorMove::Top);
        e.input.textarea.move_cursor(CursorMove::End);
        press(&mut e, KeyCode::Enter);
        assert_eq!(
            editor_lines(&e),
            vec![
                "1. parent".to_string(),
                "2. ".to_string(),
                "   1. child".to_string(),
                "   2. child".to_string(),
                "3. parent2".to_string(),
            ]
        );
    }

    #[test]
    fn enter_in_nested_numbered_list_does_not_renumber_parents() {
        let mut e = make_editor("1. parent\n   1. child\n   2. child2\n2. parent2");
        e.input.textarea.move_cursor(CursorMove::Top);
        e.input.textarea.move_cursor(CursorMove::Down);
        e.input.textarea.move_cursor(CursorMove::End);
        press(&mut e, KeyCode::Enter);
        assert_eq!(
            editor_lines(&e),
            vec![
                "1. parent".to_string(),
                "   1. child".to_string(),
                "   2. ".to_string(),
                "   3. child2".to_string(),
                "2. parent2".to_string(),
            ]
        );
    }

    #[test]
    fn enter_renumbers_non_canonical_numbered_list_fully() {
        let mut e = make_editor("1. a\n5. b\n7. c");
        e.input.textarea.move_cursor(CursorMove::Top);
        e.input.textarea.move_cursor(CursorMove::End);
        press(&mut e, KeyCode::Enter);
        assert_eq!(
            editor_lines(&e),
            vec![
                "1. a".to_string(),
                "2. ".to_string(),
                "3. b".to_string(),
                "4. c".to_string(),
            ]
        );
        assert_eq!(editor_cursor(&e), (1, 3));
    }

    #[test]
    fn enter_preserves_lists_starting_at_nonzero_number() {
        let mut e = make_editor("3. a\n4. b\n4. c");
        e.input.textarea.move_cursor(CursorMove::Top);
        e.input.textarea.move_cursor(CursorMove::End);
        press(&mut e, KeyCode::Enter);
        assert_eq!(
            editor_lines(&e),
            vec![
                "3. a".to_string(),
                "4. ".to_string(),
                "5. b".to_string(),
                "6. c".to_string(),
            ]
        );
    }

    #[test]
    fn enter_in_middle_of_list_renumbers_items_above_and_below() {
        let mut e = make_editor("1. a\n1. b\n1. c\n1. d");
        e.input.textarea.move_cursor(CursorMove::Top);
        e.input.textarea.move_cursor(CursorMove::Down);
        e.input.textarea.move_cursor(CursorMove::End);
        press(&mut e, KeyCode::Enter);
        assert_eq!(
            editor_lines(&e),
            vec![
                "1. a".to_string(),
                "2. b".to_string(),
                "3. ".to_string(),
                "4. c".to_string(),
                "5. d".to_string(),
            ]
        );
    }

    #[test]
    fn enter_below_paragraph_renumbers_only_the_list_run() {
        let mut e = make_editor("intro text\n1. foo\n2. bar");
        e.input.textarea.move_cursor(CursorMove::Top);
        e.input.textarea.move_cursor(CursorMove::Down);
        e.input.textarea.move_cursor(CursorMove::End);
        press(&mut e, KeyCode::Enter);
        assert_eq!(
            editor_lines(&e),
            vec![
                "intro text".to_string(),
                "1. foo".to_string(),
                "2. ".to_string(),
                "3. bar".to_string(),
            ]
        );
    }

    #[test]
    fn enter_does_not_touch_earlier_unrelated_numbered_list() {
        let mut e = make_editor("1. orphan\nunrelated\n1. foo\n2. bar");
        e.input.textarea.move_cursor(CursorMove::Top);
        e.input.textarea.move_cursor(CursorMove::Down);
        e.input.textarea.move_cursor(CursorMove::Down);
        e.input.textarea.move_cursor(CursorMove::End);
        press(&mut e, KeyCode::Enter);
        assert_eq!(
            editor_lines(&e),
            vec![
                "1. orphan".to_string(),
                "unrelated".to_string(),
                "1. foo".to_string(),
                "2. ".to_string(),
                "3. bar".to_string(),
            ]
        );
    }

    #[test]
    fn tab_nests_numbered_list_item_and_resets_to_one() {
        let mut e = make_editor("1. parent\n2. child");
        e.input.textarea.move_cursor(CursorMove::Bottom);
        e.input.textarea.move_cursor(CursorMove::End);
        press(&mut e, KeyCode::Tab);
        assert_eq!(
            editor_lines(&e),
            vec!["1. parent".to_string(), "   1. child".to_string()]
        );
    }

    #[test]
    fn tab_nests_into_existing_nested_run() {
        let mut e = make_editor("1. parent\n   1. existing-nested\n2. orphan");
        e.input.textarea.move_cursor(CursorMove::Bottom);
        e.input.textarea.move_cursor(CursorMove::End);
        press(&mut e, KeyCode::Tab);
        assert_eq!(
            editor_lines(&e),
            vec![
                "1. parent".to_string(),
                "   1. existing-nested".to_string(),
                "   2. orphan".to_string(),
            ]
        );
    }

    #[test]
    fn tab_nests_unordered_list_item() {
        let mut e = make_editor("- a\n- b");
        e.input.textarea.move_cursor(CursorMove::Bottom);
        e.input.textarea.move_cursor(CursorMove::End);
        press(&mut e, KeyCode::Tab);
        assert_eq!(
            editor_lines(&e),
            vec!["- a".to_string(), "   - b".to_string()]
        );
    }

    #[test]
    fn shift_tab_unnests_nested_item_and_joins_parent_list() {
        let mut e = make_editor("1. parent\n   1. child");
        e.input.textarea.move_cursor(CursorMove::Bottom);
        e.input.textarea.move_cursor(CursorMove::End);
        press(&mut e, KeyCode::BackTab);
        assert_eq!(
            editor_lines(&e),
            vec!["1. parent".to_string(), "2. child".to_string()]
        );
    }

    #[test]
    fn shift_tab_on_top_level_list_item_removes_marker() {
        let mut e = make_editor("1. a\n2. b");
        e.input.textarea.move_cursor(CursorMove::Bottom);
        e.input.textarea.move_cursor(CursorMove::End);
        press(&mut e, KeyCode::BackTab);
        assert_eq!(editor_lines(&e), vec!["1. a".to_string(), "b".to_string()]);
        assert_eq!(editor_cursor(&e), (1, 1));

        let mut e2 = make_editor("1. a\n2. b");
        e2.input.textarea.move_cursor(CursorMove::Top);
        e2.input.textarea.move_cursor(CursorMove::End);
        press(&mut e2, KeyCode::BackTab);
        assert_eq!(editor_lines(&e2), vec!["a".to_string(), "2. b".to_string()]);
        assert_eq!(editor_cursor(&e2), (0, 1));

        let mut e3 = make_editor("- first\n- second");
        e3.input.textarea.move_cursor(CursorMove::Bottom);
        e3.input.textarea.move_cursor(CursorMove::End);
        press(&mut e3, KeyCode::BackTab);
        assert_eq!(
            editor_lines(&e3),
            vec!["- first".to_string(), "second".to_string()]
        );
        assert_eq!(editor_cursor(&e3), (1, 6));
    }

    #[test]
    fn shift_tab_on_top_level_citation_removes_marker() {
        let mut e = make_editor("> quote");
        e.input.textarea.move_cursor(CursorMove::End);
        press(&mut e, KeyCode::BackTab);
        assert_eq!(editor_lines(&e), vec!["quote".to_string()]);
        assert_eq!(editor_cursor(&e), (0, 5));

        let mut e2 = make_editor("> ");
        press(&mut e2, KeyCode::BackTab);
        assert_eq!(editor_lines(&e2), vec!["".to_string()]);
        assert_eq!(editor_cursor(&e2), (0, 0));
    }

    #[test]
    fn shift_tab_on_nested_citation_decrements_quote_level() {
        let mut e = make_editor(">> quote");
        e.input.textarea.move_cursor(CursorMove::End);
        press(&mut e, KeyCode::BackTab);
        assert_eq!(editor_lines(&e), vec!["> quote".to_string()]);
        assert_eq!(editor_cursor(&e), (0, 7));

        press(&mut e, KeyCode::BackTab);
        assert_eq!(editor_lines(&e), vec!["quote".to_string()]);
        assert_eq!(editor_cursor(&e), (0, 5));

        let mut e2 = make_editor("> > spaced quote");
        e2.input.textarea.move_cursor(CursorMove::End);
        press(&mut e2, KeyCode::BackTab);
        assert_eq!(editor_lines(&e2), vec!["> spaced quote".to_string()]);
        assert_eq!(editor_cursor(&e2), (0, 14));

        press(&mut e2, KeyCode::BackTab);
        assert_eq!(editor_lines(&e2), vec!["spaced quote".to_string()]);
        assert_eq!(editor_cursor(&e2), (0, 12));
    }

    #[test]
    fn indenting_and_unindenting_snaps_to_proper_levels() {
        // Tab from irregular spaces rounds up to next multiple of 3
        let mut e = make_editor(" - bullet");
        press(&mut e, KeyCode::Tab);
        assert_eq!(editor_lines(&e), vec!["   - bullet".to_string()]);

        let mut e = make_editor("  - bullet");
        press(&mut e, KeyCode::Tab);
        assert_eq!(editor_lines(&e), vec!["   - bullet".to_string()]);

        let mut e = make_editor("   - bullet");
        press(&mut e, KeyCode::Tab);
        assert_eq!(editor_lines(&e), vec!["      - bullet".to_string()]);

        let mut e = make_editor("    - bullet");
        press(&mut e, KeyCode::Tab);
        assert_eq!(editor_lines(&e), vec!["      - bullet".to_string()]);

        let mut e = make_editor("     - bullet");
        press(&mut e, KeyCode::Tab);
        assert_eq!(editor_lines(&e), vec!["      - bullet".to_string()]);

        // Shift+Tab from irregular spaces rounds down to previous multiple of 3
        let mut e = make_editor("     - bullet");
        press(&mut e, KeyCode::BackTab);
        assert_eq!(editor_lines(&e), vec!["   - bullet".to_string()]);

        let mut e = make_editor("    - bullet");
        press(&mut e, KeyCode::BackTab);
        assert_eq!(editor_lines(&e), vec!["   - bullet".to_string()]);

        let mut e = make_editor("   - bullet");
        press(&mut e, KeyCode::BackTab);
        assert_eq!(editor_lines(&e), vec!["- bullet".to_string()]);

        let mut e = make_editor("  - bullet");
        press(&mut e, KeyCode::BackTab);
        assert_eq!(editor_lines(&e), vec!["- bullet".to_string()]);

        let mut e = make_editor(" - bullet");
        press(&mut e, KeyCode::BackTab);
        assert_eq!(editor_lines(&e), vec!["- bullet".to_string()]);

        // Citations also snap to proper levels
        let mut e = make_editor("> quote");
        press(&mut e, KeyCode::Tab);
        assert_eq!(editor_lines(&e), vec!["   > quote".to_string()]);

        let mut e = make_editor("  > quote");
        press(&mut e, KeyCode::Tab);
        assert_eq!(editor_lines(&e), vec!["   > quote".to_string()]);

        let mut e = make_editor("    > quote");
        press(&mut e, KeyCode::Tab);
        assert_eq!(editor_lines(&e), vec!["      > quote".to_string()]);

        let mut e = make_editor("      > quote");
        press(&mut e, KeyCode::BackTab);
        assert_eq!(editor_lines(&e), vec!["   > quote".to_string()]);

        let mut e = make_editor("    > quote");
        press(&mut e, KeyCode::BackTab);
        assert_eq!(editor_lines(&e), vec!["   > quote".to_string()]);

        let mut e = make_editor("   > quote");
        press(&mut e, KeyCode::BackTab);
        assert_eq!(editor_lines(&e), vec!["> quote".to_string()]);

        let mut e = make_editor("  > quote");
        press(&mut e, KeyCode::BackTab);
        assert_eq!(editor_lines(&e), vec!["> quote".to_string()]);
    }

    fn nest_at(initial: &str, row: usize, col: usize) -> MarkdownEditor {
        let mut e = make_editor(initial);
        e.input
            .textarea
            .move_cursor(CursorMove::Jump(row as u16, col as u16));
        press(&mut e, KeyCode::Tab);
        e
    }

    #[test]
    fn tab_numbered_item_joins_existing_deeper_bullet_list() {
        let e = nest_at("1. a\n   - x\n   - y\n2. b", 3, 4);
        assert_eq!(
            editor_lines(&e),
            vec![
                "1. a".to_string(),
                "   - x".to_string(),
                "   - y".to_string(),
                "   - b".to_string()
            ]
        );
        assert_eq!(editor_cursor(&e), (3, 6));
    }

    #[test]
    fn tab_bullet_item_joins_existing_deeper_numbered_list_and_continues_numbers() {
        let e = nest_at("- a\n   1. x\n   2. y\n- b", 3, 3);
        assert_eq!(
            editor_lines(&e),
            vec![
                "- a".to_string(),
                "   1. x".to_string(),
                "   2. y".to_string(),
                "   3. b".to_string()
            ]
        );
        assert_eq!(editor_cursor(&e), (3, 7));
    }

    #[test]
    fn tab_adopts_bullet_char_of_existing_deeper_list() {
        let e = nest_at("- a\n   * x\n- b", 2, 3);
        assert_eq!(
            editor_lines(&e),
            vec![
                "- a".to_string(),
                "   * x".to_string(),
                "   * b".to_string()
            ]
        );
    }

    #[test]
    fn tab_cursor_follows_text_when_marker_is_rewritten() {
        let e = nest_at("1. a\n   - x\n2. bee", 2, 6);
        assert_eq!(editor_lines(&e)[2], "   - bee");
        assert_eq!(editor_cursor(&e), (2, 8));
    }

    #[test]
    fn tab_does_not_adopt_scheme_across_a_blank_line_or_without_deeper_list() {
        let e = nest_at("1. a\n   - x\n\n2. b", 3, 4);
        assert_eq!(editor_lines(&e)[3], "   1. b");
        let e = nest_at("- a\n1. b", 1, 4);
        assert_eq!(
            editor_lines(&e),
            vec!["- a".to_string(), "   1. b".to_string()]
        );
    }

    #[test]
    fn tab_then_shift_tab_restores_the_item() {
        let mut e = nest_at("- a\n   1. x\n- b", 2, 3);
        assert_eq!(editor_lines(&e)[2], "   2. b");
        press(&mut e, KeyCode::BackTab);
        assert_eq!(
            editor_lines(&e),
            vec!["- a".to_string(), "   1. x".to_string(), "- b".to_string()]
        );
        assert_eq!(editor_cursor(&e), (2, 3));
    }

    fn unnest_at(initial: &str, row: usize, col: usize) -> MarkdownEditor {
        let mut e = make_editor(initial);
        e.input
            .textarea
            .move_cursor(CursorMove::Jump(row as u16, col as u16));
        press(&mut e, KeyCode::BackTab);
        e
    }

    #[test]
    fn shift_tab_numbered_child_of_bullet_becomes_bullet() {
        let e = unnest_at("- parent\n   1. child", 1, 12);
        assert_eq!(
            editor_lines(&e),
            vec!["- parent".to_string(), "- child".to_string()]
        );
        assert_eq!(editor_cursor(&e), (1, 7));
    }

    #[test]
    fn shift_tab_bullet_child_of_numbered_continues_numbering() {
        let e = unnest_at("1. a\n2. b\n   - c", 2, 8);
        assert_eq!(
            editor_lines(&e),
            vec!["1. a".to_string(), "2. b".to_string(), "3. c".to_string()]
        );
        assert_eq!(editor_cursor(&e), (2, 4));
    }

    #[test]
    fn shift_tab_numbered_child_continues_parent_numbers_and_renumbers_below() {
        let e = unnest_at("1. a\n   1. x\n   2. y\n2. b", 2, 7);
        assert_eq!(
            editor_lines(&e),
            vec![
                "1. a".to_string(),
                "   1. x".to_string(),
                "2. y".to_string(),
                "3. b".to_string()
            ]
        );
    }

    #[test]
    fn shift_tab_adopts_parent_bullet_char() {
        let e = unnest_at("* a\n   - b", 1, 5);
        assert_eq!(editor_lines(&e), vec!["* a".to_string(), "* b".to_string()]);
    }

    #[test]
    fn shift_tab_skips_deeper_lines_to_find_parent_scheme() {
        let e = unnest_at("- a\n   1. x\n      - deep\n   2. y", 3, 7);
        assert_eq!(
            editor_lines(&e),
            vec![
                "- a".to_string(),
                "   1. x".to_string(),
                "      - deep".to_string(),
                "- y".to_string()
            ]
        );
    }

    #[test]
    fn shift_tab_cursor_before_marker_only_loses_indent() {
        let e = unnest_at("- a\n   1. x", 1, 3);
        assert_eq!(editor_lines(&e), vec!["- a".to_string(), "- x".to_string()]);
        assert_eq!(editor_cursor(&e), (1, 0));
    }

    #[test]
    fn shift_tab_without_a_parent_item_keeps_scheme() {
        let e = unnest_at("intro\n   - a", 1, 5);
        assert_eq!(
            editor_lines(&e),
            vec!["intro".to_string(), "- a".to_string()]
        );
        let e = unnest_at("\n   1. a", 1, 6);
        assert_eq!(editor_lines(&e), vec!["".to_string(), "1. a".to_string()]);
    }

    #[test]
    fn deleting_blank_line_between_list_items_renumbers() {
        let mut e = make_editor("1. a\n\n5. b\n7. c");
        e.input.textarea.move_cursor(CursorMove::Top);
        e.input.textarea.move_cursor(CursorMove::Down);
        e.input.textarea.move_cursor(CursorMove::Head);
        press(&mut e, KeyCode::Backspace);
        assert_eq!(
            editor_lines(&e),
            vec!["1. a".to_string(), "2. b".to_string(), "3. c".to_string()]
        );
    }

    #[test]
    fn deleting_numbered_item_via_line_merge_renumbers() {
        let mut e = make_editor("1. a\n2. b\n3. c");
        e.input.textarea.move_cursor(CursorMove::Top);
        e.input.textarea.move_cursor(CursorMove::Down);
        e.input.textarea.move_cursor(CursorMove::Head);
        press(&mut e, KeyCode::Backspace);
        assert_eq!(
            editor_lines(&e),
            vec!["1. a2. b".to_string(), "2. c".to_string()]
        );
    }

    #[test]
    fn typing_does_not_trigger_renumber() {
        let mut e = make_editor("1. a\n5. b");
        e.input.textarea.move_cursor(CursorMove::Top);
        e.input.textarea.move_cursor(CursorMove::End);
        press(&mut e, KeyCode::Char('x'));
        assert_eq!(
            editor_lines(&e),
            vec!["1. ax".to_string(), "5. b".to_string()]
        );
    }

    #[test]
    fn ctrl_s_returns_save_and_resets_is_modified() {
        let mut e = make_editor("initial");
        press(&mut e, KeyCode::Char('!'));
        assert!(e.input.is_modified());

        let out = e.handle_key(
            KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL),
            None,
        );
        match out {
            InsertOutcome::Save(Command::EditCardDescription { card_id, body }) => {
                assert_eq!(card_id, "card1");
                assert_eq!(body, "initial!");
            }
            _ => panic!("expected InsertOutcome::Save"),
        }
        // After Ctrl+S, is_modified is reset
        assert!(!e.input.is_modified());

        // Subsequent Esc should now cancel directly, not prompt
        let esc_out = e.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::empty()), None);
        assert!(matches!(esc_out, InsertOutcome::Cancel));
    }

    #[test]
    fn ctrl_q_returns_confirm() {
        let mut e = make_editor("initial");
        press(&mut e, KeyCode::Char('!'));

        let out = e.handle_key(
            KeyEvent::new(KeyCode::Char('q'), KeyModifiers::CONTROL),
            None,
        );
        match out {
            InsertOutcome::Confirm(Command::EditCardDescription { card_id, body }) => {
                assert_eq!(card_id, "card1");
                assert_eq!(body, "initial!");
            }
            _ => panic!("expected InsertOutcome::Confirm"),
        }
    }

    #[test]
    fn typing_bare_s_and_q_inserts_characters() {
        let mut e = make_editor("hello ");
        press(&mut e, KeyCode::Char('s'));
        press(&mut e, KeyCode::Char('q'));
        assert_eq!(editor_lines(&e), vec!["hello sq".to_string()]);
    }
}
