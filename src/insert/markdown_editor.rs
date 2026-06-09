//! Description editor — multi-line markdown editing with list
//! auto-continuation, renumbering, and a syntax-highlighted render.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui_textarea::CursorMove;

use super::line_input::has_ctrl_or_cmd;
use super::text_area_input::TextAreaInput;
use super::{InsertHandler, InsertOutcome, InsertSurface};
use crate::app::LoadedBoard;
use crate::command::Command;
use crate::dialog::confirm_cancel_edit::ConfirmCancelEdit;
use crate::model::ids::ShortId;

const NEST_INDENT: usize = 3;

pub struct MarkdownEditor {
    pub input: TextAreaInput,
    pub card_id: ShortId,
}

impl MarkdownEditor {
    pub fn new(card_id: ShortId, initial: &str) -> Self {
        Self {
            input: TextAreaInput::from_initial(initial),
            card_id,
        }
    }
}

impl InsertHandler for MarkdownEditor {
    fn handle_key(&mut self, key: KeyEvent, _b: Option<&LoadedBoard>) -> InsertOutcome {
        let outcome = self.dispatch_key(key);
        self.update_scroll();
        outcome
    }

    fn surface(&self) -> InsertSurface { InsertSurface::CardDetail }
    fn title(&self) -> &str { "Edit Description" }
    fn as_any(&self) -> &dyn std::any::Any { self }
}

impl MarkdownEditor {
    fn dispatch_key(&mut self, key: KeyEvent) -> InsertOutcome {
        match (key.code, key.modifiers) {
            (KeyCode::Char('s'), m) if has_ctrl_or_cmd(m) => {
                let body = self.input.current_text();
                InsertOutcome::Confirm(Command::EditCardDescription {
                    card_id: self.card_id.clone(),
                    body,
                })
            }
            (KeyCode::Esc, _) => {
                if self.input.is_modified() {
                    InsertOutcome::OpenDialog(Box::new(ConfirmCancelEdit))
                } else {
                    InsertOutcome::Cancel
                }
            }
            (KeyCode::Char('z'), m) if has_ctrl_or_cmd(m) => {
                self.input.textarea.undo();
                InsertOutcome::Stay
            }
            (KeyCode::Char('y'), m) if has_ctrl_or_cmd(m) => {
                self.input.textarea.redo();
                InsertOutcome::Stay
            }
            (KeyCode::Char('b'), m) if has_ctrl_or_cmd(m) => {
                wrap_selection_or_insert(&mut self.input.textarea, "**", "**");
                InsertOutcome::Stay
            }
            (KeyCode::Char('i'), m) if has_ctrl_or_cmd(m) => {
                wrap_selection_or_insert(&mut self.input.textarea, "*", "*");
                InsertOutcome::Stay
            }
            (KeyCode::Char('k'), m) if has_ctrl_or_cmd(m) => {
                wrap_selection_or_insert(&mut self.input.textarea, "`", "`");
                InsertOutcome::Stay
            }
            (KeyCode::Char('l'), m) if has_ctrl_or_cmd(m) => {
                insert_at_line_start(&mut self.input.textarea, "- ");
                InsertOutcome::Stay
            }
            (KeyCode::Enter, _) => {
                handle_enter_in_list(&mut self.input.textarea);
                InsertOutcome::Stay
            }
            (KeyCode::Tab, m) if !m.contains(KeyModifiers::SHIFT) => {
                if !handle_tab_nest(&mut self.input.textarea) {
                    self.input.textarea.input(key);
                }
                InsertOutcome::Stay
            }
            (KeyCode::BackTab, _) | (KeyCode::Tab, _) => {
                handle_shift_tab_unnest(&mut self.input.textarea);
                InsertOutcome::Stay
            }
            (KeyCode::Up, _) => {
                move_cursor_visual(&mut self.input.textarea, -1);
                InsertOutcome::Stay
            }
            (KeyCode::Down, _) => {
                move_cursor_visual(&mut self.input.textarea, 1);
                InsertOutcome::Stay
            }
            _ => {
                let before = self.input.textarea.lines().len();
                self.input.textarea.input(key);
                let after = self.input.textarea.lines().len();
                if after != before {
                    renumber_all(&mut self.input.textarea);
                }
                InsertOutcome::Stay
            }
        }
    }

    fn update_scroll(&mut self) {
        let ratatui_textarea::DataCursor(cursor_row, _) = self.input.textarea.cursor();
        let visible_height = 20usize;
        if cursor_row < self.input.scroll {
            self.input.scroll = cursor_row;
        } else if cursor_row >= self.input.scroll + visible_height {
            self.input.scroll = cursor_row - visible_height + 1;
        }
    }
}

// ── List autocontinue + nest/unnest + renumber helpers ───────────────

