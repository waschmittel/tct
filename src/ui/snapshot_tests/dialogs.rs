use super::*;

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
        // The history dialog stays visible underneath, with real timestamps.
        let stamp = regex::Regex::new(r"\d{4}-\d{2}-\d{2} \d{2}:\d{2}(:\d{2})?").unwrap();
        let text = render_to_string(&app);
        insta::assert_snapshot!(stamp.replace_all(&text, "YYYY-MM-DD HH:MM"));
    });
}

#[test]
fn snapshot_card_history_dialog_empty() {
    with_temp_dir(|| {
        let id = seed_demo_board();
        let mut app = App::new(Some(id)).unwrap();
        press(&mut app, KeyCode::Char('h'));
        assert_eq!(app.mode, AppMode::Dialog);
        let stamp = regex::Regex::new(r"\d{4}-\d{2}-\d{2} \d{2}:\d{2}(:\d{2})?").unwrap();
        let text = render_to_string(&app);
        insta::assert_snapshot!(stamp.replace_all(&text, "YYYY-MM-DD HH:MM"));
    });
}

#[test]
fn snapshot_card_history_dialog() {
    use crate::model::card::HistoryField;
    with_temp_dir(|| {
        let id = seed_demo_board();
        let mut app = App::new(Some(id)).unwrap();
        {
            let board = app.board_mut().unwrap();
            let card_id = board.current_card_id().cloned().unwrap();
            let card = board.cards.get_mut(&card_id).unwrap();
            card.log_change(
                HistoryField::Title,
                "Edit title",
                Some("Fix login".into()),
                Some("Fix login flow".into()),
            );
            card.log_change(
                HistoryField::Labels,
                "Assign label 'bug'",
                None,
                Some("bug".into()),
            );
            card.log_change(
                HistoryField::Description,
                "Edit description",
                Some("Token expires.\n\n- check refresh".into()),
                Some("Token expires too early.\n\n- check refresh\n- add test".into()),
            );
        }
        press(&mut app, KeyCode::Char('h'));
        assert_eq!(app.mode, AppMode::Dialog);
        let stamp = regex::Regex::new(r"\d{4}-\d{2}-\d{2} \d{2}:\d{2}(:\d{2})?").unwrap();
        let text = render_to_string(&app);
        insta::assert_snapshot!(stamp.replace_all(&text, "YYYY-MM-DD HH:MM"));
    });
}

/// (column, row, width, height) of the history popup, found by its title.
fn history_popup_rect(text: &str) -> (usize, usize, usize, usize) {
    let lines: Vec<&str> = text.lines().collect();
    let row = lines.iter().position(|l| l.contains("History —")).unwrap();
    let chars: Vec<char> = lines[row].chars().collect();
    let col = chars.iter().position(|&c| c == '┌').unwrap();
    let width = chars[col..].iter().position(|&c| c == '┐').unwrap() + 1;
    let bottom = lines[row..]
        .iter()
        .position(|l| l.chars().nth(col) == Some('└'))
        .unwrap();
    (col, row, width, bottom + 1)
}

fn history_app() -> App {
    let id = seed_demo_board();
    let mut app = App::new(Some(id)).unwrap();
    set_long_history(&mut app);
    press(&mut app, KeyCode::Char('h'));
    assert_eq!(app.mode, AppMode::Dialog);
    app
}

/// The popup grows with the terminal: 12 columns left and right, 4 rows
/// below 40 rows high and 8 above, at most 200 columns wide.
#[test]
fn card_history_popup_scales_with_terminal() {
    with_temp_dir(|| {
        let app = history_app();
        // (terminal w, h) -> (x, y, w, h)
        for ((tw, th), want) in [
            ((100, 30), (2, 2, 96, 26)),
            ((140, 40), (12, 2, 116, 36)),
            ((140, 41), (12, 4, 116, 33)),
            ((200, 60), (12, 4, 176, 52)),
            ((260, 60), (30, 4, 200, 52)),
        ] {
            let got = history_popup_rect(&render_to_string_sized(&app, tw, th));
            assert_eq!(got, want, "terminal {tw}x{th}");
        }
    });
}

