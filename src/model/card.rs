use chrono::{NaiveDate, Utc};
use serde::{Deserialize, Serialize};

use super::ids::{self, ShortId};
use super::label::Label;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Card {
    pub id: ShortId,
    pub title: String,
    pub description: String,
    #[serde(default)]
    pub label_ids: Vec<ShortId>,
    pub due_date: Option<NaiveDate>,
    #[serde(default)]
    pub checklist: Vec<ChecklistItem>,
    pub archived: bool,
    pub created_at: chrono::DateTime<Utc>,
    pub updated_at: chrono::DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChecklistItem {
    pub text: String,
    pub completed: bool,
}

impl Card {
    pub fn new(title: String) -> Self {
        let now = Utc::now();
        Self {
            id: ids::new_id(),
            title,
            description: String::new(),
            label_ids: Vec::new(),
            due_date: None,
            checklist: Vec::new(),
            archived: false,
            created_at: now,
            updated_at: now,
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
            || self.label_ids.iter().any(|lid| {
                board_labels
                    .iter()
                    .any(|l| l.id == *lid && l.name.to_lowercase().contains(&q))
            })
    }

    pub fn touch(&mut self) {
        self.updated_at = Utc::now();
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
    fn search_matches_title() {
        let card = Card::new("Fix login bug".into());
        assert!(card.matches_search("login", &[]));
        assert!(card.matches_search("LOGIN", &[]));
        assert!(!card.matches_search("signup", &[]));
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
}
