use tui_textarea::CursorMove;

use super::*;

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
pub(super) fn split_line_for_new_item(
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
pub(super) fn shift_list_item(textarea: &mut tui_textarea::TextArea<'static>, nest: bool) -> bool {
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
pub(super) enum Marker {
    Bullet(char),
    Numbered,
    Citation,
}

/// Parse a list or citation marker at the start of `trimmed`: its kind and length in chars.
pub(super) fn parse_marker(trimmed: &str) -> Option<(Marker, usize)> {
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
pub(super) fn sibling_marker(lines: &[String], row: usize, indent: usize) -> Option<Marker> {
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
