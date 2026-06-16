use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::board::BoardMeta;
use super::card::Card;
use super::ids::ShortId;

/// In-memory view of a List: its metadata plus the ordered ids of the Cards
/// that belong to it (archived Cards included — visibility is decided by
/// `LoadedBoard::visible_cards`, not by membership). `card_ids` is derived from
/// each Card's `list_id` + `position` at load time; it is never persisted as a
/// `CardList`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CardList {
    pub id: ShortId,
    pub name: String,
    pub card_ids: Vec<ShortId>,
    #[serde(default)]
    pub archived: bool,
}

impl CardList {
    #[cfg(test)]
    pub fn new(name: String) -> Self {
        Self {
            id: super::ids::new_id(),
            name,
            card_ids: Vec::new(),
            archived: false,
        }
    }
}

/// Cards belonging to `list_id`, sorted ascending by `position` (ties broken by
/// `created_at` then `id` for stable, deterministic ordering).
pub fn ordered_card_ids(list_id: &str, cards: &HashMap<ShortId, Card>) -> Vec<ShortId> {
    let mut members: Vec<&Card> = cards.values().filter(|c| c.list_id == list_id).collect();
    members.sort_by(|a, b| {
        a.position
            .partial_cmp(&b.position)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.created_at.cmp(&b.created_at))
            .then(a.id.cmp(&b.id))
    });
    members.into_iter().map(|c| c.id.clone()).collect()
}

/// Build the in-memory `CardList`s for a board from its `ListMeta` definitions
/// and the full Card map. When `archived` is false, returns the active Lists in
/// board order; when true, returns the archived Lists.
pub fn build_lists(meta: &BoardMeta, cards: &HashMap<ShortId, Card>, archived: bool) -> Vec<CardList> {
    meta.lists
        .iter()
        .filter(|lm| lm.archived == archived)
        .map(|lm| CardList {
            id: lm.id.clone(),
            name: lm.name.clone(),
            card_ids: ordered_card_ids(&lm.id, cards),
            archived: lm.archived,
        })
        .collect()
}

/// A fractional rank that orders strictly between `prev` and `next`. Used when
/// inserting/moving a Card: only the moved Card's `position` changes.
pub fn fractional_between(prev: Option<f64>, next: Option<f64>) -> f64 {
    match (prev, next) {
        (None, None) => 1.0,
        (Some(p), None) => p + 1.0,
        (None, Some(n)) => n - 1.0,
        (Some(p), Some(n)) => (p + n) / 2.0,
    }
}
