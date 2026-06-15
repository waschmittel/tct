//! Board Directory — owns the collection of Boards (create, archive,
//! restore, rename, ordering, listing). The Board Editor (ADR-0001) owns
//! exactly one loaded Board; everything that spans the collection or
//! happens before a board is loaded lives here. Adapters: the Board
//! Selector input handler and the `tct boards` CLI subcommand.

use crate::board_editor::BoardEditor;
use crate::command::Command;
use crate::model::board::BoardMeta;
use crate::model::ids::ShortId;
use crate::model::label::LabelColor;
use crate::storage::board_store;

pub fn list() -> anyhow::Result<Vec<BoardMeta>> {
    Ok(board_store::list_boards()?)
}

pub fn list_archived() -> anyhow::Result<Vec<BoardMeta>> {
    Ok(board_store::list_archived_boards()?)
}

/// Create a Board with a pastel Accent Color differentiated from the
/// existing boards, persist it, and append it to the display order.
pub fn create(name: String) -> anyhow::Result<BoardMeta> {
    let existing_colors: Vec<_> = board_store::list_boards()?
        .iter()
        .map(|b| b.accent_color)
        .collect();
    let mut meta = BoardMeta::new(name);
    meta.accent_color = LabelColor::generate_pastel(&existing_colors);
    board_store::save_board(&meta)?;
    board_store::append_to_order(&meta.id)?;
    Ok(meta)
}

pub fn archive(board_id: &ShortId) -> anyhow::Result<()> {
    let mut editor = BoardEditor::load(board_id)?;
    editor.apply(Command::ArchiveBoard { board_id: board_id.clone() })?;
    board_store::remove_from_order(board_id)?;
    Ok(())
}

pub fn restore(board_id: &ShortId) -> anyhow::Result<()> {
    let mut editor = BoardEditor::load(board_id)?;
    editor.apply(Command::RestoreBoard { board_id: board_id.clone() })?;
    board_store::append_to_order(board_id)?;
    Ok(())
}

pub fn rename(board_id: &ShortId, name: String) -> anyhow::Result<()> {
    let mut editor = BoardEditor::load(board_id)?;
    editor.apply(Command::RenameBoard { name })?;
    Ok(())
}

/// Cycle the Board's Accent Color to the next named variant and persist.
pub fn cycle_accent(board_id: &ShortId) -> anyhow::Result<LabelColor> {
    let mut editor = BoardEditor::load(board_id)?;
    let next = editor.board().meta.accent_color.next();
    editor.apply(Command::SetAccentColor { color: next })?;
    Ok(next)
}

/// Set the Board's Accent Color to an explicit value and persist.
pub fn set_accent(board_id: &ShortId, color: LabelColor) -> anyhow::Result<()> {
    let mut editor = BoardEditor::load(board_id)?;
    editor.apply(Command::SetAccentColor { color })?;
    Ok(())
}

/// Permanently delete an archived Board's directory from disk.
pub fn delete(board_id: &str) -> anyhow::Result<()> {
    Ok(board_store::delete_board(board_id)?)
}

/// Swap two Boards in the persisted display order. `displayed` is the
/// current on-screen order, used to backfill boards missing from the
/// order file.
pub fn swap_order(a: &ShortId, b: &ShortId, displayed: &[ShortId]) -> anyhow::Result<()> {
    let mut order = board_store::load_board_order().unwrap_or_default();
    if !order.contains(a) || !order.contains(b) {
        for id in displayed {
            if !order.contains(id) {
                order.push(id.clone());
            }
        }
    }
    if let (Some(pos_a), Some(pos_b)) = (
        order.iter().position(|id| id == a),
        order.iter().position(|id| id == b),
    ) {
        order.swap(pos_a, pos_b);
        board_store::save_board_order(&order)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::with_temp_dir;

    #[test]
    fn create_persists_and_appends_to_order() {
        with_temp_dir(|| {
            let meta = create("Fresh".into()).unwrap();
            let listed = list().unwrap();
            assert_eq!(listed.len(), 1);
            assert_eq!(listed[0].name, "Fresh");
            let order = board_store::load_board_order().unwrap();
            assert_eq!(order, vec![meta.id]);
        });
    }

    #[test]
    fn create_differentiates_accent_colors() {
        with_temp_dir(|| {
            let a = create("A".into()).unwrap();
            let b = create("B".into()).unwrap();
            assert_ne!(a.accent_color, b.accent_color);
        });
    }

    #[test]
    fn archive_and_restore_roundtrip() {
        with_temp_dir(|| {
            let meta = create("Board".into()).unwrap();
            archive(&meta.id).unwrap();
            assert!(list().unwrap().is_empty());
            assert_eq!(list_archived().unwrap().len(), 1);
            assert!(board_store::load_board_order().unwrap().is_empty());

            restore(&meta.id).unwrap();
            assert_eq!(list().unwrap().len(), 1);
            assert!(list_archived().unwrap().is_empty());
            assert_eq!(board_store::load_board_order().unwrap(), vec![meta.id]);
        });
    }

    #[test]
    fn swap_order_persists() {
        with_temp_dir(|| {
            let a = create("A".into()).unwrap();
            let b = create("B".into()).unwrap();
            swap_order(&a.id, &b.id, &[a.id.clone(), b.id.clone()]).unwrap();
            let order = board_store::load_board_order().unwrap();
            assert_eq!(order, vec![b.id, a.id]);
        });
    }

    #[test]
    fn cycle_accent_changes_and_persists() {
        with_temp_dir(|| {
            let meta = create("Acc".into()).unwrap();
            let before = meta.accent_color;
            let after = cycle_accent(&meta.id).unwrap();
            assert_ne!(before, after);
            let reloaded = board_store::load_board(&meta.id).unwrap();
            assert_eq!(reloaded.accent_color, after);
        });
    }
}
