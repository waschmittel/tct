//! Argument-parsing primitives and shared formatting helpers used across
//! CLI subcommands. The arg helpers expect already-split positional args
//! (the leading subcommand is stripped in [`super::run`]).

use std::collections::HashMap;

use chrono::Utc;

use crate::model::board::BoardMeta;
use crate::model::card::Card;
use crate::model::ids::ShortId;
use crate::model::label::{Label, LabelColor};
use crate::model::list::CardList;
use crate::storage::{board_store, card_store, list_store};

// ── Argument parsing ──────────────────────────────────────────────────────────

pub(super) fn has_flag(args: &[String], flag: &str) -> bool {
    args.iter().any(|a| a == flag)
}

pub(super) fn flag_value<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
    args.windows(2)
        .find(|w| w[0] == flag)
        .map(|w| w[1].as_str())
}

pub(super) fn flag_values2<'a>(args: &'a [String], flag: &str) -> Option<(&'a str, &'a str)> {
    args.windows(3)
        .find(|w| w[0] == flag)
        .map(|w| (w[1].as_str(), w[2].as_str()))
}

pub(super) fn flag_values_all<'a>(args: &'a [String], flag: &str) -> Vec<&'a str> {
    args.windows(2)
        .filter(|w| w[0] == flag)
        .map(|w| w[1].as_str())
        .collect()
}

// ── Data loading ──────────────────────────────────────────────────────────────

pub(super) fn load_all_cards(board_id: &str, lists: &[CardList]) -> HashMap<ShortId, Card> {
    let mut map = HashMap::new();
    for list in lists {
        for card_id in &list.card_ids {
            if let Ok(card) = card_store::load_card(board_id, card_id) {
                map.insert(card_id.clone(), card);
            }
        }
    }
    map
}

pub(super) fn count_active(card_ids: &[ShortId], cards: &HashMap<ShortId, Card>) -> usize {
    card_ids
        .iter()
        .filter(|id| cards.get(*id).map(|c| !c.archived).unwrap_or(false))
        .count()
}

pub(super) fn board_summary_counts(board_id: &str) -> (usize, usize) {
    let meta = match board_store::load_board(board_id) {
        Ok(m) => m,
        Err(_) => return (0, 0),
    };
    let lists = match list_store::load_all_lists(board_id, &meta.list_order) {
        Ok(l) => l,
        Err(_) => return (meta.list_order.len(), 0),
    };
    let cards = load_all_cards(board_id, &lists);
    let total = lists
        .iter()
        .map(|l| count_active(&l.card_ids, &cards))
        .sum();
    (lists.len(), total)
}

// ── Output formatting ─────────────────────────────────────────────────────────

pub(super) fn print_card_line(idx: usize, card: &Card, labels: &[Label]) {
    let label_names: Vec<_> = card
        .resolved_labels(labels)
        .iter()
        .map(|l| l.name.as_str())
        .collect();
    let label_str = if label_names.is_empty() {
        String::new()
    } else {
        format!("  [{}]", label_names.join(", "))
    };
    let due_str = match card.due_date {
        None => String::new(),
        Some(d) => {
            let today = Utc::now().date_naive();
            if d < today {
                format!("  due: {d} (OVERDUE)")
            } else {
                format!("  due: {d}")
            }
        }
    };
    let checklist_str = match card.checklist_progress() {
        None => String::new(),
        Some((done, total)) => format!("  checklist: {done}/{total}"),
    };
    println!(
        "    {idx}. [{}]  {}{label_str}{due_str}{checklist_str}",
        card.id, card.title
    );
}

pub(super) fn print_card_detail(card: &Card, board: &BoardMeta, list: &CardList) {
    println!("Card:        {} [{}]", card.title, card.id);
    println!("Board:       {} [{}]", board.name, board.id);
    println!("List:        {} [{}]", list.name, list.id);

    let label_names: Vec<_> = card
        .resolved_labels(&board.labels)
        .iter()
        .map(|l| l.name.as_str())
        .collect();
    if !label_names.is_empty() {
        println!("Labels:      {}", label_names.join(", "));
    }

    if let Some(d) = card.due_date {
        let today = Utc::now().date_naive();
        if d < today {
            println!("Due:         {d} (OVERDUE)");
        } else {
            println!("Due:         {d}");
        }
    }

    if let Some((done, total)) = card.checklist_progress() {
        println!("Checklist:   {done}/{total}");
        for item in &card.checklist {
            let mark = if item.completed { "x" } else { " " };
            println!("  [{}] {}", mark, item.text);
        }
    }

    println!("Created:     {}", card.created_at.format("%Y-%m-%d"));
    println!("Updated:     {}", card.updated_at.format("%Y-%m-%d"));

    if !card.description.is_empty() {
        println!("Description:");
        for line in card.description.lines() {
            println!("  {line}");
        }
    }
}

pub(super) fn fmt_progress(card: &Card) -> String {
    match card.checklist_progress() {
        None => "no items".to_string(),
        Some((done, total)) => format!("{done}/{total}"),
    }
}

pub(super) fn label_color_name(color: &LabelColor) -> &'static str {
    match color {
        LabelColor::Red => "red",
        LabelColor::Orange => "orange",
        LabelColor::Yellow => "yellow",
        LabelColor::Green => "green",
        LabelColor::Blue => "blue",
        LabelColor::Purple => "purple",
        LabelColor::Pink => "pink",
        LabelColor::Cyan => "cyan",
        LabelColor::Custom { .. } => "custom",
    }
}
