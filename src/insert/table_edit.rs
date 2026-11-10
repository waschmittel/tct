//! Table editing inside the description textarea: live reformatting while
//! typing, cell navigation on Tab/Enter, and structural edits (rows, columns,
//! alignment). The table model itself lives in `table.rs`.
//!
//! Every operation here is one undo step; typing in one cell within a short
//! time joins into a single step.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use tui_textarea::{CursorMove, TextArea};

use super::table::{
    Edit, Table, delimiter_aligns, is_table_line, line_of_row, locate, row_of_line, split_cells,
};

type Area = TextArea<'static>;

/// Cursor offset meaning "end of the cell text".
const END: usize = usize::MAX;

/// Hint shown while the `Ctrl+T` chord waits for its key.
pub const CHORD_HINT: &str = "Table: r/c add row/column after, R/C before, x/X delete row/column, arrows swap, a align, T delete table";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TableOp {
    InsertRowAfter,
    InsertRowBefore,
    InsertColAfter,
    InsertColBefore,
    DeleteRow,
    DeleteCol,
    DeleteTable,
    SwapRowUp,
    SwapRowDown,
    SwapColLeft,
    SwapColRight,
    CycleAlign,
}

/// The key that follows `Ctrl+T` inside a table.
pub fn chord_op(key: KeyEvent) -> Option<TableOp> {
    Some(match key.code {
        KeyCode::Char('r') => TableOp::InsertRowAfter,
        KeyCode::Char('R') => TableOp::InsertRowBefore,
        KeyCode::Char('c') => TableOp::InsertColAfter,
        KeyCode::Char('C') => TableOp::InsertColBefore,
        KeyCode::Char('x') => TableOp::DeleteRow,
        KeyCode::Char('X') => TableOp::DeleteCol,
        KeyCode::Char('T') => TableOp::DeleteTable,
        KeyCode::Char('a') => TableOp::CycleAlign,
        KeyCode::Up => TableOp::SwapRowUp,
        KeyCode::Down => TableOp::SwapRowDown,
        KeyCode::Left => TableOp::SwapColLeft,
        KeyCode::Right => TableOp::SwapColRight,
        _ => return None,
    })
}

/// Direct shortcuts: `Alt+arrows` swap, `Alt+Shift+arrows` add/delete.
pub fn alt_op(key: KeyEvent) -> Option<TableOp> {
    let m = key.modifiers;
    if !m.contains(KeyModifiers::ALT) || m.contains(KeyModifiers::CONTROL) {
        return None;
    }
    let add_delete = m.contains(KeyModifiers::SHIFT);
    Some(match (key.code, add_delete) {
        (KeyCode::Right, true) => TableOp::InsertColAfter,
        (KeyCode::Down, true) => TableOp::InsertRowAfter,
        (KeyCode::Left, true) => TableOp::DeleteCol,
        (KeyCode::Up, true) => TableOp::DeleteRow,
        (KeyCode::Left, false) => TableOp::SwapColLeft,
        (KeyCode::Right, false) => TableOp::SwapColRight,
        (KeyCode::Up, false) => TableOp::SwapRowUp,
        (KeyCode::Down, false) => TableOp::SwapRowDown,
        (KeyCode::Char('a' | 'A'), _) => TableOp::CycleAlign,
        _ => return None,
    })
}

/// Run `f` as one undo step (`key` lets repeated edits of one kind join).
pub(crate) fn grouped<R>(ta: &mut Area, key: Option<u64>, f: impl FnOnce(&mut Area) -> R) -> R {
    ta.break_undo_run();
    let mark = ta.undo_mark();
    let result = f(ta);
    ta.undo_group_since(mark, key);
    result
}

/// Inclusive line range of a table.
#[derive(Clone, Copy, Debug)]
struct Span {
    start: usize,
    end: usize,
}