#[test]
fn snapshot_card_history_dialog_wide() {
    with_temp_dir(|| {
        let app = history_app();
        let stamp = regex::Regex::new(r"\d{4}-\d{2}-\d{2} \d{2}:\d{2}(:\d{2})?").unwrap();
        let text = render_to_string_sized(&app, 160, 50);
        insta::assert_snapshot!(stamp.replace_all(&text, "YYYY-MM-DD HH:MM"));
    });
}

#[test]
fn snapshot_card_history_dialog_collapsed_diff() {
    use crate::model::card::HistoryField;
    with_temp_dir(|| {
        let id = seed_demo_board();
        let mut app = App::new(Some(id)).unwrap();
        {
            let board = app.board_mut().unwrap();
            let card_id = board.current_card_id().cloned().unwrap();
            let card = board.cards.get_mut(&card_id).unwrap();
            let old = "line 01\nline 02\nline 03\nline 04\nline 05\nold target\nline 06\nline 07\nline 08\nline 09\nline 10";
            let new = "line 01\nline 02\nline 03\nline 04\nline 05\nnew target\nline 06\nline 07\nline 08\nline 09\nline 10";
            card.log_change(
                HistoryField::Description,
                "Edit description",
                Some(old.into()),
                Some(new.into()),
            );
        }
        press(&mut app, KeyCode::Char('h'));
        assert_eq!(app.mode, AppMode::Dialog);
        let stamp = regex::Regex::new(r"\d{4}-\d{2}-\d{2} \d{2}:\d{2}(:\d{2})?").unwrap();
        let text = render_to_string(&app);
        insta::assert_snapshot!(stamp.replace_all(&text, "YYYY-MM-DD HH:MM"));
    });
}

#[test]
fn snapshot_card_history_dialog_scroll_bottom() {
    with_temp_dir(|| {
        let id = seed_demo_board();
        let mut app = App::new(Some(id)).unwrap();
        set_long_history(&mut app);
        press(&mut app, KeyCode::Char('h'));
        assert_eq!(app.mode, AppMode::Dialog);
        let _ = render_to_string(&app);
        for _ in 0..20 {
            press(&mut app, KeyCode::PageDown);
        }
        let stamp = regex::Regex::new(r"\d{4}-\d{2}-\d{2} \d{2}:\d{2}(:\d{2})?").unwrap();
        let text = render_to_string(&app);
        insta::assert_snapshot!(stamp.replace_all(&text, "YYYY-MM-DD HH:MM"));
    });
}

#[test]
fn snapshot_card_history_dialog_scroll_up() {
    with_temp_dir(|| {
        let id = seed_demo_board();
        let mut app = App::new(Some(id)).unwrap();
        set_long_history(&mut app);
        press(&mut app, KeyCode::Char('h'));
        assert_eq!(app.mode, AppMode::Dialog);
        let _ = render_to_string(&app);
        for _ in 0..20 {
            press(&mut app, KeyCode::PageDown);
        }
        // Immediately scroll up
        press(&mut app, KeyCode::PageUp);
        let stamp = regex::Regex::new(r"\d{4}-\d{2}-\d{2} \d{2}:\d{2}(:\d{2})?").unwrap();
        let text = render_to_string(&app);
        insta::assert_snapshot!(stamp.replace_all(&text, "YYYY-MM-DD HH:MM"));
    });
}

#[test]
fn snapshot_label_picker() {
    with_temp_dir(|| {
        let id = seed_board_with_multiple_labels();
        let mut app = App::new(Some(id)).unwrap();
        press(&mut app, KeyCode::Char('l'));
        assert_eq!(app.mode, AppMode::Dialog);
        insta::assert_snapshot!(render_to_string(&app));
    });
}

#[test]
fn snapshot_label_picker_card_detail() {
    with_temp_dir(|| {
        let id = seed_board_with_multiple_labels();
        let mut app = App::new(Some(id)).unwrap();
        press(&mut app, KeyCode::Enter);
        assert_eq!(app.mode, AppMode::CardDetail);
        press(&mut app, KeyCode::Char('l'));
        assert_eq!(app.mode, AppMode::Dialog);
        insta::assert_snapshot!(render_to_string(&app));
    });
}

