use std::collections::HashMap;

use anyhow::{bail, Context};
use chrono::{NaiveDate, Utc};
use regex::Regex;

use crate::model::board::BoardMeta;
use crate::model::card::{Card, ChecklistItem};
use crate::model::ids::ShortId;
use crate::model::label::{Label, LabelColor};
use crate::model::list::CardList;
use crate::storage::{board_store, card_store, list_store};

const HELP: &str = "\
tct - Terminal Card Tracker

A keyboard-driven TUI Kanban board. Run without arguments to open the TUI.

USAGE:
    tct                              Open the TUI board selector
    tct --board <name>               Open TUI directly on a board (partial, case-insensitive)
    tct --help, -h                   Show this help message
    tct <COMMAND> [ARGS] [FLAGS]     Run a CLI command (no TUI)

FLAGS (apply to any command):
    --by-id                          Match all identifier arguments by exact ID instead of name

COMMANDS:
  boards                              List active boards
  boards --archived                   List archived boards
  boards --create <name>              Create a new board
  boards --archive <name>             Archive a board
  boards --restore <name>             Restore an archived board
  boards --delete <name>              Permanently delete an archived board

  lists <board>                       List all lists on a board
  lists <board> --create <name>       Create a list
  lists <board> --rename <list> <name>  Rename a list
  lists <board> --delete <list>       Delete a list and all its cards

  cards <board>                       List active cards grouped by list
  cards <board> --archived            List archived cards
  cards <board> --list <list>         List active cards in a specific list
  cards <board> --show <card>         Show full card detail
  cards <board> --create <list> <title>  Create a card in a list
  cards <board> --edit <card>         Edit card fields
    --title <text>                      New title
    --description <text>                New description
    --due <YYYY-MM-DD|none>             Set or clear due date
  cards <board> --archive <card>      Archive a card
  cards <board> --restore <card>      Restore an archived card to the first list
  cards <board> --delete <card>       Permanently delete an archived card

  checklist <board> <card>            Show checklist
  checklist <board> <card> --add <text>     Add a checklist item
  checklist <board> <card> --toggle <n>     Toggle item n (1-based index)
  checklist <board> <card> --delete <n>     Delete item n (1-based index)

  labels <board>                      List all labels
  labels <board> --create <name>      Create a label
  labels <board> --delete <label>     Delete a label (removes from all cards)
  labels <board> --assign <card> <label>   Assign a label to a card
  labels <board> --remove <card> <label>   Remove a label from a card

  search <query>                      Search cards across all boards (case-insensitive substring)
  search <query> --board <name>       Limit search to boards matching name (repeatable)
  search <query> --list <name>        Limit search to lists matching name
  search <query> --regex              Treat query as a regular expression
  search <query> --archived           Include archived cards in results

IDs are shown in listings as [xxxxxxxx]. Pass --by-id to match by ID instead of name.
Multiple name matches or a missing ID result in an error.

STORAGE:
    By default data is stored in ~/.tct/. If a .tct/ directory exists in the current
    working directory or any of its parents, that directory is used instead.
    Override with the TCT_DATA_DIR environment variable.
";

pub fn print_help() {
    print!("{HELP}");
}

/// Resolve --board <name> flag from args, return the matching board's ID.
pub fn resolve_board_flag(args: &[String]) -> anyhow::Result<Option<String>> {
    let mut i = 0;
    while i < args.len() {
        if args[i] == "--board" {
            if i + 1 < args.len() {
                let partial = &args[i + 1];
                let board = find_board(partial, false)?;
                return Ok(Some(board.id));
            } else {
                bail!("--board requires a board name argument");
            }
        }
        i += 1;
    }
    Ok(None)
}

pub fn run(args: &[String]) -> anyhow::Result<()> {
    board_store::ensure_base_dirs()?;
    let by_id = args.iter().any(|a| a == "--by-id");
    let args: Vec<String> = args.iter().filter(|a| *a != "--by-id").cloned().collect();
    let sub = args[0].as_str();
    let rest = &args[1..];
    match sub {
        "boards" => cmd_boards(rest, by_id),
        "lists" => cmd_lists(rest, by_id),
        "cards" => cmd_cards(rest, by_id),
        "checklist" => cmd_checklist(rest, by_id),
        "labels" => cmd_labels(rest, by_id),
        "search" => cmd_search(rest),
        other => bail!("Unknown command '{other}'. Run 'tct --help' for usage."),
    }
}

