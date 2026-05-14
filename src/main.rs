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
    #[allow(unused_imports)]
    use regex::Regex;

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

    // ── CLI search tests ──────────────────────────────────────────────────────

    fn run_search(args: &[&str]) -> anyhow::Result<()> {
        let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
        crate::cli::run(&args)
    }

    fn setup_search_fixture() -> (
        crate::model::board::BoardMeta,
        CardList,
        CardList,
        Card,
        Card,
    ) {
        let mut board = board_store::create_board("Alpha".into()).unwrap();

        let mut todo = CardList::new("To Do".into());
        let mut done = CardList::new("Done".into());

        let mut card1 = Card::new("Fix login bug".into());
        card1.description = "Auth token expires too early".into();
        card1.checklist.push(ChecklistItem { text: "Reproduce issue".into(), completed: false });
        card1.checklist.push(ChecklistItem { text: "Write failing test".into(), completed: false });

        let mut card2 = Card::new("Redesign dashboard".into());
        card2.description = "New layout with login metrics".into();

        card_store::save_card(&board.id, &card1).unwrap();
        card_store::save_card(&board.id, &card2).unwrap();

        todo.card_ids.push(card1.id.clone());
        done.card_ids.push(card2.id.clone());

        list_store::save_list(&board.id, &todo).unwrap();
        list_store::save_list(&board.id, &done).unwrap();

        board.list_order = vec![todo.id.clone(), done.id.clone()];
        board_store::save_board(&board).unwrap();

        (board, todo, done, card1, card2)
    }

    #[test]
    fn search_substring_match_title() {
        with_temp_dir(|| {
            setup_search_fixture();
            // "login" matches both card1 title and card2 description — no error
            run_search(&["search", "login"]).unwrap();
        });
    }

    #[test]
    fn search_case_insensitive() {
        with_temp_dir(|| {
            setup_search_fixture();
            run_search(&["search", "LOGIN"]).unwrap();
            run_search(&["search", "Login"]).unwrap();
            run_search(&["search", "lOgIn"]).unwrap();
        });
    }

    #[test]
    fn search_no_matches_returns_ok() {
        with_temp_dir(|| {
            setup_search_fixture();
            // No match — should succeed (just print nothing found)
            run_search(&["search", "xyzzy_not_found_abc"]).unwrap();
        });
    }

    #[test]
    fn search_matches_description() {
        with_temp_dir(|| {
            setup_search_fixture();
            // "Auth token" only in card1 description
            run_search(&["search", "auth token"]).unwrap();
        });
    }

    #[test]
    fn search_matches_checklist_item() {
        with_temp_dir(|| {
            setup_search_fixture();
            // "Reproduce" only in card1 checklist
            run_search(&["search", "reproduce"]).unwrap();
        });
    }

    #[test]
    fn search_board_filter_existing() {
        with_temp_dir(|| {
            setup_search_fixture();
            // Partial board name match
            run_search(&["search", "login", "--board", "Alpha"]).unwrap();
            run_search(&["search", "login", "--board", "alp"]).unwrap();
        });
    }

    #[test]
    fn search_board_filter_no_match_errors() {
        with_temp_dir(|| {
            setup_search_fixture();
            let err = run_search(&["search", "login", "--board", "nonexistent_board_xyz"]);
            assert!(err.is_err(), "should error when no boards match filter");
        });
    }

    #[test]
    fn search_list_filter() {
        with_temp_dir(|| {
            setup_search_fixture();
            // "login" matches card1 (in To Do) and card2 desc (in Done).
            // Limit to "To Do" list only.
            run_search(&["search", "login", "--list", "To Do"]).unwrap();
            run_search(&["search", "login", "--list", "to"]).unwrap();
        });
    }

    #[test]
    fn search_list_filter_no_match_succeeds() {
        with_temp_dir(|| {
            setup_search_fixture();
            // List filter with no matching list — no error, just no results
            run_search(&["search", "login", "--list", "Backlog"]).unwrap();
        });
    }

    #[test]
    fn search_regex_basic() {
        with_temp_dir(|| {
            setup_search_fixture();
            run_search(&["search", "login|dashboard", "--regex"]).unwrap();
        });
    }

    #[test]
    fn search_regex_case_insensitive_flag() {
        with_temp_dir(|| {
            setup_search_fixture();
            run_search(&["search", "(?i)LOGIN", "--regex"]).unwrap();
        });
    }

    #[test]
    fn search_regex_anchors() {
        with_temp_dir(|| {
            setup_search_fixture();
            // Anchored to start of string
            run_search(&["search", "^Fix", "--regex"]).unwrap();
            run_search(&["search", "^Redesign", "--regex"]).unwrap();
        });
    }

    #[test]
    fn search_regex_invalid_errors() {
        with_temp_dir(|| {
            setup_search_fixture();
            let err = run_search(&["search", "[unclosed", "--regex"]);
            assert!(err.is_err(), "invalid regex should return error");
        });
    }

    #[test]
    fn search_no_boards_returns_ok() {
        with_temp_dir(|| {
            // No boards at all — no error, just nothing found
            run_search(&["search", "anything"]).unwrap();
        });
    }

    #[test]
    fn search_empty_query_allowed() {
        with_temp_dir(|| {
            setup_search_fixture();
            // Empty string matches everything
            run_search(&["search", ""]).unwrap();
        });
    }

    #[test]
    fn search_multiple_boards() {
        with_temp_dir(|| {
            let mut board1 = board_store::create_board("Alpha".into()).unwrap();
            let mut board2 = board_store::create_board("Beta".into()).unwrap();

            let mut list1 = CardList::new("Work".into());
            let card1 = Card::new("Shared query term".into());
            card_store::save_card(&board1.id, &card1).unwrap();
            list1.card_ids.push(card1.id.clone());
            list_store::save_list(&board1.id, &list1).unwrap();
            board1.list_order = vec![list1.id.clone()];
            board_store::save_board(&board1).unwrap();

            let mut list2 = CardList::new("Tasks".into());
            let card2 = Card::new("Another shared query term".into());
            card_store::save_card(&board2.id, &card2).unwrap();
            list2.card_ids.push(card2.id.clone());
            list_store::save_list(&board2.id, &list2).unwrap();
            board2.list_order = vec![list2.id.clone()];
            board_store::save_board(&board2).unwrap();

            run_search(&["search", "shared query"]).unwrap();
        });
    }

    #[test]
    fn search_archived_excluded_by_default() {
        with_temp_dir(|| {
            let mut board = board_store::create_board("Proj".into()).unwrap();
            let mut list = CardList::new("To Do".into());

            let mut card = Card::new("archived unique xyz987".into());
            card.archived = true;
            card_store::save_card(&board.id, &card).unwrap();
            list.card_ids.push(card.id.clone());
            list_store::save_list(&board.id, &list).unwrap();
            board.list_order = vec![list.id.clone()];
            board_store::save_board(&board).unwrap();

            // Without --archived: no match (card is archived)
            run_search(&["search", "xyz987"]).unwrap();
        });
    }

    #[test]
    fn search_archived_included_with_flag() {
        with_temp_dir(|| {
            let mut board = board_store::create_board("Proj".into()).unwrap();
            let mut list = CardList::new("To Do".into());

            let mut card = Card::new("archived unique xyz987".into());
            card.archived = true;
            card_store::save_card(&board.id, &card).unwrap();
            list.card_ids.push(card.id.clone());
            list_store::save_list(&board.id, &list).unwrap();
            board.list_order = vec![list.id.clone()];
            board_store::save_board(&board).unwrap();

            run_search(&["search", "xyz987", "--archived"]).unwrap();
        });
    }

    // ── Checklist reorder tests ───────────────────────────────────────────────

    #[test]
    fn checklist_reorder_basic() {
        let mut card = Card::new("Test".into());
        card.checklist = vec![
            ChecklistItem { text: "A".into(), completed: false },
            ChecklistItem { text: "B".into(), completed: false },
            ChecklistItem { text: "C".into(), completed: false },
        ];

        // Swap A↔B (move index 0 down)
        card.checklist.swap(0, 1);
        assert_eq!(card.checklist[0].text, "B");
        assert_eq!(card.checklist[1].text, "A");
        assert_eq!(card.checklist[2].text, "C");
    }

    #[test]
    fn checklist_reorder_move_up() {
        let mut card = Card::new("Test".into());
        card.checklist = vec![
            ChecklistItem { text: "A".into(), completed: true },
            ChecklistItem { text: "B".into(), completed: false },
        ];

        // Move B up: swap indices 1 and 0
        card.checklist.swap(1, 0);
        assert_eq!(card.checklist[0].text, "B");
        assert_eq!(card.checklist[1].text, "A");
        // Completion state should move with the item
        assert!(!card.checklist[0].completed);
        assert!(card.checklist[1].completed);
    }

    #[test]
    fn checklist_reorder_at_top_is_noop() {
        let mut card = Card::new("Test".into());
        card.checklist = vec![
            ChecklistItem { text: "A".into(), completed: false },
            ChecklistItem { text: "B".into(), completed: false },
        ];
        let idx = 0usize;
        // Cannot move up from top (idx == 0)
        if idx > 0 {
            card.checklist.swap(idx, idx - 1);
        }
        assert_eq!(card.checklist[0].text, "A");
        assert_eq!(card.checklist[1].text, "B");
    }

    #[test]
    fn checklist_reorder_at_bottom_is_noop() {
        let mut card = Card::new("Test".into());
        card.checklist = vec![
            ChecklistItem { text: "A".into(), completed: false },
            ChecklistItem { text: "B".into(), completed: false },
        ];
        let idx = 1usize;
        // Cannot move down from last
        if idx + 1 < card.checklist.len() {
            card.checklist.swap(idx, idx + 1);
        }
        assert_eq!(card.checklist[0].text, "A");
        assert_eq!(card.checklist[1].text, "B");
    }

    #[test]
    fn checklist_reorder_empty_is_noop() {
        let mut card = Card::new("Test".into());
        let idx = 0usize;
        // No panic on empty checklist
        if idx + 1 < card.checklist.len() {
            card.checklist.swap(idx, idx + 1);
        }
        assert!(card.checklist.is_empty());
    }

    #[test]
    fn checklist_reorder_persists() {
        with_temp_dir(|| {
            let board = board_store::create_board("Board".into()).unwrap();
            let mut card = Card::new("Task".into());
            card.checklist = vec![
                ChecklistItem { text: "First".into(), completed: false },
                ChecklistItem { text: "Second".into(), completed: true },
                ChecklistItem { text: "Third".into(), completed: false },
            ];
            card_store::save_card(&board.id, &card).unwrap();

            // Reorder: move "Second" up (swap indices 1,0)
            card.checklist.swap(1, 0);
            card.touch();
            card_store::save_card(&board.id, &card).unwrap();

            let loaded = card_store::load_card(&board.id, &card.id).unwrap();
            assert_eq!(loaded.checklist[0].text, "Second");
            assert_eq!(loaded.checklist[1].text, "First");
            assert_eq!(loaded.checklist[2].text, "Third");
            assert!(loaded.checklist[0].completed);
            assert!(!loaded.checklist[1].completed);
        });
    }

    // ── Due date deletion tests ───────────────────────────────────────────────

    #[test]
    fn due_date_set_and_clear() {
        with_temp_dir(|| {
            let board = board_store::create_board("Board".into()).unwrap();
            let mut card = Card::new("Task".into());
            let date = chrono::NaiveDate::from_ymd_opt(2099, 12, 31).unwrap();
            card.due_date = Some(date);
            card_store::save_card(&board.id, &card).unwrap();

            let loaded = card_store::load_card(&board.id, &card.id).unwrap();
            assert_eq!(loaded.due_date, Some(date));

            // Clear due date
            let mut updated = loaded;
            updated.due_date = None;
            updated.touch();
            card_store::save_card(&board.id, &updated).unwrap();

            let final_card = card_store::load_card(&board.id, &updated.id).unwrap();
            assert!(final_card.due_date.is_none());
        });
    }

    #[test]
    fn due_date_clear_when_none_is_noop() {
        let mut card = Card::new("Task".into());
        assert!(card.due_date.is_none());
        // Clearing already-None due date should not panic
        card.due_date = None;
        card.touch();
        assert!(card.due_date.is_none());
    }

    #[test]
    fn due_date_cli_set_and_clear_via_none() {
        with_temp_dir(|| {
            let mut board = board_store::create_board("Board".into()).unwrap();
            let mut list = CardList::new("To Do".into());
            let card = Card::new("Deadline task".into());
            card_store::save_card(&board.id, &card).unwrap();
            list.card_ids.push(card.id.clone());
            list_store::save_list(&board.id, &list).unwrap();
            board.list_order = vec![list.id.clone()];
            board_store::save_board(&board).unwrap();

            // Set due date via CLI
            crate::cli::run(&[
                "cards".to_string(),
                "Board".to_string(),
                "--edit".to_string(),
                "Deadline".to_string(),
                "--due".to_string(),
                "2099-12-31".to_string(),
            ])
            .unwrap();

            let loaded = card_store::load_card(&board.id, &card.id).unwrap();
            assert!(loaded.due_date.is_some());

            // Clear via CLI --due none
            crate::cli::run(&[
                "cards".to_string(),
                "Board".to_string(),
                "--edit".to_string(),
                "Deadline".to_string(),
                "--due".to_string(),
                "none".to_string(),
            ])
            .unwrap();

            let cleared = card_store::load_card(&board.id, &card.id).unwrap();
            assert!(cleared.due_date.is_none());
        });
    }

    // ── card_matches_query corner cases ───────────────────────────────────────

    #[test]
    fn search_matches_checklist_text() {
        use crate::cli::card_matches_query_pub;

        let mut card = Card::new("Task".into());
        card.checklist.push(ChecklistItem { text: "Deploy to staging".into(), completed: false });

        assert!(card_matches_query_pub(&card, "deploy", &[]));
        assert!(card_matches_query_pub(&card, "DEPLOY", &[]));
        assert!(!card_matches_query_pub(&card, "production", &[]));
    }

    #[test]
    fn search_matches_label_name() {
        use crate::cli::card_matches_query_pub;

        let label = Label::new("critical-bug".into(), LabelColor::Red);
        let mut card = Card::new("Task".into());
        card.label_ids.push(label.id.clone());

        assert!(card_matches_query_pub(&card, "critical", &[label.clone()]));
        assert!(card_matches_query_pub(&card, "CRITICAL-BUG", &[label.clone()]));
        assert!(!card_matches_query_pub(&card, "feature", &[label]));
    }

    #[test]
    fn search_regex_special_chars() {
        use crate::cli::card_matches_regex_pub;
        use regex::Regex;

        let mut card = Card::new("Fix bug #42".into());
        card.description = "Error: null pointer at line 10".into();

        let re = Regex::new(r"#\d+").unwrap();
        assert!(card_matches_regex_pub(&card, &re, &[]));

        let re2 = Regex::new(r"null pointer").unwrap();
        assert!(card_matches_regex_pub(&card, &re2, &[]));

        let re3 = Regex::new(r"^\d+$").unwrap();
        assert!(!card_matches_regex_pub(&card, &re3, &[]));
    }

    #[test]
    fn search_regex_matches_checklist() {
        use crate::cli::card_matches_regex_pub;
        use regex::Regex;

        let mut card = Card::new("Task".into());
        card.checklist.push(ChecklistItem { text: "step 1: prepare".into(), completed: false });
        card.checklist.push(ChecklistItem { text: "step 2: execute".into(), completed: false });

        let re = Regex::new(r"step \d+:").unwrap();
        assert!(card_matches_regex_pub(&card, &re, &[]));

        let re_no = Regex::new(r"step 9:").unwrap();
        assert!(!card_matches_regex_pub(&card, &re_no, &[]));
    }

    // ── Label reorder tests ───────────────────────────────────────────────────

    #[test]
    fn label_reorder_basic() {
        let mut board = crate::model::board::BoardMeta::new("Board".into());
        board.labels = vec![
            Label::new("alpha".into(), LabelColor::Red),
            Label::new("beta".into(), LabelColor::Green),
            Label::new("gamma".into(), LabelColor::Blue),
        ];
        // Move "beta" down (swap 1 and 2)
        board.labels.swap(1, 2);
        assert_eq!(board.labels[0].name, "alpha");
        assert_eq!(board.labels[1].name, "gamma");
        assert_eq!(board.labels[2].name, "beta");
    }

    #[test]
    fn label_reorder_persists() {
        with_temp_dir(|| {
            let mut board = board_store::create_board("Board".into()).unwrap();
            board.labels = vec![
                Label::new("first".into(), LabelColor::Red),
                Label::new("second".into(), LabelColor::Green),
                Label::new("third".into(), LabelColor::Blue),
            ];
            board_store::save_board(&board).unwrap();

            // Swap "first" and "second"
            board.labels.swap(0, 1);
            board_store::save_board(&board).unwrap();

            let loaded = board_store::load_board(&board.id).unwrap();
            assert_eq!(loaded.labels[0].name, "second");
            assert_eq!(loaded.labels[1].name, "first");
            assert_eq!(loaded.labels[2].name, "third");
        });
    }

    #[test]
    fn label_reorder_move_last_to_first() {
        with_temp_dir(|| {
            let mut board = board_store::create_board("Board".into()).unwrap();
            board.labels = vec![
                Label::new("a".into(), LabelColor::Red),
                Label::new("b".into(), LabelColor::Green),
                Label::new("c".into(), LabelColor::Blue),
            ];
            // Move "c" up twice: c,b then c,a  (bubble-up style)
            board.labels.swap(2, 1);
            board.labels.swap(1, 0);
            board_store::save_board(&board).unwrap();

            let loaded = board_store::load_board(&board.id).unwrap();
            assert_eq!(loaded.labels[0].name, "c");
            assert_eq!(loaded.labels[1].name, "a");
            assert_eq!(loaded.labels[2].name, "b");
        });
    }

    #[test]
    fn label_reorder_at_top_is_noop() {
        let mut board = crate::model::board::BoardMeta::new("Board".into());
        board.labels = vec![
            Label::new("only".into(), LabelColor::Red),
            Label::new("two".into(), LabelColor::Green),
        ];
        let idx = 0usize;
        // Attempting to move up from index 0: idx > 0 is false, no swap
        if idx > 0 {
            board.labels.swap(idx, idx - 1);
        }
        assert_eq!(board.labels[0].name, "only");
        assert_eq!(board.labels[1].name, "two");
    }

    #[test]
    fn label_reorder_at_bottom_is_noop() {
        let mut board = crate::model::board::BoardMeta::new("Board".into());
        board.labels = vec![
            Label::new("one".into(), LabelColor::Red),
            Label::new("two".into(), LabelColor::Green),
        ];
        let idx = 1usize;
        // Attempting to move down from last index: no swap
        if idx + 1 < board.labels.len() {
            board.labels.swap(idx, idx + 1);
        }
        assert_eq!(board.labels[0].name, "one");
        assert_eq!(board.labels[1].name, "two");
    }

    #[test]
    fn label_reorder_preserves_ids() {
        let mut board = crate::model::board::BoardMeta::new("Board".into());
        let l1 = Label::new("alpha".into(), LabelColor::Red);
        let l2 = Label::new("beta".into(), LabelColor::Green);
        let id1 = l1.id.clone();
        let id2 = l2.id.clone();
        board.labels = vec![l1, l2];
        board.labels.swap(0, 1);
        // IDs travel with their labels
        assert_eq!(board.labels[0].id, id2);
        assert_eq!(board.labels[1].id, id1);
    }

    #[test]
    fn move_card_between_lists_preserves_position() {
        with_temp_dir(|| {
            let meta = board_store::create_board("Board".into()).unwrap();

            let mut list_a = CardList::new("A".into());
            let mut list_b = CardList::new("B".into());

            let c1 = Card::new("card1".into());
            let c2 = Card::new("card2".into());
            let c3 = Card::new("card3".into());
            let c4 = Card::new("card4".into());

            card_store::save_card(&meta.id, &c1).unwrap();
            card_store::save_card(&meta.id, &c2).unwrap();
            card_store::save_card(&meta.id, &c3).unwrap();
            card_store::save_card(&meta.id, &c4).unwrap();

            list_a.card_ids = vec![c1.id.clone(), c2.id.clone(), c3.id.clone()];
            list_b.card_ids = vec![c4.id.clone()];

            // Move c2 (index 1) from list_a to list_b at same position
            let ci = 1;
            let card_id = list_a.card_ids.remove(ci);
            let insert_at = ci.min(list_b.card_ids.len());
            list_b.card_ids.insert(insert_at, card_id);

            assert_eq!(list_a.card_ids, vec![c1.id.clone(), c3.id.clone()]);
            assert_eq!(
                list_b.card_ids,
                vec![c4.id.clone(), c2.id.clone()]
            );
            assert_eq!(insert_at, 1);
        });
    }

    #[test]
    fn move_card_clamps_to_end_when_dst_shorter() {
        // Moving card at index 4 to a list with only 2 items → inserts at index 2
        let mut src = vec!["a", "b", "c", "d", "e"];
        let mut dst = vec!["x", "y"];
        let ci = 4;
        let card = src.remove(ci);
        let insert_at = ci.min(dst.len());
        dst.insert(insert_at, card);
        assert_eq!(dst, vec!["x", "y", "e"]);
        assert_eq!(insert_at, 2);
    }
}
