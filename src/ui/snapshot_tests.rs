//! Golden-screen tests: render the full UI into a `TestBackend` buffer and
//! snapshot the visible text with `insta`. Catches layout/hint/help drift
//! that unit tests on handlers can't see.
//!
//! Review changed snapshots with `cargo insta review` (or set
//! `INSTA_UPDATE=always` and inspect the diff).
//!
//! Fixture rules: no due dates (rendered relative to today) and no history
//! dialogs (timestamps) — both would churn snapshots.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::backend::TestBackend;
use ratatui::Terminal;

use crate::app::{App, AppMode};
use crate::model::card::{Card, ChecklistItem};
use crate::model::id::BoardId;
use crate::model::label::{Label, LabelColor};
use crate::storage::board_store;
use crate::test_support::with_temp_dir;

const WIDTH: u16 = 100;
const HEIGHT: u16 = 30;

fn buffer_to_string(buffer: &ratatui::buffer::Buffer) -> String {
    let mut out = String::new();
    for y in 0..buffer.area.height {
        let mut line = String::new();
        for x in 0..buffer.area.width {
            line.push_str(buffer.cell((x, y)).map(|c| c.symbol()).unwrap_or(" "));
        }
        out.push_str(line.trim_end());
        out.push('\n');
    }
    out
}

fn render_to_string(app: &App) -> String {
    render_to_string_sized(app, WIDTH, HEIGHT)
}

fn render_to_string_sized(app: &App, width: u16, height: u16) -> String {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|frame| crate::ui::render(frame, app)).unwrap();
    buffer_to_string(terminal.backend().buffer())
}

fn press(app: &mut App, code: KeyCode) {
    crate::input::handle_input(app, KeyEvent::new(code, KeyModifiers::NONE)).unwrap();
}

/// Seed a deterministic demo board: two lists, cards with labels,
/// checklist, and description. No due dates (relative rendering).
fn seed_demo_board() -> BoardId {
    let mut meta = crate::model::board::BoardMeta::new("Demo".into());
    let label = Label::new("bug".into(), LabelColor::Red);
    let label_id = label.id.clone();
    meta.labels.push(label);
    // Create the board dir before any card/list writes land in it.
    board_store::save_board(&meta).unwrap();

    let todo = crate::test_support::add_list(&mut meta, "To Do");
    let done = crate::test_support::add_list(&mut meta, "Done");

    let mut c1 = Card::new("Fix login flow".into());
    c1.description = "Token expires too early.\n\n- check refresh\n- add test".into();
    c1.label_ids.push(label_id);
    c1.checklist = vec![
        ChecklistItem { text: "Reproduce".into(), completed: true },
        ChecklistItem { text: "Write failing test".into(), completed: false },
        ChecklistItem { text: "Fix".into(), completed: false },
    ];
    let c2 = Card::new("Redesign dashboard".into());
    let c3 = Card::new("Ship release notes".into());

    crate::test_support::put_cards(&meta.id, &todo, &mut [c1, c2]);
    crate::test_support::put_cards(&meta.id, &done, &mut [c3]);
    meta.id
}

fn seed_selector_boards() {
    for (idx, name) in ["Alpha", "Beta"].iter().enumerate() {
        let mut meta = crate::model::board::BoardMeta::new((*name).into());
        meta.position = (idx + 1) as f64;
        board_store::save_board(&meta).unwrap();
    }
}

#[test]
fn snapshot_board_selector() {
    with_temp_dir(|| {
        seed_selector_boards();
        let app = App::new(None).unwrap();
        insta::assert_snapshot!(render_to_string(&app));
    });
}

#[test]
fn snapshot_board_view() {
    with_temp_dir(|| {
        let id = seed_demo_board();
        let app = App::new(Some(id)).unwrap();
        insta::assert_snapshot!(render_to_string(&app));
    });
}

#[test]
fn snapshot_board_view_search_active() {
    with_temp_dir(|| {
        let id = seed_demo_board();
        let mut app = App::new(Some(id)).unwrap();
        press(&mut app, KeyCode::Char('/'));
        for ch in "dash".chars() {
            press(&mut app, KeyCode::Char(ch));
        }
        press(&mut app, KeyCode::Enter);
        insta::assert_snapshot!(render_to_string(&app));
    });
}