// ── Boards ────────────────────────────────────────────────────────────────────

fn cmd_boards(args: &[String], by_id: bool) -> anyhow::Result<()> {
    if has_flag(args, "--archived") {
        let boards = board_store::list_archived_boards()?;
        if boards.is_empty() {
            println!("No archived boards.");
        } else {
            println!("Archived boards ({}):", boards.len());
            for board in &boards {
                println!("  [{}]  {}", board.id, board.name);
            }
        }
    } else if let Some(name) = flag_value(args, "--create") {
        let existing_boards = board_store::list_boards()?;
        let existing_colors: Vec<_> = existing_boards.iter().map(|b| b.accent_color).collect();
        let mut meta = BoardMeta::new(name.to_string());
        meta.accent_color = LabelColor::generate_pastel(&existing_colors);
        board_store::save_board(&meta)?;
        board_store::append_to_order(&meta.id)?;
        println!("Created board '{}'.", meta.name);
    } else if let Some(partial) = flag_value(args, "--archive") {
        let mut board = find_board(partial, by_id)?;
        let name = board.name.clone();
        board.archived = true;
        board_store::save_board(&board)?;
        board_store::remove_from_order(&board.id)?;
        println!("Archived board '{name}'.");
    } else if let Some(partial) = flag_value(args, "--restore") {
        let mut board = find_archived_board(partial, by_id)?;
        let name = board.name.clone();
        board.archived = false;
        board_store::save_board(&board)?;
        board_store::append_to_order(&board.id)?;
        println!("Restored board '{name}'.");
    } else if let Some(partial) = flag_value(args, "--delete") {
        let board = find_archived_board(partial, by_id)?;
        let name = board.name.clone();
        board_store::delete_board(&board.id)?;
        println!("Permanently deleted board '{name}'.");
    } else {
        // Default: list active boards
        let boards = board_store::list_boards()?;
        if boards.is_empty() {
            println!("No active boards.");
        } else {
            println!("Active boards ({}):", boards.len());
            for board in &boards {
                let (lists, total_cards) = board_summary_counts(&board.id);
                println!(
                    "  [{}]  {:<30} {} lists, {} active cards",
                    board.id, board.name, lists, total_cards
                );
            }
        }
    }
    Ok(())
}

// ── Lists ─────────────────────────────────────────────────────────────────────

fn cmd_lists(args: &[String], by_id: bool) -> anyhow::Result<()> {
    let board_partial = args
        .first()
        .ok_or_else(|| anyhow::anyhow!("Usage: tct lists <board> [--create|--rename|--delete ...]"))?
        .as_str();

    if let Some(name) = flag_value(args, "--create") {
        let board = find_board(board_partial, by_id)?;
        let list = CardList::new(name.to_string());
        list_store::save_list(&board.id, &list)?;
        let mut meta = board_store::load_board(&board.id)?;
        meta.list_order.push(list.id.clone());
        board_store::save_board(&meta)?;
        println!("Created list '{}' on board '{}'.", list.name, board.name);
    } else if let Some((list_partial, new_name)) = flag_values2(args, "--rename") {
        let board = find_board(board_partial, by_id)?;
        let lists = list_store::load_all_lists(&board.id, &board.list_order)?;
        let mut list = find_list(&lists, list_partial, by_id)?.clone();
        let old_name = list.name.clone();
        list.name = new_name.to_string();
        list_store::save_list(&board.id, &list)?;
        println!("Renamed list '{old_name}' to '{new_name}'.");
    } else if let Some(list_partial) = flag_value(args, "--delete") {
        let board = find_board(board_partial, by_id)?;
        let lists = list_store::load_all_lists(&board.id, &board.list_order)?;
        let list = find_list(&lists, list_partial, by_id)?.clone();
        let name = list.name.clone();
        for card_id in &list.card_ids {
            let _ = card_store::delete_card(&board.id, card_id);
        }
        list_store::delete_list_file(&board.id, &list.id)?;
        let mut meta = board_store::load_board(&board.id)?;
        meta.list_order.retain(|id| id != &list.id);
        board_store::save_board(&meta)?;
        println!("Deleted list '{name}' and all its cards.");
    } else {
        // Default: list all lists on board
        let board = find_board(board_partial, by_id)?;
        let lists = list_store::load_all_lists(&board.id, &board.list_order)?;
        let all_cards = load_all_cards(&board.id, &lists);
        println!("Board: {} [{}]", board.name, board.id);
        if lists.is_empty() {
            println!("  (no lists)");
        } else {
            for (i, list) in lists.iter().enumerate() {
                let count = count_active(&list.card_ids, &all_cards);
                println!("  {}. [{}]  {}  ({} active cards)", i + 1, list.id, list.name, count);
            }
        }
    }
    Ok(())
}

