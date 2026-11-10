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
use crate::footnote;
use crate::model::id::CardId;

const NEST_INDENT: usize = 3;

pub struct MarkdownEditor {
    pub input: TextAreaInput,
    pub card_id: CardId,
    /// `Ctrl+T` was pressed inside a table; the next key picks the table edit.
    table_chord: bool,
    /// `Ctrl+G` was pressed inside a quote; the next key picks the alert kind.
    alert_chord: bool,
    /// Paste mode: keys and pastes insert plain text (no list continuation,
    /// renumbering or table reflow) until the text is normalized on leaving.
    paste_mode: bool,
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
            paste_mode: false,
        }
    }

    pub fn paste_mode(&self) -> bool {
        self.paste_mode
    }

    /// Leave paste mode (if on) and normalize the whole text as one undo step.
    fn leave_paste_mode(&mut self) {
        if std::mem::take(&mut self.paste_mode) {
            grouped(&mut self.input.textarea, None, normalize);
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
        let raw = self.paste_mode;
        grouped(&mut self.input.textarea, None, |ta| {
            ta.set_yank_text(normalized);
            ta.paste();
            if !raw {
                renumber_all(ta);
                table_edit::reformat_at_cursor(ta);
            }
        });
    }

    pub fn paste_from_clipboard(&mut self) {
        if let Some(text) = crate::app::get_from_system_clipboard()
            && !text.is_empty()
        {
            self.paste_text(&text);
            return;
        }
        let raw = self.paste_mode;
        grouped(&mut self.input.textarea, None, |ta| {
            ta.paste();
            if !raw {
                renumber_all(ta);
                table_edit::reformat_at_cursor(ta);
            }
        });
    }

    /// `Ctrl+N`: in a footnote definition, jump to its first reference; elsewhere
    /// insert the next numbered reference and jump to its new definition at the
    /// bottom, ready to type.
    fn footnote(&mut self) -> InsertOutcome {
        let ta = &mut self.input.textarea;
        let (row, _) = ta.cursor();
        if let Some(def) = footnote::definition_at(ta.lines(), row) {
            let Some((r, c)) = footnote::first_ref(ta.lines(), &def.label) else {
                return InsertOutcome::StayWithStatus(Some(format!(
                    "No reference to [^{}]",
                    def.label
                )));
            };
            ta.cancel_selection();
            let end = c + 3 + def.label.chars().count();
            ta.move_cursor(CursorMove::Jump(r as u16, end as u16));
            return InsertOutcome::Stay;
        }
        grouped(ta, None, insert_footnote);
        InsertOutcome::Stay
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
            && !self.paste_mode
            && table_edit::in_table(&self.input.textarea)
        {
            let refused = table_edit::run_op(&mut self.input.textarea, op);
            return InsertOutcome::StayWithStatus(refused);
        }
        match (key.code, key.modifiers) {
            (KeyCode::Char('p'), m) if has_ctrl_or_cmd(m) => {
                if self.paste_mode {
                    self.leave_paste_mode();
                    InsertOutcome::StayWithStatus(Some("Paste mode off".into()))
                } else {
                    self.paste_mode = true;
                    InsertOutcome::StayWithStatus(Some("Paste mode on".into()))
                }
            }
            (KeyCode::Char('s'), m) if has_ctrl_or_cmd(m) => {
                self.leave_paste_mode();
                let body = self.input.current_text();
                self.input.original = body.clone();
                InsertOutcome::Save(Command::EditCardDescription {
                    card_id: self.card_id.clone(),
                    body,
                })
            }
            (KeyCode::Char('q'), m) if has_ctrl_or_cmd(m) => {
                self.leave_paste_mode();
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
                    && !self.paste_mode
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
            (KeyCode::Char('n'), m) if has_ctrl_or_cmd(m) => self.footnote(),
            (KeyCode::Char('l'), m) if has_ctrl_or_cmd(m) => {
                insert_at_line_start(&mut self.input.textarea, "- ");
                InsertOutcome::Stay
            }
            (KeyCode::Char('t'), m) if has_ctrl_or_cmd(m) && !self.paste_mode => {
                if table_edit::in_table(&self.input.textarea) {
                    self.table_chord = true;
                    return InsertOutcome::StayWithStatus(Some(table_edit::CHORD_HINT.into()));
                }
                table_edit::insert_skeleton(&mut self.input.textarea);
                InsertOutcome::Stay
            }
            (KeyCode::Enter | KeyCode::Tab, _) if self.paste_mode => {
                self.input.textarea.input(key);
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
        if self.paste_mode {
            self.input.textarea.input(key);
            return;
        }
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
    if let Some(dot) = trimmed.find(". ")
        && trimmed[..dot].parse::<u64>().is_ok()
    {
        return Some((Marker::Numbered, dot + 2));
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

/// Insert a footnote reference and add its empty definition, with the cursor
/// behind it: a numbered one among the numbered definitions by its number, any
/// other at the very bottom. Without a selection the reference is the next
/// number (numbered footnotes are renumbered to ascend by first reference);
/// highlighted text stays and is named after instead (`[^two-words]`, inserted
/// behind it). A name that is already defined just jumps to that definition.
/// Definitions form one block at the end; anything else gets a blank line first.
fn insert_footnote(ta: &mut tui_textarea::TextArea<'static>) {
    let selected = ta.selection_range().map(|(start, end)| {
        let lines = ta.lines();
        let text = if start.0 == end.0 {
            lines[start.0]
                .chars()
                .skip(start.1)
                .take(end.1 - start.1)
                .collect()
        } else {
            let first: String = lines[start.0].chars().skip(start.1).collect();
            let last: String = lines[end.0].chars().take(end.1).collect();
            let middle = lines[start.0 + 1..end.0].join(" ");
            format!("{first} {middle} {last}")
        };
        (end, text)
    });
    ta.cancel_selection();
    let named = selected.and_then(|(end, text)| {
        let label = footnote::label_from(&text)?;
        ta.move_cursor(CursorMove::Jump(end.0 as u16, end.1 as u16));
        Some(label)
    });

    let label = match named {
        Some(label) => {
            leave_reference(ta);
            ta.insert_str(format!("[^{label}]"));
            table_edit::reformat_at_cursor(ta);
            label
        }
        None => insert_numbered_reference(ta),
    };

    if let Some(def) = footnote::definitions(ta.lines())
        .into_iter()
        .find(|d| d.label == label)
    {
        let end = ta.lines()[def.end].chars().count();
        ta.move_cursor(CursorMove::Jump(def.end as u16, end as u16));
        return;
    }

    if place_numbered_definition(ta, &label) {
        return;
    }

    let lines = ta.lines();
    let last = lines
        .iter()
        .rposition(|l| !l.trim().is_empty())
        .unwrap_or(0);
    if last + 1 < lines.len() {
        // Trailing blank lines make way for the definition.
        let chars: usize = lines[last + 1..]
            .iter()
            .map(|l| l.chars().count() + 1)
            .sum();
        let end = lines[last].chars().count();
        ta.move_cursor(CursorMove::Jump(last as u16, end as u16));
        ta.delete_str(chars);
    }
    let lines = ta.lines();
    let last = lines.len() - 1;
    let in_defs = footnote::definitions(lines)
        .last()
        .is_some_and(|d| d.end == last);
    ta.move_cursor(CursorMove::Jump(
        last as u16,
        lines[last].chars().count() as u16,
    ));
    let gap = if in_defs { "\n" } else { "\n\n" };
    ta.insert_str(format!("{gap}[^{label}]: "));
}

/// A cursor inside an existing `[^x]` would split it: move behind it.
fn leave_reference(ta: &mut tui_textarea::TextArea<'static>) {
    let (row, col) = ta.cursor();
    let inside = footnote::refs(&ta.lines()[row])
        .into_iter()
        .find(|(start, len, _)| *start < col && col < start + len);
    if let Some((start, len, _)) = inside {
        ta.move_cursor(CursorMove::Jump(row as u16, (start + len) as u16));
    }
}

/// Put the empty definition of a numbered footnote among the other numbered
/// definitions, where its number belongs: right behind the definition with the
/// highest smaller number, else in front of the one with the lowest larger
/// number. Cursor behind the new `[^n]: `. Returns false (nothing done) for a
/// named footnote or when there is no numbered definition to line up with.
fn place_numbered_definition(ta: &mut tui_textarea::TextArea<'static>, label: &str) -> bool {
    let Ok(n) = label.parse::<u64>() else {
        return false;
    };
    let numbered: Vec<(u64, footnote::Def)> = footnote::definitions(ta.lines())
        .into_iter()
        .filter_map(|d| Some((d.label.parse::<u64>().ok()?, d)))
        .collect();
    let before = numbered
        .iter()
        .filter(|(m, _)| *m < n)
        .max_by_key(|(m, d)| (*m, d.start));
    let after = numbered
        .iter()
        .filter(|(m, _)| *m > n)
        .min_by_key(|(m, d)| (*m, d.start));
    let marker = format!("[^{label}]: ");
    if let Some((_, def)) = before {
        let end = ta.lines()[def.end].chars().count();
        ta.move_cursor(CursorMove::Jump(def.end as u16, end as u16));
        ta.insert_str(format!("\n{marker}"));
    } else if let Some((_, def)) = after {
        ta.move_cursor(CursorMove::Jump(def.start as u16, 0));
        ta.insert_str(format!("{marker}\n"));
        let col = marker.chars().count();
        ta.move_cursor(CursorMove::Jump(def.start as u16, col as u16));
    } else {
        return false;
    }
    true
}

/// Insert `[^n]` at the cursor and renumber the numbered footnotes so they
/// ascend in the order of their references; returns the label it got.
fn insert_numbered_reference(ta: &mut tui_textarea::TextArea<'static>) -> String {
    let placeholder = footnote::next_number(ta.lines()).to_string();
    leave_reference(ta);
    ta.insert_str(format!("[^{placeholder}]"));
    let (row, col) = ta.cursor();
    let old = ta.lines().to_vec();
    let renumbered = footnote::renumbered(&old, Some(&placeholder));
    let label = renumbered.label(&placeholder);
    let prefix: String = old[row].chars().take(col).collect();
    let new_col = renumbered.relabel(&prefix).chars().count();
    for (r, (was, now)) in old.iter().zip(&renumbered.lines).enumerate() {
        if was != now {
            ta.move_cursor(CursorMove::Jump(r as u16, 0));
            ta.delete_str(was.chars().count());
            ta.insert_str(now);
        }
    }
    ta.move_cursor(CursorMove::Jump(row as u16, new_col as u16));
    table_edit::reformat_at_cursor(ta);
    label
}

/// Normalize pasted markdown: reflow tables, snap list indentation to
/// `NEST_INDENT` levels, renumber ordered lists. Fenced code is left alone.
pub(crate) fn normalize(ta: &mut tui_textarea::TextArea<'static>) {
    let fenced = fenced_rows(ta.lines());
    table_edit::reformat_all(ta, &fenced);
    fix_list_indent(ta, &fenced);
    renumber_all(ta);
}

/// Rows that are inside (or are the fence lines of) a fenced code block.
fn fenced_rows(lines: &[String]) -> Vec<bool> {
    let mut open = false;
    lines
        .iter()
        .map(|l| {
            let t = l.trim_start();
            let fence = t.starts_with("```") || t.starts_with("~~~");
            let inside = open || fence;
            if fence {
                open = !open;
            }
            inside
        })
        .collect()
}

/// Rewrite the indent of list items to `NEST_INDENT` per nesting level,
/// whatever width the source used (2, 4, tabs).
fn fix_list_indent(ta: &mut tui_textarea::TextArea<'static>, fenced: &[bool]) {
    let lines = ta.lines().to_vec();
    let (cr, cc) = ta.cursor();
    let mut stack: Vec<usize> = Vec::new();
    let mut edits = Vec::new();
    for (r, line) in lines.iter().enumerate() {
        if fenced.get(r).copied().unwrap_or(false) || line.trim().is_empty() {
            if line.trim().is_empty() {
                continue;
            }
            stack.clear();
            continue;
        }
        let trimmed = line.trim_start();
        let ws = &line[..line.len() - trimmed.len()];
        let width: usize = ws.chars().map(|c| if c == '\t' { 4 } else { 1 }).sum();
        match parse_marker(trimmed) {
            Some((Marker::Bullet(_) | Marker::Numbered, _)) => {
                while stack.last().is_some_and(|&t| t > width) {
                    stack.pop();
                }
                if stack.last() != Some(&width) {
                    stack.push(width);
                }
                let new = (stack.len() - 1) * NEST_INDENT;
                if new != ws.chars().count() || ws.contains('\t') {
                    edits.push((r, ws.chars().count(), new));
                }
            }
            _ if width == 0 => stack.clear(),
            _ => {}
        }
    }
    for (r, old, new) in edits {
        ta.move_cursor(CursorMove::Jump(r as u16, 0));
        ta.delete_str(old);
        ta.insert_str(" ".repeat(new));
    }
    // Keep the cursor on the same character.
    let lead = |l: &str| l.chars().take_while(|c| c.is_whitespace()).count();
    let col = match (lines.get(cr), ta.lines().get(cr)) {
        (Some(was), Some(now)) if cc >= lead(was) => cc + lead(now) - lead(was).min(cc + lead(now)),
        _ => cc,
    };
    ta.move_cursor(CursorMove::Jump(cr as u16, col as u16));
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

    fn paste_mode_on(e: &mut MarkdownEditor) {
        ctrl(e, 'p');
        assert!(e.paste_mode());
    }

    #[test]
    fn paste_mode_inserts_plain_text_without_list_or_table_magic() {
        let mut e = make_editor("");
        paste_mode_on(&mut e);
        e.handle_paste("1. a\n5. b\n|a|b|\n|-|-|\n|long cell|c|");
        assert_eq!(
            editor_lines(&e),
            ["1. a", "5. b", "|a|b|", "|-|-|", "|long cell|c|"]
        );
        // Enter after a list item does not continue the list.
        press(&mut e, KeyCode::Enter);
        assert_eq!(editor_lines(&e).len(), 6);
        assert_eq!(editor_lines(&e)[5], "");
    }

    #[test]
    fn leaving_paste_mode_normalizes_in_one_undo_step() {
        let mut e = make_editor("");
        paste_mode_on(&mut e);
        e.handle_paste(
            "1. a\n5. b\n  - x\n    - y\n\t- z\n|a|b|\n|-|-|\n|long cell|c|\n```\n  - keep\n```",
        );
        ctrl(&mut e, 'p');
        assert!(!e.paste_mode());
        let lines = editor_lines(&e);
        assert_eq!(&lines[..3], ["1. a", "2. b", "   - x"]);
        assert_eq!(&lines[3..5], ["      - y", "      - z"]);
        assert!(
            lines[5].contains("long cell") || lines[7].contains("long cell"),
            "{lines:?}"
        );
        assert!(lines[5].len() == lines[7].len(), "{lines:?}");
        assert_eq!(&lines[8..], ["```", "  - keep", "```"]);
        e.input.textarea.undo();
        assert_eq!(editor_lines(&e)[1], "5. b");
    }

    #[test]
    fn saving_leaves_paste_mode_and_saves_normalized_text() {
        let mut e = make_editor("");
        paste_mode_on(&mut e);
        e.handle_paste("1. a\n5. b");
        match ctrl_out(&mut e, 's') {
            InsertOutcome::Save(Command::EditCardDescription { body, .. }) => {
                assert_eq!(body, "1. a\n2. b")
            }
            _ => panic!("expected Save"),
        }
        assert!(!e.paste_mode());
    }

    #[test]
    fn paste_mode_keys_are_plain() {
        let mut e = make_editor("- a");
        paste_mode_on(&mut e);
        press(&mut e, KeyCode::Enter);
        assert_eq!(editor_lines(&e), ["- a", ""]);
    }

    fn footnote_key(e: &mut MarkdownEditor) -> InsertOutcome {
        ctrl_out(e, 'n')
    }

    #[test]
    fn ctrl_n_adds_a_reference_and_a_definition_at_the_bottom() {
        let mut e = make_editor("Hello");
        footnote_key(&mut e);
        assert_eq!(editor_lines(&e), ["Hello[^1]", "", "[^1]: "]);
        assert_eq!(editor_cursor(&e), (2, 6));
        // The cursor is ready to type the note.
        e.handle_paste("My note");
        assert_eq!(editor_lines(&e)[2], "[^1]: My note");
    }

    #[test]
    fn ctrl_n_counts_up_and_stacks_definitions_without_blank_lines() {
        let mut e = make_editor("a b\n\nmore");
        cursor_at(&mut e, 0, 1);
        footnote_key(&mut e);
        cursor_at(&mut e, 2, 4);
        footnote_key(&mut e);
        cursor_at(&mut e, 0, 0);
        footnote_key(&mut e);
        assert_eq!(
            editor_lines(&e),
            [
                "[^1]a[^2] b",
                "",
                "more[^3]",
                "",
                "[^1]: ",
                "[^2]: ",
                "[^3]: "
            ]
        );
        assert_eq!(editor_cursor(&e), (4, 6));
    }

    #[test]
    fn ctrl_n_renumbers_numbered_footnotes_in_ascending_order_of_reference() {
        let mut e = make_editor("x[^7] y[^name]\n\n[^7]: seven\n[^name]: n");
        cursor_at(&mut e, 0, 0);
        footnote_key(&mut e);
        assert_eq!(
            editor_lines(&e),
            [
                "[^1]x[^2] y[^name]",
                "",
                "[^1]: ",
                "[^2]: seven",
                "[^name]: n"
            ]
        );
    }

    #[test]
    fn ctrl_n_between_references_shifts_the_later_ones_and_keeps_the_cursor_in_place() {
        let mut e = make_editor("a[^1] b[^2] c[^3]\n\n[^1]: one\n[^2]: two\n[^3]: three");
        cursor_at(&mut e, 0, 6);
        footnote_key(&mut e);
        assert_eq!(
            editor_lines(&e),
            [
                "a[^1] [^2]b[^3] c[^4]",
                "",
                "[^1]: one",
                "[^2]: ",
                "[^3]: two",
                "[^4]: three"
            ]
        );
        assert_eq!(editor_cursor(&e), (3, 6));
        // Jumping back lands behind the new reference.
        footnote_key(&mut e);
        assert_eq!(editor_cursor(&e), (0, 10));
    }

    #[test]
    fn ctrl_n_closes_gaps_and_leaves_names_and_code_alone() {
        let mut e = make_editor("a[^5] [^n]\n```\n[^5]\n```\n\n[^5]: five\n[^n]: name");
        cursor_at(&mut e, 0, 0);
        footnote_key(&mut e);
        let lines = editor_lines(&e);
        assert_eq!(lines[0], "[^1]a[^2] [^n]");
        assert_eq!(lines[2], "[^5]");
        assert_eq!(&lines[5..], ["[^1]: ", "[^2]: five", "[^n]: name"]);
    }

    #[test]
    fn ctrl_n_replaces_trailing_blank_lines_and_is_one_undo_step() {
        let mut e = make_editor("a\n\n\n");
        cursor_at(&mut e, 0, 1);
        footnote_key(&mut e);
        assert_eq!(editor_lines(&e), ["a[^1]", "", "[^1]: "]);
        e.input.textarea.undo();
        assert_eq!(editor_lines(&e), ["a", "", "", ""]);
    }

    #[test]
    fn ctrl_n_after_a_multi_line_definition_still_stacks() {
        let mut e = make_editor("a[^1]\n\n[^1]: one  \nsecond line");
        cursor_at(&mut e, 0, 0);
        footnote_key(&mut e);
        assert_eq!(
            editor_lines(&e),
            ["[^1]a[^2]", "", "[^1]: ", "[^2]: one  ", "second line"]
        );
    }

    #[test]
    fn ctrl_n_in_a_definition_jumps_back_to_the_first_reference() {
        let mut e = make_editor("a[^2] b[^1] c[^1]\n\n[^1]: one  \nsecond\n[^2]: two");
        cursor_at(&mut e, 2, 8);
        footnote_key(&mut e);
        assert_eq!(editor_cursor(&e), (0, 11));
        // Also from a continuation line, and for the other label.
        cursor_at(&mut e, 3, 2);
        footnote_key(&mut e);
        assert_eq!(editor_cursor(&e), (0, 11));
        cursor_at(&mut e, 4, 0);
        footnote_key(&mut e);
        assert_eq!(editor_cursor(&e), (0, 5));
    }

    #[test]
    fn ctrl_n_in_an_unreferenced_definition_reports_and_stays() {
        let mut e = make_editor("text\n\n[^x]: orphan");
        cursor_at(&mut e, 2, 3);
        let out = footnote_key(&mut e);
        assert!(matches!(out, InsertOutcome::StayWithStatus(Some(m)) if m.contains("[^x]")));
        assert_eq!(editor_cursor(&e), (2, 3));
        assert_eq!(editor_lines(&e).len(), 3);
    }

    #[test]
    fn ctrl_n_in_a_table_cell_reflows_the_table() {
        let mut e = make_editor("| a | b |\n|---|---|\n| 1 | 2 |");
        cursor_at(&mut e, 2, 3);
        footnote_key(&mut e);
        let lines = editor_lines(&e);
        assert!(lines[2].contains("1[^1]"), "{lines:?}");
        assert_eq!(lines[0].len(), lines[2].len());
        assert_eq!(lines.last().unwrap(), "[^1]: ");
    }

    #[test]
    fn ctrl_n_names_the_footnote_after_highlighted_text_and_keeps_the_text() {
        let mut e = make_editor("hello big world");
        cursor_at(&mut e, 0, 6);
        e.input.textarea.start_selection();
        e.input.textarea.move_cursor(CursorMove::Jump(0, 9));
        footnote_key(&mut e);
        assert_eq!(editor_lines(&e), ["hello big[^big] world", "", "[^big]: "]);
        assert_eq!(editor_cursor(&e), (2, 8));
        assert!(!e.input.textarea.is_selecting());
        // Back from the definition lands behind the reference.
        footnote_key(&mut e);
        assert_eq!(editor_cursor(&e), (0, 15));
    }

    fn select(e: &mut MarkdownEditor, from: (usize, usize), to: (usize, usize)) {
        cursor_at(e, from.0, from.1);
        e.input.textarea.start_selection();
        e.input
            .textarea
            .move_cursor(CursorMove::Jump(to.0 as u16, to.1 as u16));
    }

    #[test]
    fn highlighted_words_and_lines_become_a_dashed_label_and_backward_selection_works() {
        let mut e = make_editor("two words\nnext line");
        select(&mut e, (0, 0), (1, 4));
        footnote_key(&mut e);
        assert_eq!(editor_lines(&e)[1], "next[^two-words-next] line");
        assert_eq!(editor_lines(&e).last().unwrap(), "[^two-words-next]: ");

        let mut e = make_editor("alpha beta");
        select(&mut e, (0, 5), (0, 0)); // cursor before the anchor
        footnote_key(&mut e);
        assert_eq!(editor_lines(&e)[0], "alpha[^alpha] beta");
    }

    #[test]
    fn highlighted_text_without_a_usable_name_falls_back_to_numbering() {
        for text in ["12", "[^]", "   "] {
            let mut e = make_editor(&format!("x {text} y"));
            select(&mut e, (0, 2), (0, 2 + text.len()));
            footnote_key(&mut e);
            let lines = editor_lines(&e);
            assert!(lines[0].contains("[^1]"), "{text}: {lines:?}");
            assert_eq!(lines.last().unwrap(), "[^1]: ", "{text}");
        }
        // An empty selection is no selection.
        let mut e = make_editor("x");
        select(&mut e, (0, 1), (0, 1));
        footnote_key(&mut e);
        assert_eq!(editor_lines(&e), ["x[^1]", "", "[^1]: "]);
    }

    #[test]
    fn a_name_that_is_already_defined_jumps_to_its_definition_without_a_second_one() {
        let mut e = make_editor("see term and term\n\n[^term]: Defined.");
        select(&mut e, (0, 13), (0, 17));
        footnote_key(&mut e);
        assert_eq!(
            editor_lines(&e),
            ["see term and term[^term]", "", "[^term]: Defined."]
        );
        assert_eq!(editor_cursor(&e), (2, 17));
    }

    #[test]
    fn named_footnotes_do_not_renumber_the_numbered_ones() {
        let mut e = make_editor("a[^2] b[^5] word");
        select(&mut e, (0, 12), (0, 16));
        footnote_key(&mut e);
        assert_eq!(editor_lines(&e)[0], "a[^2] b[^5] word[^word]");
    }

    #[test]
    fn highlighted_text_in_a_table_cell_names_the_footnote_and_reflows() {
        let mut e = make_editor("| a | b |\n|---|---|\n| one | 2 |");
        select(&mut e, (2, 2), (2, 5));
        footnote_key(&mut e);
        let lines = editor_lines(&e);
        assert!(lines[2].contains("one[^one]"), "{lines:?}");
        assert_eq!(lines[0].len(), lines[2].len());
    }

    #[test]
    fn ctrl_n_works_in_paste_mode() {
        let mut e = make_editor("x");
        paste_mode_on(&mut e);
        footnote_key(&mut e);
        assert_eq!(editor_lines(&e), ["x[^1]", "", "[^1]: "]);
    }

    /// The definition text each reference of `lines` points at (`None`: dangling).
    fn targets(lines: &[String]) -> Vec<Option<String>> {
        footnote::resolved(lines)
            .into_iter()
            .map(|row| {
                row.map(|r| {
                    // Labels inside the note text are renumbered along; blank them.
                    let text = footnote::def_start(&lines[r]).unwrap().2;
                    let mut out: Vec<char> = text.chars().collect();
                    for (start, len, _) in footnote::refs(&text).into_iter().rev() {
                        out.splice(start..start + len, "[^]".chars());
                    }
                    out.into_iter().collect()
                })
            })
            .collect()
    }

    /// How many references precede the cursor (outside code).
    fn refs_before(lines: &[String], row: usize, col: usize) -> usize {
        let fenced = footnote::fenced_rows(lines);
        let earlier: usize = (0..row)
            .filter(|&r| !fenced[r])
            .map(|r| footnote::refs(&lines[r]).len())
            .sum();
        let on_row = footnote::refs(&lines[row])
            .iter()
            .filter(|(c, _, _)| *c < col)
            .count();
        earlier + on_row
    }

    #[test]
    fn a_numbered_definition_goes_where_its_number_belongs() {
        let mut e = make_editor("a[^1] b[^3]\n\n[^1]: one  \nmore\n\n    para\n[^3]: three");
        cursor_at(&mut e, 0, 6);
        footnote_key(&mut e);
        assert_eq!(
            editor_lines(&e),
            [
                "a[^1] [^2]b[^3]",
                "",
                "[^1]: one  ",
                "more",
                "",
                "    para",
                "[^2]: ",
                "[^3]: three"
            ]
        );
        assert_eq!(editor_cursor(&e), (6, 6));
    }

    #[test]
    fn definitions_spread_through_the_text_get_the_new_one_behind_its_predecessor() {
        let mut e = make_editor("a b\n\n[^1]: one\n\ntext\n\n[^3]: three\n\nend");
        cursor_at(&mut e, 0, 1);
        footnote_key(&mut e);
        // [^1] and [^3] are unreferenced, so they keep their numbers.
        assert_eq!(
            editor_lines(&e),
            [
                "a[^2] b",
                "",
                "[^1]: one",
                "[^2]: ",
                "",
                "text",
                "",
                "[^3]: three",
                "",
                "end"
            ]
        );
    }

    #[test]
    fn named_definitions_do_not_take_part_in_the_order() {
        let mut e = make_editor("x[^1]\n\n[^note]: n\n[^1]: one");
        cursor_at(&mut e, 0, 0);
        footnote_key(&mut e);
        assert_eq!(
            editor_lines(&e),
            ["[^1]x[^2]", "", "[^note]: n", "[^1]: ", "[^2]: one"]
        );
    }

    #[test]
    fn a_footnote_named_by_selection_still_goes_to_the_bottom() {
        let mut e = make_editor("x[^1] word\n\n[^1]: one\n\ntail");
        select(&mut e, (0, 6), (0, 10));
        footnote_key(&mut e);
        let lines = editor_lines(&e);
        assert_eq!(lines[0], "x[^1] word[^word]");
        assert_eq!(&lines[4..], ["tail", "", "[^word]: "]);
    }

    #[test]
    fn mismatched_references_and_definitions_keep_resolving_to_the_same_footnotes() {
        // Three references, one definition: only [^2] is a pair.
        let mut e = make_editor("a[^1] b[^2] c[^3]\n\n[^2]: two");
        cursor_at(&mut e, 0, 0);
        footnote_key(&mut e);
        assert_eq!(
            editor_lines(&e),
            ["[^2]a[^1] b[^4] c[^3]", "", "[^2]: ", "[^4]: two"]
        );
        // Dangling references stay dangling, and no new number landed on them.
        assert_eq!(
            targets(&editor_lines(&e)),
            [Some("".into()), None, Some("two".into()), None]
        );

        // An unreferenced definition and a dangling reference.
        let mut e = make_editor("a[^5] b[^1]\n\n[^5]: five");
        cursor_at(&mut e, 0, 0);
        footnote_key(&mut e);
        assert_eq!(
            editor_lines(&e),
            ["[^2]a[^3] b[^1]", "", "[^2]: ", "[^3]: five"]
        );
        assert_eq!(
            targets(&editor_lines(&e)),
            [Some("".into()), Some("five".into()), None]
        );
    }

    #[test]
    fn a_footnote_inserted_among_others_renumbers_and_places_in_one_undo_step() {
        let doc = "a[^1] b[^2]\n\n[^1]: one\n[^2]: two";
        let mut e = make_editor(doc);
        cursor_at(&mut e, 0, 6);
        footnote_key(&mut e);
        assert_eq!(editor_lines(&e).len(), 5);
        e.input.textarea.undo();
        assert_eq!(e.input.current_text(), doc);
    }

    #[test]
    fn a_cursor_inside_a_reference_inserts_behind_it() {
        let mut e = make_editor("x[^1]y\n\n[^1]: one");
        cursor_at(&mut e, 0, 3);
        footnote_key(&mut e);
        assert_eq!(editor_lines(&e)[0], "x[^1][^2]y");
    }

    #[test]
    fn several_references_to_one_footnote_stay_together() {
        let mut e = make_editor("a[^1] b[^1] c[^1]\n\n[^1]: one");
        cursor_at(&mut e, 0, 6);
        footnote_key(&mut e);
        assert_eq!(
            editor_lines(&e),
            ["a[^1] [^2]b[^1] c[^1]", "", "[^1]: one", "[^2]: "]
        );
    }

    #[test]
    fn inserting_anywhere_never_redirects_an_existing_reference() {
        let docs = [
            "a[^1] b[^2] c[^3]\n\n[^2]: two",
            "a[^5] b[^1]\n\n[^5]: five\n[^3]: orphan",
            "a[^2] b[^2]\nc[^1] d[^7]\n\n[^1]: one[^2]\n[^2]: two\n[^9]: nine",
            "[^3]: t\nx[^3] y[^4]\n`[^3]`\n```\n[^3]\n```\n[^n]: name z[^n]",
            "plain text without footnotes",
        ];
        for doc in docs {
            let lines: Vec<String> = doc.lines().map(String::from).collect();
            let before = targets(&lines);
            let fenced = footnote::fenced_rows(&lines);
            for row in 0..lines.len() {
                // In a definition Ctrl+N jumps; in code a reference is literal.
                if fenced[row] || footnote::definition_at(&lines, row).is_some() {
                    continue;
                }
                for col in 0..=lines[row].chars().count() {
                    let mut e = make_editor(doc);
                    cursor_at(&mut e, row, col);
                    footnote_key(&mut e);
                    let after = editor_lines(&e);
                    let mut expected = before.clone();
                    // The new reference points at the new, empty definition.
                    expected.insert(refs_before(&lines, row, col), Some(String::new()));
                    assert_eq!(
                        targets(&after),
                        expected,
                        "{doc:?} at ({row},{col}): {after:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn repeated_ctrl_n_keeps_numbered_definitions_in_order() {
        let mut e = make_editor("one two three four five");
        for col in [3, 7, 13, 18] {
            cursor_at(&mut e, 0, col);
            footnote_key(&mut e);
            e.handle_paste("note");
            // Back in the text for the next one, shifted by the references so far.
        }
        let lines = editor_lines(&e);
        let defs: Vec<&String> = lines.iter().filter(|l| l.starts_with("[^")).collect();
        assert_eq!(defs.len(), 4);
        let numbers: Vec<u64> = defs
            .iter()
            .map(|l| footnote::def_start(l).unwrap().0.parse().unwrap())
            .collect();
        assert!(numbers.windows(2).all(|w| w[0] < w[1]), "{lines:?}");
    }
}
