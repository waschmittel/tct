use chrono::Utc;
use serde::{Deserialize, Serialize};

use super::ids::{self, ShortId};
use super::label::Label;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoardMeta {
    pub id: ShortId,
    pub name: String,
    pub description: String,
    pub list_order: Vec<ShortId>,
    #[serde(default)]
    pub labels: Vec<Label>,
    pub created_at: chrono::DateTime<Utc>,
    pub updated_at: chrono::DateTime<Utc>,
}

impl BoardMeta {
    pub fn new(name: String) -> Self {
        let now = Utc::now();
        Self {
            id: ids::new_id(),
            name,
            description: String::new(),
            list_order: Vec::new(),
            labels: Vec::new(),
            created_at: now,
            updated_at: now,
        }
    }
}