// ── Cards ─────────────────────────────────────────────────────────────────────

fn cmd_cards(args: &[String], by_id: bool) -> anyhow::Result<()> {
    let board_partial = args
        .first()
        .ok_or_else(|| anyhow::anyhow!("Usage: tct cards <board> [--show|--create|--edit|--archive|--restore|--delete|--list|--archived ...]"))?
        .as_str();

    if has_flag(args, "--archived") {
        let board = find_board(board_partial, by_id)?;
        let archived = card_store::list_archived_cards(&board.id);
        println!("Board: {} [{}]  (archived cards: {})", board.name, board.id, archived.len());
        for (i, card) in archived.iter().enumerate() {
            let date = card.updated_at.format("%Y-%m-%d");
            println!("  {}. [{}]  {}  (archived: {})", i + 1, card.id, card.title, date);
        }
    } else if let Some(card_partial) = flag_value(args, "--show") {
        let board = find_board(board_partial, by_id)?;
        let lists = list_store::load_all_lists(&board.id, &board.list_order)?;
        let all_cards = load_all_cards(&board.id, &lists);
        let (list, card) = find_card_in_lists(&lists, &all_cards, card_partial, false, by_id)?;
        print_card_detail(&card, &board, &list);
    } else if let Some(list_partial) = flag_value(args, "--list") {
        let board = find_board(board_partial, by_id)?;
        let lists = list_store::load_all_lists(&board.id, &board.list_order)?;
        let list = find_list(&lists, list_partial, by_id)?.clone();
        let all_cards = load_all_cards(&board.id, &lists);
        let active: Vec<_> = list
            .card_ids
            .iter()
            .filter_map(|id| all_cards.get(id))
            .filter(|c| !c.archived)
            .collect();
        println!("Board: {} [{}]  List: {} [{}]  ({} active cards)", board.name, board.id, list.name, list.id, active.len());
        for (i, card) in active.iter().enumerate() {
            print_card_line(i + 1, card, &board.labels);
        }
    } else if let Some((list_partial, title)) = flag_values2(args, "--create") {
        let board = find_board(board_partial, by_id)?;
        let lists = list_store::load_all_lists(&board.id, &board.list_order)?;
        let mut list = find_list(&lists, list_partial, by_id)?.clone();
        let card = Card::new(title.to_string());
        card_store::save_card(&board.id, &card)?;
        list.card_ids.push(card.id.clone());
        list_store::save_list(&board.id, &list)?;
        println!("Created card '{}' in list '{}' on board '{}'.", card.title, list.name, board.name);
    } else if let Some(card_partial) = flag_value(args, "--edit") {
        let board = find_board(board_partial, by_id)?;
        let lists = list_store::load_all_lists(&board.id, &board.list_order)?;
        let all_cards = load_all_cards(&board.id, &lists);
        let (_, mut card) = find_card_in_lists(&lists, &all_cards, card_partial, false, by_id)?;

        let new_title = flag_value(args, "--title");
        let new_desc = flag_value(args, "--description");
        let new_due = flag_value(args, "--due");

        if new_title.is_none() && new_desc.is_none() && new_due.is_none() {
            bail!("cards --edit requires at least one of: --title, --description, --due");
        }
        if let Some(t) = new_title {
            card.title = t.to_string();
        }
        if let Some(d) = new_desc {
            card.description = d.to_string();
        }
        if let Some(due_str) = new_due {
            if due_str == "none" {
                card.due_date = None;
            } else {
                card.due_date = Some(
                    NaiveDate::parse_from_str(due_str, "%Y-%m-%d")
                        .context("Invalid date format. Use YYYY-MM-DD or 'none'.")?,
                );
            }
        }
        card.touch();
        card_store::save_card(&board.id, &card)?;
        println!("Updated card '{}'.", card.title);
    } else if let Some(card_partial) = flag_value(args, "--archive") {
        let board = find_board(board_partial, by_id)?;
        let lists = list_store::load_all_lists(&board.id, &board.list_order)?;
        let all_cards = load_all_cards(&board.id, &lists);
        let (mut list, mut card) = find_card_in_lists(&lists, &all_cards, card_partial, false, by_id)?;
        let title = card.title.clone();
        card.archived = true;
        card.touch();
        card_store::save_card(&board.id, &card)?;
        list.card_ids.retain(|id| id != &card.id);
        list_store::save_list(&board.id, &list)?;
        println!("Archived card '{title}'.");
    } else if let Some(card_partial) = flag_value(args, "--restore") {
        let board = find_board(board_partial, by_id)?;
        let mut card = find_archived_card(&board.id, card_partial, by_id)?;
        let title = card.title.clone();
        card.archived = false;
        card.touch();
        card_store::save_card(&board.id, &card)?;
        let meta = board_store::load_board(&board.id)?;
        if let Some(first_list_id) = meta.list_order.first() {
            if let Ok(mut list) = list_store::load_list(&board.id, first_list_id) {
                list.card_ids.push(card.id.clone());
                list_store::save_list(&board.id, &list)?;
            }
        }
        println!("Restored card '{title}'.");
    } else if let Some(card_partial) = flag_value(args, "--delete") {
        let board = find_board(board_partial, by_id)?;
        let card = find_archived_card(&board.id, card_partial, by_id)?;
        let title = card.title.clone();
        card_store::delete_card(&board.id, &card.id)?;
        println!("Permanently deleted card '{title}'.");
    } else {
        // Default: list all active cards grouped by list
        let board = find_board(board_partial, by_id)?;
        let lists = list_store::load_all_lists(&board.id, &board.list_order)?;
        let all_cards = load_all_cards(&board.id, &lists);
        println!("Board: {} [{}]", board.name, board.id);
        for list in &lists {
            let active: Vec<_> = list
                .card_ids
                .iter()
                .filter_map(|id| all_cards.get(id))
                .filter(|c| !c.archived)
                .collect();
            println!("  List: {} [{}]  ({} active cards)", list.name, list.id, active.len());
            for (i, card) in active.iter().enumerate() {
                print_card_line(i + 1, card, &board.labels);
            }
        }
    }
    Ok(())
}

