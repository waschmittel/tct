use super::*;

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
        crate::test_support::put_cards(
            &board1.id,
            &list1,
            &mut [Card::new("Shared query term".into())],
        );

        let list2 = crate::test_support::add_list(&mut board2, "Tasks");
        crate::test_support::put_cards(
            &board2.id,
            &list2,
            &mut [Card::new("Another shared query term".into())],
        );

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

// ── card_matches_query corner cases ───────────────────────────────────────

#[test]
fn search_matches_checklist_text() {
    let mut card = Card::new("Task".into());
    card.checklist.push(ChecklistItem {
        text: "Deploy to staging".into(),
        completed: false,
    });

    assert!(card.matches_search("deploy", &[]));
    assert!(card.matches_search("DEPLOY", &[]));
    assert!(!card.matches_search("production", &[]));
}

#[test]
fn search_matches_label_name() {
    let label = Label::new("critical-bug".into(), LabelColor::Red);
    let mut card = Card::new("Task".into());
    card.label_ids.push(label.id.clone());

    assert!(card.matches_search("critical", std::slice::from_ref(&label)));
    assert!(card.matches_search("CRITICAL-BUG", std::slice::from_ref(&label)));
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
    card.checklist.push(ChecklistItem {
        text: "step 1: prepare".into(),
        completed: false,
    });
    card.checklist.push(ChecklistItem {
        text: "step 2: execute".into(),
        completed: false,
    });

    let re = Regex::new(r"step \d+:").unwrap();
    assert!(card_matches_regex_pub(&card, &re, &[]));

    let re_no = Regex::new(r"step 9:").unwrap();
    assert!(!card_matches_regex_pub(&card, &re_no, &[]));
}
