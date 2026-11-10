//! Golden-screen tests: render the full UI into a `TestBackend` buffer and
//! snapshot the visible text with `insta`. Catches layout/hint/help drift
//! that unit tests on handlers can't see.
//!
//! Review changed snapshots with `cargo insta review` (or set
//! `INSTA_UPDATE=always` and inspect the diff).
//!
//! Fixture rules: no due dates (rendered relative to today) and no history
//! dialogs (timestamps) — both would churn snapshots.

use crate::app::{App, AppMode};
use crate::model::card::{Card, ChecklistItem};
use crate::model::id::BoardId;
use crate::model::label::{Label, LabelColor};
use crate::storage::board_store;
use crate::test_support::with_temp_dir;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Terminal;
use ratatui::backend::TestBackend;

mod board;
mod detail;
mod dialogs;
mod editor;
mod help;
mod markdown;
mod scrolling;

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
    terminal
        .draw(|frame| crate::ui::render(frame, app))
        .unwrap();
    buffer_to_string(terminal.backend().buffer())
}

fn press(app: &mut App, code: KeyCode) {
    crate::input::handle_input(app, KeyEvent::new(code, KeyModifiers::NONE)).unwrap();
    let _ = render_to_string(app);
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
        ChecklistItem {
            text: "Reproduce".into(),
            completed: true,
        },
        ChecklistItem {
            text: "Write failing test".into(),
            completed: false,
        },
        ChecklistItem {
            text: "Fix".into(),
            completed: false,
        },
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

fn set_long_description(app: &mut App) {
    let board = app.board_mut().unwrap();
    let card_id = board.current_card_id().cloned().unwrap();
    let card = board.cards.get_mut(&card_id).unwrap();
    card.description = (1..=40)
        .map(|i| format!("line {i}"))
        .collect::<Vec<_>>()
        .join("\n");
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

fn set_long_checklist(app: &mut App) {
    let board = app.board_mut().unwrap();
    let card_id = board.current_card_id().cloned().unwrap();
    let card = board.cards.get_mut(&card_id).unwrap();
    card.checklist = (1..=12)
        .map(|i| ChecklistItem {
            text: format!("Checklist step {i:02} of the workflow"),
            completed: i % 2 == 0,
        })
        .collect();
}

const TABLE_DESCRIPTION: &str = "Plan:\n\n| Item | Qty | Note |\n|:-|-:|:-:|\n| Apples | 3 | fresh |\n| **Pears** | 12 | `ripe` |";

fn set_description(app: &mut App, text: &str) {
    let board = app.board_mut().unwrap();
    let card_id = board.current_card_id().cloned().unwrap();
    board.cards.get_mut(&card_id).unwrap().description = text.into();
}

const FOOTNOTE_DESCRIPTION: &str = "A simple footnote[^1] and a named one[^my-note].\n\n[^1]: My reference.\n[^my-note]: Two lines.  \nThis is the second line.";

fn set_long_history(app: &mut App) {
    use crate::model::card::HistoryField;
    let board = app.board_mut().unwrap();
    let card_id = board.current_card_id().cloned().unwrap();
    let card = board.cards.get_mut(&card_id).unwrap();
    let old_desc = (1..=25)
        .map(|i| format!("old line {i:02}"))
        .collect::<Vec<_>>()
        .join("\n");
    let new_desc = (1..=25)
        .map(|i| format!("new line {i:02}"))
        .collect::<Vec<_>>()
        .join("\n");
    card.log_change(
        HistoryField::Description,
        "Update description",
        Some(old_desc),
        Some(new_desc),
    );
}

fn seed_board_with_multiple_labels() -> BoardId {
    let mut meta = crate::model::board::BoardMeta::new("Labeled".into());
    let l_bug = Label::new("bug".into(), LabelColor::Red);
    let l_feat = Label::new("feature".into(), LabelColor::Green);
    let l_docs = Label::new("docs".into(), LabelColor::Blue);
    let l_urg = Label::new("urgent".into(), LabelColor::Yellow);
    let bug_id = l_bug.id.clone();
    let urg_id = l_urg.id.clone();
    meta.labels.extend([l_bug, l_feat, l_docs, l_urg]);
    board_store::save_board(&meta).unwrap();

    let todo = crate::test_support::add_list(&mut meta, "To Do");
    let mut c1 = Card::new("Implement authentication".into());
    c1.description = "Add OAuth2 and session handling.".into();
    c1.label_ids = vec![bug_id, urg_id];
    let c2 = Card::new("Write documentation".into());

    crate::test_support::put_cards(&meta.id, &todo, &mut [c1, c2]);
    meta.id
}

fn seed_long_list_board() -> BoardId {
    let mut meta = crate::model::board::BoardMeta::new("Long".into());
    board_store::save_board(&meta).unwrap();
    let list = crate::test_support::add_list(&mut meta, "Backlog");
    let mut cards: Vec<Card> = (1..=12)
        .map(|i| Card::new(format!("Card {i:02}")))
        .collect();
    crate::test_support::put_cards(&meta.id, &list, &mut cards);
    meta.id
}

fn thumb_rows(text: &str) -> Vec<usize> {
    text.lines()
        .enumerate()
        .filter(|(_, l)| l.contains('▌'))
        .map(|(y, _)| y)
        .collect()
}

/// The export dialog browsing a fixed folder: the typed path is relative to
/// the dialog's cwd (a temp dir), so nothing machine-specific is rendered.
fn export_dialog_screen(language: &str) -> String {
    use crate::dialog::Dialog;
    use crate::dialog::export_pdf::ExportPdf;
    use crate::export::Theme;
    use crate::model::id::CardId;

    let tmp = tempfile::tempdir().unwrap();
    std::fs::create_dir(tmp.path().join("notes")).unwrap();
    for name in ["report.pdf", "todo.txt"] {
        std::fs::write(tmp.path().join(name), "").unwrap();
    }
    let mut dialog = ExportPdf::new(
        CardId::generate(),
        "Quarterly plan".into(),
        language.into(),
        Theme::Mono,
        tmp.path(),
        tmp.path().to_path_buf(),
    );
    let key = |code| KeyEvent::new(code, KeyModifiers::empty());
    for _ in 0..tmp.path().as_os_str().len() + 2 {
        dialog.handle_key(key(KeyCode::Backspace), None);
    }
    for c in "./".chars() {
        dialog.handle_key(key(KeyCode::Char(c)), None);
    }
    let mut terminal = Terminal::new(TestBackend::new(90, 16)).unwrap();
    terminal
        .draw(|f| dialog.render(f, f.area(), None, ratatui::style::Color::Cyan))
        .unwrap();
    buffer_to_string(terminal.backend().buffer())
}