// ── Checklist ─────────────────────────────────────────────────────────────────

fn cmd_checklist(args: &[String], by_id: bool) -> anyhow::Result<()> {
    let board_partial = args
        .first()
        .ok_or_else(|| anyhow::anyhow!("Usage: tct checklist <board> <card> [--add|--toggle|--delete ...]"))?
        .as_str();
    let card_partial = args
        .get(1)
        .ok_or_else(|| anyhow::anyhow!("Missing required argument: card name"))?
        .as_str();

    let board = find_board(board_partial, by_id)?;
    let lists = list_store::load_all_lists(&board.id, &board.list_order)?;
    let all_cards = load_all_cards(&board.id, &lists);

    if let Some(text) = flag_value(args, "--add") {
        let (_, mut card) = find_card_in_lists(&lists, &all_cards, card_partial, false, by_id)?;
        card.checklist.push(ChecklistItem { text: text.to_string(), completed: false });
        card.touch();
        card_store::save_card(&board.id, &card)?;
        println!("Added checklist item '{}' to card '{}'.", text, card.title);
    } else if let Some(n_str) = flag_value(args, "--toggle") {
        let n: usize = n_str.parse().context("Item index must be a positive integer")?;
        let (_, mut card) = find_card_in_lists(&lists, &all_cards, card_partial, false, by_id)?;
        let idx = n.checked_sub(1).ok_or_else(|| anyhow::anyhow!("Index must be >= 1"))?;
        let total = card.checklist.len();
        let item = card
            .checklist
            .get_mut(idx)
            .ok_or_else(|| anyhow::anyhow!("Index {n} out of range (card has {total} items)"))?;
        item.completed = !item.completed;
        let state = if item.completed { "done" } else { "undone" };
        let text = item.text.clone();
        card.touch();
        card_store::save_card(&board.id, &card)?;
        println!("Toggled item {n} ('{text}') → {state}.");
    } else if let Some(n_str) = flag_value(args, "--delete") {
        let n: usize = n_str.parse().context("Item index must be a positive integer")?;
        let (_, mut card) = find_card_in_lists(&lists, &all_cards, card_partial, false, by_id)?;
        let idx = n.checked_sub(1).ok_or_else(|| anyhow::anyhow!("Index must be >= 1"))?;
        if idx >= card.checklist.len() {
            bail!("Index {n} out of range (card has {} items)", card.checklist.len());
        }
        let removed = card.checklist.remove(idx);
        card.touch();
        card_store::save_card(&board.id, &card)?;
        println!("Deleted checklist item '{}'.", removed.text);
    } else {
        // Default: show checklist
        let (_, card) = find_card_in_lists(&lists, &all_cards, card_partial, false, by_id)?;
        println!("Card: {}  Checklist: {}", card.title, fmt_progress(&card));
        if card.checklist.is_empty() {
            println!("  (no items)");
        } else {
            for (i, item) in card.checklist.iter().enumerate() {
                let mark = if item.completed { "x" } else { " " };
                println!("  {}. [{}] {}", i + 1, mark, item.text);
            }
        }
    }
    Ok(())
}

