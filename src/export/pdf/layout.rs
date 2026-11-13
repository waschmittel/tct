use std::rc::Rc;

use super::*;
use crate::export::blocks::{self, Block, Run, Style};
use crate::export::lang::{Label, Lang};
use crate::model::board::BoardMeta;
use crate::model::card::Card;

/// End (exclusive) of the section starting at `start`: up to the next
/// heading of the same or a higher level; for non-heading content, up to the
/// next heading of any level.
pub(super) fn section_end(blocks: &[Block], start: usize) -> usize {
    let level = match blocks.get(start) {
        Some(Block::Heading { level, .. }) => *level,
        _ => 6,
    };
    blocks[start + 1..]
        .iter()
        .position(|b| matches!(b, Block::Heading { level: l, .. } if *l <= level))
        .map_or(blocks.len(), |p| start + 1 + p)
}

pub(super) fn layout(
    fonts: &Fonts,
    board: &BoardMeta,
    list: &str,
    card: &Card,
    now: chrono::DateTime<chrono::Utc>,
) -> (Vec<Vec<Op>>, Vec<String>) {
    let lang = Lang::of(card);
    let doc = blocks::parse_doc(&card.description, lang);
    let mut flow = Flow::with_notes(fonts, Rc::new(doc.notes));
    flow.lang = lang;

    let title = [Run {
        text: card.title.clone(),
        style: Style {
            bold: true,
            ..Style::default()
        },
    }];
    flow.paragraph(&title, 20.0, MARGIN, CONTENT_W, flow.ink());
    flow.y += 4.0;
    let accent = if flow.mono() {
        BLACK
    } else {
        board.accent_color.to_rgb()
    };
    flow.rule(accent, 2.0);
    flow.y += 8.0;

    let muted = |text: String| {
        [Run {
            text,
            style: Style::default(),
        }]
    };
    let mut facts = vec![format!("{} › {}", board.name, list)];
    if let Some(due) = card.due_date {
        facts.push(format!(
            "{} {}",
            lang.label(Label::Due),
            due.format("%Y-%m-%d")
        ));
    }
    if let Some((done, total)) = card.checklist_progress() {
        facts.push(format!("{} {done}/{total}", lang.label(Label::Checklist)));
    }
    flow.paragraph(
        &muted(facts.join("   ·   ")),
        9.0,
        MARGIN,
        CONTENT_W,
        flow.muted(),
    );
    let dates = format!(
        "{} {}   ·   {} {}   ·   {} {}",
        lang.label(Label::Created),
        stamp(card.created_at),
        lang.label(Label::Updated),
        stamp(card.updated_at),
        lang.label(Label::Status),
        stamp(now)
    );
    flow.paragraph(&muted(dates), 9.0, MARGIN, CONTENT_W, flow.muted());
    flow.y += 6.0;

    let chips: Vec<(String, Rgb)> = card
        .label_ids
        .iter()
        .filter_map(|id| board.labels.iter().find(|l| &l.id == id))
        .map(|l| (l.name.clone(), l.color.to_rgb()))
        .collect();
    if !chips.is_empty() {
        flow.chips(&chips);
    }

    if !doc.blocks.is_empty() {
        flow.description(&doc.blocks);
    }
    if !card.checklist.is_empty() {
        let checklist = |f: &mut Flow<'_>| {
            f.section_title(lang.label(Label::Checklist));
            f.checklist(card);
        };
        flow.keep_together(&checklist);
        checklist(&mut flow);
    }

    let label_names: Vec<String> = chips.into_iter().map(|(name, _)| name).collect();
    (flow.finish(), label_names)
}
