use super::*;

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

#[test]
fn snapshot_help_description_editor() {
    with_temp_dir(|| {
        let id = seed_demo_board();
        let mut app = App::new(Some(id)).unwrap();
        app.version = "vTEST";
        press(&mut app, KeyCode::Enter);
        assert_eq!(app.mode, AppMode::CardDetail);
        press(&mut app, KeyCode::Char('e'));
        assert_eq!(app.mode, AppMode::Insert);
        press(&mut app, KeyCode::F(1));
        assert_eq!(app.mode, AppMode::Help);
        insta::assert_snapshot!(render_to_string(&app));
    });
}