#[test]
fn snapshot_label_manager() {
    with_temp_dir(|| {
        let id = seed_board_with_multiple_labels();
        let mut app = App::new(Some(id)).unwrap();
        crate::input::handle_input(
            &mut app,
            KeyEvent::new(KeyCode::Char('L'), KeyModifiers::SHIFT),
        )
        .unwrap();
        assert_eq!(app.mode, AppMode::Dialog);
        insta::assert_snapshot!(render_to_string(&app));
    });
}

/// Renaming a label to another label's name asks first, then merges through
/// the real input path: the manager reopens without the renamed label and
/// the card that carried both shows the survivor once.
#[test]
fn snapshot_label_rename_merges() {
    with_temp_dir(|| {
        let id = seed_board_with_multiple_labels();
        let mut app = App::new(Some(id)).unwrap();
        press(&mut app, KeyCode::Char('L'));
        for _ in 0..3 {
            press(&mut app, KeyCode::Down); // bug, feature, docs, urgent
        }
        press(&mut app, KeyCode::Char('e'));
        for _ in 0.."urgent".len() {
            press(&mut app, KeyCode::Backspace);
        }
        for c in "bug".chars() {
            press(&mut app, KeyCode::Char(c));
        }
        press(&mut app, KeyCode::Enter);
        // Nothing merged yet: the confirmation is up.
        assert_eq!(app.board().unwrap().meta.labels.len(), 4);
        let confirm = render_to_string(&app);
        assert!(confirm.contains("Merge 'urgent' into 'bug'?"));
        insta::assert_snapshot!("label_rename_merges_confirm", confirm);
        press(&mut app, KeyCode::Char('y'));
        assert_eq!(app.mode, AppMode::Dialog);
        assert_eq!(label_names(&app), ["bug", "feature", "docs"]);
        let manager = render_to_string(&app);
        assert!(!manager.contains("urgent"));
        insta::assert_snapshot!("label_rename_merges_manager", manager);
        press(&mut app, KeyCode::Esc);
        assert_eq!(app.mode, AppMode::Normal);
        let board = render_to_string(&app);
        assert!(!board.contains("urgent"));
        insta::assert_snapshot!("label_rename_merges_board", board);
    });
}

/// Declining the merge leaves both labels and their cards untouched.
#[test]
fn label_rename_merge_can_be_cancelled() {
    with_temp_dir(|| {
        let id = seed_board_with_multiple_labels();
        for decline in [KeyCode::Char('n'), KeyCode::Esc] {
            let mut app = App::new(Some(id.clone())).unwrap();
            press(&mut app, KeyCode::Char('L'));
            for _ in 0..3 {
                press(&mut app, KeyCode::Down);
            }
            press(&mut app, KeyCode::Char('e'));
            for _ in 0.."urgent".len() {
                press(&mut app, KeyCode::Backspace);
            }
            for c in "bug".chars() {
                press(&mut app, KeyCode::Char(c));
            }
            press(&mut app, KeyCode::Enter);
            press(&mut app, decline);
            assert_eq!(app.mode, AppMode::Dialog);
            assert_eq!(label_names(&app), ["bug", "feature", "docs", "urgent"]);
            assert!(render_to_string(&app).contains("urgent"));
        }
    });
}

fn label_names(app: &App) -> Vec<String> {
    app.board()
        .unwrap()
        .meta
        .labels
        .iter()
        .map(|l| l.name.clone())
        .collect()
}

/// A name that is not taken stays a plain rename (no merge).
#[test]
fn label_rename_to_new_name_keeps_labels() {
    with_temp_dir(|| {
        let id = seed_board_with_multiple_labels();
        let mut app = App::new(Some(id)).unwrap();
        press(&mut app, KeyCode::Char('L'));
        press(&mut app, KeyCode::Char('e'));
        press(&mut app, KeyCode::Char('x'));
        press(&mut app, KeyCode::Enter);
        assert_eq!(label_names(&app), ["bugx", "feature", "docs", "urgent"]);
    });
}

