use super::*;

#[test]
fn output_is_a_pdf_a_2b_document() {
    let mut card = Card::new("Größe prüfen ✓ 😀".into());
    card.description =
        "# Plan\n\n- eins\n  - zwei\n> zitat\n\n```\ncode\n```\n\n| a | b |\n|---|--:|\n| 1 | 2 |"
            .into();
    card.checklist = vec![
        ChecklistItem {
            text: "offen".into(),
            completed: false,
        },
        ChecklistItem {
            text: "erledigt".into(),
            completed: true,
        },
    ];
    let pdf = render(&board(), "To Do", &card, Theme::Color).unwrap();
    assert!(pdf.starts_with(b"%PDF-"));
    let text = String::from_utf8_lossy(&pdf);
    assert!(
        text.contains("pdfaid:part"),
        "XMP must declare the PDF/A part"
    );
    assert!(text.contains("<pdfaid:conformance>B"), "PDF/A level B");
}

#[test]
fn long_cards_paginate() {
    let mut card = Card::new("Long".into());
    card.description = (0..300)
        .map(|i| format!("line {i}"))
        .collect::<Vec<_>>()
        .join("\n");
    let fonts = Fonts::load().unwrap();
    let (pages, _) = layout(&fonts, &board(), "L", &card, chrono::Utc::now());
    assert!(pages.len() >= 5, "got {} pages", pages.len());
}

#[test]
fn unbreakable_words_and_wide_tables_do_not_overflow() {
    let mut card = Card::new("x".repeat(400));
    card.description = format!(
        "{}\n\n| {} | b |\n|---|---|\n| c | d |",
        "y".repeat(500),
        "z".repeat(300)
    );
    render(&board(), "L", &card, Theme::Color).unwrap();
}

#[test]
fn wide_tables_keep_short_headers_whole_and_honour_alignment() {
    let mut card = Card::new("T".into());
    card.description = "| Left | Center | Right | Default |\n|:--|:-:|--:|---|\n| a | b | c | d |\n| x | a very long centered cell text that must wrap inside a narrow column because the table is wide | 12345 | y |".into();
    let fonts = Fonts::load().unwrap();
    let (pages, _) = layout(&fonts, &board(), "L", &card, chrono::Utc::now());
    let x_of = |needle: &str| {
        pages[0].iter().find_map(|op| match op {
            Op::Text { text, x, .. } if text == needle => Some(*x),
            _ => None,
        })
    };
    for header in ["Right", "Default"] {
        assert!(x_of(header).is_some(), "{header} was split across lines");
    }
    let (c, five) = (x_of("c").unwrap(), x_of("12345").unwrap());
    let width_c = fonts.sans.width("c", 9.5);
    let width_five = fonts.sans.width("12345", 9.5);
    assert!(
        (c + width_c - (five + width_five)).abs() < 0.5,
        "right-aligned cells share a right edge"
    );
}

#[test]
fn empty_card_renders() {
    render(&board(), "L", &Card::new(String::new()), Theme::Color).unwrap();
}
