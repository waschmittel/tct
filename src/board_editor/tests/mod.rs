use std::collections::HashMap;

use super::*;
use crate::app::LoadedBoard;
use crate::command::{Command, MoveDir};
use crate::model::board::{BoardMeta, ListMeta};
use crate::model::card::{Card, HistoryField};
use crate::model::id::{CardId, ListId};
use crate::model::list::CardList;
use crate::storage::{board_store, card_store};
use crate::test_support::with_temp_dir;

mod basics;
mod history;
mod invariants;
mod persistence;

fn seed_board_with_card() -> (BoardMeta, CardId) {
    let mut meta = BoardMeta::new("Test".into());
    let list_id = ListId::generate();
    meta.lists.push(ListMeta {
        id: list_id.clone(),
        name: "Backlog".into(),
        archived: false,
    });
    let mut card = Card::new("Task".into());
    card.list_id = list_id;
    card.position = 1.0;
    board_store::save_board(&meta).unwrap();
    card_store::save_card(&meta.id, &card).unwrap();
    (meta, card.id)
}

// ── Persistence roundtrips: apply(Command) → reload() → assert on disk ──
//
// These guard the full mutate→persist→re-derive path (the gap that let the
// archived-interleave move bug ship). Each loads a fresh editor, applies a
// command, reloads from disk, and asserts the effect survived.

use crate::model::label::LabelColor;

/// Seed a board with one list and the given card titles (ascending
/// positions), returning the editor plus the card ids in order.
fn editor_with_cards(titles: &[&str]) -> (BoardEditor, ListId, Vec<CardId>) {
    let mut meta = BoardMeta::new("Test".into());
    let lid = ListId::generate();
    meta.lists.push(ListMeta {
        id: lid.clone(),
        name: "L".into(),
        archived: false,
    });
    board_store::save_board(&meta).unwrap();
    let ids_v = titles
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let mut c = Card::new((*t).into());
            c.list_id = lid.clone();
            c.position = (i + 1) as f64;
            card_store::save_card(&meta.id, &c).unwrap();
            c.id
        })
        .collect();
    (BoardEditor::load(&meta.id).unwrap(), lid, ids_v)
}

fn fixed_card(id: &str, title: &str) -> Card {
    let mut c = Card::new(title.into());
    c.id = id.into();
    c
}

fn loaded_board(cards: Vec<Card>) -> LoadedBoard {
    let card_ids: Vec<_> = cards.iter().map(|c| c.id.clone()).collect();
    let cards_map: HashMap<_, _> = cards.into_iter().map(|c| (c.id.clone(), c)).collect();
    LoadedBoard {
        meta: BoardMeta::new("X".into()),
        lists: vec![CardList {
            id: "l".into(),
            name: "L".into(),
            card_ids,
        }],
        cards: cards_map,
        selected_list: 0,
        selected_card: vec![0],
        scroll_offset: vec![std::cell::Cell::new(0)],
        label_filter: None,
        detail_item_idx: 0,
        detail_scroll: 0,
        detail_max_scroll: std::cell::Cell::new(0),
    }
}

// Build a single-list board, one card homed in that list. `mutate` can
// break it before the invariant check runs.
fn invariant_board(mutate: impl FnOnce(&mut LoadedBoard)) -> BoardEditor {
    let list_id = ListId::parse("lst00001").unwrap();
    let mut meta = BoardMeta::new("X".into());
    meta.lists.push(ListMeta {
        id: list_id.clone(),
        name: "L".into(),
        archived: false,
    });
    let mut card = fixed_card("c0000001", "a");
    card.list_id = list_id;
    card.position = 1.0;
    let cards: HashMap<_, _> = std::iter::once((card.id.clone(), card)).collect();
    let lists = crate::model::list::build_lists(&meta, &cards, false);
    let mut board = LoadedBoard {
        meta,
        lists,
        cards,
        selected_list: 0,
        selected_card: vec![0],
        scroll_offset: vec![std::cell::Cell::new(0)],
        label_filter: None,
        detail_item_idx: 0,
        detail_scroll: 0,
        detail_max_scroll: std::cell::Cell::new(0),
    };
    mutate(&mut board);
    BoardEditor::from_loaded(board)
}