#[test]
fn snapshot_board_configuration_dialog() {
    with_temp_dir(|| {
        seed_selector_boards();
        let mut app = App::new(None).unwrap();
        crate::input::handle_input(
            &mut app,
            KeyEvent::new(KeyCode::Char('C'), KeyModifiers::SHIFT),
        )
        .unwrap();
        assert_eq!(app.mode, AppMode::Dialog);
        insta::assert_snapshot!(render_to_string(&app));
    });
}

#[test]
fn snapshot_confirm_cancel_edit() {
    with_temp_dir(|| {
        let id = seed_demo_board();
        let mut app = App::new(Some(id)).unwrap();
        press(&mut app, KeyCode::Enter);
        assert_eq!(app.mode, AppMode::CardDetail);
        press(&mut app, KeyCode::Char('e'));
        assert_eq!(app.mode, AppMode::Insert);
        press(&mut app, KeyCode::Char('x'));
        press(&mut app, KeyCode::Esc);
        assert_eq!(app.mode, AppMode::Dialog);
        insta::assert_snapshot!(render_to_string(&app));
    });
}

#[test]
fn snapshot_confirm_archive_card() {
    with_temp_dir(|| {
        let id = seed_demo_board();
        let mut app = App::new(Some(id)).unwrap();
        press(&mut app, KeyCode::Char('a'));
        assert_eq!(app.mode, AppMode::Dialog);
        insta::assert_snapshot!(render_to_string(&app));
    });
}

#[test]
fn snapshot_confirm_restore_card() {
    with_temp_dir(|| {
        let id = seed_demo_board();
        let mut app = App::new(Some(id)).unwrap();
        press(&mut app, KeyCode::Char('a'));
        press(&mut app, KeyCode::Char('y'));
        press(&mut app, KeyCode::Char('v'));
        assert_eq!(app.mode, AppMode::Dialog);
        press(&mut app, KeyCode::Enter);
        assert_eq!(app.mode, AppMode::Dialog);
        insta::assert_snapshot!(render_to_string(&app));
    });
}

#[test]
fn snapshot_confirm_delete_board() {
    with_temp_dir(|| {
        let id = seed_demo_board();
        let mut app = App::new(Some(id)).unwrap();
        press(&mut app, KeyCode::Char('b'));
        press(&mut app, KeyCode::Char('a'));
        press(&mut app, KeyCode::Char('y'));
        press(&mut app, KeyCode::Char('v'));
        press(&mut app, KeyCode::Char('x'));
        assert_eq!(app.mode, AppMode::Dialog);
        insta::assert_snapshot!(render_to_string(&app));
    });
}

