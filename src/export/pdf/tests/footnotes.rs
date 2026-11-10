use super::*;

// ── Footnotes ────────────────────────────────────────────────────

/// Size of a reference in the text and of the marker in front of a note.
pub(super) const MARKER_SIZE: f32 = BODY * SUP_SCALE;

/// Is this op the marker in front of a note (a raised run at the left margin
/// inside the footnote area)?
pub(super) fn is_marker(page: &[Op], op: &Op) -> bool {
    matches!(op, Op::Text { x, y, style, .. }
            if style.sup && *x == MARGIN && note_area_top(page).is_some_and(|top| *y > top))
}

/// Baseline of the marker `label` of a note.
pub(super) fn marker_y(page: &[Op], label: &str) -> f32 {
    page.iter()
        .find_map(|op| match op {
            Op::Text { text, y, .. } if is_marker(page, op) && *text == format!("[{label}]") => {
                Some(*y)
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("no marker {label}"))
}

/// Markers of the references drawn in the text of `page`.
pub(super) fn refs_on(page: &[Op]) -> Vec<String> {
    page.iter()
        .filter_map(|op| match op {
            Op::Text { text, style, .. } if style.sup && !is_marker(page, op) => {
                // The reference in the text is bracketed: `[1]`.
                Some(text.trim_matches(['[', ']']).to_string())
            }
            _ => None,
        })
        .collect()
}

/// Markers of the footnotes that start on `page`, in drawing order.
pub(super) fn markers_on(page: &[Op]) -> Vec<String> {
    page.iter()
        .filter_map(|op| match op {
            Op::Text { text, .. } if is_marker(page, op) => {
                Some(text.trim_matches(['[', ']']).to_string())
            }
            _ => None,
        })
        .collect()
}

/// Baseline and x of the first text `text` drawn at `size`.
pub(super) fn text_at(page: &[Op], text: &str, size: f32) -> (f32, f32) {
    page.iter()
        .find_map(|op| match op {
            Op::Text {
                text: t,
                x,
                y,
                size: s,
                ..
            } if t == text && *s == size => Some((*y, *x)),
            _ => None,
        })
        .unwrap_or_else(|| panic!("{text} not drawn at {size}"))
}

/// Top of the footnote area: the short rule above it.
pub(super) fn note_area_top(page: &[Op]) -> Option<f32> {
    page.iter().find_map(|op| match op {
        Op::Line { x1, x2, y1, .. } if (x2 - x1 - CONTENT_W / 4.0).abs() < 0.01 => Some(*y1),
        _ => None,
    })
}

/// The invariants of footnote layout, for every page: each reference has
/// its note on the same page (`whole`: no note is long enough to carry
/// over), notes are in order, and the footnote area never overlaps the body.
pub(super) fn assert_notes_consistent(pages: &[Vec<Op>], whole: bool, name: &str) {
    for (n, page) in pages.iter().enumerate() {
        let markers = markers_on(page);
        let top = note_area_top(page);
        assert_eq!(
            top.is_some(),
            !markers.is_empty() || {
                // A page of continuation lines has an area but no marker.
                top.is_some()
            },
            "{name} p{n}"
        );
        if whole {
            for r in refs_on(page) {
                assert!(
                    markers.contains(&r),
                    "{name} p{n}: note {r} is not on its page"
                );
            }
        }
        assert!(
            markers
                .iter()
                .all(|m| markers.iter().filter(|o| *o == m).count() == 1),
            "{name} p{n}: notes out of order"
        );
        if let Some(top) = top {
            for op in page {
                if let Op::Text { y, size, style, .. } = op
                    && (*size == BODY || (style.sup && !is_marker(page, op)))
                {
                    assert!(*y < top, "{name} p{n}: body text runs into the notes");
                }
            }
        }
    }
}

pub(super) fn note_card(description: String) -> Card {
    let mut card = Card::new("T".into());
    card.description = description;
    card
}

#[test]
fn a_note_is_on_the_page_of_its_reference_wherever_it_is_defined() {
    for definition_first in [true, false] {
        let body = format!("{}\n\nsee[^a] here\n", lines("filler", 60));
        let def = "[^a]: The note text.";
        let text = if definition_first {
            format!("{def}\n\n{body}")
        } else {
            format!("{body}\n{def}")
        };
        let pages = laid_out(&note_card(text));
        assert!(pages.len() >= 2);
        let reference = page_of(&pages, "here");
        assert!(reference > 0);
        assert_eq!(page_of(&pages, "The note text."), reference);
        assert_eq!(refs_on(&pages[reference]), ["a"]);
        assert_notes_consistent(&pages, true, "placement");
    }
}

#[test]
fn the_reference_never_ends_up_apart_from_its_note_at_a_page_boundary() {
    // Sweep the reference line across the bottom of page one.
    let (mut first, mut second) = (false, false);
    for filler in 38..=62 {
        let text = format!(
            "{}\n\nlast line[^1]\n\n[^1]: Boundary note.",
            lines("filler", filler)
        );
        let pages = laid_out(&note_card(text));
        assert_notes_consistent(&pages, true, &format!("filler {filler}"));
        let page = page_of(&pages, "last line");
        assert_eq!(page_of(&pages, "Boundary note."), page, "filler {filler}");
        first |= page == 0;
        second |= page == 1;
    }
    assert!(first && second, "the sweep must cover both outcomes");
}

#[test]
fn named_notes_keep_their_name_and_are_drawn_in_order_of_first_reference() {
    let pages = laid_out(&note_card(
        "[^alpha]: Alpha\n[^b]: Beta\n\nx[^b] y[^alpha] z[^b]\n".into(),
    ));
    assert_eq!(refs_on(&pages[0]), ["b", "alpha", "b"]);
    assert_eq!(markers_on(&pages[0]), ["b", "alpha"]);
    let y = |t: &str| text_at(&pages[0], t, NOTE).0;
    let marker = |t: &str| marker_y(&pages[0], t);
    assert!((y("Beta") - marker("b") - BODY * 0.35).abs() < 0.01);
    assert!(y("Beta") < marker("alpha"));
    assert!((y("Alpha") - marker("alpha") - BODY * 0.35).abs() < 0.01);
}

#[test]
fn numbered_notes_count_up_by_first_reference_whatever_their_labels() {
    let pages = laid_out(&note_card(
        "[^3]: Three\n[^7]: Seven\n[^x]: Named\n\na[^7] b[^x] c[^3] d[^7]\n".into(),
    ));
    assert_eq!(refs_on(&pages[0]), ["1", "x", "2", "1"]);
    assert_eq!(markers_on(&pages[0]), ["1", "x", "2"]);
    let shown = texts(&pages[0]);
    let at = |t: &str| shown.iter().position(|s| *s == t).unwrap();
    assert!(at("Seven") < at("Named") && at("Named") < at("Three"));
}

#[test]
fn references_in_the_text_and_markers_in_front_of_notes_are_both_bracketed() {
    let pages = laid_out(&note_card("a[^1] b[^name]\n\n[^1]: A\n[^name]: B".into()));
    let page = &pages[0];
    let raised = |marker: bool| -> Vec<&str> {
        page.iter()
            .filter_map(|op| match op {
                Op::Text { text, style, .. } if style.sup && is_marker(page, op) == marker => {
                    Some(text.as_str())
                }
                _ => None,
            })
            .collect()
    };
    assert_eq!(raised(false), ["[1]", "[name]"]);
    assert_eq!(raised(true), ["[1]", "[name]"]);
}

#[test]
fn markers_are_raised_black_and_without_a_dot() {
    for theme in THEMES {
        let fonts = Fonts::load_for(theme).unwrap();
        let card = note_card("x[^a] y[^2]\n\n[^a]: A\n[^2]: B".into());
        let (pages, _) = layout(&fonts, &board(), "L", &card, chrono::Utc::now());
        let markers: Vec<(&str, Rgb)> = pages[0]
            .iter()
            .filter_map(|op| match op {
                Op::Text {
                    text,
                    size,
                    style,
                    color,
                    ..
                } if is_marker(&pages[0], op) && *size == MARKER_SIZE && style.sup => {
                    Some((text.as_str(), *color))
                }
                _ => None,
            })
            .collect();
        assert_eq!(markers, [("[a]", BLACK), ("[1]", BLACK)], "{theme:?}");
        assert!(
            !texts(&pages[0])
                .iter()
                .any(|t| t.ends_with('.') && t.len() <= 2)
        );
    }
}

#[test]
fn all_note_text_shares_one_indent_set_by_the_longest_marker() {
    let long = "a-rather-long-footnote-name";
    let text = format!("a[^1] b[^{long}] c[^2]\n\n[^1]: One\n[^{long}]: Long\n[^2]: Two");
    let pages = laid_out(&note_card(text));
    let xs: Vec<f32> = ["One", "Long", "Two"]
        .iter()
        .map(|t| text_at(&pages[0], t, NOTE).1)
        .collect();
    assert!(xs.windows(2).all(|w| (w[0] - w[1]).abs() < 0.01), "{xs:?}");
    let fonts = Fonts::load().unwrap();
    let marker_end = MARGIN + fonts.sans.width(&format!("[{long}]"), MARKER_SIZE);
    assert!(xs[0] > marker_end, "text starts behind the longest marker");
    // Short markers alone need a much smaller indent.
    let pages = laid_out(&note_card("a[^1]\n\n[^1]: One".into()));
    assert!(text_at(&pages[0], "One", NOTE).1 < xs[0] - 50.0);
}

#[test]
fn each_page_indents_by_its_own_longest_marker() {
    let long = "name-long-enough";
    let text = format!(
        "a[^{long}]\n\n{}\n\nb[^1]\n\n[^{long}]: Alpha\n[^1]: Beta",
        lines("filler", 60)
    );
    let pages = laid_out(&note_card(text));
    let (a, b) = (page_of(&pages, "Alpha"), page_of(&pages, "Beta"));
    assert_ne!(a, b);
    let (long_x, short_x) = (
        text_at(&pages[a], "Alpha", NOTE).1,
        text_at(&pages[b], "Beta", NOTE).1,
    );
    let fonts = Fonts::load().unwrap();
    assert!(long_x > MARGIN + fonts.sans.width(&format!("[{long}]"), MARKER_SIZE));
    assert!(short_x < long_x - 40.0, "{short_x} vs {long_x}");
}

#[test]
fn notes_are_set_apart_and_the_rule_is_visible() {
    let pages = laid_out(&note_card("a[^1] b[^2]\n\n[^1]: A1  \nA2\n[^2]: B".into()));
    let y = |t: &str| text_at(&pages[0], t, NOTE).0;
    assert!((y("A2") - y("A1") - NOTE_LINE).abs() < 0.01);
    assert!((y("B") - y("A2") - NOTE_LINE - NOTE_BETWEEN).abs() < 0.01);
    let rule = pages[0].iter().find_map(|op| match op {
        Op::Line { color, x1, x2, .. } if (x2 - x1 - CONTENT_W / 4.0).abs() < 0.01 => Some(*color),
        _ => None,
    });
    assert_eq!(rule, Some(MUTED));
}

#[test]
fn markers_are_as_large_as_references_and_sit_on_the_text_baseline() {
    let pages = laid_out(&note_card("a[^1]\n\n[^1]: Note".into()));
    let sizes: Vec<f32> = pages[0]
        .iter()
        .filter_map(|op| match op {
            Op::Text { style, size, .. } if style.sup => Some(*size),
            _ => None,
        })
        .collect();
    assert_eq!(sizes, [MARKER_SIZE, MARKER_SIZE]);
    // Raised by the same fraction as in the text (the baseline lift is 0.35 em).
    let lift = text_at(&pages[0], "Note", NOTE).0 - marker_y(&pages[0], "1");
    assert!((lift - BODY * 0.35).abs() < 0.01, "{lift}");
}

#[test]
fn multi_line_notes_keep_their_lines_and_unreferenced_ones_are_dropped() {
    let pages = laid_out(&note_card(
        "a[^m]\n\n[^m]: First line  \nsecond line\n[^dead]: unreferenced".into(),
    ));
    let shown = texts(&pages[0]);
    assert!(shown.contains(&"First line") && shown.contains(&"second line"));
    assert!(!shown.iter().any(|t| t.contains("unreferenced")));
    assert!(!shown.iter().any(|t| t.contains("[^")));
    // The definitions left no gap or text in the body.
    assert_eq!(count(&pages[0], "a"), 1);
}

#[test]
fn an_indented_paragraph_after_a_blank_line_belongs_to_the_note() {
    let pages = laid_out(&note_card("a[^1]\n\n[^1]: one\n\n    two\n\nafter".into()));
    let shown = texts(&pages[0]);
    assert!(shown.contains(&"two") && shown.contains(&"after"));
    let top = note_area_top(&pages[0]).unwrap();
    let y = |t: &str| {
        pages[0]
            .iter()
            .find_map(|op| match op {
                Op::Text { text, y, .. } if text == t => Some(*y),
                _ => None,
            })
            .unwrap()
    };
    assert!(y("two") > top && y("after") < top);
}

#[test]
fn unresolved_references_and_code_stay_literal() {
    let pages = laid_out(&note_card(
        "missing[^zz] and `code[^1]`\n```\n[^1]: not a definition\n```\nreal[^1]\n\n[^1]: Real"
            .into(),
    ));
    let shown = texts(&pages[0]).join("|");
    assert!(shown.contains("missing[^zz]"));
    assert!(shown.contains("code[^1]"));
    assert!(shown.contains("[^1]: not a definition"));
    assert_eq!(refs_on(&pages[0]), ["1"]);
    assert_eq!(markers_on(&pages[0]), ["1"]);
}

#[test]
fn a_note_referenced_on_several_pages_is_repeated_on_each() {
    let text = format!(
        "[^r]: Repeated\n\nfirst[^r]\n\n{}\n\nsecond[^r]\n",
        lines("filler", 60)
    );
    let pages = laid_out(&note_card(text));
    let with_ref: Vec<usize> = (0..pages.len())
        .filter(|&p| !refs_on(&pages[p]).is_empty())
        .collect();
    assert_eq!(with_ref.len(), 2);
    for p in with_ref {
        assert_eq!(markers_on(&pages[p]), ["r"]);
        assert_eq!(count(&pages[p], "Repeated"), 1);
    }
    assert_notes_consistent(&pages, true, "repeat");
}

#[test]
fn references_in_headings_lists_quotes_alerts_and_tables_get_their_notes() {
    let text = "# Head[^1]\n- item[^2]\n> quote[^3]\n> [!note]\n> alert[^4]\n\n| H[^5] | x |\n|---|---|\n| c[^6] | y |\n\n\
                    [^1]: n1\n[^2]: n2\n[^3]: n3\n[^4]: n4\n[^5]: n5\n[^6]: n6\n";
    let pages = laid_out(&note_card(text.into()));
    let expected = ["1", "2", "3", "4", "5", "6"];
    assert_eq!(refs_on(&pages[0]), expected);
    assert_eq!(markers_on(&pages[0]), expected);
    assert_notes_consistent(&pages, true, "blocks");
}

#[test]
fn a_long_table_repeats_header_references_and_keeps_row_notes_on_their_page() {
    let mut text = String::from("| H[^h] | x |\n|---|---|\n");
    for i in 0..90 {
        text.push_str(&format!("| row {i}[^{}] | v |\n", i % 7));
    }
    for label in std::iter::once("h".to_string()).chain((0..7).map(|i| i.to_string())) {
        text.push_str(&format!("\n[^{label}]: Note {label}"));
    }
    let pages = laid_out(&note_card(text));
    assert!(pages.len() >= 2);
    assert_notes_consistent(&pages, true, "table");
    for page in &pages {
        // The repeated header carries its reference, so its note is there too.
        assert!(refs_on(page).contains(&"h".to_string()));
    }
}

#[test]
fn a_note_longer_than_its_page_budget_continues_on_the_next_pages() {
    let long: String = (0..90).map(|i| format!("\nnote line {i}")).collect();
    let text = format!(
        "[^1]: start{long}\n\nref[^1] and after\n\n{}",
        lines("body", 70)
    );
    let pages = laid_out(&note_card(text));
    assert_notes_consistent(&pages, false, "long");
    assert!(pages.len() >= 3);
    // Every line is drawn once, in order, and only the first part has the marker.
    let mut seen = Vec::new();
    for page in &pages {
        for t in texts(page) {
            if let Some(n) = t.strip_prefix("note line ") {
                seen.push(n.parse::<usize>().unwrap());
            }
        }
    }
    assert_eq!(seen, (0..90).collect::<Vec<_>>());
    let marker_pages: Vec<usize> = (0..pages.len())
        .filter(|&p| markers_on(&pages[p]).contains(&"1".to_string()))
        .collect();
    assert_eq!(marker_pages, [page_of(&pages, "ref")]);
    // The area never takes more than its budget.
    for page in &pages {
        if let Some(top) = note_area_top(page) {
            assert!(BOTTOM - top <= NOTE_BUDGET + 1.0);
        }
    }
}

#[test]
fn a_note_that_ends_the_document_on_a_carried_page_is_still_drawn() {
    let long: String = (0..60).map(|i| format!("\nend line {i}")).collect();
    let pages = laid_out(&note_card(format!("ref[^1]\n\n[^1]: head{long}")));
    let drawn: usize = pages
        .iter()
        .map(|p| {
            texts(p)
                .iter()
                .filter(|t| t.starts_with("end line"))
                .count()
        })
        .sum();
    assert_eq!(drawn, 60);
    assert!(pages.iter().all(|p| !p.is_empty()));
}

#[test]
fn many_references_in_one_paragraph_never_overflow_the_page() {
    let refs: String = (0..12).map(|i| format!("w{i}[^{i}] ")).collect();
    let defs: String = (0..12)
        .map(|i| format!("\n[^{i}]: Note {i} with a few words so that it takes a little room"))
        .collect();
    let pages = laid_out(&note_card(format!("{refs}\n{defs}")));
    assert_notes_consistent(&pages, true, "many");
    assert_eq!(notes_drawn(&pages, 12), 12);
}

pub(super) fn notes_drawn(pages: &[Vec<Op>], n: usize) -> usize {
    (0..n)
        .filter(|i| {
            pages.iter().any(|p| {
                texts(p)
                    .iter()
                    .any(|t| t.starts_with(&format!("Note {i} ")))
            })
        })
        .count()
}

#[test]
fn more_notes_than_a_page_budget_spill_onto_following_pages_without_loss() {
    let refs: String = (0..40).map(|i| format!("w{i}[^{i}] ")).collect();
    let defs: String = (0..40)
        .map(|i| format!("\n[^{i}]: Note {i} with a few words so that it takes a little room"))
        .collect();
    let pages = laid_out(&note_card(format!("{refs}\n{defs}")));
    assert_notes_consistent(&pages, false, "spill");
    assert_eq!(notes_drawn(&pages, 40), 40);
    assert!(pages.len() >= 2);
}

#[test]
fn a_page_of_only_definitions_leaves_no_description() {
    let pages = laid_out(&note_card("[^1]: lonely".into()));
    assert_eq!(pages.len(), 1);
    assert!(!texts(&pages[0]).iter().any(|t| t.contains("lonely")));
    assert!(
        !texts(&pages[0])
            .iter()
            .any(|t| t.eq_ignore_ascii_case("description"))
    );
}

#[test]
fn references_are_raised_and_smaller_than_the_text() {
    let pages = laid_out(&note_card("word[^1]\n\n[^1]: n".into()));
    let (mut text_y, mut ref_y, mut ref_size) = (0.0, 0.0, 0.0);
    for op in &pages[0] {
        if let Op::Text {
            text,
            y,
            size,
            style,
            ..
        } = op
        {
            if text == "word" {
                text_y = *y;
            } else if style.sup && text == "[1]" && !is_marker(&pages[0], op) {
                (ref_y, ref_size) = (*y, *size);
            }
        }
    }
    assert!(ref_y < text_y && ref_size < BODY);
}

#[test]
fn footnote_pdfs_stay_deterministic_across_themes() {
    let (meta, card) = corner_cases()
        .into_iter()
        .find(|(n, ..)| *n == "16-footnotes")
        .map(|(_, m, c)| (m, c))
        .unwrap();
    let positions = |theme| {
        let fonts = Fonts::load_for(theme).unwrap();
        let (pages, _) = layout(&fonts, &meta, "L", &card, chrono::Utc::now());
        assert_notes_consistent(&pages, true, "example");
        pages
            .iter()
            .map(|p| {
                p.iter()
                    .filter_map(|op| match op {
                        Op::Text { text, x, y, .. } => Some((text.clone(), *x as i32, *y as i32)),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(positions(Theme::Mono), positions(Theme::Color));
}
