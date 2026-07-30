use std::collections::HashMap;

use super::board::BoardMeta;
use super::card::Card;
use super::id::{CardId, ListId};

/// In-memory view of a List: its metadata plus the ordered ids of the Cards
/// that belong to it (archived Cards included — visibility is decided by
/// `LoadedBoard::visible_cards`, not by membership). `card_ids` is derived from
/// each Card's `list_id` + `position` at load time; it is never persisted as a
/// `CardList`.
#[derive(Debug, Clone)]
pub struct CardList {
    pub id: ListId,
    pub name: String,
    pub card_ids: Vec<CardId>,
}

/// Cards belonging to `list_id`, sorted ascending by `position` (ties broken by
/// `created_at` then `id` for stable, deterministic ordering).
pub fn ordered_card_ids(list_id: &ListId, cards: &HashMap<CardId, Card>) -> Vec<CardId> {
    let mut members: Vec<&Card> = cards.values().filter(|c| c.list_id == *list_id).collect();
    members.sort_by(|a, b| {
        super::rank::cmp(a.position, b.position)
            .then(a.created_at.cmp(&b.created_at))
            .then(a.id.cmp(&b.id))
    });
    members.into_iter().map(|c| c.id.clone()).collect()
}

/// Build the in-memory `CardList`s for a board from its `ListMeta` definitions
/// and the full Card map. When `archived` is false, returns the active Lists in
/// board order; when true, returns the archived Lists.
pub fn build_lists(meta: &BoardMeta, cards: &HashMap<CardId, Card>, archived: bool) -> Vec<CardList> {
    meta.lists
        .iter()
        .filter(|lm| lm.archived == archived)
        .map(|lm| CardList {
            id: lm.id.clone(),
            name: lm.name.clone(),
            card_ids: ordered_card_ids(&lm.id, cards),
        })
        .collect()
}