#[test]
fn snapshot_archived_cards_dialog() {
    with_temp_dir(|| {
        let id = seed_demo_board();
        let mut app = App::new(Some(id)).unwrap();
        press(&mut app, KeyCode::Char('a'));
        press(&mut app, KeyCode::Char('y'));
        press(&mut app, KeyCode::Char('v'));
        assert_eq!(app.mode, AppMode::Dialog);
        let date_re = regex::Regex::new(r"\d{4}-\d{2}-\d{2}").unwrap();
        let text = render_to_string(&app);
        insta::assert_snapshot!(date_re.replace_all(&text, "YYYY-MM-DD"));
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
        confirm_delete_board::ConfirmDeleteBoard, confirm_restore_board::ConfirmRestoreBoard,
        confirm_restore_card::ConfirmRestoreCard, confirm_restore_list::ConfirmRestoreList,
        label_manager::LabelManager, label_picker::LabelPicker,
    };
    use crate::model::board::BoardMeta;
    use crate::model::list::CardList;
    with_temp_dir(|| {
        let id = seed_demo_board();
        let dialogs: Vec<Box<dyn Dialog>> = vec![
            Box::new(ConfirmArchiveCard),
            Box::new(ConfirmArchiveList),
            Box::new(ConfirmCancelEdit),
            Box::new(ConfirmClearDueDate),
            Box::new(ConfirmRestoreCard {
                card: Card::new("Test card".into()),
                cards: vec![],
                selected: 0,
            }),
            Box::new(ConfirmRestoreList {
                list: CardList::new("Test list".into()),
                lists: vec![],
                selected: 0,
            }),
            Box::new(ConfirmRestoreBoard {
                board: BoardMeta::new("Test board".into()),
                boards: vec![],
                selected: 0,
            }),
            Box::new(ConfirmDeleteBoard {
                board: BoardMeta::new("Test board".into()),
                boards: vec![],
                selected: 0,
            }),
            Box::new(CardHistory::new()),
            Box::new(LabelPicker { selected_idx: 0 }),
            Box::new(LabelManager {
                selected_idx: 0,
                from_picker: false,
            }),
            Box::new(ArchivedCards {
                cards: vec![],
                selected: 0,
            }),
            Box::new(ArchivedLists {
                lists: vec![],
                selected: 0,
            }),
            Box::new(ArchivedBoards {
                boards: vec![],
                selected: 0,
            }),
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
                let line = text.lines().find(|l| l.contains(desc)).unwrap_or_else(|| {
                    panic!("{}: '{desc}' wrapped or missing\n{text}", help.title)
                });
                let k = line
                    .find(keys)
                    .unwrap_or_else(|| panic!("{}: '{keys}' missing", help.title));
                let gap = &line[k + keys.len()..line.find(desc).unwrap()];
                assert!(
                    gap.starts_with("  "),
                    "{}: keys crowd '{desc}'\n{text}",
                    help.title
                );
            }
            let top = text.lines().position(|l| l.contains(help.title)).unwrap();
            let bottom = text.lines().position(|l| l.contains("Esc:close")).unwrap();
            assert_eq!(
                bottom - top,
                help.rows.len() + 2,
                "{}: popup height",
                help.title
            );
        }
    });
}

/// Dialogs opened from the card detail pop over it instead of replacing it.
#[test]
fn dialogs_from_card_detail_keep_the_detail_visible() {
    with_temp_dir(|| {
        let id = seed_demo_board();
        let mut app = App::new(Some(id)).unwrap();
        let card_id = app.board().unwrap().current_card_id().unwrap().clone();
        app.board_mut()
            .unwrap()
            .cards
            .get_mut(&card_id)
            .unwrap()
            .due_date = Some(chrono::NaiveDate::from_ymd_opt(2099, 1, 1).unwrap());
        press(&mut app, KeyCode::Enter);
        assert_eq!(app.mode, AppMode::CardDetail);

        // (The history dialog is large enough to cover the description.)
        for (key, dialog_text) in [('U', "Clear due date of"), ('l', "Labels")] {
            let shift = if key.is_ascii_uppercase() {
                KeyModifiers::SHIFT
            } else {
                KeyModifiers::NONE
            };
            crate::input::handle_input(&mut app, KeyEvent::new(KeyCode::Char(key), shift)).unwrap();
            assert_eq!(app.mode, AppMode::Dialog, "{key}");
            let text = render_to_string(&app);
            assert!(text.contains(dialog_text), "{key}: dialog missing\n{text}");
            assert!(
                text.contains("Token expires too early."),
                "{key}: detail hidden\n{text}"
            );
            press(&mut app, KeyCode::Esc);
            assert_eq!(app.mode, AppMode::CardDetail);
        }
    });
}

/// From the board view the same dialogs sit over the board, not a detail.
#[test]
fn dialogs_from_board_view_do_not_open_the_detail() {
    with_temp_dir(|| {
        let id = seed_demo_board();
        let mut app = App::new(Some(id)).unwrap();
        press(&mut app, KeyCode::Char('l'));
        assert_eq!(app.mode, AppMode::Dialog);
        assert!(!render_to_string(&app).contains("Token expires too early."));
    });
}

