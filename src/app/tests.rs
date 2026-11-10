use super::*;
use crate::model::card::Card;
use crate::model::id::ListId;
use crate::model::label::LabelColor;
use crate::model::list::CardList;
use crate::storage::card_store;
use crate::test_support::with_temp_dir;

/// Build a `Card` with a fixed `id` for stable test assertions.
fn fixed_card(id: &str, title: &str) -> Card {
    let mut c = Card::new(title.into());
    c.id = id.into();
    c
}

/// Build a `LoadedBoard` with a single list containing the given cards.
fn single_list_board(cards: Vec<Card>) -> LoadedBoard {
    let card_ids: Vec<_> = cards.iter().map(|c| c.id.clone()).collect();
    let cards_map: HashMap<_, _> = cards.into_iter().map(|c| (c.id.clone(), c)).collect();
    LoadedBoard {
        meta: BoardMeta::new("X".into()),
        lists: vec![CardList {
            id: "l1".into(),
            name: "L".into(),
            card_ids,
        }],
        cards: cards_map,
        selected_list: 0,
        selected_card: vec![0],
        scroll_offset: vec![std::cell::Cell::new(0)],
        detail_item_idx: 0,
        detail_scroll: 0,
        detail_max_scroll: std::cell::Cell::new(0),
    }
}

/// Build a bare `App` (no disk, no board). Caller can override fields.
fn bare_app() -> App {
    App {
        mode: AppMode::BoardSelector,
        previous_mode: None,
        export_dir: None,
        web_sink: None,
        should_quit: false,
        status_message: None,
        boards: vec![],
        selected_board_idx: 0,
        editor: None,
        search_query: String::new(),
        search_active: false,
        label_filter: None,
        last_reload: Instant::now(),
        reload_interval: Duration::from_secs(15),
        dialog: None,
        insert: None,
        version: crate::VERSION,
        data_dir_display: data_dir_display(),
        caps: crate::term_caps::TermCaps::full(),
        grab_active: false,
        focused_cards: HashMap::new(),
        scroll_offsets: HashMap::new(),
    }
}

fn make_board_with_cards() -> (BoardMeta, ListId, Vec<Card>) {
    let mut meta = board_store::create_board("Board".into()).unwrap();
    let list_id = crate::test_support::add_list(&mut meta, "To Do");
    let mut cards: Vec<Card> = (0..3).map(|i| Card::new(format!("Card {i}"))).collect();
    crate::test_support::put_cards(&meta.id, &list_id, &mut cards);
    (meta, list_id, cards)
}

#[test]
fn clamp_selection_within_bounds_unchanged() {
    let cards = vec![
        fixed_card("aaaaaaaa", "A"),
        fixed_card("bbbbbbbb", "B"),
        fixed_card("cccccccc", "C"),
    ];
    let mut board = single_list_board(cards);
    board.selected_card[0] = 1;
    board.clamp_selection();
    assert_eq!(board.selected_card[0], 1);
}

#[test]
fn clamp_selection_caps_to_last_visible() {
    let cards = vec![fixed_card("aaaaaaaa", "A"), fixed_card("bbbbbbbb", "B")];
    let mut board = single_list_board(cards);
    board.selected_card[0] = 5; // out of bounds
    board.clamp_selection();
    assert_eq!(board.selected_card[0], 1);
}

#[test]
fn clamp_selection_zeros_when_no_visible_cards() {
    let mut board = single_list_board(vec![]);
    board.selected_card[0] = 3;
    board.clamp_selection();
    assert_eq!(board.selected_card[0], 0);
}

#[test]
fn clamp_selection_skips_archived() {
    let mut cards = vec![fixed_card("aaaaaaaa", "A"), fixed_card("bbbbbbbb", "B")];
    cards[1].archived = true; // only first visible
    let mut board = single_list_board(cards);
    board.selected_card[0] = 1; // points to archived
    board.clamp_selection();
    assert_eq!(board.selected_card[0], 0);
}