// ── Labels ────────────────────────────────────────────────────────────────────

fn cmd_labels(args: &[String], by_id: bool) -> anyhow::Result<()> {
    let board_partial = args
        .first()
        .ok_or_else(|| anyhow::anyhow!("Usage: tct labels <board> [--create|--delete|--assign|--remove ...]"))?
        .as_str();

    if let Some(name) = flag_value(args, "--create") {
        let mut board = find_board(board_partial, by_id)?;
        let existing: Vec<_> = board.labels.iter().map(|l| l.color).collect();
        let color = LabelColor::generate_pastel(&existing);
        let label = Label::new(name.to_string(), color);
        board.labels.push(label);
        board_store::save_board(&board)?;
        println!("Created label '{name}' on board '{}'.", board.name);
    } else if let Some(label_partial) = flag_value(args, "--delete") {
        let mut board = find_board(board_partial, by_id)?;
        let label = find_label(&board.labels, label_partial, by_id)?.clone();
        let label_name = label.name.clone();
        board.labels.retain(|l| l.id != label.id);
        let lists = list_store::load_all_lists(&board.id, &board.list_order)?;
        for list in &lists {
            for card_id in &list.card_ids {
                if let Ok(mut card) = card_store::load_card(&board.id, card_id) {
                    if card.label_ids.contains(&label.id) {
                        card.label_ids.retain(|id| id != &label.id);
                        card.touch();
                        let _ = card_store::save_card(&board.id, &card);
                    }
                }
            }
        }
        board_store::save_board(&board)?;
        println!("Deleted label '{label_name}' from board '{}'.", board.name);
    } else if let Some((card_partial, label_partial)) = flag_values2(args, "--assign") {
        let board = find_board(board_partial, by_id)?;
        let label = find_label(&board.labels, label_partial, by_id)?.clone();
        let lists = list_store::load_all_lists(&board.id, &board.list_order)?;
        let all_cards = load_all_cards(&board.id, &lists);
        let (_, mut card) = find_card_in_lists(&lists, &all_cards, card_partial, false, by_id)?;
        if !card.label_ids.contains(&label.id) {
            card.label_ids.push(label.id.clone());
            card.touch();
            card_store::save_card(&board.id, &card)?;
            println!("Assigned label '{}' to card '{}'.", label.name, card.title);
        } else {
            println!("Label '{}' already assigned to card '{}'.", label.name, card.title);
        }
    } else if let Some((card_partial, label_partial)) = flag_values2(args, "--remove") {
        let board = find_board(board_partial, by_id)?;
        let label = find_label(&board.labels, label_partial, by_id)?.clone();
        let lists = list_store::load_all_lists(&board.id, &board.list_order)?;
        let all_cards = load_all_cards(&board.id, &lists);
        let (_, mut card) = find_card_in_lists(&lists, &all_cards, card_partial, false, by_id)?;
        if card.label_ids.contains(&label.id) {
            card.label_ids.retain(|id| id != &label.id);
            card.touch();
            card_store::save_card(&board.id, &card)?;
            println!("Removed label '{}' from card '{}'.", label.name, card.title);
        } else {
            println!("Label '{}' is not assigned to card '{}'.", label.name, card.title);
        }
    } else {
        // Default: list labels
        let board = find_board(board_partial, by_id)?;
        println!("Board: {} [{}]  Labels ({}):", board.name, board.id, board.labels.len());
        if board.labels.is_empty() {
            println!("  (no labels)");
        } else {
            for label in &board.labels {
                println!("  [{}]  {}  ({})", label.id, label.name, label_color_name(&label.color));
            }
        }
    }
    Ok(())
}

