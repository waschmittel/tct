//! Turning the quote under the cursor into an alert (`Ctrl+G`, then a key),
//! changing its kind, or back into a plain quote.

use crossterm::event::{KeyCode, KeyEvent};
use tui_textarea::{CursorMove, TextArea};

use crate::alert::Alert;
use crate::ui::markdown::{citation_marker_len, closes_quote, is_quote_fence, quote_alert};

type Area = TextArea<'static>;

/// Hint shown while the `Ctrl+G` chord waits for its key.
pub const CHORD_HINT: &str =
    "Alert: n note, t tip, i important, c caution, w warning, x plain quote";

/// The key that follows `Ctrl+G`: `Some(None)` removes the alert.
pub fn chord_kind(key: KeyEvent) -> Option<Option<Alert>> {
    let KeyCode::Char(c) = key.code else {
        return None;
    };
    Some(Some(match c.to_ascii_lowercase() {
        'n' => Alert::Note,
        't' => Alert::Tip,
        'i' => Alert::Important,
        'c' => Alert::Caution,
        'w' => Alert::Warning,
        'x' => return Some(None),
        _ => return None,
    }))
}

enum Region {
    /// Multiline quote opened by the fence on this line.
    Fence(usize),
    /// Run of `>` lines starting on this line.
    Cite(usize),
}

fn region(lines: &[String], row: usize) -> Option<Region> {
    let (mut in_code, mut open) = (false, None);
    for (i, line) in lines.iter().enumerate().take(row + 1) {
        if line.starts_with("```") {
            in_code = !in_code;
        } else if !in_code && is_quote_fence(line) && (open.is_none() || closes_quote(line)) {
            match open {
                Some(start) if i == row => return Some(Region::Fence(start)),
                Some(_) => open = None,
                None => open = Some(i),
            }
        }
    }
    if let Some(i) = open {
        return Some(Region::Fence(i));
    }
    let cite = |l: &String| citation_marker_len(l.trim_start()).is_some();
    if in_code || !cite(&lines[row]) {
        return None;
    }
    let first = (0..=row).rev().take_while(|&i| cite(&lines[i])).last()?;
    Some(Region::Cite(first))
}

pub fn in_quote(ta: &Area) -> bool {
    region(ta.lines(), ta.cursor().0).is_some()
}

fn replace_line(ta: &mut Area, row: usize, text: &str) {
    ta.move_cursor(CursorMove::Jump(row as u16, 0));
    ta.delete_line_by_end();
    ta.insert_str(text);
}

/// Set (or with `None` remove) the alert of the quote under the cursor.
/// Returns a message when there is nothing to do.
pub fn set(ta: &mut Area, kind: Option<Alert>) -> Option<String> {
    let (row, col) = ta.cursor();
    let lines: Vec<String> = ta.lines().to_vec();
    let Some(region) = region(&lines, row) else {
        return Some("Not in a quote".into());
    };
    let mut target = row;
    match region {
        Region::Fence(open) => {
            let lead = &lines[open][..lines[open].len() - lines[open].trim_start().len()];
            let text = match kind {
                Some(k) => format!("{lead}>>> {}", k.tag()),
                None => format!("{lead}>>>"),
            };
            replace_line(ta, open, &text);
        }
        Region::Cite(first) => {
            let has_tag = quote_alert(&lines[first]).is_some();
            match (kind, has_tag) {
                (Some(k), true) => replace_line(ta, first, &format!("> {}", k.tag())),
                (Some(k), false) => {
                    ta.move_cursor(CursorMove::Jump(first as u16, 0));
                    ta.insert_str(format!("> {}\n", k.tag()));
                    target += 1;
                }
                (None, true) => {
                    replace_line(ta, first, "");
                    if first + 1 < lines.len() {
                        ta.delete_next_char();
                    } else if first > 0 {
                        ta.delete_char();
                    }
                    if first < row {
                        target -= 1;
                    }
                }
                (None, false) => {}
            }
        }
    }
    ta.move_cursor(CursorMove::Jump(target as u16, col as u16));
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn area(text: &str, cursor: (usize, usize)) -> Area {
        let mut ta = TextArea::from(text.lines().map(str::to_string));
        ta.move_cursor(CursorMove::Jump(cursor.0 as u16, cursor.1 as u16));
        ta
    }

    fn text(ta: &Area) -> String {
        ta.lines().join("\n")
    }

    #[test]
    fn plain_quote_gets_a_title_line() {
        let mut ta = area("a\n> one\n> two\nb", (2, 3));
        assert_eq!(set(&mut ta, Some(Alert::Tip)), None);
        assert_eq!(text(&ta), "a\n> [!tip]\n> one\n> two\nb");
        assert_eq!(ta.cursor(), (3, 3));
    }

    #[test]
    fn kind_is_replaced_and_removed() {
        let mut ta = area("> [!note]\n> one", (1, 2));
        set(&mut ta, Some(Alert::Warning));
        assert_eq!(text(&ta), "> [!warning]\n> one");
        set(&mut ta, None);
        assert_eq!(text(&ta), "> one");
        assert_eq!(ta.cursor(), (0, 2));
    }

    #[test]
    fn removing_from_a_title_only_quote_at_the_end() {
        let mut ta = area("a\n> [!note]", (1, 0));
        set(&mut ta, None);
        assert_eq!(text(&ta), "a");
    }

    #[test]
    fn multiline_quote_fence_takes_the_tag() {
        let mut ta = area("x\n>>>\none\n\ntwo\n>>>\ny", (4, 1));
        set(&mut ta, Some(Alert::Important));
        assert_eq!(text(&ta), "x\n>>> [!important]\none\n\ntwo\n>>>\ny");
        set(&mut ta, Some(Alert::Note));
        assert_eq!(text(&ta), "x\n>>> [!note]\none\n\ntwo\n>>>\ny");
        set(&mut ta, None);
        assert_eq!(text(&ta), "x\n>>>\none\n\ntwo\n>>>\ny");
    }

    #[test]
    fn works_from_the_closing_fence_but_not_outside() {
        let mut ta = area(">>>\none\n>>>\nafter", (2, 0));
        assert_eq!(set(&mut ta, Some(Alert::Caution)), None);
        assert!(text(&ta).starts_with(">>> [!caution]\n"));
        let mut ta = area(">>>\none\n>>>\nafter", (3, 0));
        assert_eq!(
            set(&mut ta, Some(Alert::Note)).as_deref(),
            Some("Not in a quote")
        );
        assert!(!in_quote(&ta));
    }

    #[test]
    fn quotes_in_code_blocks_are_left_alone() {
        let ta = area("```\n> x\n```", (1, 0));
        assert!(!in_quote(&ta));
    }

    #[test]
    fn chord_keys() {
        let k = |c| KeyEvent::new(KeyCode::Char(c), crossterm::event::KeyModifiers::empty());
        assert_eq!(chord_kind(k('N')), Some(Some(Alert::Note)));
        assert_eq!(chord_kind(k('x')), Some(None));
        assert_eq!(chord_kind(k('z')), None);
    }
}
