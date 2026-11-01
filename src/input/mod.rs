pub mod board_selector_input;
pub mod card_detail_input;
mod command;
mod dialog_input;
mod insert;
pub mod keymap;
pub mod normal;

use crossterm::event::{KeyEvent, KeyEventKind};

use crate::app::{App, AppMode};

pub fn handle_input(app: &mut App, key: KeyEvent) -> anyhow::Result<()> {
    if key.kind != KeyEventKind::Press {
        return Ok(());
    }

    let result = match &app.mode {
        AppMode::BoardSelector => board_selector_input::handle(app, key),
        AppMode::Normal => normal::handle(app, key),
        AppMode::CardDetail => card_detail_input::handle(app, key),
        AppMode::Insert => insert::handle(app, key),
        AppMode::Command => command::handle(app, key),
        AppMode::Dialog => dialog_input::handle(app, key),
        AppMode::Help => {
            let dismiss = matches!(
                key.code,
                crossterm::event::KeyCode::Esc
                    | crossterm::event::KeyCode::Char('q' | '?')
                    | crossterm::event::KeyCode::F(1)
            ) || (matches!(key.code, crossterm::event::KeyCode::Char('h'))
                && crate::insert::line_input::has_ctrl_or_cmd(key.modifiers));
            if dismiss {
                // Help opened over a live dialog or insert handler
                // returns to it; the recorded base return mode stays put
                // for the overlay's own close.
                if app.dialog.is_some() {
                    app.mode = AppMode::Dialog;
                } else if app.insert.is_some() {
                    app.mode = AppMode::Insert;
                } else {
                    app.mode = app.take_return_mode();
                }
            }
            Ok(())
        }
    };
    app.enforce_mode_invariants();
    result
}

pub fn handle_paste(app: &mut App, text: String) -> anyhow::Result<()> {
    if text.is_empty() {
        return Ok(());
    }
    match &app.mode {
        AppMode::Insert => {
            insert::handle_paste(app, &text)?;
        }
        AppMode::Command => {
            let clean = text.replace(['\r', '\n'], " ");
            app.search_query.push_str(&clean);
        }
        _ => {}
    }
    app.enforce_mode_invariants();
    Ok(())
}

#[cfg(test)]
mod tests {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    use super::handle_input;
    use crate::app::{App, AppMode};
    use crate::model::card::Card;
    use crate::storage::board_store;
    use crate::test_support::with_temp_dir;

    fn app_in_card_detail() -> App {
        let mut meta = board_store::create_board("Test".into()).unwrap();
        let list = crate::test_support::add_list(&mut meta, "To Do");
        crate::test_support::put_cards(&meta.id, &list, &mut [Card::new("Card".into())]);

        let mut app = App::new(Some(meta.id)).unwrap();
        app.mode = AppMode::CardDetail;
        app
    }

    fn press(app: &mut App, code: KeyCode) {
        handle_input(app, KeyEvent::new(code, KeyModifiers::empty())).unwrap();
    }

    /// Regression: card detail → picker → manager → create label → close
    /// manager → close picker froze the UI. `previous_mode` was clobbered
    /// with overlay modes (Dialog, then Insert) during the dialog → insert
    /// → dialog round-trip, so the final close restored `Insert` with no
    /// insert handler — a mode that swallows every key. The return target
    /// is now only ever a base mode.
    #[test]
    fn label_create_round_trip_from_card_detail_does_not_freeze() {
        with_temp_dir(|| {
            let mut app = app_in_card_detail();
            press(&mut app, KeyCode::Char('l')); // label picker
            press(&mut app, KeyCode::Char('L')); // label manager
            press(&mut app, KeyCode::Char('n')); // new-label insert
            assert!(matches!(app.mode, AppMode::Insert));
            for c in "bug".chars() {
                press(&mut app, KeyCode::Char(c));
            }
            press(&mut app, KeyCode::Enter); // create → back to manager
            assert!(matches!(app.mode, AppMode::Dialog));
            press(&mut app, KeyCode::Esc); // manager → picker
            assert!(matches!(app.mode, AppMode::Dialog));
            press(&mut app, KeyCode::Esc); // picker → card detail (froze here)
            assert!(
                matches!(app.mode, AppMode::CardDetail),
                "picker close must land on CardDetail, got {:?}",
                app.mode
            );
            // Created label exists but was NOT implicitly assigned.
            let board = app.board().unwrap();
            assert_eq!(board.meta.labels.len(), 1);
            assert!(board.current_card().unwrap().label_ids.is_empty());
            press(&mut app, KeyCode::Esc); // card detail → board view
            assert!(matches!(app.mode, AppMode::Normal));
        });
    }