#[test]
fn snapshot_status_toast() {
    with_temp_dir(|| {
        let id = seed_demo_board();
        let mut app = App::new(Some(id)).unwrap();
        app.set_status("Archived card 'Fix login flow'".into());
        insta::assert_snapshot!(render_to_string(&app));
    });
}

#[test]
fn snapshot_card_detail() {
    with_temp_dir(|| {
        let id = seed_demo_board();
        let mut app = App::new(Some(id)).unwrap();
        press(&mut app, KeyCode::Enter);
        assert_eq!(app.mode, AppMode::CardDetail);
        insta::assert_snapshot!(render_to_string(&app));
    });
}

/// Empty sections are hidden — except when every section is empty, all
/// four show with their placeholders (discoverability).
#[test]
fn snapshot_card_detail_all_sections_empty() {
    with_temp_dir(|| {
        let id = seed_demo_board();
        let mut app = App::new(Some(id)).unwrap();
        press(&mut app, KeyCode::Down); // "Redesign dashboard": no content at all
        press(&mut app, KeyCode::Enter);
        assert_eq!(app.mode, AppMode::CardDetail);
        insta::assert_snapshot!(render_to_string(&app));
    });
}

fn set_long_description(app: &mut App) {
    let board = app.board_mut().unwrap();
    let card_id = board.current_card_id().cloned().unwrap();
    let card = board.cards.get_mut(&card_id).unwrap();
    card.description = (1..=40)
        .map(|i| format!("line {i}"))
        .collect::<Vec<_>>()
        .join("\n");
}

#[test]
fn snapshot_card_detail_long_description_scrollbar() {
    with_temp_dir(|| {
        let id = seed_demo_board();
        let mut app = App::new(Some(id)).unwrap();
        set_long_description(&mut app);
        press(&mut app, KeyCode::Enter);
        assert_eq!(app.mode, AppMode::CardDetail);
        // The renderer reports the description's max scroll; draw once so
        // the scroll keys have a bound (as in the real render loop).
        let _ = render_to_string(&app);
        press(&mut app, KeyCode::PageDown);
        press(&mut app, KeyCode::PageDown);
        insta::assert_snapshot!(render_to_string(&app));
    });
}

#[test]
fn snapshot_card_detail_scrollbar_at_bottom() {
    with_temp_dir(|| {
        let id = seed_demo_board();
        let mut app = App::new(Some(id)).unwrap();
        set_long_description(&mut app);
        press(&mut app, KeyCode::Enter);
        assert_eq!(app.mode, AppMode::CardDetail);
        // The renderer reports the description's max scroll; draw once so
        // the scroll keys have a bound (as in the real render loop).
        let _ = render_to_string(&app);
        // Scroll well past the end; input clamps at max scroll. The thumb
        // must sit at the very bottom of the track.
        for _ in 0..80 {
            press(&mut app, KeyCode::PageDown);
        }
        insta::assert_snapshot!(render_to_string(&app));
    });
}

/// Give the selected card enough labels to wrap across several lines.
fn add_many_labels(app: &mut App) {
    let board = app.board_mut().unwrap();
    let card_id = board.current_card_id().cloned().unwrap();
    let mut ids = Vec::new();
    for name in [
        "frontend",
        "backend",
        "urgent",
        "blocked",
        "needs-review",
        "good first issue", // spaces: must wrap as one unit
        "documentation",
        "performance",
    ] {
        let label = Label::new(name.into(), LabelColor::Red);
        ids.push(label.id.clone());
        board.meta.labels.push(label);
    }
    let card = board.cards.get_mut(&card_id).unwrap();
    card.label_ids.extend(ids);
}

/// Labels wrap at chip boundaries in the list view, and the info line
/// (due/checklist/description markers) keeps its row below them.
#[test]
fn snapshot_board_view_many_labels_wrap() {
    with_temp_dir(|| {
        let id = seed_demo_board();
        let mut app = App::new(Some(id)).unwrap();
        add_many_labels(&mut app);
        insta::assert_snapshot!(render_to_string(&app));
    });
}

