use super::*;

/// Awkward inputs, each rendered as an example PDF and checked for
/// validity and layout invariants. Names become the example file names.
pub(super) fn corner_cases() -> Vec<(&'static str, BoardMeta, Card)> {
    use crate::model::label::{Label, LabelColor};

    let plain = |title: &str, description: String| {
        let mut card = Card::new(title.into());
        card.description = description;
        card
    };
    let mut meta = board();
    let colors = [
        LabelColor::Red,
        LabelColor::Blue,
        LabelColor::Green,
        LabelColor::Purple,
    ];
    for i in 0..24 {
        meta.labels.push(Label::new(
            format!("label number {i}"),
            colors[i % colors.len()],
        ));
    }

    let mut cases = Vec::new();
    let mut showcase = plain("Release 2.0 — planning overview", SHOWCASE.into());
    showcase.label_ids = meta.labels.iter().take(3).map(|l| l.id.clone()).collect();
    showcase.due_date = chrono::NaiveDate::from_ymd_opt(2030, 6, 30);
    showcase.checklist = [
        ("Freeze the feature list", true),
        ("Write the migration guide", true),
        ("Run the load test", false),
        ("Tag the release", false),
    ]
    .into_iter()
    .map(|(text, completed)| ChecklistItem {
        text: text.into(),
        completed,
    })
    .collect();
    cases.push(("00-showcase", meta.clone(), showcase));
    cases.push(("01-empty-card", meta.clone(), Card::new(String::new())));
    cases.push((
        "02-very-long-title",
        meta.clone(),
        plain(
            &"An exceedingly long card title that has to wrap over several lines ".repeat(6),
            "Short.".into(),
        ),
    ));
    cases.push((
            "03-unicode-and-missing-glyphs",
            meta.clone(),
            plain(
                "Größe, 日本語 and emoji 😀",
                "Latin: äöüß ÄÖÜ € — “quotes” … ✓ → ★\n\nThree dots... become an ellipsis, but `code...` and\n\n```\nfenced...\n```\n\nstay literal.\n\nMissing from the fonts (shown as ?): 日本語 العربية 🧑\u{200d}💻\n\ntab\there and a control\u{7}char".into(),
            ),
        ));
    cases.push((
            "03b-german-quotes-and-alert-titles",
            meta.clone(),
            plain(
                "Anführungszeichen und Hinweise",
                "Er sagte \"Hallo\" und 'tschüss', aber nicht \"**fett**\" oder `\"Code\"`. Das ist der Tag, an dem wir ihn nicht mehr gesehen haben, und es war schön.\n\n> [!warning]\n> Das ist eine \"Warnung\" für alle Leser.\n\n> [!tip]\n> Ein Tipp: \"so\" geht es.\n\n```\nlet s = \"unverändert\";\n```".into(),
            ),
        ));
    cases.push((
            "03c-french-guillemets",
            meta.clone(),
            plain(
                "Guillemets français",
                "Il a dit \"bonjour\" et nous sommes partis, car c'est la vie que nous avons choisie ensemble avec les enfants.\n\n> [!note]\n> Une \"remarque\" pour tout le monde dans la maison.".into(),
            ),
        ));
    cases.push((
        "03d-spanish-guillemets",
        meta.clone(),
        plain(
            "Comillas españolas",
            "Ella dijo \"hola\" y nos fuimos a casa, porque es la vida que hemos elegido juntos con los niños.\n\n> [!caution]\n> Una \"precaución\" para todos los lectores.\n\n```\nlet s = \"sin cambios\";\n```".into(),
        ),
    ));
    cases.push((
        "03e-italian-guillemets",
        meta.clone(),
        plain(
            "Virgolette italiane",
            "Lei disse \"ciao\" e siamo tornati a casa, perché è la vita che abbiamo scelto insieme ai bambini.\n\n> [!important]\n> Un \"avviso\" importante per tutti i lettori.\n\n```\nlet s = \"invariato\";\n```".into(),
        ),
    ));
    cases.push((
        "03f-dutch-quotes",
        meta.clone(),
        plain(
            "Nederlandse aanhalingstekens",
            "Zij zei \"hallo\" en we gingen naar huis, want dat is het leven dat we samen met de kinderen hebben gekozen.\n\n> [!warning]\n> Een \"waarschuwing\" voor alle lezers.\n\n```\nlet s = \"ongewijzigd\";\n```".into(),
        ),
    ));
    cases.push((
        "03g-portuguese-quotes",
        meta.clone(),
        plain(
            "Aspas portuguesas",
            "Ela disse \"olá\" e fomos para casa, porque é a vida que escolhemos juntos com as crianças.\n\n> [!tip]\n> Uma \"dica\" para todos os leitores.\n\n```\nlet s = \"inalterado\";\n```".into(),
        ),
    ));
    cases.push((
        "03h-swedish-quotes",
        meta.clone(),
        plain(
            "Svenska citattecken",
            "Hon sa \"hej\" och vi gick hem, för det är livet som vi har valt tillsammans med barnen.\n\n> [!caution]\n> En \"varning\" till alla läsare i huset.\n\n```\nlet s = \"oförändrad\";\n```".into(),
        ),
    ));
    cases.push((
        "03i-turkish-quotes",
        meta.clone(),
        plain(
            "Türkçe tırnak işaretleri",
            "O \"merhaba\" dedi ve biz eve gittik, çünkü bu çocuklarla birlikte seçtiğimiz hayattır.\n\n> [!important]\n> Tüm okuyucular için \"önemli\" bir not.\n\n```\nlet s = \"değişmedi\";\n```".into(),
        ),
    ));
    cases.push((
        "04-unbreakable-words",
        meta.clone(),
        plain(
            "Unbreakable",
            format!(
                "{}\n\n- {}\n\n> {}\n\n`{}`",
                "x".repeat(300),
                "y".repeat(260),
                "z".repeat(260),
                "c".repeat(200)
            ),
        ),
    ));
    let wide_header: Vec<String> = (0..10).map(|i| format!("Column {i}")).collect();
    let wide_row: Vec<String> = (0..10).map(|i| format!("cell {i}")).collect();
    cases.push((
        "05-wide-table",
        meta.clone(),
        plain(
            "Wide table",
            format!(
                "| {} |\n|{}|\n| {} |",
                wide_header.join(" | "),
                ["---"; 10].join("|"),
                wide_row.join(" | ")
            ),
        ),
    ));
    let rows: Vec<String> = (0..150)
        .map(|i| format!("| item {i} | {} | {i}.50 |", "note ".repeat(i % 7 + 1)))
        .collect();
    cases.push((
        "06-long-table-repeats-header",
        meta.clone(),
        plain(
            "Long table",
            format!(
                "| Item | Note | Price |\n|:--|:-:|--:|\n{}",
                rows.join("\n")
            ),
        ),
    ));
    let mut hierarchy = String::from("# Overview\n\n");
    hierarchy.push_str(&lines("Overview line", 20));
    for chapter in 1..=3 {
        hierarchy.push_str(&format!("\n\n# Chapter {chapter}\n\n{}", lines("Intro", 4)));
        for part in 1..=3 {
            hierarchy.push_str(&format!(
                "\n\n## Part {chapter}.{part}\n\n{}",
                lines("Text", 14)
            ));
            hierarchy.push_str(&format!(
                "\n\n### Detail {chapter}.{part}\n\n{}",
                lines("Detail", 6)
            ));
        }
    }
    cases.push((
        "07-heading-hierarchy",
        meta.clone(),
        plain("Heading hierarchy", hierarchy),
    ));
    cases.push((
            "08-lists-and-quotes",
            meta.clone(),
            plain(
                "Lists and quotes",
                "- one\n  - two\n    - three\n      - four\n1. first\n2. second with a very long line that wraps around the page because it just keeps going and going and going\n> quote\n>> nested quote\n> > spaced nested quote\n\n>>>\nmultiline quote\n\nsecond paragraph of it\n>>>\n\n* star item\n10. ten".into(),
            ),
        ));
    cases.push((
            "09-code-blocks",
            meta.clone(),
            plain(
                "Code blocks",
                format!(
                    "```\n{}\n```\n\n```\n{}\n```\n\n```\nunterminated",
                    "a very long line of code that goes far beyond the right margin of the page and must be clipped".repeat(2),
                    lines("line of a block that is taller than half a page", 60)
                ),
            ),
        ));
    let mut checklist = plain("Checklist page break", lines("filler", 30));
    checklist.checklist = (0..14)
        .map(|i| ChecklistItem {
            text: if i == 3 {
                "a long checklist item ".repeat(12)
            } else {
                format!("item {i}")
            },
            completed: i % 3 == 0,
        })
        .collect();
    cases.push(("10-checklist-page-break", meta.clone(), checklist));
    let mut labelled = plain("Many labels", "Labels wrap onto several rows.".into());
    labelled.label_ids = meta.labels.iter().map(|l| l.id.clone()).collect();
    labelled.due_date = chrono::NaiveDate::from_ymd_opt(2030, 1, 31);
    cases.push(("11-many-labels", meta.clone(), labelled));
    cases.push((
            "12-markdown-oddities",
            meta.clone(),
            plain(
                "Markdown oddities",
                "Unclosed **bold and `code and *italic\n\n2 * 3 * 4 = 24\n\n**\n\n####### seven hashes\n\n#no space\n\n| only | header |\n|---|---|\n\n| ragged | row |\n|---|---|\n| a |\n| a | b | c |\n| esc \\| pipe | `x` |\n|  |  |\n\n---\n\n<b>html</b> &amp; stuff".into(),
            ),
        ));
    cases.push((
            "13-emphasis",
            meta.clone(),
            plain(
                "Emphasis",
                "**bold** *italic* ***bold italic*** and ***unclosed\n\n- ***item*** with `code`\n\n> ***quoted*** text\n\n| ***head*** | **b** |\n|---|---|\n| *i* | ***both*** |\n\n***\n\n******".into(),
            ),
        ));
    let mut matrix =
        String::from("A matrix has an empty top-left cell; its first column labels the rows.\n\n");
    matrix.push_str("|  | A | B |\n|---|:-:|--:|\n| **x** | 1 | 2 |\n| y | 3 | 4 |\n\n");
    matrix.push_str("Whitespace-only corner and emphasis in the labels:\n\n|   | north | south | east | west |\n|---|---|---|---|---|\n| *italic* | 1 | 2 | 3 | 4 |\n| ***both*** | 5 | 6 | 7 | 8 |\n| `code` | 9 | 10 | 11 | 12 |\n\n");
    matrix.push_str("Wrapping labels and cells, and empty row labels:\n\n|  | Short | Long |\n|---|---|---|\n| a row label that is long enough to wrap inside its column | x | a cell that is long enough to wrap as well, over several lines |\n|  | no label | still fine |\n\n");
    matrix.push_str("Header-only matrix and a one-column matrix:\n\n|  | only header |\n|---|---|\n\n|  |\n|---|\n| lonely |\n\n");
    matrix.push_str("Many rows: a grid with row separators, repeating its header across pages:\n\n|  | Q1 | Q2 | Q3 | Q4 |\n|---|--:|--:|--:|--:|\n");
    for i in 0..90 {
        matrix.push_str(&format!(
            "| region {i} | {} | {} | {} | {} |\n",
            i,
            i * 2,
            i * 3,
            i * 4
        ));
    }
    cases.push(("14-matrix", meta.clone(), plain("Matrix tables", matrix)));
    cases.push((
            "15-alerts",
            meta.clone(),
            plain(
                "Alerts",
                "> [!note]\n> Useful information, with **bold** and `code`.\n\n> [!tip]\n> A tip that is long enough to wrap around the page because it just keeps going and going and going and going\n> - with a list\n\n>>> [!important]\nYou should consider the following ramifications:\n\n1. consideration 1\n2. consideration 2\n>>>\n\n> [!caution]\n> Careful.\n\n>>> [!warning]\nDanger!\n>>>\n\n> [!NOTE]\n\n> [!bogus]\n> stays an ordinary quote".into(),
            ),
        ));
    cases.push((
        "16-footnotes",
        meta.clone(),
        plain("Footnotes", footnote_example()),
    ));
    cases
}

