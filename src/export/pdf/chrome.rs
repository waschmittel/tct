use chrono::Local;

use super::*;
use crate::export::blocks::Style;
use crate::model::card::Card;

pub(super) fn fit(face: &Face, text: &str, size: f32, max: f32) -> String {
    let mut text = face.clean(text);
    if face.width(&text, size) <= max {
        return text;
    }
    while face.width(&text, size) + face.width("…", size) > max && text.pop().is_some() {}
    format!("{}…", text.trim_end())
}

pub(super) fn stamp(t: chrono::DateTime<chrono::Utc>) -> String {
    t.with_timezone(&Local).format("%Y-%m-%d %H:%M").to_string()
}

/// Running header (from the second page on) and footer of page `n` of `total`.
pub(super) fn page_chrome(
    fonts: &Fonts,
    card: &Card,
    location: &str,
    created: &str,
    n: usize,
    total: usize,
) -> Vec<Op> {
    let (muted, rule) = (MUTED, RULE);
    let small = |x: f32, y: f32, text: String| Op::Text {
        x,
        y,
        text,
        style: Style::default(),
        size: 8.0,
        color: muted,
    };
    let right_aligned = |text: &str| PAGE_W - MARGIN - fonts.sans.width(text, 8.0);
    let mut chrome = Vec::new();
    if n > 0 {
        let created_x = right_aligned(created);
        let room = created_x - MARGIN - 16.0;
        chrome.push(small(
            MARGIN,
            HEADER_BASELINE,
            fit(&fonts.sans, &card.title, 8.0, room),
        ));
        chrome.push(small(created_x, HEADER_BASELINE, created.to_string()));
        chrome.push(Op::Line {
            x1: MARGIN,
            y1: HEADER_BASELINE + 5.0,
            x2: PAGE_W - MARGIN,
            y2: HEADER_BASELINE + 5.0,
            color: rule,
            width: 0.6,
        });
    }
    let counter = format!("{} / {}", n + 1, total);
    let room = CONTENT_W - fonts.sans.width(&counter, 8.0) - 16.0;
    chrome.push(small(
        MARGIN,
        FOOTER_BASELINE,
        fit(&fonts.sans, location, 8.0, room),
    ));
    chrome.push(small(right_aligned(&counter), FOOTER_BASELINE, counter));
    chrome
}
