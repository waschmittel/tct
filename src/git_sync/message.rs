//! Commit messages from board/card changes — pure, no git. Titles are
//! user (or repository) controlled, so they are flattened to one line and
//! shortened before they reach a message.

use crate::model::board::BoardMeta;
use crate::model::card::Card;
use crate::model::id::BoardId;

/// Most detail lines in a body; the rest collapse into "… and N more".
const MAX_DETAILS: usize = 20;
/// Longest title shown, in characters.
const MAX_TITLE: usize = 60;
/// Boards named in a multi-board subject before "and N more".
const MAX_NAMED_BOARDS: usize = 3;

pub struct BoardChange {
    pub id: BoardId,
    pub old: Option<BoardMeta>,
    pub new: Option<BoardMeta>,
    pub cards: Vec<CardChange>,
}

pub struct CardChange {
    pub old: Option<Card>,
    pub new: Option<Card>,
}

struct Detail {
    text: String,
    card: bool,
}

/// `"Title"`, one line, at most [`MAX_TITLE`] characters.
fn q(title: &str) -> String {
    let flat: String = title
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    let flat = flat.split_whitespace().collect::<Vec<_>>().join(" ");
    if flat.chars().count() > MAX_TITLE {
        let cut: String = flat.chars().take(MAX_TITLE - 1).collect();
        format!("\"{cut}…\"")
    } else {
        format!("\"{flat}\"")
    }
}

fn board_name(b: &BoardChange) -> String {
    let meta = b.new.as_ref().or(b.old.as_ref());
    meta.map_or_else(|| q(b.id.as_str()), |m| q(&m.name))
}

fn list_name(board: Option<&BoardMeta>, id: &crate::model::id::ListId) -> Option<String> {
    board?
        .lists
        .iter()
        .find(|l| l.id == *id)
        .map(|l| q(&l.name))
}

fn json<T: serde::Serialize>(v: &T) -> String {
    serde_json::to_string(v).unwrap_or_default()
}

fn card_detail(c: &CardChange, board: Option<&BoardMeta>) -> Option<String> {
    Some(match (&c.old, &c.new) {
        (None, Some(n)) => match list_name(board, &n.list_id) {
            Some(l) => format!("card {} created in {l}", q(&n.title)),
            None => format!("card {} created", q(&n.title)),
        },
        (Some(o), None) => format!("card {} deleted", q(&o.title)),
        (Some(o), Some(n)) => {
            let mut verbs = Vec::new();
            if o.archived != n.archived {
                verbs.push(if n.archived { "archived" } else { "restored" }.to_string());
            }
            if o.list_id != n.list_id {
                verbs.push(match list_name(board, &n.list_id) {
                    Some(l) => format!("moved to {l}"),
                    None => "moved".to_string(),
                });
            }
            if o.title != n.title {
                verbs.push(format!("renamed from {}", q(&o.title)));
            }
            if o.description != n.description
                || o.due_date != n.due_date
                || o.label_ids != n.label_ids
                || json(&o.checklist) != json(&n.checklist)
            {
                verbs.push("edited".to_string());
            }
            if verbs.is_empty() {
                verbs.push(
                    if o.position != n.position {
                        "reordered"
                    } else {
                        "edited"
                    }
                    .to_string(),
                );
            }
            format!("card {} {}", q(&n.title), verbs.join(", "))
        }
        (None, None) => return None,
    })
}

