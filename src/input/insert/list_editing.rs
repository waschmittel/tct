//! Markdown list autocontinue and renumbering logic for the description editor.
//!
//! - `handle_enter_in_list` — on Enter, continue a bullet/numbered list, or
//!   exit the list if the current item is empty.
//! - `handle_tab_nest` / `handle_shift_tab_unnest` — change the current
//!   item's indent by [`NEST_INDENT`] spaces.
//! - `renumber_all` — walk the whole document and rewrite numbered list
//!   prefixes so every run is sequential. Independent runs (separated by
//!   blank lines or non-list lines) are renumbered independently.

use ratatui_textarea::CursorMove;

use crate::app::App;

pub(super) const NEST_INDENT: usize = 3;

pub(super) fn handle_enter_in_list(app: &mut App) {
    let Some(textarea) = &mut app.description_editor else {
        return;
    };
    let ratatui_textarea::DataCursor(row, col) = textarea.cursor();

    // At the very start of the document: insert a blank line above without
    // list continuation, so users can add content before a leading list item.
    // Only special-cased for (0, 0) — for other positions, users can navigate
    // to the previous line and continue normally.
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
            // Placeholder number; renumber_all rewrites it to the
            // correct value, so the literal here doesn't matter beyond shape.
            textarea.insert_str(format!("{indent_str}1. "));
            renumber_all(textarea);
            // Park the cursor at the end of the new item, regardless of how
            // many digits its prefix ended up with.
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

/// Returns true if the cursor was on a list line and the nest was applied.
/// On a numbered list the inserted item is forced to start at 1 so it
/// becomes the start of a fresh nested list (`renumber_all` then joins it
/// to any existing nested run at the new indent).
pub(super) fn handle_tab_nest(app: &mut App) -> bool {
    let Some(textarea) = &mut app.description_editor else {
        return false;
    };
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

/// Remove one indent level (`NEST_INDENT` leading spaces) if the cursor
/// is on a list line that has enough leading spaces. After dedenting,
/// `renumber_all` rejoins the line to the parent-level list.
pub(super) fn handle_shift_tab_unnest(app: &mut App) {
    let Some(textarea) = &mut app.description_editor else {
        return;
    };
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

/// Walk the entire document with a per-indent stack of "next expected
/// number" counters and rewrite every numbered list item so each
/// contiguous run is sequential. Each run's start number is preserved
/// from the run's first item, so lists that intentionally start at a
/// non-1 number keep their starting point. Blank lines reset all
/// active runs; non-numbered lines end runs at indent >= their indent
/// so unrelated lists stay independent. Cursor position is preserved.
pub(super) fn renumber_all(textarea: &mut ratatui_textarea::TextArea<'static>) {
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
            // A non-blank, non-numbered line ends any run whose indent
            // is >= this line's indent. Deeper-indented continuation
            // lines (indent strictly greater than every active level)
            // are left alone.
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
