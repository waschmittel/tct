//! Widths are measured in one place and compared in another (a table column
//! is the widest word plus padding on both sides; the cell then wraps at the
//! column minus that padding). In f32 `(w + 2p) - 2p` can be a hair below
//! `w`, so a word that exactly fits must not be taken as too wide: it would
//! be broken per character ("Desig / n"). `EPS` is the tolerance.

use super::*;
use crate::export::blocks::Run;

/// Deterministic pseudo-random words over letters DejaVu has, 2–14 chars.
fn words(n: usize) -> Vec<String> {
    let alphabet: Vec<char> =
        "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZäöüßÄÖÜéèñ0123456789"
            .chars()
            .collect();
    let mut state = 0x2545_F491_4F6C_DD1D_u64;
    let mut next = move || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    (0..n)
        .map(|_| {
            let len = 2 + (next() % 13) as usize;
            (0..len)
                .map(|_| alphabet[(next() % alphabet.len() as u64) as usize])
                .collect()
        })
        .collect()
}

fn styles() -> [Style; 3] {
    [
        Style::default(),
        Style {
            bold: true,
            ..Style::default()
        },
        Style {
            code: true,
            ..Style::default()
        },
    ]
}

#[test]
fn a_word_never_breaks_in_the_width_it_was_measured_for() {
    let fonts = Fonts::load().unwrap();
    let flow = Flow::new(&fonts);
    for style in styles() {
        for size in [9.5_f32, 8.5, 7.5, 10.0] {
            for pad in [5.0_f32, 3.0, 2.5, 2.0, 0.37, 1.13, 4.71] {
                for word in words(500) {
                    let runs = [Run {
                        text: word.clone(),
                        style,
                    }];
                    let natural = flow
                        .wrap(&runs, size, f32::MAX)
                        .iter()
                        .map(|l| l.last().map_or(0.0, |g| g.x + g.w))
                        .fold(0.0, f32::max);
                    // The column width as `table_widths` builds it, and the
                    // room the cell then wraps in.
                    let room = (natural + 2.0 * pad) - 2.0 * pad;
                    let lines = flow.wrap(&runs, size, room);
                    assert_eq!(lines.len(), 1, "{word:?} {style:?} {size} {pad}");
                }
            }
        }
    }
}

#[test]
fn table_words_stay_whole_when_the_columns_just_fit() {
    for word in words(400) {
        // A bold header word wider than the body cell sets the column width.
        let page = table_page(&format!("| {word} |\n|---|\n| x |"));
        assert!(
            texts(&page).contains(&word.as_str()),
            "{word:?} was broken: {:?}",
            texts(&page)
        );
    }
}

#[test]
fn squeezed_tables_keep_their_longest_words_whole() {
    // Too wide for the page, so every column shrinks to its longest word.
    let w = words(40);
    for chunk in w.chunks(6) {
        let head: Vec<&str> = chunk.iter().map(|s| s.as_str()).collect();
        let long = chunk.iter().map(|s| format!("{s} {s} {s} {s}")).collect();
        let long: Vec<String> = long;
        let table = format!(
            "| {} |\n|{}|\n| {} |",
            head.join(" | "),
            vec!["---"; head.len()].join("|"),
            long.join(" | ")
        );
        let page = table_page(&table);
        for word in &head {
            assert!(texts(&page).contains(word), "{word:?} was broken");
        }
    }
}

#[test]
fn table_columns_leave_room_for_their_words() {
    // `table_widths` then wrap, with paddings that expose f32 error.
    let fonts = Fonts::load().unwrap();
    let flow = Flow::new(&fonts);
    let run = |text: &str, bold: bool| {
        vec![Run {
            text: text.into(),
            style: Style {
                bold,
                ..Style::default()
            },
        }]
    };
    for pad in [5.0_f32, 0.37, 1.13, 4.71] {
        for word in words(1000) {
            let cells = vec![vec![run(&word, true)], vec![run("x", false)]];
            let (widths, _) = flow.table_widths(&cells, 1, 9.5, pad);
            let lines = flow.wrap(&cells[0][0], 9.5, widths[0] - 2.0 * pad);
            assert_eq!(lines.len(), 1, "{word:?} pad {pad}");
        }
    }
}

#[test]
fn a_block_that_exactly_fills_the_page_stays_on_it() {
    // Heights add up from many small steps, so they are never exact.
    let fonts = Fonts::load().unwrap();
    for n in 1..400 {
        let mut flow = Flow::new(&fonts);
        let step = (flow.bottom() - flow.y) / n as f32;
        let height = (0..n).fold(0.0_f32, |sum, _| sum + step);
        flow.need(height);
        assert_eq!(flow.pages.len(), 1, "{n} steps of {step}");
    }
}
