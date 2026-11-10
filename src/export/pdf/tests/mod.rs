use super::*;
use crate::model::card::ChecklistItem;

mod alerts;
mod basics;
mod corner_cases;
mod footnotes;
mod pagination;
mod tables;
mod text;
mod themes;
use corner_cases::*;

const THEMES: [Theme; 2] = [Theme::Color, Theme::Mono];

fn board() -> BoardMeta {
    BoardMeta::new("Projekt Ä".into())
}

fn texts(page: &[Op]) -> Vec<&str> {
    page.iter()
        .filter_map(|op| {
            if let Op::Text { text, .. } = op {
                Some(text.as_str())
            } else {
                None
            }
        })
        .collect()
}

fn page_of(pages: &[Vec<Op>], needle: &str) -> usize {
    pages
        .iter()
        .position(|p| texts(p).iter().any(|t| t.contains(needle)))
        .unwrap_or_else(|| panic!("{needle} not drawn"))
}

fn lines(prefix: &str, n: usize) -> String {
    (0..n)
        .map(|i| format!("{prefix} {i}"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn laid_out(card: &Card) -> Vec<Vec<Op>> {
    let fonts = Fonts::load().unwrap();
    layout(&fonts, &board(), "L", card, chrono::Utc::now()).0
}

fn count(page: &[Op], needle: &str) -> usize {
    texts(page).iter().filter(|t| **t == needle).count()
}

/// Vertical lines of the given weight, as (x, y1, y2).
fn verticals(page: &[Op], weight: f32) -> Vec<(f32, f32, f32)> {
    page.iter()
        .filter_map(|op| match op {
            Op::Line {
                x1,
                y1,
                x2,
                y2,
                width,
                ..
            } if x1 == x2 && *width == weight => Some((*x1, *y1, *y2)),
            _ => None,
        })
        .collect()
}

/// Table rules of the given weight, as (y, x1, x2) in drawing order.
fn rules(page: &[Op], weight: f32) -> Vec<(f32, f32, f32)> {
    page.iter()
        .filter_map(|op| match op {
            Op::Line {
                x1,
                y1,
                x2,
                y2,
                width,
                ..
            } if y1 == y2 && *width == weight => Some((*y1, *x1, *x2)),
            _ => None,
        })
        .collect()
}

fn table_page(table: &str) -> Vec<Op> {
    let mut card = Card::new("T".into());
    card.description = table.into();
    laid_out(&card).remove(0)
}

fn matrix(cols: usize, body_rows: usize) -> String {
    let head: Vec<String> = (1..cols).map(|c| format!("C{c}")).collect();
    let delim = vec!["---"; cols].join("|");
    let mut t = format!("|  | {} |\n|{delim}|\n", head.join(" | "));
    for r in 0..body_rows {
        t.push_str(&format!("| r{r} |{}\n", " x |".repeat(cols - 1)));
    }
    t
}
