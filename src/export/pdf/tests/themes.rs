use super::*;

#[test]
fn mono_uses_only_black_and_grays() {
    let fonts = Fonts::load_for(Theme::Mono).unwrap();
    let gray = |c: Rgb| c == BLACK || c == INK || c == MUTED || c == RULE || c == TINT;
    for (name, meta, card) in corner_cases() {
        let (pages, _) = layout(&fonts, &meta, "L", &card, chrono::Utc::now());
        for (n, page) in pages.iter().enumerate() {
            let chrome = page_chrome(
                &fonts,
                &card,
                "B › L",
                "Status 2026-01-01 00:00",
                n,
                pages.len(),
            );
            for op in page.iter().chain(&chrome) {
                match op {
                    Op::Text { color, .. } | Op::Line { color, .. } | Op::Icon { color, .. } => {
                        assert!(gray(*color), "{name} p{n}: {color:?}")
                    }
                    Op::Rect { fill, stroke, .. } => {
                        assert!(fill.is_none_or(|c| c == TINT), "{name} p{n}: colored fill");
                        assert!(stroke.is_none_or(gray), "{name} p{n}: colored outline");
                    }
                }
            }
        }
    }
}

#[test]
fn mono_keeps_the_modern_layout_so_only_colour_differs() {
    let modern = Fonts::load_for(Theme::Color).unwrap();
    let mono = Fonts::load_for(Theme::Mono).unwrap();
    for (name, meta, card) in corner_cases() {
        let now = chrono::Utc::now();
        let place = |fonts: &Fonts| -> Vec<(usize, String, i32, i32)> {
            layout(fonts, &meta, "L", &card, now)
                .0
                .iter()
                .enumerate()
                .flat_map(|(n, page)| {
                    page.iter().filter_map(move |op| match op {
                        Op::Text { text, x, y, .. } => {
                            Some((n, text.clone(), (*x * 10.0) as i32, (*y * 10.0) as i32))
                        }
                        _ => None,
                    })
                })
                .collect()
        };
        assert_eq!(place(&modern), place(&mono), "{name}: text moved");
    }
}

#[test]
fn mono_outlines_labels_but_tints_code_gray() {
    let mut card = Card::new("T".into());
    card.description = "`inline`\n\n```\none\n```\n\n> quoted".into();
    let mut meta = board();
    meta.labels.push(crate::model::label::Label::new(
        "urgent".into(),
        crate::model::label::LabelColor::Red,
    ));
    card.label_ids.push(meta.labels[0].id.clone());
    let fonts = Fonts::load_for(Theme::Mono).unwrap();
    let (pages, _) = layout(&fonts, &meta, "L", &card, chrono::Utc::now());
    let outlined = pages[0]
        .iter()
        .filter(|op| {
            matches!(
                op,
                Op::Rect {
                    stroke: Some(_),
                    fill: None,
                    ..
                }
            )
        })
        .count();
    assert_eq!(outlined, 1, "only the label chip is outlined");
    let tinted = pages[0]
        .iter()
        .filter(|op| {
            matches!(
                op,
                Op::Rect {
                    fill: Some(TINT),
                    ..
                }
            )
        })
        .count();
    assert_eq!(tinted, 2, "inline code and the code block");
    let gray_quote = pages[0].iter().any(
        |op| matches!(op, Op::Text { text, color, .. } if text == "quoted" && *color == MUTED),
    );
    assert!(gray_quote, "quotes are gray");
}
