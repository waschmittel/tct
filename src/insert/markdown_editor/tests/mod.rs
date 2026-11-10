use super::*;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

mod basics;
mod enter;
mod footnotes;
mod nesting;
mod numbered;
mod quotes;
mod renumber;
mod session;

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

fn ctrl(e: &mut MarkdownEditor, c: char) {
    e.handle_key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL), None);
}

fn cursor_at(e: &mut MarkdownEditor, row: usize, col: usize) {
    e.input
        .textarea
        .move_cursor(CursorMove::Jump(row as u16, col as u16));
}

fn ctrl_out(e: &mut MarkdownEditor, c: char) -> InsertOutcome {
    e.handle_key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL), None)
}

fn plain(e: &mut MarkdownEditor, c: char) -> InsertOutcome {
    e.handle_key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::empty()), None)
}

fn nest_at(initial: &str, row: usize, col: usize) -> MarkdownEditor {
    let mut e = make_editor(initial);
    e.input
        .textarea
        .move_cursor(CursorMove::Jump(row as u16, col as u16));
    press(&mut e, KeyCode::Tab);
    e
}

fn unnest_at(initial: &str, row: usize, col: usize) -> MarkdownEditor {
    let mut e = make_editor(initial);
    e.input
        .textarea
        .move_cursor(CursorMove::Jump(row as u16, col as u16));
    press(&mut e, KeyCode::BackTab);
    e
}

fn paste_mode_on(e: &mut MarkdownEditor) {
    ctrl(e, 'p');
    assert!(e.paste_mode());
}

fn footnote_key(e: &mut MarkdownEditor) -> InsertOutcome {
    ctrl_out(e, 'n')
}

fn select(e: &mut MarkdownEditor, from: (usize, usize), to: (usize, usize)) {
    cursor_at(e, from.0, from.1);
    e.input.textarea.start_selection();
    e.input
        .textarea
        .move_cursor(CursorMove::Jump(to.0 as u16, to.1 as u16));
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
