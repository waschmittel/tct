//! Identifier resolution for CLI args.
//!
//! Names are matched case-insensitively as a substring. When `--by-id` is
//! present in the command, identifier args are matched as exact IDs
//! instead. Multiple name matches produce an error listing all candidates
//! so the user can re-run with `--by-id` and the unambiguous ID.

use std::collections::HashMap;

use anyhow::bail;

use crate::model::board::BoardMeta;
use crate::model::card::Card;
use crate::model::ids::ShortId;
use crate::model::label::Label;
use crate::model::list::CardList;
use crate::storage::{board_store, card_store};

/// Find one item from a borrowed slice by exact ID or by case-insensitive name substring.
pub(super) fn resolve_one_ref<'a, T>(
    items: &'a [T],
    partial: &str,
    by_id: bool,
    id_of: impl Fn(&T) -> &str,
    name_of: impl Fn(&T) -> &str,
    entity: &str,
) -> anyhow::Result<&'a T> {
    if by_id {
        return items
            .iter()
            .find(|x| id_of(x) == partial)
            .ok_or_else(|| anyhow::anyhow!("No {entity} with ID '{partial}'."));
    }
    let q = partial.to_lowercase();
    let matches: Vec<&T> = items
        .iter()
        .filter(|x| name_of(x).to_lowercase().contains(&q))
        .collect();
    match matches.len() {
        0 => bail!("No {entity} matches '{partial}'."),
        1 => Ok(matches.into_iter().next().unwrap()),
        _ => {
            let descs: Vec<_> = matches
                .iter()
                .map(|x| format!("{} [{}]", name_of(x), id_of(x)))
                .collect();
            bail!("Multiple {}s match '{partial}': {}.", entity, descs.join(", "))
        }
    }
}

/// Find one item from an owned Vec by exact ID or by case-insensitive name substring.
pub(super) fn resolve_one_owned<T>(
    items: Vec<T>,
    partial: &str,
    by_id: bool,
    id_of: impl Fn(&T) -> &str,
    name_of: impl Fn(&T) -> &str,
    entity: &str,
) -> anyhow::Result<T> {
    if by_id {
        return items
            .into_iter()
            .find(|x| id_of(x) == partial)
            .ok_or_else(|| anyhow::anyhow!("No {entity} with ID '{partial}'."));
    }
    let q = partial.to_lowercase();
    let matches: Vec<T> = items
        .into_iter()
        .filter(|x| name_of(x).to_lowercase().contains(&q))
        .collect();
    match matches.len() {
        0 => bail!("No {entity} matches '{partial}'."),
        1 => Ok(matches.into_iter().next().unwrap()),
        _ => {
            let descs: Vec<_> = matches
                .iter()
                .map(|x| format!("{} [{}]", name_of(x), id_of(x)))
                .collect();
            bail!("Multiple {}s match '{partial}': {}.", entity, descs.join(", "))
        }
    }
}

pub(super) fn find_board(partial: &str, by_id: bool) -> anyhow::Result<BoardMeta> {
    resolve_one_owned(
        board_store::list_boards()?,
        partial,
        by_id,
        |b| &b.id,
        |b| &b.name,
        "active board",
    )
}

pub(super) fn find_archived_board(partial: &str, by_id: bool) -> anyhow::Result<BoardMeta> {
    resolve_one_owned(
        board_store::list_archived_boards()?,
        partial,
        by_id,
        |b| &b.id,
        |b| &b.name,
        "archived board",
    )
}

pub(super) fn find_list<'a>(
    lists: &'a [CardList],
    partial: &str,
    by_id: bool,
) -> anyhow::Result<&'a CardList> {
    resolve_one_ref(lists, partial, by_id, |l| &l.id, |l| &l.name, "list")
}

pub(super) fn find_card_in_lists(
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
                if let Some(card) = all_cards.get(card_id)
                    && (include_archived || !card.archived)
                    && card.id == partial
                {
                    matches.push((list.clone(), card.clone()));
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
                if let Some(card) = all_cards.get(card_id)
                    && (include_archived || !card.archived)
                    && card.title.to_lowercase().contains(&q)
                {
                    matches.push((list.clone(), card.clone()));
                }
            }
        }
        match matches.len() {
            0 => bail!("No card matches '{partial}'."),
            1 => Ok(matches.into_iter().next().unwrap()),
            _ => {
                let names: Vec<_> = matches
                    .iter()
                    .map(|(_, c)| format!("{} [{}]", c.title, c.id))
                    .collect();
                bail!("Multiple cards match '{partial}': {}.", names.join(", "))
            }
        }
    }
}

pub(super) fn find_archived_card(
    board_id: &str,
    partial: &str,
    by_id: bool,
) -> anyhow::Result<Card> {
    resolve_one_owned(
        card_store::list_archived_cards(board_id)?,
        partial,
        by_id,
        |c| &c.id,
        |c| &c.title,
        "archived card",
    )
}

pub(super) fn find_label<'a>(
    labels: &'a [Label],
    partial: &str,
    by_id: bool,
) -> anyhow::Result<&'a Label> {
    resolve_one_ref(labels, partial, by_id, |l| &l.id, |l| &l.name, "label")
}