/// The table containing `row`: a run of pipe lines whose second line is a delimiter row.
fn span_at(lines: &[String], row: usize) -> Option<Span> {
    if !is_table_line(lines.get(row)?) {
        return None;
    }
    let mut first = row;
    while first > 0 && is_table_line(&lines[first - 1]) {
        first -= 1;
    }
    let mut end = row;
    while end + 1 < lines.len() && is_table_line(&lines[end + 1]) {
        end += 1;
    }
    let delimiter = (first + 1..=end).find(|&i| delimiter_aligns(&lines[i]).is_some())?;
    (row >= delimiter - 1).then_some(Span {
        start: delimiter - 1,
        end,
    })
}

/// A table with the cursor in it.
struct Context {
    span: Span,
    table: Table,
    /// Line within the table (0 header, 1 delimiter, 2.. body rows).
    line: usize,
    cell: usize,
    /// Cursor offset into the cell text; beyond it counts spaces typed after the text.
    off: usize,
}

impl Context {
    fn row(&self) -> usize {
        row_of_line(self.line)
    }
}

fn context(ta: &Area) -> Option<Context> {
    let (row, col) = ta.cursor();
    let lines = ta.lines();
    let span = span_at(lines, row)?;
    let table = Table::parse(&lines[span.start..=span.end])?;
    let line = row - span.start;
    let (cell, off) = locate(&lines[row], col);
    let off = if line == 1 { 0 } else { off };
    Some(Context {
        span,
        cell: cell.min(table.cols() - 1),
        table,
        line,
        off,
    })
}

pub(crate) fn in_table(ta: &Area) -> bool {
    span_at(ta.lines(), ta.cursor().0).is_some()
}

/// Identifies the cell under the cursor, to join typing in it into one undo step.
pub(crate) fn cell_key(ta: &Area) -> Option<u64> {
    let ctx = context(ta)?;
    Some(((ctx.span.start * 4096 + ctx.line) * 4096 + ctx.cell) as u64)
}

/// Before typing a character: a cursor outside the row's pipes (after the closing
/// one, or in front of the opening one) would add a cell or break the row, so it
/// moves into the nearest cell's text.
pub(crate) fn snap_into_row(ta: &mut Area) {
    let (row, col) = ta.cursor();
    let lines = ta.lines();
    if span_at(lines, row).is_none() {
        return;
    }
    let line = &lines[row];
    let cells = split_cells(line);
    let (Some(first), Some(last)) = (cells.first(), cells.last()) else {
        return;
    };
    let closed = line.chars().nth(last.end) == Some('|');
    let target = if col < first.start {
        first.text_start
    } else if closed && col > last.end {
        last.text_end
    } else {
        return;
    };
    ta.move_cursor(CursorMove::Jump(row as u16, target as u16));
}

struct Target {
    line: usize,
    cell: usize,
    off: usize,
}

/// Replace the lines of `span` with the rendering of `table` and put the cursor
/// at `target`. Returns the table's new extent.
fn put(ta: &mut Area, span: Span, table: &Table, target: Target) -> Span {
    let rendered = table.render();
    replace_lines(ta, span, &rendered.lines);
    let new_span = Span {
        start: span.start,
        end: span.start + rendered.lines.len() - 1,
    };
    let starts = &rendered.text_starts[target.line];
    let cell = target.cell.min(starts.len() - 1);
    let text_len = if target.line == 1 {
        0
    } else {
        table.rows[row_of_line(target.line)][cell].chars().count()
    };
    let col = starts[cell] + target.off.min(text_len);
    ta.move_cursor(CursorMove::Jump(
        (new_span.start + target.line) as u16,
        col as u16,
    ));
    new_span
}

