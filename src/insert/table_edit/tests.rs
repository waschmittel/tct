use super::*;
use crate::insert::InsertHandler;
use crate::insert::markdown_editor::MarkdownEditor;

fn editor(text: &str, row: usize, col: usize) -> MarkdownEditor {
    let mut e = MarkdownEditor::new("card1".into(), text);
    e.input
        .textarea
        .move_cursor(CursorMove::Jump(row as u16, col as u16));
    e
}

fn key(e: &mut MarkdownEditor, code: KeyCode, mods: KeyModifiers) {
    e.handle_key(KeyEvent::new(code, mods), None);
}

fn press(e: &mut MarkdownEditor, code: KeyCode) {
    key(e, code, KeyModifiers::empty());
}

fn type_str(e: &mut MarkdownEditor, s: &str) {
    for c in s.chars() {
        press(e, KeyCode::Char(c));
    }
}

fn lines(e: &MarkdownEditor) -> Vec<String> {
    e.input.textarea.lines().to_vec()
}

fn cursor(e: &MarkdownEditor) -> (usize, usize) {
    e.input.textarea.cursor()
}

fn chord(e: &mut MarkdownEditor, code: KeyCode) {
    key(e, KeyCode::Char('t'), KeyModifiers::CONTROL);
    press(e, code);
}

const T: &str = "| a | b |\n| --- | --- |\n| 1 | 2 |";

#[test]
fn typing_in_a_cell_reflows_the_table_and_keeps_the_cursor() {
    let mut e = editor(T, 0, 3);
    type_str(&mut e, "bcdef");
    assert_eq!(
        lines(&e),
        ["| abcdef |  b  |", "| ------ | --- |", "| 1      | 2   |"]
    );
    assert_eq!(cursor(&e), (0, 8));
}

#[test]
fn a_space_typed_after_the_text_is_not_swallowed() {
    let mut e = editor(T, 0, 3);
    type_str(&mut e, " x");
    assert_eq!(lines(&e)[0], "| a x |  b  |");
    assert_eq!(cursor(&e), (0, 5));
}

#[test]
fn deleting_text_shrinks_the_column_again() {
    let mut e = editor("| abcdef | b |\n| --- | --- |\n| 1 | 2 |", 0, 8);
    for _ in 0..5 {
        press(&mut e, KeyCode::Backspace);
    }
    assert_eq!(lines(&e)[0], "|  a  |  b  |");
    assert_eq!(cursor(&e), (0, 4));
}

#[test]
fn right_aligned_cells_keep_the_cursor_on_the_text() {
    let mut e = editor("| a | bbb |\n| --- | --: |\n| 1 | 2 |", 2, 7);
    type_str(&mut e, "9");
    assert_eq!(lines(&e)[2], "| 1   |  29 |");
    assert_eq!(cursor(&e), (2, 11));
}

#[test]
fn typing_after_the_closing_pipe_extends_the_last_cell_instead_of_adding_one() {
    let mut e = editor(T, 2, 9);
    type_str(&mut e, " x");
    assert_eq!(lines(&e)[2], "| 1   | 2 x |");
    assert_eq!(lines(&e)[0], "|  a  |  b  |");
    assert_eq!(cursor(&e), (2, 11));
}

#[test]
fn typing_in_front_of_the_opening_pipe_goes_into_the_first_cell() {
    let mut e = editor(T, 2, 0);
    type_str(&mut e, "x");
    assert_eq!(lines(&e)[2], "| x1  | 2   |");
}

#[test]
fn typing_a_delimiter_row_turns_the_lines_into_a_table() {
    let mut e = editor("| a | b |\n|-|-", 1, 4);
    type_str(&mut e, "|");
    assert_eq!(lines(&e), ["|  a  |  b  |", "| --- | --- |"]);
}

#[test]
fn tab_walks_the_cells_and_wraps_to_the_next_row() {
    let mut e = editor(T, 0, 3);
    press(&mut e, KeyCode::Tab);
    assert_eq!(cursor(&e), (0, 10));
    press(&mut e, KeyCode::Tab);
    assert_eq!(cursor(&e), (2, 3));
    press(&mut e, KeyCode::Tab);
    assert_eq!(cursor(&e), (2, 9));
}

