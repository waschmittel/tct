mod app;
mod board_directory;
mod board_editor;
mod cli;
mod command;
mod dialog;
mod event;
mod input;
mod insert;
mod model;
mod storage;
mod term_caps;
mod term_out;
mod ui;

#[cfg(test)]
mod test_support;

use std::time::Duration;

use app::App;
use event::{AppEvent, EventHandler};

/// Build-stamped version: a release tag (set by `build.rs`) or a
/// `snap-<date>` for untagged builds.
pub const VERSION: &str = env!("TCT_VERSION");

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();

    // Version flag (check before anything else so it always works)
    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!("tct {VERSION}");
        return Ok(());
    }

    // Help flag (check before anything else so it always works)
    if args.iter().any(|a| a == "--help" || a == "-h") || args.first().map(|a| a.as_str()) == Some("help") {
        cli::print_help();
        return Ok(());
    }

    // Subcommand mode: first arg is a non-flag word
    if args.first().map(|a| !a.starts_with('-')).unwrap_or(false) {
        if let Err(e) = cli::run(&args) {
            errln!("Error: {e}");
            std::process::exit(1);
        }
        return Ok(());
    }

    // TUI mode — resolve optional --board flag before initialising the terminal
    let open_board_id = cli::resolve_board_flag(&args)?;

    let mut terminal = ratatui::init();
    // The Linux console has no alternate screen buffer, so entering it does
    // not blank the screen — without a forced clear, the diff renderer
    // never repaints "empty" cells and stale shell output shines through.
    terminal.clear()?;
    let mut app = App::new(open_board_id)?;
    // Only the real TUI entry consults the environment — App::new defaults
    // to full caps so tests and goldens stay deterministic.
    app.caps = term_caps::TermCaps::detect();
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
    use crate::model::id::{CardId, ListId};
    
    use crate::storage::{board_store, card_store};
    #[allow(unused_imports)]
    use regex::Regex;

    use crate::test_support::with_temp_dir;

    #[test]
    fn version_is_stamped() {
        // build.rs always sets a non-empty TCT_VERSION (tag or snap-…).
        assert!(!crate::VERSION.is_empty());
    }

    #[test]
    fn board_roundtrip() {
        with_temp_dir(|| {
            let board = board_store::create_board("My Board".into()).unwrap();
            let loaded = board_store::load_board(&board.id).unwrap();
            assert_eq!(loaded.name, "My Board");
            assert!(loaded.lists.is_empty());

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
    fn lists_persist_inline_in_the_board() {
        with_temp_dir(|| {
            let mut board = board_store::create_board("Board".into()).unwrap();
            let list_id = crate::test_support::add_list(&mut board, "To Do");

            let loaded = board_store::load_board(&board.id).unwrap();

            assert_eq!(loaded.lists.len(), 1);
            assert_eq!(loaded.lists[0].id, list_id);
            assert_eq!(loaded.lists[0].name, "To Do");
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

    /// Moving a Card between Lists is one Card write; membership is derived
    /// from `list_id`, never stored on the List (ADR-0006).
    #[test]
    fn full_lifecycle() {
        with_temp_dir(|| {
            let mut board = board_store::create_board("Project".into()).unwrap();
            let todo = crate::test_support::add_list(&mut board, "To Do");
            let done = crate::test_support::add_list(&mut board, "Done");
            let mut cards = vec![Card::new("Task 1".into()), Card::new("Task 2".into())];
            crate::test_support::put_cards(&board.id, &todo, &mut cards);

            cards[1].list_id = done.clone();
            cards[1].position = 1.0;
            card_store::save_card(&board.id, &cards[1]).unwrap();

            let stored = card_store::load_all_cards(&board.id).unwrap();
            let lists = crate::model::list::build_lists(&board, &stored, false);
            assert_eq!(lists[0].card_ids, vec![cards[0].id.clone()]);
            assert_eq!(lists[1].card_ids, vec![cards[1].id.clone()]);
        });
    }

    // ── CLI search tests ──────────────────────────────────────────────────────

    fn run_search(args: &[&str]) -> anyhow::Result<()> {
        let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
        crate::cli::run(&args)
    }

    fn setup_search_fixture() -> (crate::model::board::BoardMeta, Card, Card) {
        let mut board = board_store::create_board("Alpha".into()).unwrap();

        let todo = crate::test_support::add_list(&mut board, "To Do");
        let done = crate::test_support::add_list(&mut board, "Done");

        let mut card1 = Card::new("Fix login bug".into());
        card1.description = "Auth token expires too early".into();
        card1.checklist.push(ChecklistItem { text: "Reproduce issue".into(), completed: false });
        card1.checklist.push(ChecklistItem { text: "Write failing test".into(), completed: false });

        let mut card2 = Card::new("Redesign dashboard".into());
        card2.description = "New layout with login metrics".into();

        crate::test_support::put_cards(&board.id, &todo, std::slice::from_mut(&mut card1));
        crate::test_support::put_cards(&board.id, &done, std::slice::from_mut(&mut card2));

        (board, card1, card2)
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

            let list1 = crate::test_support::add_list(&mut board1, "Work");
            crate::test_support::put_cards(&board1.id, &list1, &mut [Card::new("Shared query term".into())]);

            let list2 = crate::test_support::add_list(&mut board2, "Tasks");
            crate::test_support::put_cards(&board2.id, &list2, &mut [Card::new("Another shared query term".into())]);

            run_search(&["search", "shared query"]).unwrap();
        });
    }

    #[test]
    fn search_archived_excluded_by_default() {
        with_temp_dir(|| {
            let mut board = board_store::create_board("Proj".into()).unwrap();
            let list = crate::test_support::add_list(&mut board, "To Do");
            let mut card = Card::new("archived unique xyz987".into());
            card.archived = true;
            crate::test_support::put_cards(&board.id, &list, std::slice::from_mut(&mut card));

            // Without --archived: no match (card is archived)
            run_search(&["search", "xyz987"]).unwrap();
        });
    }

    #[test]
    fn search_archived_included_with_flag() {
        with_temp_dir(|| {
            let mut board = board_store::create_board("Proj".into()).unwrap();
            let list = crate::test_support::add_list(&mut board, "To Do");
            let mut card = Card::new("archived unique xyz987".into());
            card.archived = true;
            crate::test_support::put_cards(&board.id, &list, std::slice::from_mut(&mut card));

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
            let list = crate::test_support::add_list(&mut board, "To Do");
            let mut card = Card::new("Deadline task".into());
            crate::test_support::put_cards(&board.id, &list, std::slice::from_mut(&mut card));

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
        let mut card = Card::new("Task".into());
        card.checklist.push(ChecklistItem { text: "Deploy to staging".into(), completed: false });

        assert!(card.matches_search("deploy", &[]));
        assert!(card.matches_search("DEPLOY", &[]));
        assert!(!card.matches_search("production", &[]));
    }

    #[test]
    fn search_matches_label_name() {
        let label = Label::new("critical-bug".into(), LabelColor::Red);
        let mut card = Card::new("Task".into());
        card.label_ids.push(label.id.clone());

        assert!(card.matches_search("critical", &[label.clone()]));
        assert!(card.matches_search("CRITICAL-BUG", &[label.clone()]));
        assert!(!card.matches_search("feature", &[label]));
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

    use crate::board_editor::BoardEditor;
    use crate::command::{Command, MoveDir};
    use crate::model::board::{BoardMeta, ListMeta};

    /// Build a two-list board on disk and load an editor. `a`/`b` give the
    /// titles for list A / list B; cards get ascending integer positions.
    fn two_list_editor(a: &[&str], b: &[&str]) -> (BoardEditor, Vec<CardId>, Vec<CardId>) {
        let mut meta = BoardMeta::new("Board".into());
        let la = ListId::generate();
        let lb = ListId::generate();
        meta.lists.push(ListMeta { id: la.clone(), name: "A".into(), archived: false });
        meta.lists.push(ListMeta { id: lb.clone(), name: "B".into(), archived: false });
        board_store::save_board(&meta).unwrap();
        let save = |titles: &[&str], lid: &ListId| {
            titles
                .iter()
                .enumerate()
                .map(|(i, t)| {
                    let mut c = Card::new((*t).into());
                    c.list_id = lid.clone();
                    c.position = (i + 1) as f64;
                    card_store::save_card(&meta.id, &c).unwrap();
                    c.id
                })
                .collect()
        };
        let a_ids = save(a, &la);
        let b_ids = save(b, &lb);
        (BoardEditor::load(&meta.id).unwrap(), a_ids, b_ids)
    }

    #[test]
    fn move_card_between_lists_lands_at_same_visible_slot() {
        with_temp_dir(|| {
            let (mut ed, a, b) = two_list_editor(&["a0", "a1", "a2"], &["b0"]);
            // Move a1 (visible slot 1) right into list B.
            ed.apply(Command::MoveCard { card_id: a[1].clone(), direction: MoveDir::Right })
                .unwrap();
            assert_eq!(ed.board().lists[0].card_ids, vec![a[0].clone(), a[2].clone()]);
            assert_eq!(ed.board().lists[1].card_ids, vec![b[0].clone(), a[1].clone()]);
            assert_eq!(ed.board().cards[&a[1]].list_id, ed.board().lists[1].id);
            // Persisted.
            ed.reload().unwrap();
            assert_eq!(ed.board().lists[1].card_ids, vec![b[0].clone(), a[1].clone()]);
        });
    }

    #[test]
    fn move_card_clamps_to_end_when_dst_shorter() {
        with_temp_dir(|| {
            let (mut ed, a, b) = two_list_editor(&["a0", "a1", "a2", "a3", "a4"], &["b0", "b1"]);
            // a4 is at visible slot 4; list B has only 2 slots → lands at end.
            ed.apply(Command::MoveCard { card_id: a[4].clone(), direction: MoveDir::Right })
                .unwrap();
            assert_eq!(ed.board().lists[1].card_ids, vec![b[0].clone(), b[1].clone(), a[4].clone()]);
            ed.reload().unwrap();
            assert_eq!(ed.board().lists[1].card_ids, vec![b[0].clone(), b[1].clone(), a[4].clone()]);
        });
    }
}