/// Minimal textarea edit turning the lines of `span` into `new`.
fn replace_lines(ta: &mut Area, span: Span, new: &[String]) {
    let old: Vec<String> = ta.lines()[span.start..=span.end].to_vec();
    if old == new {
        return;
    }
    ta.cancel_selection();
    let max_shared = old.len().min(new.len());
    let prefix = old.iter().zip(new).take_while(|(a, b)| a == b).count();
    let suffix = old
        .iter()
        .rev()
        .zip(new.iter().rev())
        .take(max_shared - prefix)
        .take_while(|(a, b)| a == b)
        .count();
    let at = span.start + prefix;
    let old_mid = &old[prefix..old.len() - suffix];
    let new_mid = &new[prefix..new.len() - suffix];
    if old_mid.len() == new_mid.len() {
        for (k, (o, n)) in old_mid.iter().zip(new_mid).enumerate() {
            if o != n {
                ta.move_cursor(CursorMove::Jump((at + k) as u16, 0));
                ta.delete_str(o.chars().count());
                ta.insert_str(n);
            }
        }
    } else if !old_mid.is_empty() && !new_mid.is_empty() {
        let chars = old_mid.iter().map(|l| l.chars().count()).sum::<usize>() + old_mid.len() - 1;
        ta.move_cursor(CursorMove::Jump(at as u16, 0));
        ta.delete_str(chars);
        ta.insert_str(new_mid.join("\n"));
    } else {
        delete_lines(ta, at, old_mid);
        insert_lines(ta, at, new_mid);
    }
}

fn delete_lines(ta: &mut Area, at: usize, lines: &[String]) {
    if lines.is_empty() {
        return;
    }
    let chars = lines.iter().map(|l| l.chars().count()).sum::<usize>() + lines.len();
    let total = ta.lines().len();
    if at + lines.len() < total {
        ta.move_cursor(CursorMove::Jump(at as u16, 0));
        ta.delete_str(chars);
    } else if at > 0 {
        let prev_len = ta.lines()[at - 1].chars().count();
        ta.move_cursor(CursorMove::Jump((at - 1) as u16, prev_len as u16));
        ta.delete_str(chars);
    } else {
        ta.move_cursor(CursorMove::Jump(0, 0));
        ta.delete_str(chars - 1);
    }
}

fn insert_lines(ta: &mut Area, at: usize, lines: &[String]) {
    if lines.is_empty() {
        return;
    }
    let text = lines.join("\n");
    let total = ta.lines().len();
    if at < total {
        ta.move_cursor(CursorMove::Jump(at as u16, 0));
        ta.insert_str(format!("{text}\n"));
    } else {
        let last_len = ta.lines()[total - 1].chars().count();
        ta.move_cursor(CursorMove::Jump((total - 1) as u16, last_len as u16));
        ta.insert_str(format!("\n{text}"));
    }
}

/// Reformat the table around the cursor, keeping the cursor on the same text.
/// Spaces typed after the cell text are kept while the cursor is in the cell.
/// Returns whether the cursor was in a table.
pub(crate) fn reformat_at_cursor(ta: &mut Area) -> bool {
    let Some(mut ctx) = context(ta) else {
        return false;
    };
    if ctx.line != 1 {
        let row = ctx.row();
        let text = &mut ctx.table.rows[row][ctx.cell];
        let len = text.chars().count();
        if ctx.off > len {
            text.push_str(&" ".repeat(ctx.off - len));
        }
    }
    put(
        ta,
        ctx.span,
        &ctx.table,
        Target {
            line: ctx.line,
            cell: ctx.cell,
            off: ctx.off,
        },
    );
    true
}

/// Rewrite the text of the cell under the cursor (not the delimiter row, not an
/// empty cell) with `f` and reformat. Returns whether it did.
pub(crate) fn map_cell_text(ta: &mut Area, f: impl FnOnce(&str) -> Option<String>) -> bool {
    let Some(mut ctx) = context(ta) else {
        return false;
    };
    if ctx.line == 1 {
        return false;
    }
    let row = ctx.row();
    let Some(new) = f(&ctx.table.rows[row][ctx.cell]) else {
        return false;
    };
    ctx.table.rows[row][ctx.cell] = new;
    put(
        ta,
        ctx.span,
        &ctx.table,
        Target {
            line: ctx.line,
            cell: ctx.cell,
            off: END,
        },
    );
    true
}