pub(crate) fn handle_enter_in_list(textarea: &mut ratatui_textarea::TextArea<'static>) {
    let ratatui_textarea::DataCursor(row, col) = textarea.cursor();

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
    if let Some(num_str) = trimmed.strip_suffix(". ").or_else(|| trimmed.strip_suffix('.'))
        && num_str.parse::<u64>().is_ok()
    {
        textarea.move_cursor(CursorMove::Head);
        textarea.delete_line_by_end();
        textarea.insert_newline();
        return;
    }

    if trimmed.starts_with("- ") || trimmed.starts_with("* ") {
        let indent = current_line.len() - trimmed.len();
        let prefix_char = &trimmed[..2];
        let indent_str = " ".repeat(indent);
        textarea.move_cursor(CursorMove::End);
        textarea.insert_newline();
        textarea.insert_str(format!("{indent_str}{prefix_char}"));
        return;
    }

    if let Some(dot_pos) = trimmed.find(". ") {
        let num_part = &trimmed[..dot_pos];
        if num_part.parse::<u64>().is_ok() {
            let indent = current_line.len() - trimmed.len();
            let indent_str = " ".repeat(indent);
            textarea.move_cursor(CursorMove::End);
            textarea.insert_newline();
            textarea.insert_str(format!("{indent_str}1. "));
            renumber_all(textarea);
            let new_row = row + 1;
            let new_len = textarea
                .lines()
                .get(new_row)
                .map(|l| l.chars().count())
                .unwrap_or(0);
            textarea.move_cursor(CursorMove::Jump(new_row as u16, new_len as u16));
            return;
        }
    }

    textarea.insert_newline();
}

pub(crate) fn handle_tab_nest(textarea: &mut ratatui_textarea::TextArea<'static>) -> bool {
    let ratatui_textarea::DataCursor(row, col) = textarea.cursor();
    let line = match textarea.lines().get(row) {
        Some(l) => l.clone(),
        None => return false,
    };
    let trimmed = line.trim_start();
    let indent = line.len() - trimmed.len();

    let numbered_prefix = trimmed
        .find(". ")
        .and_then(|p| trimmed[..p].parse::<u64>().ok().map(|_| p));
    let is_unordered = trimmed.starts_with("- ") || trimmed.starts_with("* ");

    if numbered_prefix.is_none() && !is_unordered {
        return false;
    }

    let pad = " ".repeat(NEST_INDENT);
    textarea.move_cursor(CursorMove::Jump(row as u16, 0));
    textarea.insert_str(&pad);

    if let Some(dot_pos) = numbered_prefix {
        let old_num_str = &trimmed[..dot_pos];
        let new_indent = indent + NEST_INDENT;
        textarea.move_cursor(CursorMove::Jump(row as u16, new_indent as u16));
        textarea.delete_str(old_num_str.chars().count());
        textarea.insert_str("1");
    }

    renumber_all(textarea);

    let new_line_len = textarea
        .lines()
        .get(row)
        .map(|l| l.chars().count())
        .unwrap_or(0);
    let target_col = (col + NEST_INDENT).min(new_line_len);
    textarea.move_cursor(CursorMove::Jump(row as u16, target_col as u16));
    true
}

pub(crate) fn handle_shift_tab_unnest(textarea: &mut ratatui_textarea::TextArea<'static>) {
    let ratatui_textarea::DataCursor(row, col) = textarea.cursor();
    let line = match textarea.lines().get(row) {
        Some(l) => l.clone(),
        None => return,
    };
    let trimmed = line.trim_start();
    let indent = line.len() - trimmed.len();
    if indent < NEST_INDENT {
        return;
    }

    let is_numbered = trimmed
        .find(". ")
        .and_then(|p| trimmed[..p].parse::<u64>().ok())
        .is_some();
    let is_unordered = trimmed.starts_with("- ") || trimmed.starts_with("* ");
    if !is_numbered && !is_unordered {
        return;
    }

    textarea.move_cursor(CursorMove::Jump(row as u16, 0));
    textarea.delete_str(NEST_INDENT);

    renumber_all(textarea);

    let new_line_len = textarea
        .lines()
        .get(row)
        .map(|l| l.chars().count())
        .unwrap_or(0);
    let target_col = col.saturating_sub(NEST_INDENT).min(new_line_len);
    textarea.move_cursor(CursorMove::Jump(row as u16, target_col as u16));
}

pub(crate) fn renumber_all(textarea: &mut ratatui_textarea::TextArea<'static>) {
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

    let ratatui_textarea::DataCursor(sr, sc) = saved;
    textarea.move_cursor(CursorMove::Jump(sr as u16, sc as u16));
}

