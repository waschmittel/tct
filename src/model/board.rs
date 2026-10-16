use chrono::Utc;
use serde::{Deserialize, Serialize};

use super::id::{BoardId, ListId};
use super::label::{Label, LabelColor};

/// Persisted definition of a List. Membership (which Cards belong to it) is
/// NOT stored here — each Card carries its own `list_id`. This keeps a Card's
/// archived flag and its List membership in one file, so they cannot diverge.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListMeta {
    pub id: ListId,
    pub name: String,
    #[serde(default)]
    pub archived: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoardMeta {
    /// Not persisted — a Board's id is the directory holding its `board.json`,
    /// and `board_store` stamps it on load. See ADR-0007.
    #[serde(skip)]
    pub id: BoardId,
    pub name: String,
    /// Fractional rank in the board selector. A Board owns its own place in
    /// the collection for the same reason a Card owns its place in a List
    /// (ADR-0006/0008) — the alternative, a central order file, states
    /// membership twice and drifts.
    #[serde(default)]
    pub position: f64,
    /// Ordered List definitions (active and archived). Order in this Vec is
    /// the board's left-to-right List order; archived Lists are filtered out
    /// of the active view but kept here for restore.
    #[serde(default)]
    pub lists: Vec<ListMeta>,
    #[serde(default)]
    pub labels: Vec<Label>,
    #[serde(default)]
    pub accent_color: LabelColor,
    #[serde(default)]
    pub archived: bool,
    /// Show each Card's creation date in its frame (bottom right).
    #[serde(default)]
    pub show_created_date: bool,
    /// Show each Card's last-change date in its frame (top right).
    #[serde(default)]
    pub show_updated_date: bool,
    pub created_at: chrono::DateTime<Utc>,
    pub updated_at: chrono::DateTime<Utc>,
}

impl BoardMeta {
    pub fn new(name: String) -> Self {
        let now = Utc::now();
        Self {
            id: BoardId::generate(),
            name,
            position: 0.0,
            lists: Vec::new(),
            labels: Vec::new(),
            accent_color: LabelColor::default(),
            archived: false,
            show_created_date: false,
            show_updated_date: false,
            created_at: now,
            updated_at: now,
        }
    }
}