    /// Manager opened directly (not via picker) returns to where it was
    /// opened from — card detail included.
    #[test]
    fn label_manager_from_card_detail_returns_to_card_detail() {
        with_temp_dir(|| {
            let mut app = app_in_card_detail();
            press(&mut app, KeyCode::Char('L'));
            assert!(matches!(app.mode, AppMode::Dialog));
            press(&mut app, KeyCode::Esc);
            assert!(matches!(app.mode, AppMode::CardDetail));
        });
    }

    /// `?` inside the date picker shows Help and Esc returns to the
    /// still-alive insert handler, not to the base mode.
    #[test]
    fn help_over_date_picker_returns_to_insert() {
        with_temp_dir(|| {
            let mut app = app_in_card_detail();
            press(&mut app, KeyCode::Char('u')); // due-date picker
            assert!(matches!(app.mode, AppMode::Insert));
            press(&mut app, KeyCode::Char('?'));
            assert!(matches!(app.mode, AppMode::Help));
            assert!(app.insert.is_some());
            press(&mut app, KeyCode::Esc);
            assert!(matches!(app.mode, AppMode::Insert));
            assert!(app.insert.is_some());
            press(&mut app, KeyCode::Esc); // cancel picker
            assert!(matches!(app.mode, AppMode::CardDetail));
        });
    }

    /// `F1` inside the description editor shows Help and Esc returns to the
    /// still-alive description editor, not to the base mode.
    #[test]
    fn help_over_description_editor_returns_to_insert() {
        with_temp_dir(|| {
            let mut app = app_in_card_detail();
            press(&mut app, KeyCode::Char('e')); // description editor
            assert!(matches!(app.mode, AppMode::Insert));
            press(&mut app, KeyCode::F(1));
            assert!(matches!(app.mode, AppMode::Help));
            assert!(app.insert.is_some());
            press(&mut app, KeyCode::Esc);
            assert!(matches!(app.mode, AppMode::Insert));
            assert!(app.insert.is_some());
        });
    }

    #[test]
    fn handle_paste_inserts_into_active_editor() {
        with_temp_dir(|| {
            let mut app = app_in_card_detail();
            press(&mut app, KeyCode::Char('e'));
            assert!(matches!(app.mode, AppMode::Insert));
            super::handle_paste(&mut app, "pasted note".to_string()).unwrap();
            let editor = app
                .insert
                .as_ref()
                .unwrap()
                .as_any()
                .downcast_ref::<crate::insert::markdown_editor::MarkdownEditor>()
                .unwrap();
            assert!(editor.input.current_text().contains("pasted note"));
        });
    }

    /// `?` inside the label manager shows Help and Esc returns to the
    /// still-alive dialog, not to the base mode.
    #[test]
    fn help_over_dialog_returns_to_dialog() {
        with_temp_dir(|| {
            let mut app = app_in_card_detail();
            press(&mut app, KeyCode::Char('L'));
            press(&mut app, KeyCode::Char('?'));
            assert!(matches!(app.mode, AppMode::Help));
            assert!(app.dialog.is_some());
            press(&mut app, KeyCode::Esc);
            assert!(matches!(app.mode, AppMode::Dialog));
            press(&mut app, KeyCode::Esc);
            assert!(matches!(app.mode, AppMode::CardDetail));
        });
    }
}
