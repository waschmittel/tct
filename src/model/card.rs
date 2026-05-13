use chrono::{NaiveDate, Utc};
use serde::{Deserialize, Serialize};

use super::ids::{self, ShortId};
use super::label::Label;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Card {
    pub id: ShortId,
    pub title: String,
    pub description: String,
    pub labels: Vec<Label>,
    pub due_date: Option<NaiveDate>,
    pub checklists: Vec<Checklist>,
    pub archived: bool,
    pub created_at: chrono::DateTime<Utc>,
    pub updated_at: chrono::DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checklist {
    pub title: String,
    pub items: Vec<ChecklistItem>,
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
            labels: Vec::new(),
            due_date: None,
            checklists: Vec::new(),
            archived: false,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn checklist_progress(&self) -> Option<(usize, usize)> {
        let total: usize = self.checklists.iter().map(|c| c.items.len()).sum();
        if total == 0 {
            return None;
        }
        let done: usize = self
            .checklists
            .iter()
            .flat_map(|c| &c.items)
            .filter(|i| i.completed)
            .count();
        Some((done, total))
    }

    pub fn matches_search(&self, query: &str) -> bool {
        let q = query.to_lowercase();
        self.title.to_lowercase().contains(&q)
            || self.description.to_lowercase().contains(&q)
            || self.labels.iter().any(|l| l.name.to_lowercase().contains(&q))
    }

    pub fn touch(&mut self) {
        self.updated_at = Utc::now();
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
        assert!(card.labels.is_empty());
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
        card.checklists.push(Checklist {
            title: "TODO".into(),
            items: vec![
                ChecklistItem { text: "A".into(), completed: true },
                ChecklistItem { text: "B".into(), completed: false },
                ChecklistItem { text: "C".into(), completed: true },
            ],
        });
        assert_eq!(card.checklist_progress(), Some((2, 3)));
    }

    #[test]
    fn search_matches_title() {
        let card = Card::new("Fix login bug".into());
        assert!(card.matches_search("login"));
        assert!(card.matches_search("LOGIN"));
        assert!(!card.matches_search("signup"));
    }

    #[test]
    fn search_matches_label() {
        let mut card = Card::new("Task".into());
        card.labels.push(Label { name: "BUG".into(), color: LabelColor::Red });
        assert!(card.matches_search("bug"));
    }
}