/// Definitions first, references spread over several pages: a note is on
/// the page of its reference, wherever it is defined.
pub(super) fn footnote_example() -> String {
    let mut text = String::from(
        "[^late]: Defined at the top, shown on the page of its reference.\n\
             [^1]: A numbered note.  \nWith a second line, after two trailing spaces.\n\
             [^fn-long]: A long note wraps over several lines inside the footnote area at the bottom of the page, set smaller than the body text.\n\
             [^unused]: Never referenced, so never printed.\n\n\
             A simple footnote[^1] and a named one[^late].\n\n",
    );
    for i in 0..40 {
        text.push_str(&format!(
            "Filler paragraph {i} pushes the next reference down.\n"
        ));
    }
    text.push_str("\nLater on[^fn-long], citing the first again[^1], and a missing one[^nope].\n\n| Cell | Note |\n|---|---|\n| a | with a note[^late] |\n\n- list item[^fn-long]\n\n> quote[^1]\n");
    text
}

pub(super) const SHOWCASE: &str = "\
Short, readable summary of the release plan with **bold**, *italic* and `inline code`.

This second paragraph is deliberately a little longer, so that it wraps over several lines and the spacing between consecutive paragraphs can be judged: the gap should be clearly visible, yet smaller than the one above a heading.

A third paragraph follows directly. It is here to show that prose reads evenly, and that line spacing inside a paragraph differs from the space between paragraphs.

