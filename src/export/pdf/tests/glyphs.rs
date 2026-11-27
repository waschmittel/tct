//! The PDF fonts are DejaVu because nothing else covers what the demos use
//! (arrows, ✓, ★, Greek/Cyrillic, monochrome emoji) — ADR-0027. These tests
//! pin that: every character of the demo PDFs and of the fixed texts has a
//! glyph; the one case that deliberately has none shows `?`.

use super::*;
use crate::alert::Alert;
use crate::export::lang::{Label, Lang};
use std::collections::BTreeSet;

/// The corner case that exists to show missing glyphs.
const MISSING_CASE: &str = "03-unicode-and-missing-glyphs";

fn faces(fonts: &Fonts) -> [&fonts::Face; 4] {
    [
        &fonts.sans,
        &fonts.bold,
        &fonts.oblique,
        &fonts.bold_oblique,
    ]
}

/// Characters of `text` that `clean` would replace by `?` in any of `faces`.
fn missing_in(faces: &[&fonts::Face], text: &str) -> BTreeSet<char> {
    text.chars()
        .filter(|&c| c != '?' && faces.iter().any(|f| f.clean(&c.to_string()) == "?"))
        .collect()
}

/// Text may be set in any proportional style, so those four faces must all
/// cover it; the mono face only has to cover code (it has less, e.g. no emoji).
fn missing(fonts: &Fonts, text: &str) -> BTreeSet<char> {
    let mut gone = missing_in(&faces(fonts), text);
    gone.extend(missing_in(&[&fonts.mono], &code_of(text)));
    gone
}

/// Fenced blocks and inline spans of Markdown `text`.
fn code_of(text: &str) -> String {
    let mut code = String::new();
    let mut fenced = false;
    for line in text.lines() {
        if line.trim_start().starts_with("```") {
            fenced = !fenced;
        } else if fenced {
            code += line;
        } else {
            code += &line.split('`').skip(1).step_by(2).collect::<String>();
        }
    }
    code
}

/// Everything the card contributes to the page, as typed.
fn card_text(meta: &BoardMeta, card: &Card) -> String {
    let mut parts = vec![
        meta.name.clone(),
        card.title.clone(),
        card.description.clone(),
    ];
    parts.extend(meta.labels.iter().map(|l| l.name.clone()));
    parts.extend(card.checklist.iter().map(|i| i.text.clone()));
    parts.join("\n")
}

#[test]
fn demo_pdfs_only_use_glyphs_the_fonts_have() {
    let fonts = Fonts::load().unwrap();
    for (name, meta, card) in corner_cases() {
        let gone = missing(&fonts, &card_text(&meta, &card));
        if name == MISSING_CASE {
            assert!(
                ["日", "ع", "🧑"]
                    .iter()
                    .all(|s| gone.contains(&s.chars().next().unwrap())),
                "{name} must keep showing characters the fonts lack, got {gone:?}"
            );
        } else {
            assert!(gone.is_empty(), "{name}: no glyph for {gone:?}");
        }
    }
}

#[test]
fn fixed_texts_and_typography_have_glyphs() {
    use Lang::*;
    let fonts = Fonts::load().unwrap();
    let langs = [En, De, Fr, Es, It, Nl, Pt, Sv, Tr];
    // Compile error when a language is added without being listed above.
    let _: fn(Lang) = |l| match l {
        En | De | Fr | Es | It | Nl | Pt | Sv | Tr => {}
    };
    let mut text = String::from("•◦›—–…\u{a0}");
    for lang in langs {
        let q = lang.quotes();
        text += &[q.open2, q.close2, q.open1, q.close1].concat();
        for label in [
            Label::Description,
            Label::Checklist,
            Label::Due,
            Label::Created,
            Label::Updated,
            Label::Status,
        ] {
            text += lang.label(label);
        }
        for alert in Alert::ALL {
            text += &lang.alert_title_upper(alert);
        }
    }
    let gone = missing(&fonts, &text);
    assert!(gone.is_empty(), "no glyph for {gone:?}");
}

#[test]
fn a_character_without_a_glyph_is_drawn_as_question_mark() {
    let fonts = Fonts::load().unwrap();
    assert_eq!(missing(&fonts, "日"), BTreeSet::from(['日']));
    let mut card = Card::new("T".into());
    card.description = "x日y 😀 z".into();
    let pages = laid_out(&card);
    let drawn: Vec<&str> = pages.iter().flat_map(|p| texts(p)).collect();
    assert!(drawn.iter().any(|t| t.contains("x?y")), "{drawn:?}");
    assert!(
        drawn.iter().all(|t| !t.contains('日')),
        "a missing glyph must never reach the PDF"
    );
}
