use crate::board_editor::BoardEditor;
use crate::command::{Command, MoveDir};
use crate::model::board::{BoardMeta, ListMeta};
use crate::model::card::{Card, ChecklistItem};
use crate::model::id::{CardId, ListId};
use crate::model::label::{Label, LabelColor};
use crate::storage::{board_store, card_store};
use crate::test_support::with_temp_dir;

mod editing;
mod persistence;
mod search;

#[allow(unused_imports)]
use regex::Regex;

// ── CLI search tests ──────────────────────────────────────────────────────

fn run_search(args: &[&str]) -> anyhow::Result<()> {
    let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    crate::cli::run(&args)
}

fn setup_search_fixture() -> (crate::model::board::BoardMeta, Card, Card) {
    let mut board = board_store::create_board("Alpha".into()).unwrap();

    let todo = crate::test_support::add_list(&mut board, "To Do");
    let done = crate::test_support::add_list(&mut board, "Done");

    let mut card1 = Card::new("Fix login bug".into());
    card1.description = "Auth token expires too early".into();
    card1.checklist.push(ChecklistItem {
        text: "Reproduce issue".into(),
        completed: false,
    });
    card1.checklist.push(ChecklistItem {
        text: "Write failing test".into(),
        completed: false,
    });

    let mut card2 = Card::new("Redesign dashboard".into());
    card2.description = "New layout with login metrics".into();

    crate::test_support::put_cards(&board.id, &todo, std::slice::from_mut(&mut card1));
    crate::test_support::put_cards(&board.id, &done, std::slice::from_mut(&mut card2));

    (board, card1, card2)
}

/// Build a two-list board on disk and load an editor. `a`/`b` give the
/// titles for list A / list B; cards get ascending integer positions.
fn two_list_editor(a: &[&str], b: &[&str]) -> (BoardEditor, Vec<CardId>, Vec<CardId>) {
    let mut meta = BoardMeta::new("Board".into());
    let la = ListId::generate();
    let lb = ListId::generate();
    meta.lists.push(ListMeta {
        id: la.clone(),
        name: "A".into(),
        archived: false,
    });
    meta.lists.push(ListMeta {
        id: lb.clone(),
        name: "B".into(),
        archived: false,
    });
    board_store::save_board(&meta).unwrap();
    let save = |titles: &[&str], lid: &ListId| {
        titles
            .iter()
            .enumerate()
            .map(|(i, t)| {
                let mut c = Card::new((*t).into());
                c.list_id = lid.clone();
                c.position = (i + 1) as f64;
                card_store::save_card(&meta.id, &c).unwrap();
                c.id
            })
            .collect()
    };
    let a_ids = save(a, &la);
    let b_ids = save(b, &lb);
    (BoardEditor::load(&meta.id).unwrap(), a_ids, b_ids)
}