/// Detail view shows label chips side by side, wrapped like on the card.
#[test]
fn snapshot_card_detail_many_labels_wrap() {
    with_temp_dir(|| {
        let id = seed_demo_board();
        let mut app = App::new(Some(id)).unwrap();
        add_many_labels(&mut app);
        press(&mut app, KeyCode::Enter);
        assert_eq!(app.mode, AppMode::CardDetail);
        insta::assert_snapshot!(render_to_string(&app));
    });
}

/// Long checklist items wrap; continuation lines align under the item text.
#[test]
fn snapshot_card_detail_long_checklist_item_wraps() {
    with_temp_dir(|| {
        let id = seed_demo_board();
        let mut app = App::new(Some(id)).unwrap();
        {
            let board = app.board_mut().unwrap();
            let card_id = board.current_card_id().cloned().unwrap();
            let card = board.cards.get_mut(&card_id).unwrap();
            card.checklist[1].text =
                "Write a failing test that reproduces the premature token expiry \
                 on refresh, including the clock-skew edge case"
                    .into();
        }
        press(&mut app, KeyCode::Enter);
        assert_eq!(app.mode, AppMode::CardDetail);
        insta::assert_snapshot!(render_to_string(&app));
    });
}

/// The popup stops growing once the description hits WRAP_WIDTH — on a
/// wide terminal it stays at wrap width + padding instead of 80% of the
/// screen.
#[test]
fn snapshot_card_detail_wide_terminal_capped() {
    with_temp_dir(|| {
        let id = seed_demo_board();
        let mut app = App::new(Some(id)).unwrap();
        press(&mut app, KeyCode::Enter);
        assert_eq!(app.mode, AppMode::CardDetail);
        insta::assert_snapshot!(render_to_string_sized(&app, 160, 30));
    });
}

#[test]
fn snapshot_description_editor_scrollbar() {
    with_temp_dir(|| {
        let id = seed_demo_board();
        let mut app = App::new(Some(id)).unwrap();
        set_long_description(&mut app);
        press(&mut app, KeyCode::Enter);
        press(&mut app, KeyCode::Char('e'));
        assert_eq!(app.mode, AppMode::Insert);
        insta::assert_snapshot!(render_to_string(&app));
    });
}

#[test]
fn snapshot_description_editor() {
    with_temp_dir(|| {
        let id = seed_demo_board();
        let mut app = App::new(Some(id)).unwrap();
        press(&mut app, KeyCode::Enter);
        assert_eq!(app.mode, AppMode::CardDetail);
        press(&mut app, KeyCode::Char('e'));
        assert_eq!(app.mode, AppMode::Insert);
        insta::assert_snapshot!(render_to_string(&app));
    });
}

#[test]
fn snapshot_help_board_selector() {
    with_temp_dir(|| {
        seed_selector_boards();
        let mut app = App::new(None).unwrap();
        app.version = "vTEST";
        press(&mut app, KeyCode::Char('?'));
        insta::assert_snapshot!(render_to_string(&app));
    });
}

#[test]
fn snapshot_help_board_view() {
    with_temp_dir(|| {
        let id = seed_demo_board();
        let mut app = App::new(Some(id)).unwrap();
        app.version = "vTEST";
        press(&mut app, KeyCode::Char('?'));
        insta::assert_snapshot!(render_to_string(&app));
    });
}

#[test]
fn snapshot_help_card_detail() {
    with_temp_dir(|| {
        let id = seed_demo_board();
        let mut app = App::new(Some(id)).unwrap();
        app.version = "vTEST";
        press(&mut app, KeyCode::Enter);
        press(&mut app, KeyCode::Char('?'));
        insta::assert_snapshot!(render_to_string(&app));
    });
}

/// Widget-level golden for the date-picker popup (layout broke twice:
/// oversized popup, off-center calendar). Rendered with a fixed date so
/// the text is deterministic — the today-highlight is style-only and
/// invisible in the text golden.
#[test]
fn snapshot_date_picker() {
    let backend = TestBackend::new(40, 14);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| {
            crate::ui::widgets::date_picker::render(
                frame,
                frame.area(),
                "2026-05-18",
                10,
                chrono::NaiveDate::from_ymd_opt(2026, 5, 18),
                ratatui::style::Color::Cyan,
            );
        })
        .unwrap();
    insta::assert_snapshot!(buffer_to_string(terminal.backend().buffer()));
}

