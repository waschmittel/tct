//! Due-date picker — calendar grid + parallel text buffer.

use chrono::{Datelike, NaiveDate};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::line_input::has_ctrl_or_cmd;
use super::{InsertHandler, InsertOutcome, InsertSurface};
use crate::app::LoadedBoard;
use crate::command::Command;
use crate::model::ids::ShortId;

pub struct DatePicker {
    pub card_id: ShortId,
    pub buffer: String,
    pub cursor: usize,
    pub picker_date: Option<NaiveDate>,
    pub surface: InsertSurface,
}

impl DatePicker {
    pub fn new(card_id: ShortId, prefill: &str, surface: InsertSurface) -> Self {
        let initial = NaiveDate::parse_from_str(prefill, "%Y-%m-%d")
            .ok()
            .unwrap_or_else(|| chrono::Local::now().date_naive());
        let buf = initial.format("%Y-%m-%d").to_string();
        Self {
            card_id,
            cursor: buf.len(),
            buffer: buf,
            picker_date: Some(initial),
            surface,
        }
    }
}

impl InsertHandler for DatePicker {
    fn handle_key(&mut self, key: KeyEvent, _b: Option<&LoadedBoard>) -> InsertOutcome {
        let shift = key.modifiers.contains(KeyModifiers::SHIFT);

        match key.code {
            KeyCode::Esc => InsertOutcome::Cancel,
            KeyCode::Enter => self.confirm(),
            KeyCode::Left => {
                self.shift_days(-1);
                InsertOutcome::Stay
            }
            KeyCode::Right => {
                self.shift_days(1);
                InsertOutcome::Stay
            }
            KeyCode::Up => {
                self.shift_days(-7);
                InsertOutcome::Stay
            }
            KeyCode::Down => {
                self.shift_days(7);
                InsertOutcome::Stay
            }
            KeyCode::PageUp => {
                self.shift_months(if shift { -12 } else { -1 });
                InsertOutcome::Stay
            }
            KeyCode::PageDown => {
                self.shift_months(if shift { 12 } else { 1 });
                InsertOutcome::Stay
            }
            KeyCode::Char('t') | KeyCode::Char('T') => {
                let today = chrono::Local::now().date_naive();
                self.set_date(today);
                InsertOutcome::Stay
            }
            KeyCode::Home => {
                if let Some(d) = self.picker_date
                    && let Some(first) = NaiveDate::from_ymd_opt(d.year(), d.month(), 1)
                {
                    self.set_date(first);
                }
                InsertOutcome::Stay
            }
            KeyCode::End => {
                if let Some(d) = self.picker_date {
                    let next_month = if d.month() == 12 {
                        NaiveDate::from_ymd_opt(d.year() + 1, 1, 1)
                    } else {
                        NaiveDate::from_ymd_opt(d.year(), d.month() + 1, 1)
                    };
                    if let Some(nm) = next_month {
                        self.set_date(nm.pred_opt().unwrap_or(d));
                    }
                }
                InsertOutcome::Stay
            }
            KeyCode::Backspace if !self.buffer.is_empty() => {
                self.buffer.pop();
                self.cursor = self.buffer.len();
                self.sync_from_buffer();
                InsertOutcome::Stay
            }
            KeyCode::Char('u') if has_ctrl_or_cmd(key.modifiers) => {
                self.buffer.clear();
                self.cursor = 0;
                self.picker_date = None;
                InsertOutcome::Stay
            }
            KeyCode::Char(c) if c.is_ascii_digit() || c == '-' => {
                self.buffer.push(c);
                self.cursor = self.buffer.len();
                self.sync_from_buffer();
                InsertOutcome::Stay
            }
            _ => InsertOutcome::Stay,
        }
    }

    fn surface(&self) -> InsertSurface { self.surface }
    fn as_any(&self) -> &dyn std::any::Any { self }
}

impl DatePicker {
    fn confirm(&mut self) -> InsertOutcome {
        let trimmed = self.buffer.trim().to_string();

        if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("none") {
            return InsertOutcome::Confirm(Command::ClearDueDate {
                card_id: self.card_id.clone(),
            });
        }

        let parsed = self
            .picker_date
            .or_else(|| NaiveDate::parse_from_str(&trimmed, "%Y-%m-%d").ok());

        match parsed {
            Some(date) => InsertOutcome::Confirm(Command::SetDueDate {
                card_id: self.card_id.clone(),
                date,
            }),
            None => InsertOutcome::CancelWithStatus(
                "Invalid date format. Use YYYY-MM-DD".into(),
            ),
        }
    }

    fn shift_days(&mut self, days: i64) {
        let base = self
            .picker_date
            .unwrap_or_else(|| chrono::Local::now().date_naive());
        let new = base + chrono::Duration::days(days);
        self.set_date(new);
    }