#[test]
fn tab_in_the_last_cell_appends_a_row_and_focuses_its_first_cell() {
    let mut e = editor(T, 2, 9);
    press(&mut e, KeyCode::Tab);
    assert_eq!(
        lines(&e),
        [
            "|  a  |  b  |",
            "| --- | --- |",
            "| 1   | 2   |",
            "|     |     |"
        ]
    );
    assert_eq!(cursor(&e), (3, 2));
}

#[test]
fn shift_tab_walks_back_and_leaves_above_the_table() {
    let mut e = editor(&format!("text\n\n{T}"), 4, 2);
    key(&mut e, KeyCode::BackTab, KeyModifiers::SHIFT);
    assert_eq!(cursor(&e), (2, 10));
    key(&mut e, KeyCode::BackTab, KeyModifiers::SHIFT);
    assert_eq!(cursor(&e), (2, 4));
    key(&mut e, KeyCode::BackTab, KeyModifiers::SHIFT);
    assert_eq!(cursor(&e), (1, 0));
}

#[test]
fn enter_moves_down_the_same_column() {
    let mut e = editor(T, 0, 9);
    press(&mut e, KeyCode::Enter);
    assert_eq!(cursor(&e), (2, 9));
    assert_eq!(lines(&e).len(), 3);
}

#[test]
fn enter_in_the_last_row_leaves_the_table_downwards() {
    let mut e = editor(T, 2, 3);
    press(&mut e, KeyCode::Enter);
    assert_eq!(
        lines(&e),
        ["|  a  |  b  |", "| --- | --- |", "| 1   | 2   |", "", ""]
    );
    assert_eq!(cursor(&e), (4, 0));

    let mut e = editor(&format!("{T}\nafter"), 2, 3);
    press(&mut e, KeyCode::Enter);
    assert_eq!(cursor(&e), (3, 0));
    assert_eq!(lines(&e).len(), 4);
}

#[test]
fn enter_on_a_header_line_creates_the_table() {
    let mut e = editor("| Name | Qty |", 0, 5);
    press(&mut e, KeyCode::Enter);
    assert_eq!(
        lines(&e),
        ["| Name | Qty |", "| ---- | --- |", "|      |     |"]
    );
    assert_eq!(cursor(&e), (2, 2));
}

#[test]
fn enter_in_a_cell_never_splits_the_row() {
    let mut e = editor("| ab | b |\n| - | - |\n| 1 | 2 |", 0, 3);
    press(&mut e, KeyCode::Enter);
    assert_eq!(lines(&e).len(), 3);
}

#[test]
fn ctrl_t_outside_a_table_inserts_a_skeleton() {
    let mut e = editor("intro\nafter", 0, 2);
    key(&mut e, KeyCode::Char('t'), KeyModifiers::CONTROL);
    assert_eq!(
        lines(&e),
        [
            "intro",
            "",
            "|     |     |",
            "| --- | --- |",
            "|     |     |",
            "",
            "after"
        ]
    );
    assert_eq!(cursor(&e), (2, 3));

    let mut e = editor("", 0, 0);
    key(&mut e, KeyCode::Char('t'), KeyModifiers::CONTROL);
    assert_eq!(lines(&e).len(), 3);
    assert_eq!(cursor(&e), (0, 3));
}

#[test]
fn chord_adds_and_deletes_rows_and_columns() {
    let mut e = editor(T, 0, 3);
    chord(&mut e, KeyCode::Char('r'));
    assert_eq!(lines(&e).len(), 4);
    assert_eq!(lines(&e)[2], "|     |     |");
    assert_eq!(cursor(&e).0, 2);

    chord(&mut e, KeyCode::Char('c'));
    assert_eq!(lines(&e)[0], "|  a  |     |  b  |");
    assert_eq!(cursor(&e), (2, 8));

    chord(&mut e, KeyCode::Char('X'));
    assert_eq!(lines(&e)[0], "|  a  |  b  |");
    chord(&mut e, KeyCode::Char('x'));
    assert_eq!(lines(&e).len(), 3);
}