/// Grab state: the status-bar chip flips to MOVE while `m` has a card
/// grabbed (arrow keys move it instead of navigating).
#[test]
fn snapshot_board_view_grab_mode() {
    with_temp_dir(|| {
        let id = seed_demo_board();
        let mut app = App::new(Some(id)).unwrap();
        press(&mut app, KeyCode::Char('m'));
        assert!(app.grab_active);
        insta::assert_snapshot!(render_to_string(&app));
    });
}

/// Degraded tier (Linux console): the selected card gets a double-line
/// border instead of the heavy one — visible in the text golden.
#[test]
fn snapshot_board_view_linux_tty() {
    with_temp_dir(|| {
        let id = seed_demo_board();
        let mut app = App::new(Some(id)).unwrap();
        app.caps = crate::term_caps::TermCaps::linux_tty();
        insta::assert_snapshot!(render_to_string(&app));
    });
}

/// Degraded tier: checklist marks render as `x` (✓ is missing from
/// console fonts).
#[test]
fn snapshot_card_detail_linux_tty() {
    with_temp_dir(|| {
        let id = seed_demo_board();
        let mut app = App::new(Some(id)).unwrap();
        app.caps = crate::term_caps::TermCaps::linux_tty();
        press(&mut app, KeyCode::Enter);
        assert_eq!(app.mode, AppMode::CardDetail);
        insta::assert_snapshot!(render_to_string(&app));
    });
}

/// On the bottom tier no RGB color may survive to the terminal (the Linux
/// console collapses them to unreadable black/white), and the selected
/// card must keep a visible background. Style-level check — the text
/// goldens above can't see colors.
#[test]
fn linux_tty_render_has_no_rgb_and_visible_selection() {
    with_temp_dir(|| {
        let id = seed_demo_board();
        let mut app = App::new(Some(id)).unwrap();
        app.caps = crate::term_caps::TermCaps::linux_tty();

        let backend = TestBackend::new(WIDTH, HEIGHT);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| crate::ui::render(frame, &app)).unwrap();
        let buffer = terminal.backend().buffer();

        use ratatui::style::Color;
        let mut selection_bg_cells = 0;
        for cell in buffer.content.iter() {
            assert!(
                !matches!(cell.fg, Color::Rgb(..)) && !matches!(cell.bg, Color::Rgb(..)),
                "RGB color leaked into a degraded frame: {cell:?}"
            );
            if cell.bg == app.caps.selection_bg() {
                selection_bg_cells += 1;
            }
        }
        assert!(selection_bg_cells > 0, "selected card background not visible");
    });
}

#[test]
fn snapshot_board_view_empty() {
    with_temp_dir(|| {
        let meta = crate::model::board::BoardMeta::new("Empty".into());
        board_store::save_board(&meta).unwrap();
        let app = App::new(Some(meta.id.clone())).unwrap();
        insta::assert_snapshot!(render_to_string(&app));
    });
}

/// Dialog help pages are one short list: sized to their rows, full width,
/// key column sized to the longest key (card history has the longest).
#[test]
fn snapshot_help_card_history_dialog() {
    with_temp_dir(|| {
        let id = seed_demo_board();
        let mut app = App::new(Some(id)).unwrap();
        app.version = "vTEST";
        press(&mut app, KeyCode::Char('h'));
        press(&mut app, KeyCode::Char('?'));
        assert_eq!(app.mode, AppMode::Help);
        insta::assert_snapshot!(render_to_string(&app));
    });
}