#[test]
fn visible_card_count_excludes_archived_and_missing() {
    let mut cards = vec![fixed_card("aaaaaaaa", "A"), fixed_card("bbbbbbbb", "B")];
    cards[1].archived = true;
    let mut board = single_list_board(cards);
    // Inject orphan card_id with no matching card
    board.lists[0].card_ids.push("orphanid".into());
    assert_eq!(board.visible_card_count(0), 1);
}

#[test]
fn current_card_returns_selected() {
    let cards = vec![fixed_card("aaaaaaaa", "A"), fixed_card("bbbbbbbb", "B")];
    let mut board = single_list_board(cards);
    board.selected_card[0] = 1;
    assert_eq!(board.current_card_id().unwrap(), "bbbbbbbb");
    assert_eq!(board.current_card().unwrap().title, "B");
}

#[test]
fn current_card_none_on_empty_list() {
    let board = single_list_board(vec![]);
    assert!(board.current_card_id().is_none());
    assert!(board.current_card().is_none());
}

#[test]
fn accent_color_defaults_to_cyan() {
    assert_eq!(bare_app().accent_color(), Color::Cyan);
}

#[test]
fn accent_color_uses_board_meta() {
    let mut board = single_list_board(vec![]);
    board.meta.accent_color = LabelColor::Purple;
    let mut app = bare_app();
    app.mode = AppMode::Normal;
    app.editor = Some(BoardEditor::from_loaded(board));
    let (r, g, b) = LabelColor::Purple.to_rgb();
    assert_eq!(app.accent_color(), Color::Rgb(r, g, b));
}

#[test]
fn on_tick_clears_old_status_message() {
    let mut app = bare_app();
    app.mode = AppMode::Normal;
    app.status_message = Some(("hi".into(), Instant::now() - Duration::from_secs(7)));
    app.on_tick();
    assert!(app.status_message.is_none());
}

#[test]
fn on_tick_keeps_fresh_status_message() {
    let mut app = bare_app();
    app.mode = AppMode::Normal;
    app.status_message = Some(("hi".into(), Instant::now()));
    app.on_tick();
    assert!(app.status_message.is_some());
}

#[test]
fn reload_picks_up_new_card() {
    with_temp_dir(|| {
        let (meta, list, _cards) = make_board_with_cards();
        let mut app = App::new(Some(meta.id.clone())).unwrap();
        assert_eq!(app.board().unwrap().lists[0].card_ids.len(), 3);

        // Add a new card on disk
        let mut new_card = Card::new("Disk-added".into());
        new_card.list_id = list.clone();
        new_card.position = 4.0;
        card_store::save_card(&meta.id, &new_card).unwrap();

        // Force reload
        app.reload_interval = Duration::from_millis(0);
        app.last_reload = Instant::now() - Duration::from_secs(1);
        app.on_tick();

        let board = app.board().unwrap();
        assert_eq!(board.lists[0].card_ids.len(), 4);
        assert!(board.cards.contains_key(&new_card.id));
    });
}

#[test]
fn reload_preserves_selection() {
    with_temp_dir(|| {
        let (meta, _, _) = make_board_with_cards();
        let mut app = App::new(Some(meta.id.clone())).unwrap();
        // Move selection to card index 2
        app.board_mut().unwrap().selected_card[0] = 2;
        app.board_mut().unwrap().detail_item_idx = 5;
        app.board_mut().unwrap().detail_scroll = 7;

        app.reload_interval = Duration::from_millis(0);
        app.last_reload = Instant::now() - Duration::from_secs(1);
        app.on_tick();

        let board = app.board().unwrap();
        assert_eq!(board.selected_card[0], 2);
        assert_eq!(board.detail_item_idx, 5);
        assert_eq!(board.detail_scroll, 7);
    });
}