// ── Search ────────────────────────────────────────────────────────────────────

fn cmd_search(args: &[String]) -> anyhow::Result<()> {
    let query = args
        .first()
        .ok_or_else(|| anyhow::anyhow!("Usage: tct search <query> [--board <name>] [--list <name>] [--regex] [--archived]"))?
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
                board_filters.iter().any(|f| name_lower.contains(&f.to_lowercase()))
            })
            .collect()
    };

    if boards_to_search.is_empty() && !board_filters.is_empty() {
        bail!("No boards match the --board filter.");
    }

    let mode = if use_regex { "regex" } else { "substring" };
    let board_count = boards_to_search.len();
    println!(
        "Searching {} board(s) for {:?} ({mode}):",
        board_count, query
    );

    let mut total = 0usize;

    for board in &boards_to_search {
        let lists = list_store::load_all_lists(&board.id, &board.list_order)?;
        let all_cards = load_all_cards(&board.id, &lists);

        for list in &lists {
            if let Some(lf) = list_filter {
                if !list.name.to_lowercase().contains(&lf.to_lowercase()) {
                    continue;
                }
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
                        None => card_matches_query(card, query, &board.labels),
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
        println!("\nFound {} card(s).", total);
    }

    Ok(())
}

#[cfg(test)]
pub(crate) fn card_matches_query_pub(card: &Card, query: &str, board_labels: &[Label]) -> bool {
    card_matches_query(card, query, board_labels)
}

#[cfg(test)]
pub(crate) fn card_matches_regex_pub(card: &Card, re: &Regex, board_labels: &[Label]) -> bool {
    card_matches_regex(card, re, board_labels)
}

fn card_matches_query(card: &Card, query: &str, board_labels: &[Label]) -> bool {
    let q = query.to_lowercase();
    card.title.to_lowercase().contains(&q)
        || card.description.to_lowercase().contains(&q)
        || card.checklist.iter().any(|item| item.text.to_lowercase().contains(&q))
        || card.label_ids.iter().any(|lid| {
            board_labels.iter().any(|l| l.id == *lid && l.name.to_lowercase().contains(&q))
        })
}

fn card_matches_regex(card: &Card, re: &Regex, board_labels: &[Label]) -> bool {
    re.is_match(&card.title)
        || re.is_match(&card.description)
        || card.checklist.iter().any(|item| re.is_match(&item.text))
        || card.label_ids.iter().any(|lid| {
            board_labels.iter().any(|l| l.id == *lid && re.is_match(&l.name))
        })
}

// ── Lookup helpers ────────────────────────────────────────────────────────────

fn find_board(partial: &str, by_id: bool) -> anyhow::Result<BoardMeta> {
    let boards = board_store::list_boards()?;
    if by_id {
        boards
            .into_iter()
            .find(|b| b.id == partial)
            .ok_or_else(|| anyhow::anyhow!("No active board with ID '{partial}'."))
    } else {
        let q = partial.to_lowercase();
        let matches: Vec<_> = boards.into_iter().filter(|b| b.name.to_lowercase().contains(&q)).collect();
        match matches.len() {
            0 => bail!("No active board matches '{partial}'."),
            1 => Ok(matches.into_iter().next().unwrap()),
            _ => {
                let names: Vec<_> = matches.iter().map(|b| format!("{} [{}]", b.name, b.id)).collect();
                bail!("Multiple boards match '{partial}': {}.", names.join(", "))
            }
        }
    }
}

fn find_archived_board(partial: &str, by_id: bool) -> anyhow::Result<BoardMeta> {
    let boards = board_store::list_archived_boards()?;
    if by_id {
        boards
            .into_iter()
            .find(|b| b.id == partial)
            .ok_or_else(|| anyhow::anyhow!("No archived board with ID '{partial}'."))
    } else {
        let q = partial.to_lowercase();
        let matches: Vec<_> = boards.into_iter().filter(|b| b.name.to_lowercase().contains(&q)).collect();
        match matches.len() {
            0 => bail!("No archived board matches '{partial}'."),
            1 => Ok(matches.into_iter().next().unwrap()),
            _ => {
                let names: Vec<_> = matches.iter().map(|b| format!("{} [{}]", b.name, b.id)).collect();
                bail!("Multiple archived boards match '{partial}': {}.", names.join(", "))
            }
        }
    }
}

fn find_list<'a>(lists: &'a [CardList], partial: &str, by_id: bool) -> anyhow::Result<&'a CardList> {
    if by_id {
        lists
            .iter()
            .find(|l| l.id == partial)
            .ok_or_else(|| anyhow::anyhow!("No list with ID '{partial}'."))
    } else {
        let q = partial.to_lowercase();
        let matches: Vec<_> = lists.iter().filter(|l| l.name.to_lowercase().contains(&q)).collect();
        match matches.len() {
            0 => bail!("No list matches '{partial}'."),
            1 => Ok(matches.into_iter().next().unwrap()),
            _ => {
                let names: Vec<_> = matches.iter().map(|l| format!("{} [{}]", l.name, l.id)).collect();
                bail!("Multiple lists match '{partial}': {}.", names.join(", "))
            }
        }
    }
}