/// Move the cursor out of the table, below it. Opens a text line when the
/// table ends the description (a blank line keeps following text out of the table).
fn leave_below(ta: &mut Area, span: Span) {
    if span.end + 1 < ta.lines().len() {
        ta.move_cursor(CursorMove::Jump((span.end + 1) as u16, 0));
    } else {
        let len = ta.lines()[span.end].chars().count();
        ta.move_cursor(CursorMove::Jump(span.end as u16, len as u16));
        ta.insert_newline();
        ta.insert_newline();
    }
}

/// Tab / Shift+Tab. Returns whether the cursor was in a table.
///
/// Tab in the last cell appends a row; Shift+Tab in the first cell leaves the
/// table upwards.
pub(crate) fn tab(ta: &mut Area, backwards: bool) -> bool {
    let Some(mut ctx) = context(ta) else {
        return false;
    };
    grouped(ta, None, |ta| {
        let (row, cell) = (ctx.row(), ctx.cell);
        let last_row = ctx.table.rows.len() - 1;
        let last_cell = ctx.table.cols() - 1;
        let (row, cell) = if backwards {
            match (row, cell) {
                (0, 0) => {
                    leave_above(ta, ctx.span, &ctx.table);
                    return;
                }
                (_, 0) => (row - 1, last_cell),
                _ => (row, cell - 1),
            }
        } else if cell < last_cell {
            (row, cell + 1)
        } else if row < last_row {
            (row + 1, 0)
        } else {
            ctx.table.insert_row(last_row, true);
            (last_row + 1, 0)
        };
        put(
            ta,
            ctx.span,
            &ctx.table,
            Target {
                line: line_of_row(row),
                cell,
                off: END,
            },
        );
    });
    true
}

fn leave_above(ta: &mut Area, span: Span, table: &Table) {
    put(
        ta,
        span,
        table,
        Target {
            line: 0,
            cell: 0,
            off: END,
        },
    );
    if span.start > 0 {
        let len = ta.lines()[span.start - 1].chars().count();
        ta.move_cursor(CursorMove::Jump((span.start - 1) as u16, len as u16));
    }
}

/// Enter: next row, same column; below the last row it leaves the table.
/// On a lone `| header |` line it turns the line into a table.
/// Returns whether it handled the key.
pub(crate) fn enter(ta: &mut Area) -> bool {
    if let Some(ctx) = context(ta) {
        grouped(ta, None, |ta| {
            let row = ctx.row();
            if row + 1 < ctx.table.rows.len() {
                put(
                    ta,
                    ctx.span,
                    &ctx.table,
                    Target {
                        line: line_of_row(row + 1),
                        cell: ctx.cell,
                        off: END,
                    },
                );
            } else {
                let span = put(
                    ta,
                    ctx.span,
                    &ctx.table,
                    Target {
                        line: ctx.line,
                        cell: ctx.cell,
                        off: END,
                    },
                );
                leave_below(ta, span);
            }
        });
        return true;
    }
    header_to_table(ta)
}

fn header_to_table(ta: &mut Area) -> bool {
    let (row, _) = ta.cursor();
    let lines = ta.lines();
    let line = &lines[row];
    let next_is_pipe = lines.get(row + 1).is_some_and(|l| is_table_line(l));
    if !is_table_line(line) || !line.trim_end().ends_with('|') || next_is_pipe {
        return false;
    }
    let cells = split_cells(line);
    if cells.is_empty() {
        return false;
    }
    let chars: Vec<char> = line.chars().collect();
    let header = cells
        .iter()
        .map(|c| chars[c.text_start..c.text_end].iter().collect::<String>())
        .collect();
    let indent: String = chars.iter().take_while(|c| c.is_whitespace()).collect();
    let table = Table::from_header(&indent, header);
    grouped(ta, None, |ta| {
        put(
            ta,
            Span {
                start: row,
                end: row,
            },
            &table,
            Target {
                line: 2,
                cell: 0,
                off: END,
            },
        );
    });
    true
}

