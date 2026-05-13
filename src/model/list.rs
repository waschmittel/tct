use serde::{Deserialize, Serialize};

use super::ids::{self, ShortId};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CardList {
    pub id: ShortId,
    pub name: String,
    pub card_ids: Vec<ShortId>,
}

impl CardList {
    pub fn new(name: String) -> Self {
        Self {
            id: ids::new_id(),
            name,
            card_ids: Vec::new(),
        }
    }
}
