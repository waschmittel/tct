use std::collections::{HashMap, HashSet};

use crate::model::board::BoardMeta;
use crate::model::card::Card;
use crate::model::id::{BoardId, CardId, LabelId, ListId};
use crate::storage::card_store;

/// Detect and auto-repair dangling references in freshly loaded cards, persisting
/// each card that changed. Returns a human-readable line per repair (empty on a
/// clean board). Two reference hazards are handled:
///
/// * **Orphan card** — `list_id` names a List that no longer exists (or is
///   empty). The card would be invisible (never built into any List), so it is
///   reattached to the board's first active List with a position past the
///   current tail.
/// * **Dangling label** — `label_ids` names a Label absent from the board; the
///   id is stripped.
pub(super) fn repair_references(
    board_id: &BoardId,
    meta: &BoardMeta,
    cards: &mut HashMap<CardId, Card>,
) -> Vec<String> {
    let valid_lists: HashSet<&ListId> = meta.lists.iter().map(|l| &l.id).collect();
    let valid_labels: HashSet<&LabelId> = meta.labels.iter().map(|l| &l.id).collect();
    let default_list = meta
        .lists
        .iter()
        .find(|l| !l.archived)
        .or_else(|| meta.lists.first())
        .map(|l| l.id.clone());

    // Next free position in the reattach target, so reattached orphans land at
    // the tail rather than colliding at 0.
    let mut next_pos = default_list
        .as_ref()
        .map(|dst| {
            cards
                .values()
                .filter(|c| &c.list_id == dst)
                .map(|c| c.position)
                .fold(0.0_f64, f64::max)
                + 1.0
        })
        .unwrap_or(1.0);

    let mut warnings = Vec::new();
    let mut changed: Vec<CardId> = Vec::new();

    for card in cards.values_mut() {
        let mut card_changed = false;

        if !valid_lists.contains(&card.list_id) {
            match &default_list {
                Some(dst) => {
                    warnings.push(format!(
                        "card \"{}\" pointed at missing list — reattached to first list",
                        card.title
                    ));
                    card.list_id = dst.clone();
                    card.position = next_pos;
                    next_pos += 1.0;
                    card_changed = true;
                }
                None => warnings.push(format!(
                    "card \"{}\" is orphaned and the board has no lists to attach it to",
                    card.title
                )),
            }
        }

        let before = card.label_ids.len();
        card.label_ids.retain(|lid| valid_labels.contains(lid));
        let stripped = before - card.label_ids.len();
        if stripped > 0 {
            warnings.push(format!(
                "card \"{}\" had {stripped} unknown label ref(s) removed",
                card.title
            ));
            card_changed = true;
        }

        if card_changed {
            changed.push(card.id.clone());
        }
    }

    for id in &changed {
        if let Some(card) = cards.get(id) {
            let _ = card_store::save_card(board_id, card);
        }
    }

    warnings
}