/// `Ctrl+T` outside a table: insert an empty 2x2 table at the cursor line,
/// separated from neighbouring text by blank lines.
pub(crate) fn insert_skeleton(ta: &mut Area) {
    grouped(ta, None, |ta| {
        let (row, _) = ta.cursor();
        let lines = ta.lines();
        let on_blank = lines[row].trim().is_empty();
        let text_follows = lines.get(row + 1).is_some_and(|l| !l.trim().is_empty());
        let rendered = Table::blank(2, 1).render();
        let mut text = rendered.lines.join("\n");
        if text_follows {
            text.push('\n');
        }
        let table_row = if on_blank {
            ta.move_cursor(CursorMove::Jump(row as u16, 0));
            row
        } else {
            ta.move_cursor(CursorMove::Jump(
                row as u16,
                lines[row].chars().count() as u16,
            ));
            text.insert_str(0, "\n\n");
            row + 2
        };
        ta.insert_str(text);
        ta.move_cursor(CursorMove::Jump(
            table_row as u16,
            rendered.text_starts[0][0] as u16,
        ));
    });
}

/// Run a structural edit on the table at the cursor. Returns a message when the
/// edit is not possible.
pub(crate) fn run_op(ta: &mut Area, op: TableOp) -> Option<String> {
    let mut ctx = context(ta)?;
    grouped(ta, None, |ta| {
        let (row, cell) = (ctx.row(), ctx.cell);
        let t = &mut ctx.table;
        let (edit, moves_row) = match op {
            TableOp::InsertRowAfter => (t.insert_row(row, true), true),
            TableOp::InsertRowBefore => (t.insert_row(row, false), true),
            TableOp::DeleteRow => (t.delete_row(row), true),
            TableOp::SwapRowUp => (t.swap_row(row, false), true),
            TableOp::SwapRowDown => (t.swap_row(row, true), true),
            TableOp::InsertColAfter => (t.insert_col(cell, true), false),
            TableOp::InsertColBefore => (t.insert_col(cell, false), false),
            TableOp::DeleteCol => (t.delete_col(cell), false),
            TableOp::SwapColLeft => (t.swap_col(cell, false), false),
            TableOp::SwapColRight => (t.swap_col(cell, true), false),
            TableOp::CycleAlign => {
                t.cycle_align(cell);
                (Edit::Place(cell), false)
            }
            TableOp::DeleteTable => (Edit::Gone, false),
        };
        match edit {
            Edit::Refused(why) => Some(why.to_string()),
            Edit::Gone => {
                remove_table(ta, ctx.span);
                Some("Table deleted".to_string())
            }
            Edit::Place(n) => {
                let target = match (moves_row, op) {
                    (true, _) => Target {
                        line: line_of_row(n),
                        cell,
                        off: END,
                    },
                    (false, TableOp::CycleAlign) => Target {
                        line: ctx.line,
                        cell,
                        off: ctx.off,
                    },
                    (false, _) => Target {
                        line: ctx.line,
                        cell: n,
                        off: END,
                    },
                };
                put(ta, ctx.span, &ctx.table, target);
                None
            }
        }
    })
}

fn remove_table(ta: &mut Area, span: Span) {
    replace_lines(ta, span, &[]);
    let row = span.start.min(ta.lines().len() - 1);
    ta.move_cursor(CursorMove::Jump(row as u16, 0));
}

#[cfg(test)]
mod tests {
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
            [
                &"|  a  |  b  |"[..],
                "| --- | --- |",
                "| 1   | 2   |",
                "",
                ""
            ]
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
}
