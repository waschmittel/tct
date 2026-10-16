use chrono::{NaiveDate, Utc};
use serde::{Deserialize, Serialize};

use super::id::{CardId, LabelId, ListId};
use super::label::Label;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Card {
    /// Not persisted — a Card's id is its file name (`card-<id>.json`), and
    /// `card_store` stamps it on load. See ADR-0007.
    #[serde(skip)]
    pub id: CardId,
    /// Id of the List this Card belongs to. Source of truth for membership —
    /// the List no longer stores its Cards. Empty only for legacy cards before
    /// migration stamps it.
    #[serde(default)]
    pub list_id: ListId,
    /// Fractional rank within the List. Cards render in ascending `position`
    /// order; inserting between two cards uses the midpoint of their ranks, so
    /// a move rewrites only the moved card.
    #[serde(default)]
    pub position: f64,
    pub title: String,
    pub description: String,
    #[serde(default)]
    pub label_ids: Vec<LabelId>,
    pub due_date: Option<NaiveDate>,
    #[serde(default)]
    pub checklist: Vec<ChecklistItem>,
    pub archived: bool,
    pub created_at: chrono::DateTime<Utc>,
    pub updated_at: chrono::DateTime<Utc>,
    #[serde(default)]
    pub history: Vec<HistoryEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChecklistItem {
    pub text: String,
    pub completed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub at: chrono::DateTime<Utc>,
    /// Human-readable summary, e.g. "Added label 'foo'".
    pub action: String,
    /// Which part of the Card changed, kept apart from the summary text.
    #[serde(default)]
    pub field: HistoryField,
    /// The field's value before the change, rendered as text. `None` when the
    /// field had no value (or the entry records no field change).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub old: Option<String>,
    /// The field's value after the change, rendered as text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub new: Option<String>,
}

/// The Card field a history entry is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HistoryField {
    /// Lifecycle events without a value diff (created, archived, restored).
    #[default]
    General,
    Title,
    Description,
    DueDate,
    Labels,
    Checklist,
}

impl HistoryField {
    pub fn label(self) -> &'static str {
        match self {
            HistoryField::General => "General",
            HistoryField::Title => "Title",
            HistoryField::Description => "Description",
            HistoryField::DueDate => "Due date",
            HistoryField::Labels => "Labels",
            HistoryField::Checklist => "Checklist",
        }
    }
}

pub const HISTORY_LIMIT: usize = 50;

impl Card {
    pub fn new(title: String) -> Self {
        let now = Utc::now();
        Self {
            id: CardId::generate(),
            list_id: ListId::default(),
            position: 0.0,
            title,
            description: String::new(),
            label_ids: Vec::new(),
            due_date: None,
            checklist: Vec::new(),
            archived: false,
            created_at: now,
            updated_at: now,
            history: Vec::new(),
        }
    }

    pub fn checklist_progress(&self) -> Option<(usize, usize)> {
        let total = self.checklist.len();
        if total == 0 {
            return None;
        }
        let done = self.checklist.iter().filter(|i| i.completed).count();
        Some((done, total))
    }

    pub fn matches_search(&self, query: &str, board_labels: &[Label]) -> bool {
        let q = query.to_lowercase();
        self.title.to_lowercase().contains(&q)
            || self.description.to_lowercase().contains(&q)
            || self.checklist.iter().any(|item| item.text.to_lowercase().contains(&q))
            || self.label_ids.iter().any(|lid| {
                board_labels
                    .iter()
                    .any(|l| l.id == *lid && l.name.to_lowercase().contains(&q))
            })
    }

    pub fn has_description(&self) -> bool {
        !self.description.is_empty()
    }

    /// When the Card last changed: its newest history entry, or its creation
    /// when nothing has been recorded.
    pub fn last_changed_at(&self) -> chrono::DateTime<Utc> {
        self.history.last().map_or(self.created_at, |e| e.at)
    }

    pub fn touch(&mut self) {
        self.updated_at = Utc::now();
    }

    /// Record a change in history and bump `updated_at`. Caps history at
    /// HISTORY_LIMIT entries (FIFO eviction).
    pub fn log(&mut self, action: impl Into<String>) {
        self.push_history(action.into(), HistoryField::General, None, None);
    }