fn board_details(b: &BoardChange) -> Vec<Detail> {
    let board_level = |text: String| Detail { text, card: false };
    let counted = |what: &str, n: usize| match n {
        0 => what.to_string(),
        1 => format!("{what} (1 card)"),
        n => format!("{what} ({n} cards)"),
    };
    let (old, new) = match (&b.old, &b.new) {
        (None, Some(_)) => {
            let n = b.cards.iter().filter(|c| c.new.is_some()).count();
            return vec![board_level(counted("created", n))];
        }
        (Some(_), None) => {
            let n = b.cards.iter().filter(|c| c.old.is_some()).count();
            return vec![board_level(counted("deleted", n))];
        }
        (Some(o), Some(n)) => (o, n),
        (None, None) => return Vec::new(),
    };

    let mut out = Vec::new();
    if old.name != new.name {
        out.push(board_level(format!("renamed from {}", q(&old.name))));
    }
    if old.archived != new.archived {
        out.push(board_level(
            if new.archived { "archived" } else { "restored" }.to_string(),
        ));
    }
    for l in &new.lists {
        match old.lists.iter().find(|o| o.id == l.id) {
            None => out.push(board_level(format!("list {} added", q(&l.name)))),
            Some(o) => {
                if o.name != l.name {
                    out.push(board_level(format!(
                        "list {} renamed from {}",
                        q(&l.name),
                        q(&o.name)
                    )));
                }
                if o.archived != l.archived {
                    let verb = if l.archived { "archived" } else { "restored" };
                    out.push(board_level(format!("list {} {verb}", q(&l.name))));
                }
            }
        }
    }
    for o in &old.lists {
        if !new.lists.iter().any(|l| l.id == o.id) {
            out.push(board_level(format!("list {} deleted", q(&o.name))));
        }
    }
    let common = |a: &BoardMeta, b: &BoardMeta| -> Vec<String> {
        a.lists
            .iter()
            .filter(|l| b.lists.iter().any(|x| x.id == l.id))
            .map(|l| l.id.to_string())
            .collect()
    };
    if common(old, new) != common(new, old) {
        out.push(board_level("lists reordered".to_string()));
    }
    if old.labels != new.labels {
        out.push(board_level("labels changed".to_string()));
    }
    if old.position != new.position {
        out.push(board_level("board reordered".to_string()));
    }
    if json(&old.accent_color) != json(&new.accent_color)
        || json(&old.new_card_position) != json(&new.new_card_position)
        || old.show_created_date != new.show_created_date
        || old.show_updated_date != new.show_updated_date
    {
        out.push(board_level("settings changed".to_string()));
    }

    let mut cards: Vec<(String, String)> = b
        .cards
        .iter()
        .filter_map(|c| {
            let title = c.new.as_ref().or(c.old.as_ref())?.title.to_lowercase();
            Some((title, card_detail(c, b.new.as_ref())?))
        })
        .collect();
    cards.sort();
    out.extend(
        cards
            .into_iter()
            .map(|(_, text)| Detail { text, card: true }),
    );
    out
}

/// `(subject, body)` — the body is empty when the subject says it all.
pub fn message(changes: &[BoardChange]) -> (String, String) {
    let touched: Vec<(&BoardChange, Vec<Detail>)> = changes
        .iter()
        .map(|b| (b, board_details(b)))
        .filter(|(_, d)| !d.is_empty())
        .collect();

    if touched.is_empty() {
        return ("Update tct data".to_string(), String::new());
    }

    if let [(b, details)] = touched.as_slice() {
        let name = board_name(b);
        if let [only] = details.as_slice() {
            return (format!("Board {name}: {}", only.text), String::new());
        }
        let n = details.len();
        let subject = if details.iter().all(|d| d.card) {
            format!("Board {name}: {n} cards updated")
        } else {
            format!("Board {name}: {n} changes")
        };
        let mut lines = Lines::default();
        for d in details {
            lines.push(format!("- {}", d.text));
        }
        return (subject, lines.finish());
    }

    let names: Vec<String> = touched.iter().map(|(b, _)| board_name(b)).collect();
    let shown = names[..names.len().min(MAX_NAMED_BOARDS)].join(", ");
    let subject = if names.len() > MAX_NAMED_BOARDS {
        format!(
            "Boards {shown} and {} more updated",
            names.len() - MAX_NAMED_BOARDS
        )
    } else {
        format!("Boards {shown} updated")
    };
    let mut lines = Lines::default();
    for (i, (b, details)) in touched.iter().enumerate() {
        if i > 0 {
            lines.gap();
        }
        lines.heading(format!("Board {}:", board_name(b)));
        for d in details {
            lines.push(format!("- {}", d.text));
        }
    }
    (subject, lines.finish())
}

