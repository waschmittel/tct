use chrono::Utc;
use serde::{Deserialize, Serialize};

use super::ids::{self, ShortId};
use super::label::{Label, LabelColor};

/// Persisted definition of a List. Membership (which Cards belong to it) is
/// NOT stored here — each Card carries its own `list_id`. This keeps a Card's
/// archived flag and its List membership in one file, so they cannot diverge.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListMeta {
    pub id: ShortId,
    pub name: String,
    #[serde(default)]
    pub archived: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoardMeta {
    pub id: ShortId,
    pub name: String,
    pub description: String,
    /// Ordered List definitions (active and archived). Order in this Vec is
    /// the board's left-to-right List order; archived Lists are filtered out
    /// of the active view but kept here for restore.
    #[serde(default)]
    pub lists: Vec<ListMeta>,
    /// Legacy field: pre-migration boards stored only an ordered list of List
    /// ids and kept membership in separate `list-*.json` files. Read on load
    /// to drive the one-time migration, then never written again.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub list_order: Vec<ShortId>,
    #[serde(default)]
    pub labels: Vec<Label>,
    #[serde(default)]
    pub accent_color: LabelColor,
    #[serde(default)]
    pub archived: bool,
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
            lists: Vec::new(),
            list_order: Vec::new(),
            labels: Vec::new(),
            accent_color: LabelColor::default(),
            archived: false,
            created_at: now,
            updated_at: now,
        }
    }
}