#[test]
fn reload_clamps_selection_when_card_removed() {
    with_temp_dir(|| {
        let (meta, _list, cards) = make_board_with_cards();
        let mut app = App::new(Some(meta.id.clone())).unwrap();
        app.board_mut().unwrap().selected_card[0] = 2; // points at last

        // Remove last card from disk
        card_store::delete_card(&meta.id, &cards[2].id).unwrap();

        app.reload_interval = Duration::from_millis(0);
        app.last_reload = Instant::now() - Duration::from_secs(1);
        app.on_tick();

        let board = app.board().unwrap();
        assert_eq!(board.lists[0].card_ids.len(), 2);
        // Selection clamped to new last index
        assert_eq!(board.selected_card[0], 1);
    });
}

#[test]
fn reload_handles_board_deleted_externally() {
    with_temp_dir(|| {
        let (meta, _, _) = make_board_with_cards();
        let mut app = App::new(Some(meta.id.clone())).unwrap();
        assert!(app.editor.is_some());

        // Delete board from disk
        board_store::delete_board(&meta.id).unwrap();

        app.reload_interval = Duration::from_millis(0);
        app.last_reload = Instant::now() - Duration::from_secs(1);
        app.on_tick();

        // App should drop the board and switch to selector
        assert!(app.editor.is_none());
        assert_eq!(app.mode, AppMode::BoardSelector);
    });
}

#[test]
fn reload_skipped_in_insert_mode() {
    with_temp_dir(|| {
        let (meta, list, _) = make_board_with_cards();
        let mut app = App::new(Some(meta.id.clone())).unwrap();
        app.start_insert(Box::new(crate::insert::line_editor::NewCardTitle::new()));

        // Add a card on disk
        let mut new_card = Card::new("Should not appear".into());
        new_card.list_id = list.clone();
        new_card.position = 4.0;
        card_store::save_card(&meta.id, &new_card).unwrap();

        app.reload_interval = Duration::from_millis(0);
        app.last_reload = Instant::now() - Duration::from_secs(1);
        app.on_tick();

        // Should NOT have reloaded
        assert_eq!(app.board().unwrap().lists[0].card_ids.len(), 3);
    });
}

#[test]
fn reload_skipped_while_card_grabbed() {
    with_temp_dir(|| {
        let (meta, list, _) = make_board_with_cards();
        let mut app = App::new(Some(meta.id.clone())).unwrap();
        app.grab_active = true;

        let mut new_card = Card::new("Disk".into());
        new_card.list_id = list.clone();
        new_card.position = 4.0;
        card_store::save_card(&meta.id, &new_card).unwrap();

        app.reload_interval = Duration::from_millis(0);
        app.last_reload = Instant::now() - Duration::from_secs(1);
        app.on_tick();

        // Reload would clobber the in-flight move — skipped.
        assert_eq!(app.board().unwrap().lists[0].card_ids.len(), 3);
    });
}

#[test]
fn reload_skipped_when_description_editor_active() {
    with_temp_dir(|| {
        let (meta, list, _) = make_board_with_cards();
        let mut app = App::new(Some(meta.id.clone())).unwrap();
        app.mode = AppMode::CardDetail;
        // Simulate active description edit
        let card_id = app.board().unwrap().current_card_id().cloned().unwrap();
        app.start_insert(Box::new(
            crate::insert::markdown_editor::MarkdownEditor::new(card_id, "initial"),
        ));

        let mut new_card = Card::new("Disk".into());
        new_card.list_id = list.clone();
        new_card.position = 4.0;
        card_store::save_card(&meta.id, &new_card).unwrap();

        app.reload_interval = Duration::from_millis(0);
        app.last_reload = Instant::now() - Duration::from_secs(1);
        app.on_tick();

        // Editor active → skip reload
        assert_eq!(app.board().unwrap().lists[0].card_ids.len(), 3);
    });
}