/// Help opened from the card detail, from a dialog over it, or from the
/// description editor pops over them instead of replacing them.
#[test]
fn help_from_card_detail_keeps_the_detail_visible() {
    with_temp_dir(|| {
        let id = seed_demo_board();
        let mut app = App::new(Some(id)).unwrap();
        press(&mut app, KeyCode::Enter);

        press(&mut app, KeyCode::Char('?'));
        assert_eq!(app.mode, AppMode::Help);
        let text = render_to_string(&app);
        assert!(text.contains("Help — Card Detail"), "{text}");
        assert!(text.contains("Fix login flow"), "detail hidden\n{text}");
        press(&mut app, KeyCode::Esc);
        assert_eq!(app.mode, AppMode::CardDetail);

        press(&mut app, KeyCode::Char('l'));
        press(&mut app, KeyCode::Char('?'));
        assert_eq!(app.mode, AppMode::Help);
        let text = render_to_string(&app);
        assert!(
            text.contains("Help —") && text.contains("Fix login flow"),
            "{text}"
        );
        assert!(
            text.contains("Labels"),
            "dialog hidden under its help\n{text}"
        );
    });
}

#[test]
fn snapshot_export_dialog_shows_detected_language() {
    insta::assert_snapshot!(export_dialog_screen("Deutsch"));
}

#[test]
fn snapshot_export_dialog_default_language() {
    insta::assert_snapshot!(export_dialog_screen("English (default)"));
}

#[test]
fn snapshot_no_lists_notice() {
    with_temp_dir(|| {
        let meta = board_store::create_board("Empty".into()).unwrap();
        let mut app = App::new(Some(meta.id)).unwrap();
        press(&mut app, KeyCode::Char('n'));
        assert_eq!(app.mode, AppMode::Dialog);
        insta::assert_snapshot!(render_to_string(&app));
    });
}

#[test]
fn snapshot_help_no_lists_notice() {
    with_temp_dir(|| {
        let meta = board_store::create_board("Empty".into()).unwrap();
        let mut app = App::new(Some(meta.id)).unwrap();
        app.version = "vTEST";
        press(&mut app, KeyCode::Char('n'));
        press(&mut app, KeyCode::Char('?'));
        assert_eq!(app.mode, AppMode::Help);
        insta::assert_snapshot!(render_to_string(&app));
    });
}

#[test]
fn snapshot_label_filter_picker() {
    with_temp_dir(|| {
        let id = seed_board_with_multiple_labels();
        let mut app = App::new(Some(id)).unwrap();
        press(&mut app, KeyCode::Char('f'));
        assert_eq!(app.mode, AppMode::Dialog);
        insta::assert_snapshot!(render_to_string(&app));
    });
}

#[test]
fn snapshot_label_filter_applied() {
    with_temp_dir(|| {
        let id = seed_board_with_multiple_labels();
        let mut app = App::new(Some(id)).unwrap();
        press(&mut app, KeyCode::Char('f'));
        press(&mut app, KeyCode::Enter); // first label: bug
        assert_eq!(app.mode, AppMode::Normal);
        let text = render_to_string(&app);
        assert!(text.contains("[label: bug]"));
        assert!(text.contains("Implement authentication"));
        assert!(!text.contains("Write documentation"));
        insta::assert_snapshot!(text);
    });
}

#[test]
fn label_filter_toggles_and_clears() {
    with_temp_dir(|| {
        let id = seed_board_with_multiple_labels();
        let mut app = App::new(Some(id)).unwrap();
        press(&mut app, KeyCode::Char('f'));
        press(&mut app, KeyCode::Enter);
        assert!(app.board().unwrap().label_filter.is_some());
        // Picking the active label again clears it.
        press(&mut app, KeyCode::Char('f'));
        press(&mut app, KeyCode::Enter);
        assert!(app.board().unwrap().label_filter.is_none());
        // A label no card has hides everything; F restores.
        press(&mut app, KeyCode::Char('f'));
        press(&mut app, KeyCode::Down);
        press(&mut app, KeyCode::Enter);
        assert!(!render_to_string(&app).contains("Write documentation"));
        press(&mut app, KeyCode::Char('F'));
        assert!(render_to_string(&app).contains("Write documentation"));
    });
}