fn find_card_in_lists(
    lists: &[CardList],
    all_cards: &HashMap<ShortId, Card>,
    partial: &str,
    include_archived: bool,
    by_id: bool,
) -> anyhow::Result<(CardList, Card)> {
    let mut matches: Vec<(CardList, Card)> = Vec::new();
    if by_id {
        for list in lists {
            for card_id in &list.card_ids {
                if let Some(card) = all_cards.get(card_id) {
                    if (include_archived || !card.archived) && card.id == partial {
                        matches.push((list.clone(), card.clone()));
                    }
                }
            }
        }
        match matches.len() {
            0 => bail!("No card with ID '{partial}'."),
            _ => Ok(matches.into_iter().next().unwrap()),
        }
    } else {
        let q = partial.to_lowercase();
        for list in lists {
            for card_id in &list.card_ids {
                if let Some(card) = all_cards.get(card_id) {
                    if (include_archived || !card.archived) && card.title.to_lowercase().contains(&q) {
                        matches.push((list.clone(), card.clone()));
                    }
                }
            }
        }
        match matches.len() {
            0 => bail!("No card matches '{partial}'."),
            1 => Ok(matches.into_iter().next().unwrap()),
            _ => {
                let names: Vec<_> = matches.iter().map(|(_, c)| format!("{} [{}]", c.title, c.id)).collect();
                bail!("Multiple cards match '{partial}': {}.", names.join(", "))
            }
        }
    }
}

fn find_archived_card(board_id: &str, partial: &str, by_id: bool) -> anyhow::Result<Card> {
    let cards = card_store::list_archived_cards(board_id);
    if by_id {
        cards
            .into_iter()
            .find(|c| c.id == partial)
            .ok_or_else(|| anyhow::anyhow!("No archived card with ID '{partial}'."))
    } else {
        let q = partial.to_lowercase();
        let matches: Vec<_> = cards.into_iter().filter(|c| c.title.to_lowercase().contains(&q)).collect();
        match matches.len() {
            0 => bail!("No archived card matches '{partial}'."),
            1 => Ok(matches.into_iter().next().unwrap()),
            _ => {
                let names: Vec<_> = matches.iter().map(|c| format!("{} [{}]", c.title, c.id)).collect();
                bail!("Multiple archived cards match '{partial}': {}.", names.join(", "))
            }
        }
    }
}