fn move_cursor_visual(textarea: &mut ratatui_textarea::TextArea<'static>, direction: i32) {
    use crate::ui::markdown;
    use ratatui::style::Color;

    let accent = Color::Cyan;
    let ratatui_textarea::DataCursor(cursor_row, cursor_col) = textarea.cursor();
    let lines: Vec<String> = textarea.lines().to_vec();

    let visual_map = markdown::build_visual_map(&lines, accent, markdown::WRAP_WIDTH);
    let (current_vrow, visual_col) =
        markdown::source_to_visual(&visual_map, cursor_row, cursor_col);

    let target_vrow = if direction < 0 {
        current_vrow.checked_sub(1)
    } else {
        let next = current_vrow + 1;
        if next < visual_map.len() {
            Some(next)
        } else {
            None
        }
    };

    let Some(target_vrow) = target_vrow else {
        return;
    };

    let (target_src_row, target_src_offset, target_vlen, target_vindent) = visual_map[target_vrow];
    let actual_target_vlen = target_vlen.saturating_sub(target_vindent);
    let target_col =
        target_src_offset + (visual_col.saturating_sub(target_vindent)).min(actual_target_vlen);

    textarea.move_cursor(CursorMove::Jump(target_src_row as u16, target_col as u16));
}

fn wrap_selection_or_insert(
    textarea: &mut ratatui_textarea::TextArea<'static>,
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

fn insert_at_line_start(textarea: &mut ratatui_textarea::TextArea<'static>, prefix: &str) {
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
        e.input.textarea.lines().iter().map(|s| s.to_string()).collect()
    }

    fn editor_cursor(e: &MarkdownEditor) -> (usize, usize) {
        let ratatui_textarea::DataCursor(r, c) = e.input.textarea.cursor();
        (r, c)
    }

    fn press(e: &mut MarkdownEditor, code: KeyCode) {
        e.handle_key(KeyEvent::new(code, KeyModifiers::empty()), None);
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
    fn enter_at_col0_of_inner_list_line_still_continues_list() {
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
                "- first".to_string(),
                "- ".to_string(),
                "- second".to_string()
            ]
        );
    }

    #[test]
    fn enter_at_end_of_list_item_continues_list() {
        let mut e = make_editor("- first");
        e.input.textarea.move_cursor(CursorMove::End);

        press(&mut e, KeyCode::Enter);

        assert_eq!(editor_lines(&e), vec!["- first".to_string(), "- ".to_string()]);
    }

    #[test]
    fn enter_at_end_of_numbered_item_continues_with_next_number() {
        let mut e = make_editor("1. first");
        e.input.textarea.move_cursor(CursorMove::End);
        press(&mut e, KeyCode::Enter);
        assert_eq!(editor_lines(&e), vec!["1. first".to_string(), "2. ".to_string()]);
    }

    #[test]
    fn enter_in_numbered_list_renumbers_following_items() {
        let mut e = make_editor("1. a\n2. b\n3. c");
        e.input.textarea.move_cursor(CursorMove::Top);
        e.input.textarea.move_cursor(CursorMove::End);
        press(&mut e, KeyCode::Enter);
        assert_eq!(
            editor_lines(&e),
            vec!["1. a".to_string(), "2. ".to_string(), "3. b".to_string(), "4. c".to_string(),]
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
            vec!["1. a".to_string(), "2. ".to_string(), "3. b".to_string(), "4. c".to_string(),]
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
            vec!["3. a".to_string(), "4. ".to_string(), "5. b".to_string(), "6. c".to_string(),]
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
        assert_eq!(editor_lines(&e), vec!["1. parent".to_string(), "   1. child".to_string()]);
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
        assert_eq!(editor_lines(&e), vec!["- a".to_string(), "   - b".to_string()]);
    }

    #[test]
    fn shift_tab_unnests_nested_item_and_joins_parent_list() {
        let mut e = make_editor("1. parent\n   1. child");
        e.input.textarea.move_cursor(CursorMove::Bottom);
        e.input.textarea.move_cursor(CursorMove::End);
        press(&mut e, KeyCode::BackTab);
        assert_eq!(editor_lines(&e), vec!["1. parent".to_string(), "2. child".to_string()]);
    }

    #[test]
    fn shift_tab_on_top_level_list_item_is_noop() {
        let mut e = make_editor("1. a\n2. b");
        e.input.textarea.move_cursor(CursorMove::Bottom);
        e.input.textarea.move_cursor(CursorMove::End);
        press(&mut e, KeyCode::BackTab);
        assert_eq!(editor_lines(&e), vec!["1. a".to_string(), "2. b".to_string()]);
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
        assert_eq!(editor_lines(&e), vec!["1. a2. b".to_string(), "2. c".to_string()]);
    }

    #[test]
    fn typing_does_not_trigger_renumber() {
        let mut e = make_editor("1. a\n5. b");
        e.input.textarea.move_cursor(CursorMove::Top);
        e.input.textarea.move_cursor(CursorMove::End);
        press(&mut e, KeyCode::Char('x'));
        assert_eq!(editor_lines(&e), vec!["1. ax".to_string(), "5. b".to_string()]);
    }
}
