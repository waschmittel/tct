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
use crate::model::label::{Label, LabelColor};
use crate::model::list::CardList;
use crate::storage::{board_store, card_store, list_store};
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
    let backend = TestBackend::new(WIDTH, HEIGHT);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|frame| crate::ui::render(frame, app)).unwrap();
    buffer_to_string(terminal.backend().buffer())
}

fn press(app: &mut App, code: KeyCode) {
    crate::input::handle_input(app, KeyEvent::new(code, KeyModifiers::NONE)).unwrap();
}

/// Seed a deterministic demo board: two lists, cards with labels,
/// checklist, and description. No due dates (relative rendering).
fn seed_demo_board() -> String {
    let mut meta = crate::model::board::BoardMeta::new("Demo".into());
    let label = Label::new("bug".into(), LabelColor::Red);
    let label_id = label.id.clone();
    meta.labels.push(label);
    // Create the board dir before any card/list writes land in it.
    board_store::save_board(&meta).unwrap();

    let mut todo = CardList::new("To Do".into());
    let mut done = CardList::new("Done".into());

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

    for c in [&c1, &c2, &c3] {
        card_store::save_card(&meta.id, c).unwrap();
    }
    todo.card_ids = vec![c1.id.clone(), c2.id.clone()];
    done.card_ids = vec![c3.id.clone()];
    list_store::save_list(&meta.id, &todo).unwrap();
    list_store::save_list(&meta.id, &done).unwrap();
    meta.list_order = vec![todo.id.clone(), done.id.clone()];
    board_store::save_board(&meta).unwrap();
    board_store::append_to_order(&meta.id).unwrap();
    meta.id
}

fn seed_selector_boards() {
    for name in ["Alpha", "Beta"] {
        let meta = crate::model::board::BoardMeta::new(name.into());
        board_store::save_board(&meta).unwrap();
        board_store::append_to_order(&meta.id).unwrap();
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

#[test]
fn snapshot_board_view_empty() {
    with_temp_dir(|| {
        let meta = crate::model::board::BoardMeta::new("Empty".into());
        board_store::save_board(&meta).unwrap();
        board_store::append_to_order(&meta.id).unwrap();
        let app = App::new(Some(meta.id.clone())).unwrap();
        insta::assert_snapshot!(render_to_string(&app));
    });
}