# Goals

The goals below are ordered by priority. This paragraph precedes a list, so the distance to the first bullet matters.

- Ship the new **export** pipeline
- Cut start-up time in half
  - measured on the reference laptop
  - reported in the release notes
1. Freeze
2. Test
3. Release

A paragraph directly after a list must not stick to its last item, and should not float away either.

> Scope is fixed; dates are not.

Prose after a quote, to judge the gap below it.

Footnotes[^scope] sit at the bottom of the page of their reference.

[^scope]: Defined anywhere in the text; set small below a rule.

> [!warning]
> The freeze date is firm.

Prose between two alerts and before the next block: it should neither touch the alert nor leave a gulf.

>>> [!tip]
A multiline alert may hold several paragraphs.

Like this one.
>>>

Text right below an alert, followed by a plain multiline quote.

>>>
A multiline quote spans several lines
without a `>` on each of them.

Even paragraphs.
>>>

# Timeline

The schedule has three phases. This paragraph introduces the table, so check the gap between the text and the table's top rule.

| Phase | Owner | Weeks |
|:--|:-:|--:|
| Design | Ada | 2 |
| Build | Grace | 6 |
| Verify | Linus | 3 |

The paragraph after a table follows the bottom rule at a comfortable distance, and is itself long enough to wrap onto a second line.