    /// Like [`Card::log`], recording which field changed and its old and new
    /// values.
    pub fn log_change(
        &mut self,
        field: HistoryField,
        action: impl Into<String>,
        old: Option<String>,
        new: Option<String>,
    ) {
        self.push_history(action.into(), field, old, new);
    }

    /// Log a checklist change; `old` is the markdown rendering from before it.
    pub fn log_checklist(&mut self, action: impl Into<String>, old: String) {
        let new = self.checklist_as_markdown();
        self.push_history(action.into(), HistoryField::Checklist, Some(old), Some(new));
    }

    fn push_history(
        &mut self,
        action: String,
        field: HistoryField,
        old: Option<String>,
        new: Option<String>,
    ) {
        let now = Utc::now();
        self.history.push(HistoryEntry { at: now, action, field, old, new });
        let len = self.history.len();
        if len > HISTORY_LIMIT {
            self.history.drain(..len - HISTORY_LIMIT);
        }
        self.updated_at = now;
    }

    /// Label names in card order, one per line — the value recorded in
    /// history for [`HistoryField::Labels`].
    pub fn label_names_text(&self, board_labels: &[Label]) -> String {
        self.resolved_labels(board_labels)
            .iter()
            .map(|l| l.name.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Render the full checklist as GitHub-flavored markdown task list.
    /// Returns an empty string when the checklist is empty.
    pub fn checklist_as_markdown(&self) -> String {
        self.checklist
            .iter()
            .map(|item| {
                let mark = if item.completed { "x" } else { " " };
                format!("- [{mark}] {}", item.text)
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub fn resolved_labels<'a>(&self, board_labels: &'a [Label]) -> Vec<&'a Label> {
        board_labels
            .iter()
            .filter(|l| self.label_ids.contains(&l.id))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::label::LabelColor;

    #[test]
    fn new_card_defaults() {
        let card = Card::new("Test".into());
        assert_eq!(card.title, "Test");
        assert!(card.description.is_empty());
        assert!(card.label_ids.is_empty());
        assert!(card.due_date.is_none());
        assert!(!card.archived);
    }

    #[test]
    fn checklist_progress_empty() {
        let card = Card::new("Test".into());
        assert_eq!(card.checklist_progress(), None);
    }

    #[test]
    fn checklist_progress_counts() {
        let mut card = Card::new("Test".into());
        card.checklist = vec![
            ChecklistItem { text: "A".into(), completed: true },
            ChecklistItem { text: "B".into(), completed: false },
            ChecklistItem { text: "C".into(), completed: true },
        ];
        assert_eq!(card.checklist_progress(), Some((2, 3)));
    }

    #[test]
    fn has_description_false_when_empty() {
        let card = Card::new("Test".into());
        assert!(!card.has_description());
    }

    #[test]
    fn has_description_true_when_set() {
        let mut card = Card::new("Test".into());
        card.description = "some content".into();
        assert!(card.has_description());
    }

    #[test]
    fn search_matches_title() {
        let card = Card::new("Fix login bug".into());
        assert!(card.matches_search("login", &[]));
        assert!(card.matches_search("LOGIN", &[]));
        assert!(!card.matches_search("signup", &[]));
    }

    #[test]
    fn search_matches_checklist() {
        let mut card = Card::new("Task".into());
        card.checklist.push(ChecklistItem { text: "Deploy to staging".into(), completed: false });
        assert!(card.matches_search("deploy", &[]));
        assert!(card.matches_search("DEPLOY", &[]));
        assert!(!card.matches_search("production", &[]));
    }

    #[test]
    fn search_matches_label() {
        let label = Label::new("BUG".into(), LabelColor::Red);
        let mut card = Card::new("Task".into());
        card.label_ids.push(label.id.clone());
        assert!(card.matches_search("bug", &[label]));
    }

    #[test]
    fn resolved_labels_follow_board_order() {
        let l1 = Label::new("alpha".into(), LabelColor::Red);
        let l2 = Label::new("beta".into(), LabelColor::Green);
        let l3 = Label::new("gamma".into(), LabelColor::Blue);
        let board_labels = vec![l1.clone(), l2.clone(), l3.clone()];

        let mut card = Card::new("Task".into());
        // Assign in reverse order
        card.label_ids = vec![l3.id.clone(), l1.id.clone(), l2.id.clone()];

        let resolved = card.resolved_labels(&board_labels);
        // Should follow board order: alpha, beta, gamma
        assert_eq!(resolved[0].name, "alpha");
        assert_eq!(resolved[1].name, "beta");
        assert_eq!(resolved[2].name, "gamma");
    }

    #[test]
    fn log_appends_entry_and_advances_updated_at() {
        let mut card = Card::new("Task".into());
        let original = card.updated_at;
        std::thread::sleep(std::time::Duration::from_millis(2));
        card.log("Did something");
        assert_eq!(card.history.len(), 1);
        assert_eq!(card.history[0].action, "Did something");
        assert!(card.updated_at > original);
    }

    #[test]
    fn log_caps_at_history_limit() {
        let mut card = Card::new("Task".into());
        for i in 0..HISTORY_LIMIT + 10 {
            card.log(format!("event {i}"));
        }
        assert_eq!(card.history.len(), HISTORY_LIMIT);
        // Oldest entries dropped — first remaining entry must be "event 10".
        assert_eq!(card.history.first().unwrap().action, "event 10");
        assert_eq!(
            card.history.last().unwrap().action,
            format!("event {}", HISTORY_LIMIT + 9)
        );
    }

    #[test]
    fn touch_advances_updated_at() {
        let mut card = Card::new("Task".into());
        let original = card.updated_at;
        std::thread::sleep(std::time::Duration::from_millis(2));
        card.touch();
        assert!(card.updated_at > original);
    }

    #[test]
    fn checklist_progress_all_done() {
        let mut card = Card::new("Test".into());
        card.checklist = vec![
            ChecklistItem { text: "A".into(), completed: true },
            ChecklistItem { text: "B".into(), completed: true },
        ];
        assert_eq!(card.checklist_progress(), Some((2, 2)));
    }

    #[test]
    fn checklist_progress_none_done() {
        let mut card = Card::new("Test".into());
        card.checklist = vec![
            ChecklistItem { text: "A".into(), completed: false },
            ChecklistItem { text: "B".into(), completed: false },
        ];
        assert_eq!(card.checklist_progress(), Some((0, 2)));
    }

    #[test]
    fn checklist_as_markdown_mixed() {
        let mut card = Card::new("Task".into());
        card.checklist = vec![
            ChecklistItem { text: "Reproduce".into(), completed: true },
            ChecklistItem { text: "Fix".into(), completed: false },
            ChecklistItem { text: "Test".into(), completed: true },
        ];
        let md = card.checklist_as_markdown();
        assert_eq!(md, "- [x] Reproduce\n- [ ] Fix\n- [x] Test");
    }

    #[test]
    fn checklist_as_markdown_empty() {
        let card = Card::new("Task".into());
        assert_eq!(card.checklist_as_markdown(), "");
    }

    #[test]
    fn checklist_as_markdown_single_item() {
        let mut card = Card::new("Task".into());
        card.checklist = vec![ChecklistItem { text: "alone".into(), completed: false }];
        assert_eq!(card.checklist_as_markdown(), "- [ ] alone");
    }

    #[test]
    fn checklist_as_markdown_preserves_special_chars() {
        let mut card = Card::new("Task".into());
        card.checklist = vec![
            ChecklistItem { text: "with **bold** and `code`".into(), completed: true },
        ];
        assert_eq!(
            card.checklist_as_markdown(),
            "- [x] with **bold** and `code`"
        );
    }

    #[test]
    fn archive_round_trip() {
        let mut card = Card::new("Task".into());
        assert!(!card.archived);
        card.archived = true;
        assert!(card.archived);
        card.archived = false;
        assert!(!card.archived);
    }

    #[test]
    fn search_empty_query_matches_everything() {
        let card = Card::new("Anything".into());
        assert!(card.matches_search("", &[]));
    }

    #[test]
    fn search_matches_description() {
        let mut card = Card::new("Title".into());
        card.description = "Auth token expires".into();
        assert!(card.matches_search("auth", &[]));
        assert!(card.matches_search("EXPIRES", &[]));
        assert!(!card.matches_search("nothing", &[]));
    }

    #[test]
    fn resolved_labels_reflect_reorder() {
        let l1 = Label::new("first".into(), LabelColor::Red);
        let l2 = Label::new("second".into(), LabelColor::Green);
        let mut board_labels = vec![l1.clone(), l2.clone()];

        let mut card = Card::new("Task".into());
        card.label_ids = vec![l1.id.clone(), l2.id.clone()];

        // Before reorder: first, second
        let resolved = card.resolved_labels(&board_labels);
        assert_eq!(resolved[0].name, "first");
        assert_eq!(resolved[1].name, "second");

        // Reorder board labels
        board_labels.swap(0, 1);

        // After reorder: second, first
        let resolved = card.resolved_labels(&board_labels);
        assert_eq!(resolved[0].name, "second");
        assert_eq!(resolved[1].name, "first");
    }

    #[test]
    fn last_changed_at_is_newest_history_entry_or_creation() {
        let mut card = Card::new("Task".into());
        assert_eq!(card.last_changed_at(), card.created_at);
        card.log("a");
        card.history[0].at = card.created_at + chrono::Duration::days(3);
        assert_eq!(card.last_changed_at(), card.history[0].at);
    }

    #[test]
    fn log_records_general_entry_without_values() {
        let mut card = Card::new("Task".into());
        card.log("Archived");
        let e = &card.history[0];
        assert_eq!(e.field, HistoryField::General);
        assert!(e.old.is_none() && e.new.is_none());
    }

    #[test]
    fn log_change_records_field_and_values() {
        let mut card = Card::new("Task".into());
        card.log_change(HistoryField::Title, "Edited title", Some("a".into()), Some("b".into()));
        let e = &card.history[0];
        assert_eq!(e.field, HistoryField::Title);
        assert_eq!((e.old.as_deref(), e.new.as_deref()), (Some("a"), Some("b")));
    }

    #[test]
    fn log_checklist_snapshots_old_and_current_markdown() {
        let mut card = Card::new("Task".into());
        let old = card.checklist_as_markdown();
        card.checklist.push(ChecklistItem { text: "x".into(), completed: true });
        card.log_checklist("Added", old);
        let e = &card.history[0];
        assert_eq!(e.field, HistoryField::Checklist);
        assert_eq!(e.old.as_deref(), Some(""));
        assert_eq!(e.new.as_deref(), Some("- [x] x"));
    }

    #[test]
    fn label_names_text_lists_names_in_board_order() {
        let l1 = Label::new("alpha".into(), LabelColor::Red);
        let l2 = Label::new("beta".into(), LabelColor::Green);
        let mut card = Card::new("Task".into());
        card.label_ids = vec![l2.id.clone(), l1.id.clone()];
        assert_eq!(card.label_names_text(&[l1, l2]), "alpha\nbeta");
        assert_eq!(Card::new("x".into()).label_names_text(&[]), "");
    }

    #[test]
    fn history_entry_without_field_values_loads_as_general() {
        let json = r#"{"at":"2024-01-01T00:00:00Z","action":"Edited title"}"#;
        let e: HistoryEntry = serde_json::from_str(json).unwrap();
        assert_eq!(e.field, HistoryField::General);
        assert!(e.old.is_none() && e.new.is_none());
    }

    #[test]
    fn history_entry_round_trips_and_omits_absent_values() {
        let mut card = Card::new("Task".into());
        card.log("Archived");
        card.log_change(HistoryField::DueDate, "Set", None, Some("2099-01-01".into()));
        let json = serde_json::to_string(&card.history).unwrap();
        assert!(json.contains(r#""field":"due_date""#));
        assert_eq!(json.matches("\"old\"").count(), 0);
        let back: Vec<HistoryEntry> = serde_json::from_str(&json).unwrap();
        assert_eq!(back[1].field, HistoryField::DueDate);
        assert_eq!(back[1].new.as_deref(), Some("2099-01-01"));
    }

    #[test]
    fn history_field_labels_are_distinct() {
        use HistoryField::*;
        let all = [General, Title, Description, DueDate, Labels, Checklist];
        let names: std::collections::HashSet<_> = all.iter().map(|f| f.label()).collect();
        assert_eq!(names.len(), all.len());
    }
}
