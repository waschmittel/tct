use tui_textarea::CursorMove;

use super::*;
use crate::insert::table_edit::{self};

/// Normalize pasted markdown: reflow tables, snap list indentation to
/// `NEST_INDENT` levels, renumber ordered lists. Fenced code is left alone.
pub(crate) fn normalize(ta: &mut tui_textarea::TextArea<'static>) {
    let fenced = fenced_rows(ta.lines());
    table_edit::reformat_all(ta, &fenced);
    fix_list_indent(ta, &fenced);
    renumber_all(ta);
}

/// Rows that are inside (or are the fence lines of) a fenced code block.
pub(super) fn fenced_rows(lines: &[String]) -> Vec<bool> {
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
pub(super) fn fix_list_indent(ta: &mut tui_textarea::TextArea<'static>, fenced: &[bool]) {
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