fn find_label<'a>(labels: &'a [Label], partial: &str, by_id: bool) -> anyhow::Result<&'a Label> {
    if by_id {
        labels
            .iter()
            .find(|l| l.id == partial)
            .ok_or_else(|| anyhow::anyhow!("No label with ID '{partial}'."))
    } else {
        let q = partial.to_lowercase();
        let matches: Vec<_> = labels.iter().filter(|l| l.name.to_lowercase().contains(&q)).collect();
        match matches.len() {
            0 => bail!("No label matches '{partial}'."),
            1 => Ok(matches.into_iter().next().unwrap()),
            _ => {
                let names: Vec<_> = matches.iter().map(|l| format!("{} [{}]", l.name, l.id)).collect();
                bail!("Multiple labels match '{partial}': {}.", names.join(", "))
            }
        }
    }
}

// ── Data loading helpers ──────────────────────────────────────────────────────

fn load_all_cards(board_id: &str, lists: &[CardList]) -> HashMap<ShortId, Card> {
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

fn count_active(card_ids: &[ShortId], cards: &HashMap<ShortId, Card>) -> usize {
    card_ids.iter().filter(|id| cards.get(*id).map(|c| !c.archived).unwrap_or(false)).count()
}

fn board_summary_counts(board_id: &str) -> (usize, usize) {
    let meta = match board_store::load_board(board_id) {
        Ok(m) => m,
        Err(_) => return (0, 0),
    };
    let lists = match list_store::load_all_lists(board_id, &meta.list_order) {
        Ok(l) => l,
        Err(_) => return (meta.list_order.len(), 0),
    };
    let cards = load_all_cards(board_id, &lists);
    let total = lists.iter().map(|l| count_active(&l.card_ids, &cards)).sum();
    (lists.len(), total)
}

fn has_flag(args: &[String], flag: &str) -> bool {
    args.iter().any(|a| a == flag)
}

fn flag_value<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
    args.windows(2)
        .find(|w| w[0] == flag)
        .map(|w| w[1].as_str())
}

fn flag_values2<'a>(args: &'a [String], flag: &str) -> Option<(&'a str, &'a str)> {
    args.windows(3)
        .find(|w| w[0] == flag)
        .map(|w| (w[1].as_str(), w[2].as_str()))
}

fn flag_values_all<'a>(args: &'a [String], flag: &str) -> Vec<&'a str> {
    args.windows(2)
        .filter(|w| w[0] == flag)
        .map(|w| w[1].as_str())
        .collect()
}

// ── Output formatting ─────────────────────────────────────────────────────────

fn print_card_line(idx: usize, card: &Card, labels: &[Label]) {
    let label_names: Vec<_> = card.resolved_labels(labels).iter().map(|l| l.name.as_str()).collect();
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
                format!("  due: {} (OVERDUE)", d)
            } else {
                format!("  due: {d}")
            }
        }
    };
    let checklist_str = match card.checklist_progress() {
        None => String::new(),
        Some((done, total)) => format!("  checklist: {done}/{total}"),
    };
    println!("    {idx}. [{}]  {}{label_str}{due_str}{checklist_str}", card.id, card.title);
}

fn print_card_detail(card: &Card, board: &BoardMeta, list: &CardList) {
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

    match card.due_date {
        None => {}
        Some(d) => {
            let today = Utc::now().date_naive();
            if d < today {
                println!("Due:         {} (OVERDUE)", d);
            } else {
                println!("Due:         {d}");
            }
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

fn fmt_progress(card: &Card) -> String {
    match card.checklist_progress() {
        None => "no items".to_string(),
        Some((done, total)) => format!("{done}/{total}"),
    }
}

fn label_color_name(color: &LabelColor) -> &'static str {
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
