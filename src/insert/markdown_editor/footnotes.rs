use tui_textarea::CursorMove;

use crate::footnote;
use crate::insert::table_edit::{self};

/// Insert a footnote reference and add its empty definition, with the cursor
/// behind it: a numbered one among the numbered definitions by its number, any
/// other at the very bottom. Without a selection the reference is the next
/// number (numbered footnotes are renumbered to ascend by first reference);
/// highlighted text stays and is named after instead (`[^two-words]`, inserted
/// behind it). A name that is already defined just jumps to that definition.
/// Definitions form one block at the end; anything else gets a blank line first.
pub(super) fn insert_footnote(ta: &mut tui_textarea::TextArea<'static>) {
    let selected = ta.selection_range().map(|(start, end)| {
        let lines = ta.lines();
        let text = if start.0 == end.0 {
            lines[start.0]
                .chars()
                .skip(start.1)
                .take(end.1 - start.1)
                .collect()
        } else {
            let first: String = lines[start.0].chars().skip(start.1).collect();
            let last: String = lines[end.0].chars().take(end.1).collect();
            let middle = lines[start.0 + 1..end.0].join(" ");
            format!("{first} {middle} {last}")
        };
        (end, text)
    });
    ta.cancel_selection();
    let named = selected.and_then(|(end, text)| {
        let label = footnote::label_from(&text)?;
        ta.move_cursor(CursorMove::Jump(end.0 as u16, end.1 as u16));
        Some(label)
    });

    let label = match named {
        Some(label) => {
            leave_reference(ta);
            ta.insert_str(format!("[^{label}]"));
            table_edit::reformat_at_cursor(ta);
            label
        }
        None => insert_numbered_reference(ta),
    };

    if let Some(def) = footnote::definitions(ta.lines())
        .into_iter()
        .find(|d| d.label == label)
    {
        let end = ta.lines()[def.end].chars().count();
        ta.move_cursor(CursorMove::Jump(def.end as u16, end as u16));
        return;
    }

    if place_numbered_definition(ta, &label) {
        return;
    }

    let lines = ta.lines();
    let last = lines
        .iter()
        .rposition(|l| !l.trim().is_empty())
        .unwrap_or(0);
    if last + 1 < lines.len() {
        // Trailing blank lines make way for the definition.
        let chars: usize = lines[last + 1..]
            .iter()
            .map(|l| l.chars().count() + 1)
            .sum();
        let end = lines[last].chars().count();
        ta.move_cursor(CursorMove::Jump(last as u16, end as u16));
        ta.delete_str(chars);
    }
    let lines = ta.lines();
    let last = lines.len() - 1;
    let in_defs = footnote::definitions(lines)
        .last()
        .is_some_and(|d| d.end == last);
    ta.move_cursor(CursorMove::Jump(
        last as u16,
        lines[last].chars().count() as u16,
    ));
    let gap = if in_defs { "\n" } else { "\n\n" };
    ta.insert_str(format!("{gap}[^{label}]: "));
}

/// A cursor inside an existing `[^x]` would split it: move behind it.
pub(super) fn leave_reference(ta: &mut tui_textarea::TextArea<'static>) {
    let (row, col) = ta.cursor();
    let inside = footnote::refs(&ta.lines()[row])
        .into_iter()
        .find(|(start, len, _)| *start < col && col < start + len);
    if let Some((start, len, _)) = inside {
        ta.move_cursor(CursorMove::Jump(row as u16, (start + len) as u16));
    }
}

/// Put the empty definition of a numbered footnote among the other numbered
/// definitions, where its number belongs: right behind the definition with the
/// highest smaller number, else in front of the one with the lowest larger
/// number. Cursor behind the new `[^n]: `. Returns false (nothing done) for a
/// named footnote or when there is no numbered definition to line up with.
pub(super) fn place_numbered_definition(
    ta: &mut tui_textarea::TextArea<'static>,
    label: &str,
) -> bool {
    let Ok(n) = label.parse::<u64>() else {
        return false;
    };
    let numbered: Vec<(u64, footnote::Def)> = footnote::definitions(ta.lines())
        .into_iter()
        .filter_map(|d| Some((d.label.parse::<u64>().ok()?, d)))
        .collect();
    let before = numbered
        .iter()
        .filter(|(m, _)| *m < n)
        .max_by_key(|(m, d)| (*m, d.start));
    let after = numbered
        .iter()
        .filter(|(m, _)| *m > n)
        .min_by_key(|(m, d)| (*m, d.start));
    let marker = format!("[^{label}]: ");
    if let Some((_, def)) = before {
        let end = ta.lines()[def.end].chars().count();
        ta.move_cursor(CursorMove::Jump(def.end as u16, end as u16));
        ta.insert_str(format!("\n{marker}"));
    } else if let Some((_, def)) = after {
        ta.move_cursor(CursorMove::Jump(def.start as u16, 0));
        ta.insert_str(format!("{marker}\n"));
        let col = marker.chars().count();
        ta.move_cursor(CursorMove::Jump(def.start as u16, col as u16));
    } else {
        return false;
    }
    true
}

/// Insert `[^n]` at the cursor and renumber the numbered footnotes so they
/// ascend in the order of their references; returns the label it got.
pub(super) fn insert_numbered_reference(ta: &mut tui_textarea::TextArea<'static>) -> String {
    let placeholder = footnote::next_number(ta.lines()).to_string();
    leave_reference(ta);
    ta.insert_str(format!("[^{placeholder}]"));
    let (row, col) = ta.cursor();
    let old = ta.lines().to_vec();
    let renumbered = footnote::renumbered(&old, Some(&placeholder));
    let label = renumbered.label(&placeholder);
    let prefix: String = old[row].chars().take(col).collect();
    let new_col = renumbered.relabel(&prefix).chars().count();
    for (r, (was, now)) in old.iter().zip(&renumbered.lines).enumerate() {
        if was != now {
            ta.move_cursor(CursorMove::Jump(r as u16, 0));
            ta.delete_str(was.chars().count());
            ta.insert_str(now);
        }
    }
    ta.move_cursor(CursorMove::Jump(row as u16, new_col as u16));
    table_edit::reformat_at_cursor(ta);
    label
}
