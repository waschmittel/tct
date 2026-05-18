//! Due-date picker input handling (calendar grid + text buffer).
//!
//! Owns the `Insert(EditDueDate)` mode. Two ways to set a date coexist:
//! arrow keys move a single-day cursor; typing digits/hyphens edits the
//! text buffer and re-parses into the picker date.

use chrono::Datelike;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::{cancel_insert, has_ctrl_or_cmd};
use crate::app::App;
use crate::storage::card_store;

pub(super) fn handle(app: &mut App, key: KeyEvent) -> anyhow::Result<()> {
    let shift = key.modifiers.contains(KeyModifiers::SHIFT);

    match key.code {
        KeyCode::Esc => {
            app.picker_date = None;
            cancel_insert(app);
            return Ok(());
        }
        KeyCode::Enter => {
            return confirm(app);
        }

        // Grid navigation
        KeyCode::Left => {
            shift_days(app, -1);
        }
        KeyCode::Right => {
            shift_days(app, 1);
        }
        KeyCode::Up => {
            shift_days(app, -7);
        }
        KeyCode::Down => {
            shift_days(app, 7);
        }
        KeyCode::PageUp => {
            shift_months(app, if shift { -12 } else { -1 });
        }
        KeyCode::PageDown => {
            shift_months(app, if shift { 12 } else { 1 });
        }
        KeyCode::Char('t') | KeyCode::Char('T') => {
            let today = chrono::Local::now().date_naive();
            set_date(app, today);
        }
        KeyCode::Home => {
            // Jump to first day of month
            if let Some(d) = app.picker_date
                && let Some(first) = chrono::NaiveDate::from_ymd_opt(d.year(), d.month(), 1)
            {
                set_date(app, first);
            }
        }
        KeyCode::End => {
            // Jump to last day of month
            if let Some(d) = app.picker_date {
                let next_month = if d.month() == 12 {
                    chrono::NaiveDate::from_ymd_opt(d.year() + 1, 1, 1)
                } else {
                    chrono::NaiveDate::from_ymd_opt(d.year(), d.month() + 1, 1)
                };
                if let Some(nm) = next_month {
                    set_date(app, nm.pred_opt().unwrap_or(d));
                }
            }
        }

        // Text editing
        KeyCode::Backspace if !app.input_buffer.is_empty() => {
            app.input_buffer.pop();
            app.input_cursor = app.input_buffer.len();
            sync_from_buffer(app);
        }
        KeyCode::Char('u') if has_ctrl_or_cmd(key.modifiers) => {
            app.input_buffer.clear();
            app.input_cursor = 0;
            app.picker_date = None;
        }
        KeyCode::Char(c) if c.is_ascii_digit() || c == '-' => {
            app.input_buffer.push(c);
            app.input_cursor = app.input_buffer.len();
            sync_from_buffer(app);
        }
        _ => {}
    }
    Ok(())
}

fn shift_days(app: &mut App, days: i64) {
    let base = app
        .picker_date
        .unwrap_or_else(|| chrono::Local::now().date_naive());
    let new = base + chrono::Duration::days(days);
    set_date(app, new);
}

fn shift_months(app: &mut App, delta_months: i32) {
    let base = app
        .picker_date
        .unwrap_or_else(|| chrono::Local::now().date_naive());
    let total = base.year() * 12 + base.month() as i32 - 1 + delta_months;
    let new_year = total.div_euclid(12);
    let new_month = (total.rem_euclid(12) + 1) as u32;
    let max_day = days_in_month(new_year, new_month);
    let day = base.day().min(max_day);
    if let Some(new_date) = chrono::NaiveDate::from_ymd_opt(new_year, new_month, day) {
        set_date(app, new_date);
    }
}

fn days_in_month(year: i32, month: u32) -> u32 {
    let (ny, nm) = if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    };
    let first_next = chrono::NaiveDate::from_ymd_opt(ny, nm, 1).unwrap();
    let last = first_next.pred_opt().unwrap();
    last.day()
}

fn set_date(app: &mut App, date: chrono::NaiveDate) {
    app.picker_date = Some(date);
    app.input_buffer = date.format("%Y-%m-%d").to_string();
    app.input_cursor = app.input_buffer.len();
}