# Responsibilities

A matrix labels its rows as well as its columns (empty top-left cell):

|  | Design | Build | Verify |
|---|:-:|:-:|:-:|
| Ada | owner | review | — |
| Grace | review | owner | support |
| Linus | — | support | owner |

Closing prose after the matrix, before a heading and a code block.

# Notes

Run the following to package a release:

```
cargo build --release
./scripts/package.sh
```

That is all — a last paragraph after the code block.
";

#[test]
fn corner_cases_are_set_in_their_language() {
    for (name, _, card) in corner_cases() {
        let expected = match name {
            "03b-german-quotes-and-alert-titles" => Some(Lang::De),
            "03c-french-guillemets" => Some(Lang::Fr),
            "03d-spanish-guillemets" => Some(Lang::Es),
            "03e-italian-guillemets" => Some(Lang::It),
            "03f-dutch-quotes" => Some(Lang::Nl),
            "03g-portuguese-quotes" => Some(Lang::Pt),
            "03h-swedish-quotes" => Some(Lang::Sv),
            "03i-turkish-quotes" => Some(Lang::Tr),
            _ => None,
        };
        if let Some(lang) = expected {
            assert_eq!(Lang::detect(&card), Some(lang), "{name}");
        }
    }
}

#[test]
fn corner_cases_render_as_valid_pdf_a() {
    for theme in THEMES {
        for (name, meta, card) in corner_cases() {
            let pdf = render(&meta, "List", &card, theme)
                .unwrap_or_else(|e| panic!("{name} {theme:?}: {e:#}"));
            assert!(pdf.starts_with(b"%PDF-"), "{name} {theme:?}");
        }
    }
}

