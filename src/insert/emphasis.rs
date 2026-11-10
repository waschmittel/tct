//! `Ctrl+B` / `Ctrl+E` in the description editor: toggle bold / italic.
//!
//! The emphasis of a text range is the run of `*` directly around it: one star
//! is italic, two bold, three both — so a toggle is an XOR on those bits. The
//! formatted text never includes leading or trailing whitespace, and inside a
//! table cell without a selection the whole cell is formatted.

use tui_textarea::{CursorMove, TextArea};

use super::table_edit;

type Area = TextArea<'static>;

pub const ITALIC: usize = 1;
pub const BOLD: usize = 2;

/// Replace `from..to` of a line with `text`; `content` is where the formatted text starts afterwards.
#[derive(Debug, PartialEq, Eq)]
struct LineEdit {
    from: usize,
    to: usize,
    text: String,
    content: usize,
}

fn stars(n: usize) -> String {
    "*".repeat(n)
}

fn count_stars(chars: &[char], range: impl Iterator<Item = usize>) -> usize {
    range.take(3).take_while(|&i| chars[i] == '*').count()
}

/// Toggle `want` on the text `s..e` of `chars`, ignoring surrounding whitespace.
fn toggle_range(chars: &[char], s: usize, e: usize, want: usize) -> Option<LineEdit> {
    let (mut s, mut e) = (s, e);
    while s < e && chars[s].is_whitespace() {
        s += 1;
    }
    while e > s && chars[e - 1].is_whitespace() {
        e -= 1;
    }
    if s == e {
        return None;
    }
    // A selection that includes its own markers is treated as the text inside them.
    let lead = count_stars(chars, s..e);
    if lead < e - s {
        let own = lead.min(count_stars(chars, (s + lead..e).rev()));
        if own > 0 {
            s += own;
            e -= own;
        }
    }
    let n = if chars[s..e].contains(&'*') {
        0
    } else {
        let before = count_stars(chars, (0..s).rev());
        let after = count_stars(chars, e..chars.len());
        before.min(after)
    };
    let new = n ^ want;
    let content: String = chars[s..e].iter().collect();
    Some(LineEdit {
        from: s - n,
        to: e + n,
        text: format!("{0}{content}{0}", stars(new)),
        content: s - n + new,
    })
}

/// Content range of the emphasis span containing `col`, mirroring the read view's parser.
fn enclosing(chars: &[char], col: usize) -> Option<(usize, usize)> {
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '*' {
            let open = count_stars(chars, i..chars.len());
            let end = (i + open..chars.len())
                .find(|&j| count_stars(chars, j..chars.len()) >= open && j > i + open);
            if let Some(end) = end {
                if (i..=end + open).contains(&col) {
                    return Some((i + open, end));
                }
                i = end + open;
                continue;
            }
            i += open;
            continue;
        }
        i += 1;
    }
    None
}

fn apply(ta: &mut Area, row: usize, edit: &LineEdit) {
    ta.cancel_selection();
    ta.move_cursor(CursorMove::Jump(row as u16, edit.from as u16));
    ta.delete_str(edit.to - edit.from);
    ta.insert_str(&edit.text);
}

fn toggle_text(text: &str, want: usize) -> Option<String> {
    let chars: Vec<char> = text.chars().collect();
    let edit = toggle_range(&chars, 0, chars.len(), want)?;
    let mut out: String = chars[..edit.from].iter().collect();
    out.push_str(&edit.text);
    out.extend(&chars[edit.to..]);
    Some(out)
}

/// Toggle bold (`BOLD`) or italic (`ITALIC`) at the cursor / on the selection.
pub fn toggle(ta: &mut Area, want: usize) {
    let (row, col) = ta.cursor();
    if let Some((a, b)) = ta.selection_range() {
        if a.0 == b.0 {
            let chars: Vec<char> = ta.lines()[a.0].chars().collect();
            if let Some(edit) = toggle_range(&chars, a.1, b.1.min(chars.len()), want) {
                apply(ta, a.0, &edit);
            }
        } else {
            wrap_lines(ta, want);
        }
    } else if table_edit::map_cell_text(ta, |t| toggle_text(t, want)) {
        return;
    } else {
        let chars: Vec<char> = ta.lines()[row].chars().collect();
        let span = enclosing(&chars, col);
        let edit =
            span.and_then(|(cs, ce)| toggle_range(&chars, cs, ce, want).map(|e| (e, cs, ce)));
        if let Some((edit, cs, ce)) = edit {
            apply(ta, row, &edit);
            let at = edit.content + col.clamp(cs, ce) - cs;
            ta.move_cursor(CursorMove::Jump(row as u16, at as u16));
        } else {
            let marks = stars(want);
            ta.insert_str(format!("{marks}{marks}"));
            for _ in 0..want {
                ta.move_cursor(CursorMove::Back);
            }
        }
    }
    table_edit::reformat_at_cursor(ta);
}

/// Multi-line selection: wrap the whole text (minus outer whitespace) in markers.
fn wrap_lines(ta: &mut Area, want: usize) {
    ta.cut();
    let text = ta.yank_text().to_string();
    let trimmed = text.trim();
    let lead = &text[..text.len() - text.trim_start().len()];
    let trail = &text[lead.len() + trimmed.len()..];
    let marks = stars(want);
    ta.insert_str(format!("{lead}{marks}{trimmed}{marks}{trail}"));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(text: &str, s: usize, e: usize, want: usize) -> String {
        let chars: Vec<char> = text.chars().collect();
        let edit = toggle_range(&chars, s, e, want).unwrap();
        let mut out: String = chars[..edit.from].iter().collect();
        out.push_str(&edit.text);
        out.extend(&chars[edit.to..]);
        out
    }

    #[test]
    fn selection_excludes_surrounding_whitespace() {
        assert_eq!(
            run("one two three four", 3, 14, BOLD),
            "one **two three** four"
        );
    }

    #[test]
    fn selecting_formatted_text_removes_formatting() {
        assert_eq!(run("one **two** three", 6, 9, BOLD), "one two three");
        assert_eq!(run("one **two** three", 4, 11, BOLD), "one two three");
        assert_eq!(run("one *two* three", 4, 9, ITALIC), "one two three");
    }

    #[test]
    fn bold_and_italic_combine_and_separate() {
        assert_eq!(run("a **b** c", 4, 5, ITALIC), "a ***b*** c");
        assert_eq!(run("a ***b*** c", 5, 6, ITALIC), "a **b** c");
        assert_eq!(run("a ***b*** c", 5, 6, BOLD), "a *b* c");
    }

    #[test]
    fn cursor_in_formatted_text_finds_span() {
        let chars: Vec<char> = "a **bold** b".chars().collect();
        assert_eq!(enclosing(&chars, 6), Some((4, 8)));
        assert_eq!(enclosing(&chars, 0), None);
        let chars: Vec<char> = "x ***both*** y".chars().collect();
        assert_eq!(enclosing(&chars, 7), Some((5, 9)));
    }

    #[test]
    fn whole_text_toggle() {
        assert_eq!(toggle_text("cell", BOLD).unwrap(), "**cell**");
        assert_eq!(toggle_text("**cell**", BOLD).unwrap(), "cell");
        assert_eq!(toggle_text("**cell**", ITALIC).unwrap(), "***cell***");
    }
}
