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
use footnotes::*;
pub(crate) use lists::*;
pub(crate) use normalize::*;

mod footnotes;
mod lists;
mod normalize;
#[cfg(test)]
mod tests;

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

    /// Put the cursor at `(row, col)` (clamped if the text got shorter).
    pub fn restore(&mut self, (row, col): (usize, usize)) {
        let clamp = |n: usize| n.min(u16::MAX as usize) as u16;
        self.input
            .textarea
            .move_cursor(CursorMove::Jump(clamp(row), clamp(col)));
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
                if !self.input.is_modified() {
                    // Nothing but maybe the cursor moved: not worth a save.
                    return InsertOutcome::Stay;
                }
                let body = self.input.current_text();
                self.input.original = body.clone();
                InsertOutcome::Save(Command::EditCardDescription {
                    card_id: self.card_id.clone(),
                    body,
                    cursor: Some(self.input.textarea.cursor()),
                })
            }
            (KeyCode::Char('q'), m) if has_ctrl_or_cmd(m) => {
                self.leave_paste_mode();
                if !self.input.is_modified() {
                    return InsertOutcome::Cancel;
                }
                let body = self.input.current_text();
                InsertOutcome::Confirm(Command::EditCardDescription {
                    card_id: self.card_id.clone(),
                    body,
                    cursor: Some(self.input.textarea.cursor()),
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
                if !table_edit::in_table(ta) && grouped(ta, None, |ta| shift_selection(ta, true)) {
                    return InsertOutcome::Stay;
                }
                if !table_edit::tab(ta, false) && !grouped(ta, None, handle_tab_nest) {
                    ta.input(key);
                }
                InsertOutcome::Stay
            }
            (KeyCode::BackTab, _) | (KeyCode::Tab, _) => {
                let ta = &mut self.input.textarea;
                if !table_edit::in_table(ta) && grouped(ta, None, |ta| shift_selection(ta, false)) {
                    return InsertOutcome::Stay;
                }
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