#[test]
fn start_insert_with_prefill() {
    with_temp_dir(|| {
        let mut app = App::new(None).unwrap();
        app.start_insert(Box::new(crate::insert::line_editor::EditCardTitle::new(
            "abc".into(),
            "hello",
            true,
        )));
        let h = app.insert.as_ref().unwrap();
        assert_eq!(h.line_buffer(), Some("hello"));
        assert_eq!(h.line_cursor(), Some(5));
        assert!(matches!(app.mode, AppMode::Insert));
    });
}

#[test]
fn base64_rfc4648_vectors() {
    assert_eq!(base64_encode(b""), "");
    assert_eq!(base64_encode(b"f"), "Zg==");
    assert_eq!(base64_encode(b"fo"), "Zm8=");
    assert_eq!(base64_encode(b"foo"), "Zm9v");
    assert_eq!(base64_encode(b"foob"), "Zm9vYg==");
    assert_eq!(base64_encode(b"fooba"), "Zm9vYmE=");
    assert_eq!(base64_encode(b"foobar"), "Zm9vYmFy");
}

#[test]
fn osc52_formats_escape_sequence() {
    assert_eq!(
        osc52_sequence("Fix login bug"),
        "\x1b]52;c;Rml4IGxvZ2luIGJ1Zw==\x07"
    );
}

#[test]
fn copy_to_clipboard_uses_native_when_available() {
    let mut app = bare_app();
    let mut osc_buf = Vec::new();
    app.copy_to_clipboard_with(
        "Fix login bug",
        "title 'Fix login bug'",
        Ok(()),
        &mut osc_buf,
    );
    assert_eq!(
        app.status_message.as_ref().map(|(m, _)| m.as_str()),
        Some("Copied title 'Fix login bug' to clipboard")
    );
    assert!(
        osc_buf.is_empty(),
        "should not emit OSC 52 when native succeeds"
    );
}

#[test]
fn copy_to_clipboard_falls_back_to_osc52_when_native_fails() {
    let mut app = bare_app();
    app.caps = crate::term_caps::TermCaps::full();
    let mut osc_buf = Vec::new();
    app.copy_to_clipboard_with(
        "Fix login bug",
        "title 'Fix login bug'",
        Err("no display".into()),
        &mut osc_buf,
    );
    assert_eq!(
        app.status_message.as_ref().map(|(m, _)| m.as_str()),
        Some("Copied title 'Fix login bug' to clipboard using terminal")
    );
    assert_eq!(
        String::from_utf8(osc_buf).unwrap(),
        "\x1b]52;c;Rml4IGxvZ2luIGJ1Zw==\x07"
    );
}

#[test]
fn copy_to_clipboard_skips_osc52_on_linux_tty() {
    let mut app = bare_app();
    app.caps = crate::term_caps::TermCaps::linux_tty();
    let mut osc_buf = Vec::new();
    app.copy_to_clipboard_with(
        "Fix login bug",
        "title 'Fix login bug'",
        Err("no display".into()),
        &mut osc_buf,
    );
    assert_eq!(
        app.status_message.as_ref().map(|(m, _)| m.as_str()),
        Some("Clipboard not available: no display")
    );
    assert!(osc_buf.is_empty(), "should not emit OSC 52 on linux tty");
}

#[test]
fn copy_to_clipboard_reports_error_when_osc52_write_fails() {
    struct FailingWriter;
    impl std::io::Write for FailingWriter {
        fn write(&mut self, _buf: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "pipe closed",
            ))
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    let mut app = bare_app();
    app.caps = crate::term_caps::TermCaps::full();
    let mut writer = FailingWriter;
    app.copy_to_clipboard_with(
        "Fix login bug",
        "title 'Fix login bug'",
        Err("no display".into()),
        &mut writer,
    );
    assert_eq!(
        app.status_message.as_ref().map(|(m, _)| m.as_str()),
        Some("Clipboard not available: no display")
    );
}
