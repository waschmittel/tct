mod app;
mod cli;
mod event;
mod input;
mod model;
mod storage;
mod ui;

use std::time::Duration;

use app::App;
use event::{AppEvent, EventHandler};

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();

    // Help flag (check before anything else so it always works)
    if args.iter().any(|a| a == "--help" || a == "-h") || args.first().map(|a| a.as_str()) == Some("help") {
        cli::print_help();
        return Ok(());
    }

    // Subcommand mode: first arg is a non-flag word
    if args.first().map(|a| !a.starts_with('-')).unwrap_or(false) {
        if let Err(e) = cli::run(&args) {
            eprintln!("Error: {e}");
            std::process::exit(1);
        }
        return Ok(());
    }

    // TUI mode — resolve optional --board flag before initialising the terminal
    let open_board_id = cli::resolve_board_flag(&args)?;

    let mut terminal = ratatui::init();
    let mut app = App::new(open_board_id)?;
    let events = EventHandler::new(Duration::from_millis(250));

    let result = run_app(&mut terminal, &mut app, &events);

    ratatui::restore();
    result
}

fn run_app(
    terminal: &mut ratatui::Terminal<ratatui::backend::CrosstermBackend<std::io::Stdout>>,
    app: &mut App,
    events: &EventHandler,
) -> anyhow::Result<()> {
    while !app.should_quit {
        terminal.draw(|frame| ui::render(frame, app))?;
        match events.next()? {
            AppEvent::Key(key) => input::handle_input(app, key)?,
            AppEvent::Tick => app.on_tick(),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::model::card::{Card, ChecklistItem};
    use crate::model::label::{Label, LabelColor};
    use crate::model::list::CardList;
    use crate::storage::{board_store, card_store, list_store};
    use std::env;

    fn with_temp_dir<F: FnOnce()>(f: F) {
        let dir = tempfile::tempdir().unwrap();
        unsafe { env::set_var("TCT_DATA_DIR", dir.path()) };
        board_store::ensure_base_dirs().unwrap();
        f();
        unsafe { env::remove_var("TCT_DATA_DIR") };
    }

    #[test]
    fn board_roundtrip() {
        with_temp_dir(|| {
            let board = board_store::create_board("My Board".into()).unwrap();
            let loaded = board_store::load_board(&board.id).unwrap();
            assert_eq!(loaded.name, "My Board");
            assert!(loaded.list_order.is_empty());

            let boards = board_store::list_boards().unwrap();
            assert_eq!(boards.len(), 1);
            assert_eq!(boards[0].name, "My Board");
        });
    }

    #[test]
    fn board_delete() {
        with_temp_dir(|| {
            let board = board_store::create_board("Delete Me".into()).unwrap();
            board_store::delete_board(&board.id).unwrap();
            let boards = board_store::list_boards().unwrap();
            assert!(boards.is_empty());
        });
    }

    #[test]
    fn list_roundtrip() {
        with_temp_dir(|| {
            let board = board_store::create_board("Board".into()).unwrap();
            let list = CardList::new("To Do".into());
            list_store::save_list(&board.id, &list).unwrap();
            let loaded = list_store::load_list(&board.id, &list.id).unwrap();
            assert_eq!(loaded.name, "To Do");
            assert!(loaded.card_ids.is_empty());
        });
    }

    #[test]
    fn card_roundtrip() {
        with_temp_dir(|| {
            let board = board_store::create_board("Board".into()).unwrap();
            let label = Label::new("BUG".into(), LabelColor::Red);
            let label_id = label.id.clone();
            let mut card = Card::new("Fix bug".into());
            card.description = "Important fix".into();
            card.label_ids.push(label_id.clone());
            card.checklist.push(ChecklistItem { text: "Reproduce".into(), completed: true });
            card.checklist.push(ChecklistItem { text: "Fix".into(), completed: false });

            card_store::save_card(&board.id, &card).unwrap();
            let loaded = card_store::load_card(&board.id, &card.id).unwrap();
            assert_eq!(loaded.title, "Fix bug");
            assert_eq!(loaded.description, "Important fix");
            assert_eq!(loaded.label_ids.len(), 1);
            assert_eq!(loaded.label_ids[0], label_id);
            assert_eq!(loaded.checklist.len(), 2);
            assert!(loaded.checklist[0].completed);
        });
    }

    #[test]
    fn full_lifecycle() {
        with_temp_dir(|| {
            let mut board = board_store::create_board("Project".into()).unwrap();

            let mut todo = CardList::new("To Do".into());
            let mut done = CardList::new("Done".into());

            let card1 = Card::new("Task 1".into());
            let card2 = Card::new("Task 2".into());

            card_store::save_card(&board.id, &card1).unwrap();
            card_store::save_card(&board.id, &card2).unwrap();

            todo.card_ids.push(card1.id.clone());
            todo.card_ids.push(card2.id.clone());

            list_store::save_list(&board.id, &todo).unwrap();
            list_store::save_list(&board.id, &done).unwrap();

            board.list_order = vec![todo.id.clone(), done.id.clone()];
            board_store::save_board(&board).unwrap();

            todo.card_ids.retain(|id| id != &card2.id);
            done.card_ids.push(card2.id.clone());
            list_store::save_list(&board.id, &todo).unwrap();
            list_store::save_list(&board.id, &done).unwrap();

            let lists = list_store::load_all_lists(&board.id, &board.list_order).unwrap();
            assert_eq!(lists[0].card_ids.len(), 1);
            assert_eq!(lists[1].card_ids.len(), 1);
            assert_eq!(lists[1].card_ids[0], card2.id);
        });
    }
}