    fn shift_months(&mut self, delta_months: i32) {
        let base = self
            .picker_date
            .unwrap_or_else(|| chrono::Local::now().date_naive());
        let total = base.year() * 12 + base.month() as i32 - 1 + delta_months;
        let new_year = total.div_euclid(12);
        let new_month = (total.rem_euclid(12) + 1) as u32;
        let max_day = days_in_month(new_year, new_month);
        let day = base.day().min(max_day);
        if let Some(new_date) = NaiveDate::from_ymd_opt(new_year, new_month, day) {
            self.set_date(new_date);
        }
    }

    fn set_date(&mut self, date: NaiveDate) {
        self.picker_date = Some(date);
        self.buffer = date.format("%Y-%m-%d").to_string();
        self.cursor = self.buffer.len();
    }

    fn sync_from_buffer(&mut self) {
        if let Ok(d) = NaiveDate::parse_from_str(&self.buffer, "%Y-%m-%d") {
            self.picker_date = Some(d);
        }
    }
}

fn days_in_month(year: i32, month: u32) -> u32 {
    let (ny, nm) = if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    };
    let first_next = NaiveDate::from_ymd_opt(ny, nm, 1).unwrap();
    first_next.pred_opt().unwrap().day()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_picker() -> DatePicker {
        DatePicker::new("card1".into(), "2026-05-18", InsertSurface::CardDetail)
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
        let mut p = make_picker();
        p.handle_key(KeyEvent::new(KeyCode::Left, KeyModifiers::empty()), None);
        assert_eq!(p.picker_date, NaiveDate::from_ymd_opt(2026, 5, 17));
        assert_eq!(p.buffer, "2026-05-17");
    }

    #[test]
    fn down_arrow_advances_one_week() {
        let mut p = make_picker();
        p.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::empty()), None);
        assert_eq!(p.picker_date, NaiveDate::from_ymd_opt(2026, 5, 25));
    }

    #[test]
    fn pageup_subtracts_one_month_clamping_day() {
        let mut p = make_picker();
        p.picker_date = Some(NaiveDate::from_ymd_opt(2025, 3, 31).unwrap());
        p.buffer = "2025-03-31".into();
        p.handle_key(KeyEvent::new(KeyCode::PageUp, KeyModifiers::empty()), None);
        assert_eq!(p.picker_date, NaiveDate::from_ymd_opt(2025, 2, 28));
    }

    #[test]
    fn shift_pagedown_advances_one_year() {
        let mut p = make_picker();
        p.handle_key(KeyEvent::new(KeyCode::PageDown, KeyModifiers::SHIFT), None);
        assert_eq!(p.picker_date, NaiveDate::from_ymd_opt(2027, 5, 18));
    }

    #[test]
    fn t_jumps_to_today() {
        let mut p = make_picker();
        p.picker_date = Some(NaiveDate::from_ymd_opt(1999, 1, 1).unwrap());
        p.handle_key(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::empty()), None);
        let today = chrono::Local::now().date_naive();
        assert_eq!(p.picker_date, Some(today));
    }

    #[test]
    fn home_jumps_to_first_of_month() {
        let mut p = make_picker();
        p.handle_key(KeyEvent::new(KeyCode::Home, KeyModifiers::empty()), None);
        assert_eq!(p.picker_date, NaiveDate::from_ymd_opt(2026, 5, 1));
    }

    #[test]
    fn end_jumps_to_last_of_month() {
        let mut p = make_picker();
        p.picker_date = Some(NaiveDate::from_ymd_opt(2026, 2, 1).unwrap());
        p.handle_key(KeyEvent::new(KeyCode::End, KeyModifiers::empty()), None);
        assert_eq!(p.picker_date, NaiveDate::from_ymd_opt(2026, 2, 28));
    }

    #[test]
    fn enter_returns_setduedate_confirm() {
        let mut p = make_picker();
        let out = p.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()), None);
        match out {
            InsertOutcome::Confirm(Command::SetDueDate { date, .. }) => {
                assert_eq!(date, NaiveDate::from_ymd_opt(2026, 5, 18).unwrap());
            }
            _ => panic!("expected Confirm(SetDueDate)"),
        }
    }

    #[test]
    fn enter_with_empty_buffer_clears() {
        let mut p = make_picker();
        p.buffer.clear();
        p.picker_date = None;
        let out = p.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()), None);
        assert!(matches!(out, InsertOutcome::Confirm(Command::ClearDueDate { .. })));
    }

    #[test]
    fn enter_with_invalid_buffer_returns_cancel_with_status() {
        let mut p = make_picker();
        p.buffer = "not-a-date".into();
        p.picker_date = None;
        let out = p.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()), None);
        assert!(matches!(out, InsertOutcome::CancelWithStatus(_)));
    }

    #[test]
    fn typing_digits_reparses_picker() {
        let mut p = make_picker();
        p.buffer.clear();
        p.picker_date = None;
        for c in "2026-05-19".chars() {
            p.handle_key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::empty()), None);
        }
        assert_eq!(p.buffer, "2026-05-19");
        assert_eq!(p.picker_date, NaiveDate::from_ymd_opt(2026, 5, 19));
    }
}
