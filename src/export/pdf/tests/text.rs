use super::*;

#[test]
fn short_code_blocks_and_paragraphs_are_kept_whole() {
    let mut card = Card::new("T".into());
    card.description = format!(
        "{}\n\n```\n{}\n```\n\n{}",
        lines("filler", 38),
        lines("code", 8),
        lines("para", 6)
    );
    let pages = laid_out(&card);
    assert_eq!(page_of(&pages, "code 0"), page_of(&pages, "code 7"));
    assert_eq!(page_of(&pages, "para 0"), page_of(&pages, "para 5"));
}

#[test]
fn long_paragraphs_never_leave_a_single_line_behind() {
    let mut card = Card::new("T".into());
    for filler in 30..60 {
        card.description = format!("{}\n\n{}", lines("filler", filler), lines("para", 30));
        let pages = laid_out(&card);
        let on_first = texts(&pages[0])
            .iter()
            .filter(|t| t.starts_with("para"))
            .count();
        assert_ne!(on_first, 1, "orphaned line with {filler} filler lines");
        assert_ne!(on_first, 29, "widow line with {filler} filler lines");
    }
}

#[test]
fn long_code_lines_wrap_instead_of_being_clipped() {
    let mut card = Card::new("T".into());
    card.description = format!("```\n{}END\n```", "abcdefghij".repeat(40));
    let pages = laid_out(&card);
    let drawn: String = texts(&pages[0]).concat();
    assert!(
        drawn.contains(&"abcdefghij".repeat(40)) && drawn.contains("END"),
        "code text was lost"
    );
}

#[test]
fn crowded_tables_shrink_padding_and_type_before_breaking_words() {
    let (_, meta, card) = corner_cases()
        .into_iter()
        .find(|(n, _, _)| n.contains("wide-table"))
        .unwrap();
    let fonts = Fonts::load().unwrap();
    let (pages, _) = layout(&fonts, &meta, "L", &card, chrono::Utc::now());
    assert_eq!(count(&pages[0], "Column"), 10, "a header word was split");
}

#[test]
fn wide_list_markers_do_not_touch_the_text() {
    let mut card = Card::new("T".into());
    card.description = "99. ninety-nine\n100. hundred".into();
    let pages = laid_out(&card);
    let x_of = |needle: &str| {
        pages[0].iter().find_map(|op| match op {
            Op::Text { text, x, .. } if text == needle => Some(*x),
            _ => None,
        })
    };
    let fonts = Fonts::load().unwrap();
    let marker_end = x_of("100.").unwrap() + fonts.sans.width("100.", BODY);
    assert!(x_of("hundred").unwrap() >= marker_end + 3.0);
}

fn german_card(description: &str) -> Card {
    let mut card = Card::new("Ein Titel für die Beschreibung der Aufgabe".into());
    card.description = description.into();
    assert_eq!(Lang::of(&card), Lang::De);
    card
}

#[test]
fn fixed_texts_follow_the_detected_language() {
    let mut de =
        german_card("Das ist ein deutscher Text, der lang genug ist, um erkannt zu werden.");
    de.checklist.push(ChecklistItem {
        text: "Eins".into(),
        completed: false,
    });
    let all: Vec<_> = laid_out(&de).into_iter().flatten().collect();
    let all = texts(&all).join("|");
    for word in ["BESCHREIBUNG", "CHECKLISTE", "Checkliste 0/1"] {
        assert!(all.contains(word), "{word} missing in {all}");
    }
    assert!(!all.contains("DESCRIPTION") && !all.contains("Checklist 0"));
    let mut en = Card::new("Title".into());
    en.description = "Body".into();
    let all: Vec<_> = laid_out(&en).into_iter().flatten().collect();
    assert!(texts(&all).join("|").contains("DESCRIPTION"));
}