#[test]
fn chord_cycles_alignment_and_swaps() {
    let mut e = editor(T, 0, 3);
    chord(&mut e, KeyCode::Char('a'));
    assert_eq!(lines(&e)[1], "| :-- | --- |");
    chord(&mut e, KeyCode::Right);
    assert_eq!(lines(&e)[0], "|  b  |  a  |");
    assert_eq!(lines(&e)[1], "| --- | :-- |");
    assert_eq!(cursor(&e), (0, 10));
}

#[test]
fn deleting_the_header_row_or_the_last_column_removes_the_table() {
    let mut e = editor(&format!("x\n{T}\ny"), 1, 3);
    chord(&mut e, KeyCode::Char('x'));
    assert_eq!(lines(&e), ["x", "y"]);

    let mut e = editor("| a |\n| - |\n| 1 |", 0, 3);
    chord(&mut e, KeyCode::Char('X'));
    assert_eq!(lines(&e), [""]);

    let mut e = editor(T, 1, 3);
    chord(&mut e, KeyCode::Char('T'));
    assert_eq!(lines(&e), [""]);
}

#[test]
fn the_header_cannot_move_or_gain_a_row_above() {
    let mut e = editor(T, 0, 3);
    chord(&mut e, KeyCode::Down);
    chord(&mut e, KeyCode::Char('R'));
    assert_eq!(lines(&e), T.split('\n').collect::<Vec<_>>());
}

#[test]
fn alt_arrows_swap_and_alt_shift_arrows_add_and_delete() {
    let mut e = editor(T, 0, 3);
    key(
        &mut e,
        KeyCode::Right,
        KeyModifiers::ALT | KeyModifiers::SHIFT,
    );
    assert_eq!(lines(&e)[0], "|  a  |     |  b  |");
    key(
        &mut e,
        KeyCode::Left,
        KeyModifiers::ALT | KeyModifiers::SHIFT,
    );
    assert_eq!(lines(&e)[0], "|  a  |  b  |");
    let mut e = editor(T, 0, 3);
    key(&mut e, KeyCode::Right, KeyModifiers::ALT);
    assert_eq!(lines(&e)[0], "|  b  |  a  |");
}

#[test]
fn plain_text_keys_are_not_captured_inside_cells() {
    let mut e = editor(T, 0, 3);
    type_str(&mut e, "rcxXaT");
    assert_eq!(lines(&e)[0], "| arcxXaT |  b  |");
}

#[test]
fn chord_swallows_unknown_keys_and_esc_does_not_close_the_editor() {
    let mut e = editor(T, 0, 3);
    key(&mut e, KeyCode::Char('t'), KeyModifiers::CONTROL);
    let out = e.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::empty()), None);
    assert!(matches!(
        out,
        crate::insert::InsertOutcome::StayWithStatus(None)
    ));
    type_str(&mut e, "z");
    assert_eq!(lines(&e)[0], "| az  |  b  |");
}

#[test]
fn a_table_op_is_one_undo_step() {
    let mut e = editor(T, 0, 3);
    chord(&mut e, KeyCode::Char('c'));
    assert_eq!(lines(&e)[0], "|  a  |     |  b  |");
    key(&mut e, KeyCode::Char('z'), KeyModifiers::CONTROL);
    assert_eq!(lines(&e), T.split('\n').collect::<Vec<_>>());
}

#[test]
fn typing_in_one_cell_undoes_together_with_its_reformat() {
    let mut e = editor(T, 0, 3);
    type_str(&mut e, "bcdef");
    key(&mut e, KeyCode::Char('z'), KeyModifiers::CONTROL);
    assert_eq!(lines(&e), T.split('\n').collect::<Vec<_>>());
    key(&mut e, KeyCode::Char('y'), KeyModifiers::CONTROL);
    assert_eq!(lines(&e)[0], "| abcdef |  b  |");
}

#[test]
fn pasting_into_a_table_reformats_it() {
    let mut e = editor(T, 0, 3);
    e.paste_text("long text");
    assert_eq!(lines(&e)[0], "| along text |  b  |");
    assert_eq!(lines(&e)[2].len(), lines(&e)[0].len());
}

#[test]
fn lines_above_the_header_are_not_part_of_the_table() {
    let mut e = editor(&format!("| stray |\n{T}"), 0, 3);
    type_str(&mut e, "x");
    assert_eq!(lines(&e)[0], "| sxtray |");
    assert_eq!(lines(&e)[1], "| a | b |");
}