fn sync_from_buffer(app: &mut App) {
    if let Ok(d) = chrono::NaiveDate::parse_from_str(&app.input_buffer, "%Y-%m-%d") {
        app.picker_date = Some(d);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{App, AppMode, InsertTarget};
    use crate::model::card::Card;
    use crate::model::list::CardList;
    use crate::storage::{board_store, card_store, list_store};
    use crate::test_support::with_temp_dir;
    use chrono::NaiveDate;

    fn setup() -> App {
        let mut meta = board_store::create_board("B".into()).unwrap();
        let mut list = CardList::new("L".into());
        let card = Card::new("C".into());
        card_store::save_card(&meta.id, &card).unwrap();
        list.card_ids.push(card.id.clone());
        list_store::save_list(&meta.id, &list).unwrap();
        meta.list_order = vec![list.id.clone()];
        board_store::save_board(&meta).unwrap();

        let mut app = App::new(Some(meta.id)).unwrap();
        app.mode = AppMode::CardDetail;
        app.start_due_date_picker("2026-05-18");
        app
    }

    fn press(app: &mut App, code: KeyCode) {
        handle(app, KeyEvent::new(code, KeyModifiers::empty())).unwrap();
    }

    #[test]
    fn days_in_month_handles_leap_years() {
        assert_eq!(days_in_month(2024, 2), 29);
        assert_eq!(days_in_month(2025, 2), 28);
        assert_eq!(days_in_month(2025, 4), 30);
        assert_eq!(days_in_month(2025, 12), 31);
    }

    #[test]
    fn left_arrow_subtracts_one_day() {
        with_temp_dir(|| {
            let mut app = setup();
            app.picker_date = Some(NaiveDate::from_ymd_opt(2026, 5, 18).unwrap());
            press(&mut app, KeyCode::Left);
            assert_eq!(app.picker_date, NaiveDate::from_ymd_opt(2026, 5, 17));
            assert_eq!(app.input_buffer, "2026-05-17");
        });
    }

    #[test]
    fn down_arrow_advances_one_week() {
        with_temp_dir(|| {
            let mut app = setup();
            app.picker_date = Some(NaiveDate::from_ymd_opt(2026, 5, 18).unwrap());
            press(&mut app, KeyCode::Down);
            assert_eq!(app.picker_date, NaiveDate::from_ymd_opt(2026, 5, 25));
        });
    }

    #[test]
    fn pageup_subtracts_one_month_clamping_day() {
        with_temp_dir(|| {
            let mut app = setup();
            // March 31 → February has no 31, expect Feb 28 (2025 not leap).
            app.picker_date = Some(NaiveDate::from_ymd_opt(2025, 3, 31).unwrap());
            app.input_buffer = "2025-03-31".into();
            press(&mut app, KeyCode::PageUp);
            assert_eq!(app.picker_date, NaiveDate::from_ymd_opt(2025, 2, 28));
        });
    }

    #[test]
    fn shift_pagedown_advances_one_year() {
        with_temp_dir(|| {
            let mut app = setup();
            app.picker_date = Some(NaiveDate::from_ymd_opt(2026, 5, 18).unwrap());
            handle(
                &mut app,
                KeyEvent::new(KeyCode::PageDown, KeyModifiers::SHIFT),
            )
            .unwrap();
            assert_eq!(app.picker_date, NaiveDate::from_ymd_opt(2027, 5, 18));
        });
    }

    #[test]
    fn t_jumps_to_today() {
        with_temp_dir(|| {
            let mut app = setup();
            app.picker_date = Some(NaiveDate::from_ymd_opt(1999, 1, 1).unwrap());
            press(&mut app, KeyCode::Char('t'));
            let today = chrono::Local::now().date_naive();
            assert_eq!(app.picker_date, Some(today));
        });
    }

    #[test]
    fn home_jumps_to_first_of_month() {
        with_temp_dir(|| {
            let mut app = setup();
            app.picker_date = Some(NaiveDate::from_ymd_opt(2026, 5, 18).unwrap());
            press(&mut app, KeyCode::Home);
            assert_eq!(app.picker_date, NaiveDate::from_ymd_opt(2026, 5, 1));
        });
    }

    #[test]
    fn end_jumps_to_last_of_month() {
        with_temp_dir(|| {
            let mut app = setup();
            app.picker_date = Some(NaiveDate::from_ymd_opt(2026, 2, 1).unwrap());
            press(&mut app, KeyCode::End);
            assert_eq!(app.picker_date, NaiveDate::from_ymd_opt(2026, 2, 28));
        });
    }

    #[test]
    fn enter_persists_due_date_and_exits_picker() {
        with_temp_dir(|| {
            let mut app = setup();
            app.picker_date = Some(NaiveDate::from_ymd_opt(2026, 5, 18).unwrap());
            app.input_buffer = "2026-05-18".into();
            press(&mut app, KeyCode::Enter);
            let card = app.board.as_ref().unwrap().current_card().unwrap();
            assert_eq!(card.due_date, NaiveDate::from_ymd_opt(2026, 5, 18));
            assert!(!matches!(
                app.mode,
                AppMode::Insert(InsertTarget::EditDueDate)
            ));
        });
    }

    #[test]
    fn enter_with_empty_buffer_clears_due_date() {
        with_temp_dir(|| {
            let mut app = setup();
            // Pre-set a due date on the card.
            {
                let board = app.board.as_mut().unwrap();
                let card_id = board.current_card_id().cloned().unwrap();
                board.cards.get_mut(&card_id).unwrap().due_date =
                    Some(NaiveDate::from_ymd_opt(2030, 1, 1).unwrap());
            }
            app.input_buffer.clear();
            app.picker_date = None;
            press(&mut app, KeyCode::Enter);
            let card = app.board.as_ref().unwrap().current_card().unwrap();
            assert!(card.due_date.is_none());
        });
    }

    #[test]
    fn enter_with_invalid_buffer_sets_status_and_stays_in_picker() {
        with_temp_dir(|| {
            let mut app = setup();
            app.input_buffer = "not-a-date".into();
            app.picker_date = None;
            press(&mut app, KeyCode::Enter);
            assert!(matches!(
                app.mode,
                AppMode::Insert(InsertTarget::EditDueDate)
            ));
            assert!(app.status_message.as_ref().unwrap().0.contains("Invalid"));
        });
    }

    #[test]
    fn esc_cancels_picker_without_changing_due_date() {
        with_temp_dir(|| {
            let mut app = setup();
            let original = app.board.as_ref().unwrap().current_card().unwrap().due_date;
            press(&mut app, KeyCode::Esc);
            assert!(!matches!(
                app.mode,
                AppMode::Insert(InsertTarget::EditDueDate)
            ));
            assert!(app.picker_date.is_none());
            let card = app.board.as_ref().unwrap().current_card().unwrap();
            assert_eq!(card.due_date, original);
        });
    }

    #[test]
    fn typing_digit_appends_to_buffer_and_reparses_picker() {
        with_temp_dir(|| {
            let mut app = setup();
            app.input_buffer.clear();
            app.picker_date = None;
            for c in "2026-05-19".chars() {
                handle(
                    &mut app,
                    KeyEvent::new(KeyCode::Char(c), KeyModifiers::empty()),
                )
                .unwrap();
            }
            assert_eq!(app.input_buffer, "2026-05-19");
            assert_eq!(app.picker_date, NaiveDate::from_ymd_opt(2026, 5, 19));
        });
    }
}

fn confirm(app: &mut App) -> anyhow::Result<()> {
    let trimmed = app.input_buffer.trim().to_string();

    // Empty or "none" clears the due date.
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("none") {
        if let Some(board) = &mut app.board
            && let Some(card_id) = board.current_card_id().cloned()
            && let Some(card) = board.cards.get_mut(&card_id)
        {
            let was_set = card.due_date.is_some();
            card.due_date = None;
            if was_set {
                card.log("Cleared due date");
            } else {
                card.touch();
            }
            card_store::save_card(&board.meta.id, card)?;
        }
        app.set_status("Cleared due date".into());
        app.picker_date = None;
        cancel_insert(app);
        return Ok(());
    }

    // Prefer the picker's parsed date; fall back to parsing the buffer.
    let parsed = app
        .picker_date
        .or_else(|| chrono::NaiveDate::parse_from_str(&trimmed, "%Y-%m-%d").ok());

    if let Some(date) = parsed {
        if let Some(board) = &mut app.board
            && let Some(card_id) = board.current_card_id().cloned()
            && let Some(card) = board.cards.get_mut(&card_id)
        {
            let prev = card.due_date;
            card.due_date = Some(date);
            if prev != Some(date) {
                card.log(format!("Set due date to {date}"));
            } else {
                card.touch();
            }
            card_store::save_card(&board.meta.id, card)?;
        }
        app.set_status(format!("Due date set to {date}"));
        app.picker_date = None;
        cancel_insert(app);
    } else {
        app.set_status("Invalid date format. Use YYYY-MM-DD".into());
    }
    Ok(())
}
