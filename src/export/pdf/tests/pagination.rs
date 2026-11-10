use super::*;

#[test]
fn a_level_one_section_that_fits_a_page_is_not_split() {
    let mut card = Card::new("T".into());
    card.description = format!(
        "{}\n\n# Section\n\n{}",
        lines("filler", 38),
        lines("body", 30)
    );
    let pages = laid_out(&card);
    let heading = page_of(&pages, "Section");
    assert_eq!(heading, 1, "section should start on a fresh page");
    assert_eq!(page_of(&pages, "body 29"), heading);
}

#[test]
fn oversized_sections_fall_back_to_keeping_their_subsections_whole() {
    let mut card = Card::new("T".into());
    let subs: Vec<String> = (0..6)
        .map(|n| format!("## Sub {n}\n\n{}", lines(&format!("s{n}"), 22)))
        .collect();
    card.description = format!("# Huge\n\n{}", subs.join("\n\n"));
    let pages = laid_out(&card);
    assert_eq!(
        page_of(&pages, "Huge"),
        0,
        "an oversized section starts where it is"
    );
    for n in 0..6 {
        assert_eq!(
            page_of(&pages, &format!("Sub {n}")),
            page_of(&pages, &format!("s{n} 21")),
            "Sub {n} is split"
        );
    }
}

#[test]
fn checklist_moves_to_a_fresh_page_when_it_would_not_fit() {
    let mut card = Card::new("T".into());
    card.description = lines("filler", 30);
    card.checklist = (0..12)
        .map(|i| ChecklistItem {
            text: format!("item {i}"),
            completed: false,
        })
        .collect();
    let pages = laid_out(&card);
    assert_eq!(page_of(&pages, "CHECKLIST"), 1);
    assert_eq!(page_of(&pages, "item 11"), 1);
}

#[test]
fn checklist_stays_when_it_fits() {
    let mut card = Card::new("T".into());
    card.description = lines("filler", 5);
    card.checklist = vec![ChecklistItem {
        text: "one".into(),
        completed: false,
    }];
    assert_eq!(laid_out(&card).len(), 1);
}

#[test]
fn footer_shows_the_location_and_later_pages_get_a_titled_header_with_the_status_date() {
    let fonts = Fonts::load().unwrap();
    let mut card = Card::new("My Card".into());
    card.description = "x".into();
    let created = "Status 2026-01-02 03:04";
    let first = page_chrome(&fonts, &card, "Board › List", created, 0, 2);
    let second = page_chrome(&fonts, &card, "Board › List", created, 1, 2);
    assert!(texts(&first).contains(&"Board › List"));
    assert!(
        !texts(&first).iter().any(|t| t.contains("My Card")),
        "title is not repeated in the footer"
    );
    assert!(
        !texts(&first).iter().any(|t| t.contains("Status")),
        "no header on page 1"
    );
    assert!(texts(&second).contains(&"My Card"));
    assert!(texts(&second).contains(&created));
    assert!(texts(&second).contains(&"2 / 2"));
}

#[test]
fn first_page_intro_states_the_status_date() {
    let mut card = Card::new("T".into());
    card.description = "x".into();
    let pages = laid_out(&card);
    assert!(texts(&pages[0]).iter().any(|t| t.contains("Status")));
}