#[test]
fn corner_cases_stay_inside_the_page() {
    for theme in THEMES {
        let fonts = Fonts::load_for(theme).unwrap();
        for (name, meta, card) in corner_cases() {
            let name = format!("{name} {theme:?}");
            let (pages, _) = layout(&fonts, &meta, "List", &card, chrono::Utc::now());
            for (n, page) in pages.iter().enumerate() {
                for op in page {
                    match op {
                        Op::Text {
                            x,
                            y,
                            text,
                            style,
                            size,
                            ..
                        } => {
                            let right = x + fonts.face(*style).width(text, *size);
                            assert!(
                                *x >= MARGIN - 1.0 && right <= PAGE_W - MARGIN + 1.0,
                                "{name} p{n}: '{text}' leaves the margins ({x}..{right})"
                            );
                            assert!(
                                *y <= BOTTOM + 1.0,
                                "{name} p{n}: '{text}' below the content area ({y})"
                            );
                        }
                        Op::Rect { x, w, y, h, .. } => {
                            assert!(
                                *x >= MARGIN - 1.0 && x + w <= PAGE_W - MARGIN + 1.0,
                                "{name} p{n}: rect leaves the margins"
                            );
                            assert!(
                                y + h <= BOTTOM + 1.0,
                                "{name} p{n}: rect below the content area"
                            );
                        }
                        Op::Icon { x, y, size, .. } => {
                            assert!(
                                *x >= MARGIN - 1.0 && x + size <= PAGE_W - MARGIN + 1.0,
                                "{name} p{n}: icon leaves the margins"
                            );
                            assert!(
                                y + size <= BOTTOM + 1.0,
                                "{name} p{n}: icon below the content area"
                            );
                        }
                        Op::Line { x1, x2, y1, y2, .. } => {
                            assert!(
                                x1.min(*x2) >= MARGIN - 1.0 && x1.max(*x2) <= PAGE_W - MARGIN + 1.0,
                                "{name} p{n}: line leaves the margins"
                            );
                            assert!(
                                y1.max(*y2) <= BOTTOM + 1.0,
                                "{name} p{n}: line below the content area"
                            );
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn corner_cases_never_produce_blank_pages() {
    for theme in THEMES {
        let fonts = Fonts::load_for(theme).unwrap();
        for (name, meta, card) in corner_cases() {
            let (pages, _) = layout(&fonts, &meta, "List", &card, chrono::Utc::now());
            for (n, page) in pages.iter().enumerate() {
                assert!(!page.is_empty(), "{name}: page {n} is blank");
            }
        }
    }
}

/// Fixed "made at" time (2000-01-01 00:00 UTC) and fixed created/updated
/// times of every example card, so regenerated examples only change with
/// their content. Dates print in local time: regenerate with `TZ=UTC`.
/// Fixed: the build's version is `snap-<today>` and would change every example daily.
pub(super) const EXAMPLE_VERSION: &str = "snap-2026-11-06";

pub(super) const EXAMPLE_TIME: chrono::DateTime<chrono::Utc> =
    chrono::DateTime::from_timestamp(946_684_800, 0).unwrap();

fn at_example_time(mut card: Card) -> Card {
    card.created_at = chrono::DateTime::from_timestamp(946_022_400, 0).unwrap(); // 1999-12-24 08:00
    card.updated_at = chrono::DateTime::from_timestamp(946_684_740, 0).unwrap(); // 1999-12-31 23:59
    card
}

#[test]
fn rendering_at_a_fixed_time_is_deterministic() {
    for (_, meta, card) in corner_cases().into_iter().take(3) {
        let card = at_example_time(card);
        let a = render_at(
            &meta,
            "To Do",
            &card,
            Theme::Color,
            EXAMPLE_TIME,
            EXAMPLE_VERSION,
        )
        .unwrap();
        let b = render_at(
            &meta,
            "To Do",
            &card,
            Theme::Color,
            EXAMPLE_TIME,
            EXAMPLE_VERSION,
        )
        .unwrap();
        assert!(
            a == b,
            "PDF output must not depend on anything but its input"
        );
    }
}

/// Every example PDF with its path under `docs/export-examples/`.
fn example_pdfs() -> Vec<(std::path::PathBuf, Vec<u8>)> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/export-examples");
    let mut out = Vec::new();
    for (theme, dir) in [
        (Theme::Mono, root.clone()),
        (Theme::Color, root.join("color")),
    ] {
        for (name, meta, card) in corner_cases() {
            // Color is shown by the showcase and the alerts only; the rest are monochrome.
            if !name.starts_with("00-") && !name.starts_with("15-") && theme == Theme::Color {
                continue;
            }
            let card = at_example_time(card);
            let pdf =
                render_at(&meta, "To Do", &card, theme, EXAMPLE_TIME, EXAMPLE_VERSION).unwrap();
            out.push((dir.join(format!("{name}.pdf")), pdf));
        }
    }
    out
}

/// Regenerates `docs/export-examples/*.pdf`:
/// `TZ=UTC cargo test write_example_pdfs -- --ignored`.
#[test]
#[ignore = "writes files into docs/"]
fn write_example_pdfs() {
    for (path, pdf) in example_pdfs() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, pdf).unwrap();
    }
}

/// The committed example PDFs must match what the code renders now; fails
/// after a layout change until they are regenerated.
#[test]
fn example_pdfs_are_up_to_date() {
    let stale: Vec<_> = example_pdfs()
        .into_iter()
        .filter(|(path, pdf)| std::fs::read(path).ok().as_ref() != Some(pdf))
        .map(|(path, _)| path.display().to_string())
        .collect();
    assert!(
        stale.is_empty(),
        "stale example PDFs (run `TZ=UTC cargo test write_example_pdfs -- --ignored`): {stale:#?}"
    );
}