/// Every dialog's help page renders each row on one line, with a gap
/// between keys and description, in a popup no taller than its rows.
#[test]
fn dialog_help_pages_are_compact_and_unwrapped() {
    use crate::dialog::{
        Dialog, archived_boards::ArchivedBoards, archived_cards::ArchivedCards,
        archived_lists::ArchivedLists, card_history::CardHistory,
        confirm_archive_card::ConfirmArchiveCard, confirm_archive_list::ConfirmArchiveList,
        confirm_cancel_edit::ConfirmCancelEdit, confirm_clear_due_date::ConfirmClearDueDate,
        label_manager::LabelManager, label_picker::LabelPicker,
    };
    with_temp_dir(|| {
        let id = seed_demo_board();
        let dialogs: Vec<Box<dyn Dialog>> = vec![
            Box::new(ConfirmArchiveCard),
            Box::new(ConfirmArchiveList),
            Box::new(ConfirmCancelEdit),
            Box::new(ConfirmClearDueDate),
            Box::new(CardHistory::new()),
            Box::new(LabelPicker { selected_idx: 0 }),
            Box::new(LabelManager { selected_idx: 0, from_picker: false }),
            Box::new(ArchivedCards { cards: vec![], selected: 0 }),
            Box::new(ArchivedLists { lists: vec![], selected: 0 }),
            Box::new(ArchivedBoards { boards: vec![], selected: 0 }),
        ];
        for dialog in dialogs {
            let help = dialog.help().expect("dialog has a help page");
            let mut app = App::new(Some(id.clone())).unwrap();
            app.version = "vTEST";
            app.open_dialog(dialog);
            press(&mut app, KeyCode::Char('?'));
            assert_eq!(app.mode, AppMode::Help);
            let text = render_to_string(&app);

            for (keys, desc) in &help.rows {
                let line = text
                    .lines()
                    .find(|l| l.contains(desc))
                    .unwrap_or_else(|| panic!("{}: '{desc}' wrapped or missing\n{text}", help.title));
                let k = line.find(keys).unwrap_or_else(|| panic!("{}: '{keys}' missing", help.title));
                let gap = &line[k + keys.len()..line.find(desc).unwrap()];
                assert!(gap.starts_with("  "), "{}: keys crowd '{desc}'\n{text}", help.title);
            }
            let top = text.lines().position(|l| l.contains(help.title)).unwrap();
            let bottom = text.lines().position(|l| l.contains("Esc:close")).unwrap();
            assert_eq!(bottom - top, help.rows.len() + 2, "{}: popup height", help.title);
        }
    });
}

/// The date stamps sit in the card-detail frame's top/bottom right corners
/// in the accent color. Dates are pinned to a past year so the text is
/// deterministic (no snapshot: fixtures must not carry timestamps).
#[test]
fn card_detail_shows_date_stamps_in_accent_color() {
    use chrono::TimeZone;
    with_temp_dir(|| {
        let id = seed_demo_board();
        let mut app = App::new(Some(id)).unwrap();
        let card_id = app.board().unwrap().current_card_id().unwrap().clone();
        {
            let card = app.board_mut().unwrap().cards.get_mut(&card_id).unwrap();
            card.created_at = chrono::Utc.with_ymd_and_hms(2020, 1, 2, 12, 0, 0).unwrap();
            card.history.clear();
            card.log("Edited");
            card.history[0].at = chrono::Utc.with_ymd_and_hms(2021, 3, 4, 12, 0, 0).unwrap();
        }
        press(&mut app, KeyCode::Enter);
        assert_eq!(app.mode, AppMode::CardDetail);
        assert!(!render_to_string(&app).contains("2020-01-02"));

        press(&mut app, KeyCode::Char('c'));
        press(&mut app, KeyCode::Char('d'));
        let text = render_to_string(&app);
        let rows: Vec<&str> = text.lines().collect();
        let top = rows.iter().find(|r| r.contains("┌ Fix login flow")).expect("popup top");
        assert!(top.contains("2021-03-04"), "{text}");
        // The board card behind the popup carries a stamp too: the popup's is lowest.
        let bottom = rows.iter().rev().find(|r| r.contains("2020-01-02")).expect("created stamp");
        assert!(top.contains(" 2021-03-04 ┐") && bottom.contains(" 2020-01-02 ┘"), "{text}");

        let backend = TestBackend::new(WIDTH, HEIGHT);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| crate::ui::render(frame, &app)).unwrap();
        let buf = terminal.backend().buffer();
        let y = rows.iter().rposition(|r| r.contains("2020-01-02")).unwrap() as u16;
        let x = rows[y as usize].find("2020-01-02").map(|b| rows[y as usize][..b].chars().count()).unwrap() as u16;
        assert_eq!(buf[(x, y)].fg, app.accent_color());
    });
}
