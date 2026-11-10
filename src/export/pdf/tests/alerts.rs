use super::*;

#[test]
fn alerts_draw_a_bar_and_a_capitalized_title_in_their_color_or_black() {
    let (meta, card) = corner_cases()
        .into_iter()
        .find(|(n, ..)| *n == "15-alerts")
        .map(|(_, m, c)| (m, c))
        .unwrap();
    for theme in THEMES {
        let fonts = Fonts::load_for(theme).unwrap();
        let (pages, _) = layout(&fonts, &meta, "L", &card, chrono::Utc::now());
        let ops: Vec<&Op> = pages.iter().flatten().collect();
        let titles: Vec<(&str, Rgb)> = ops
            .iter()
            .filter_map(|op| match op {
                Op::Text {
                    text, style, color, ..
                } if style.bold
                    && text != "DESCRIPTION"
                    && text.chars().all(|c| c.is_ascii_uppercase()) =>
                {
                    Some((text.as_str(), *color))
                }
                _ => None,
            })
            .collect();
        let names: Vec<&str> = titles.iter().map(|t| t.0).collect();
        assert_eq!(
            names,
            ["NOTE", "TIP", "IMPORTANT", "CAUTION", "WARNING", "NOTE"],
            "{theme:?}"
        );
        for ((_, color), kind) in titles.iter().zip([
            Alert::Note,
            Alert::Tip,
            Alert::Important,
            Alert::Caution,
            Alert::Warning,
            Alert::Note,
        ]) {
            assert_eq!(
                *color,
                if theme == Theme::Mono {
                    BLACK
                } else {
                    kind.rgb()
                }
            );
        }
        let icons: Vec<(Alert, Rgb)> = ops
            .iter()
            .filter_map(|op| {
                if let Op::Icon { kind, color, .. } = op {
                    Some((*kind, *color))
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(
            icons.iter().map(|i| i.0).collect::<Vec<_>>(),
            [
                Alert::Note,
                Alert::Tip,
                Alert::Important,
                Alert::Caution,
                Alert::Warning,
                Alert::Note
            ]
        );
        assert!(
            icons
                .iter()
                .all(|(k, c)| *c == if theme == Theme::Mono { BLACK } else { k.rgb() })
        );
        let bars = ops
            .iter()
            .filter(|op| matches!(op, Op::Line { width, .. } if *width == 2.5))
            .count();
        assert!(bars >= 12, "{theme:?}: {bars} bar segments");
        assert!(
            ops.iter()
                .any(|op| matches!(op, Op::Text { text, .. } if text.contains("[!bogus]")))
        );
    }
}

#[test]
fn list_markers_are_ink_except_in_quotes() {
    let meta = board();
    let mut card = Card::new("T".into());
    card.description = "- a\n1. b\n> alert\n\n> [!note]\n> - c\n> 2. d".into();
    for theme in THEMES {
        let fonts = Fonts::load_for(theme).unwrap();
        let ink = if theme == Theme::Mono { BLACK } else { INK };
        let (pages, _) = layout(&fonts, &meta, "L", &card, chrono::Utc::now());
        let color = |m: &str| {
            pages[0].iter().find_map(|op| match op {
                Op::Text { text, color, .. } if text == m => Some(*color),
                _ => None,
            })
        };
        for m in ["•", "1.", "2."] {
            assert_eq!(color(m), Some(ink), "{theme:?} {m}");
        }
    }
}
