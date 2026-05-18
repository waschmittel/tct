//! `tct search` subcommand: find cards across all (or filtered) boards.

use anyhow::{bail, Context};
use regex::Regex;

use super::util::{flag_value, flag_values_all, has_flag, load_all_cards, print_card_line};
use crate::model::board::BoardMeta;
use crate::model::card::Card;
use crate::model::label::Label;
use crate::storage::{board_store, list_store};

pub(super) fn run(args: &[String]) -> anyhow::Result<()> {
    let query = args
        .first()
        .ok_or_else(|| {
            anyhow::anyhow!(
                "Usage: tct search <query> [--board <name>] [--list <name>] [--regex] [--archived]"
            )
        })?
        .as_str();

    let use_regex = has_flag(args, "--regex");
    let include_archived = has_flag(args, "--archived");
    let board_filters: Vec<&str> = flag_values_all(args, "--board");
    let list_filter = flag_value(args, "--list");

    let compiled_regex = if use_regex {
        Some(Regex::new(query).context("Invalid regular expression")?)
    } else {
        None
    };

    let all_boards = board_store::list_boards()?;
    let boards_to_search: Vec<BoardMeta> = if board_filters.is_empty() {
        all_boards
    } else {
        all_boards
            .into_iter()
            .filter(|b| {
                let name_lower = b.name.to_lowercase();
                board_filters
                    .iter()
                    .any(|f| name_lower.contains(&f.to_lowercase()))
            })
            .collect()
    };

    if boards_to_search.is_empty() && !board_filters.is_empty() {
        bail!("No boards match the --board filter.");
    }

    let mode = if use_regex { "regex" } else { "substring" };
    let board_count = boards_to_search.len();
    println!("Searching {board_count} board(s) for {query:?} ({mode}):");

    let mut total = 0usize;

    for board in &boards_to_search {
        let lists = list_store::load_all_lists(&board.id, &board.list_order)?;
        let all_cards = load_all_cards(&board.id, &lists);

        for list in &lists {
            if let Some(lf) = list_filter
                && !list.name.to_lowercase().contains(&lf.to_lowercase())
            {
                continue;
            }

            let matching: Vec<&Card> = list
                .card_ids
                .iter()
                .filter_map(|id| all_cards.get(id))
                .filter(|card| {
                    if !include_archived && card.archived {
                        return false;
                    }
                    match &compiled_regex {
                        Some(re) => card_matches_regex(card, re, &board.labels),
                        None => card.matches_search(query, &board.labels),
                    }
                })
                .collect();

            if !matching.is_empty() {
                println!(
                    "\n  Board: {} [{}]  List: {} [{}]",
                    board.name, board.id, list.name, list.id
                );
                for card in &matching {
                    total += 1;
                    print_card_line(total, card, &board.labels);
                }
            }
        }
    }

    if total == 0 {
        println!("\nNo cards found.");
    } else {
        println!("\nFound {total} card(s).");
    }

    Ok(())
}

pub(crate) fn card_matches_regex(card: &Card, re: &Regex, board_labels: &[Label]) -> bool {
    re.is_match(&card.title)
        || re.is_match(&card.description)
        || card.checklist.iter().any(|item| re.is_match(&item.text))
        || card.label_ids.iter().any(|lid| {
            board_labels
                .iter()
                .any(|l| l.id == *lid && re.is_match(&l.name))
        })
}