/// Body builder that caps the number of detail lines.
#[derive(Default)]
struct Lines {
    out: Vec<String>,
    details: usize,
    omitted: usize,
}

impl Lines {
    fn push(&mut self, line: String) {
        if self.details < MAX_DETAILS {
            self.details += 1;
            self.out.push(line);
        } else {
            self.omitted += 1;
        }
    }

    fn heading(&mut self, line: String) {
        if self.details < MAX_DETAILS {
            self.out.push(line);
        }
    }

    fn gap(&mut self) {
        if self.details < MAX_DETAILS {
            self.out.push(String::new());
        }
    }

    fn finish(mut self) -> String {
        while self.out.last().is_some_and(String::is_empty) {
            self.out.pop();
        }
        if self.omitted > 0 {
            self.out.push(format!("- … and {} more", self.omitted));
        }
        self.out.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::board::ListMeta;
    use crate::model::id::ListId;

    fn board(name: &str, lists: &[(&str, &str)]) -> BoardMeta {
        let mut b = BoardMeta::new(name.into());
        b.lists = lists
            .iter()
            .map(|(id, n)| ListMeta {
                id: (*id).into(),
                name: (*n).into(),
                archived: false,
            })
            .collect();
        b
    }

    fn card(title: &str, list: &str) -> Card {
        let mut c = Card::new(title.into());
        c.list_id = list.into();
        c
    }

    fn change(
        old: Option<BoardMeta>,
        new: Option<BoardMeta>,
        cards: Vec<CardChange>,
    ) -> BoardChange {
        let id = new.as_ref().or(old.as_ref()).unwrap().id.clone();
        BoardChange {
            id,
            old,
            new,
            cards,
        }
    }

    fn edited(old: Card, f: impl FnOnce(&mut Card)) -> CardChange {
        let mut new = old.clone();
        f(&mut new);
        CardChange {
            old: Some(old),
            new: Some(new),
        }
    }

    #[test]
    fn single_card_move_is_the_subject() {
        let b = board("Sprint", &[("l1", "To Do"), ("l2", "Done")]);
        let c = edited(card("Fix login", "l1"), |c| c.list_id = ListId::from("l2"));
        let (s, body) = message(&[change(Some(b.clone()), Some(b), vec![c])]);
        assert_eq!(s, "Board \"Sprint\": card \"Fix login\" moved to \"Done\"");
        assert_eq!(body, "");
    }

    #[test]
    fn card_verbs_combine_and_fall_back() {
        let b = board("B", &[("l1", "A")]);
        let c = edited(card("Old", "l1"), |c| {
            c.title = "New".into();
            c.description = "x".into();
            c.archived = true;
        });
        let (s, _) = message(&[change(Some(b.clone()), Some(b.clone()), vec![c])]);
        assert!(
            s.ends_with("card \"New\" archived, renamed from \"Old\", edited"),
            "{s}"
        );

        let c = edited(card("T", "l1"), |c| c.position = 9.0);
        let (s, _) = message(&[change(Some(b.clone()), Some(b.clone()), vec![c])]);
        assert!(s.ends_with("card \"T\" reordered"), "{s}");

        // Only timestamps/history changed.
        let c = edited(card("T", "l1"), |c| c.touch());
        let (s, _) = message(&[change(Some(b.clone()), Some(b), vec![c])]);
        assert!(s.ends_with("card \"T\" edited"), "{s}");
    }

    #[test]
    fn created_and_deleted_cards() {
        let b = board("B", &[("l1", "To Do")]);
        let created = CardChange {
            old: None,
            new: Some(card("Fresh", "l1")),
        };
        let deleted = CardChange {
            old: Some(card("Gone", "l1")),
            new: None,
        };
        let (s, body) = message(&[change(Some(b.clone()), Some(b), vec![created, deleted])]);
        assert_eq!(s, "Board \"B\": 2 cards updated");
        assert_eq!(
            body,
            "- card \"Fresh\" created in \"To Do\"\n- card \"Gone\" deleted"
        );
    }

    #[test]
    fn board_created_and_deleted_count_cards() {
        let b = board("New", &[]);
        let cards = vec![
            CardChange {
                old: None,
                new: Some(card("a", "l1")),
            },
            CardChange {
                old: None,
                new: Some(card("b", "l1")),
            },
        ];
        let (s, _) = message(&[change(None, Some(b.clone()), cards)]);
        assert_eq!(s, "Board \"New\": created (2 cards)");
        let (s, _) = message(&[change(Some(b.clone()), None, vec![])]);
        assert_eq!(s, "Board \"New\": deleted");
        let one = vec![CardChange {
            old: Some(card("a", "l1")),
            new: None,
        }];
        let (s, _) = message(&[change(Some(b), None, one)]);
        assert_eq!(s, "Board \"New\": deleted (1 card)");
    }

    #[test]
    fn board_level_changes() {
        let old = board("Old", &[("l1", "A"), ("l2", "B"), ("l3", "C")]);
        let mut new = old.clone();
        new.name = "New".into();
        new.lists[0].name = "A2".into();
        new.lists[1].archived = true;
        new.lists.remove(2);
        new.lists.push(ListMeta {
            id: "l4".into(),
            name: "D".into(),
            archived: false,
        });
        new.lists.swap(0, 1);
        let (s, body) = message(&[change(Some(old), Some(new), vec![])]);
        assert_eq!(s, "Board \"New\": 6 changes");
        for expected in [
            "- renamed from \"Old\"",
            "- list \"D\" added",
            "- list \"A2\" renamed from \"A\"",
            "- list \"B\" archived",
            "- list \"C\" deleted",
            "- lists reordered",
        ] {
            assert!(body.contains(expected), "missing {expected} in {body}");
        }
    }

    #[test]
    fn several_boards_are_named_and_grouped() {
        let mk = |name: &str, title: &str| {
            let b = board(name, &[("l1", "L")]);
            let c = edited(card(title, "l1"), |c| c.description = "x".into());
            change(Some(b.clone()), Some(b), vec![c])
        };
        let (s, body) = message(&[mk("A", "one"), mk("B", "two")]);
        assert_eq!(s, "Boards \"A\", \"B\" updated");
        assert_eq!(
            body,
            "Board \"A\":\n- card \"one\" edited\n\nBoard \"B\":\n- card \"two\" edited"
        );
        let (s, _) = message(&[
            mk("A", "1"),
            mk("B", "2"),
            mk("C", "3"),
            mk("D", "4"),
            mk("E", "5"),
        ]);
        assert_eq!(s, "Boards \"A\", \"B\", \"C\" and 2 more updated");
    }

    #[test]
    fn long_bodies_are_capped() {
        let b = board("B", &[("l1", "L")]);
        let cards = (0..25)
            .map(|i| {
                edited(card(&format!("c{i:02}"), "l1"), |c| {
                    c.description = "x".into()
                })
            })
            .collect();
        let (s, body) = message(&[change(Some(b.clone()), Some(b), cards)]);
        assert_eq!(s, "Board \"B\": 25 cards updated");
        let lines: Vec<&str> = body.lines().collect();
        assert_eq!(lines.len(), MAX_DETAILS + 1);
        assert_eq!(lines[MAX_DETAILS], "- … and 5 more");
    }

    #[test]
    fn titles_are_flattened_and_shortened() {
        assert_eq!(q("a\nb\x1b[31m  c"), "\"a b [31m c\"");
        let long = "x".repeat(100);
        let shown = q(&long);
        assert_eq!(shown.chars().count(), MAX_TITLE + 2);
        assert!(shown.ends_with("…\""));
    }

    #[test]
    fn nothing_describable_is_a_generic_subject() {
        assert_eq!(message(&[]), ("Update tct data".to_string(), String::new()));
    }
}
