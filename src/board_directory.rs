//! Board Directory — owns the collection of Boards (create, archive,
//! restore, rename, ordering, listing). The Board Editor (ADR-0001) owns
//! exactly one loaded Board; everything that spans the collection or
//! happens before a board is loaded lives here. Adapters: the Board
//! Selector input handler and the `tct boards` CLI subcommand.

use crate::board_editor::BoardEditor;
use crate::command::Command;
use crate::model::board::BoardMeta;
use crate::model::id::BoardId;
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
    let existing = board_store::list_boards()?;
    let mut meta = BoardMeta::new(name);
    meta.accent_color =
        LabelColor::generate_pastel(&existing.iter().map(|b| b.accent_color).collect::<Vec<_>>());
    meta.position = crate::model::rank::after_last(existing.iter().map(|b| b.position));
    board_store::save_board(&meta)?;
    Ok(meta)
}

pub fn archive(board_id: &BoardId) -> anyhow::Result<()> {
    let mut editor = BoardEditor::load(board_id)?;
    editor.apply(Command::ArchiveBoard { board_id: board_id.clone() })?;
    Ok(())
}

pub fn restore(board_id: &BoardId) -> anyhow::Result<()> {
    let mut editor = BoardEditor::load(board_id)?;
    editor.apply(Command::RestoreBoard { board_id: board_id.clone() })?;
    Ok(())
}

pub fn rename(board_id: &BoardId, name: String) -> anyhow::Result<()> {
    let mut editor = BoardEditor::load(board_id)?;
    editor.apply(Command::RenameBoard { name })?;
    Ok(())
}

/// Cycle the Board's Accent Color to the next named variant and persist.
pub fn cycle_accent(board_id: &BoardId) -> anyhow::Result<LabelColor> {
    let mut editor = BoardEditor::load(board_id)?;
    let next = editor.board().meta.accent_color.next();
    editor.apply(Command::SetAccentColor { color: next })?;
    Ok(next)
}

/// Set the Board's Accent Color to an explicit value and persist.
pub fn set_accent(board_id: &BoardId, color: LabelColor) -> anyhow::Result<()> {
    let mut editor = BoardEditor::load(board_id)?;
    editor.apply(Command::SetAccentColor { color })?;
    Ok(())
}

/// Permanently delete an archived Board's directory from disk.
pub fn delete(board_id: &BoardId) -> anyhow::Result<()> {
    Ok(board_store::delete_board(board_id)?)
}

/// Move the Board at `from` to index `to` within `displayed`, the current
/// on-screen order, and persist the result.
///
/// Renumbers the whole (small) collection rather than taking the midpoint of
/// the moved Board's new neighbours: Boards written before ranks existed all
/// sit at 0.0, where a midpoint would not separate them. Each Board is
/// re-read before its rank is written, so a concurrent edit elsewhere in the
/// file is not clobbered, and only Boards whose rank actually changes are
/// written.
pub fn reorder(displayed: &[BoardMeta], from: usize, to: usize) -> anyhow::Result<()> {
    if from >= displayed.len() || to >= displayed.len() {
        return Ok(());
    }
    let mut order: Vec<&BoardMeta> = displayed.iter().collect();
    let moved = order.remove(from);
    order.insert(to, moved);

    for (idx, meta) in order.iter().enumerate() {
        let rank = (idx + 1) as f64;
        if meta.position != rank {
            let mut fresh = board_store::load_board(&meta.id)?;
            fresh.position = rank;
            board_store::save_board(&fresh)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::with_temp_dir;

    #[test]
    fn create_persists_and_ranks_after_the_last_board() {
        with_temp_dir(|| {
            let first = create("First".into()).unwrap();
            let second = create("Second".into()).unwrap();

            let listed = list().unwrap();

            assert_eq!(listed.iter().map(|b| b.name.as_str()).collect::<Vec<_>>(), ["First", "Second"]);
            assert!(second.position > first.position);
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

            restore(&meta.id).unwrap();
            assert_eq!(list().unwrap().len(), 1);
            assert!(list_archived().unwrap().is_empty());
        });
    }

    #[test]
    fn reorder_persists() {
        with_temp_dir(|| {
            create("A".into()).unwrap();
            create("B".into()).unwrap();
            create("C".into()).unwrap();
            let displayed = list().unwrap();

            reorder(&displayed, 2, 0).unwrap();

            let names: Vec<_> = list().unwrap().iter().map(|b| b.name.clone()).collect();
            assert_eq!(names, ["C", "A", "B"]);
        });
    }

    /// Boards written before ranks existed all sit at 0.0; a midpoint would
    /// not separate them, so the reorder has to renumber.
    #[test]
    fn reorder_separates_boards_that_all_share_a_rank() {
        with_temp_dir(|| {
            for name in ["A", "B", "C"] {
                let mut meta = crate::model::board::BoardMeta::new(name.into());
                meta.position = 0.0;
                board_store::save_board(&meta).unwrap();
            }
            let displayed = list().unwrap();

            reorder(&displayed, 0, 2).unwrap();

            let ranks: Vec<_> = list().unwrap().iter().map(|b| b.position).collect();
            assert_eq!(ranks, [1.0, 2.0, 3.0]);
            let names: Vec<_> = list().unwrap().iter().map(|b| b.name.clone()).collect();
            assert_eq!(names, ["B", "C", "A"]);
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
