//! History viewer for the currently selected card: a summary list on the
//! left, the diff of the selected entry's field on the right. Entries can be
//! deleted (after confirmation).

use chrono::{DateTime, Local, Utc};
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};

use super::{Dialog, DialogOutcome};
use crate::app::LoadedBoard;
use crate::command::Command;
use crate::model::card::{Card, HistoryField};
use crate::text_diff::{DiffKind, diff_lines};

const SUMMARY_WIDTH: u16 = 40;

#[derive(Default)]
pub struct CardHistory {
    /// Row in display order: 0 is the newest entry, the last row is the
    /// synthetic "Created" row.
    selected: usize,
    detail_scroll: u16,
    confirming_delete: bool,
}

impl CardHistory {
    pub fn new() -> Self {
        Self::default()
    }

    /// Index into `card.history` for the selected row; `None` on the
    /// "Created" row.
    fn history_index(&self, card: &Card) -> Option<usize> {
        card.history.len().checked_sub(self.selected + 1)
    }

    fn row_count(card: &Card) -> usize {
        card.history.len() + 1
    }

    fn select(&mut self, row: usize) {
        if row != self.selected {
            self.selected = row;
            self.detail_scroll = 0;
        }
    }
}

impl Dialog for CardHistory {
    fn render(
        &self,
        frame: &mut Frame,
        area: Rect,
        board: Option<&LoadedBoard>,
        accent: Color,
    ) {
        let Some(card) = board.and_then(|b| b.current_card()) else {
            return;
        };

        let width = 110u16.min(area.width.saturating_sub(4)).max(40);
        let height = 28u16.min(area.height.saturating_sub(4)).max(8);
        let x = area.width.saturating_sub(width) / 2;
        let y = area.height.saturating_sub(height) / 2;
        let popup = Rect::new(x, y, width, height);

        frame.render_widget(Clear, popup);
        let block = Block::default()
            .title(format!(" History — {} ", truncate(&card.title, 40)))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(accent));
        let inner = block.inner(popup);
        frame.render_widget(block, popup);

        let summary_width = SUMMARY_WIDTH.min(inner.width / 2);
        let [left, right] =
            Layout::horizontal([Constraint::Length(summary_width), Constraint::Min(1)]).areas(inner);

        self.render_summary(frame, left, card, accent);
        self.render_detail(frame, right, card);
    }

    fn handle_key(&mut self, key: KeyEvent, board: Option<&LoadedBoard>) -> DialogOutcome {
        let Some(card) = board.and_then(|b| b.current_card()) else {
            return DialogOutcome::close();
        };
        let last = Self::row_count(card) - 1;

        if self.confirming_delete {
            self.confirming_delete = false;
            return match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') => {
                    let Some(index) = self.history_index(card) else {
                        return DialogOutcome::stay();
                    };
                    // The row now under the cursor is the next older entry;
                    // when the oldest history entry goes, fall onto "Created".
                    self.selected = self.selected.min(last - 1);
                    self.detail_scroll = 0;
                    DialogOutcome::apply(Command::DeleteHistoryEntry {
                        card_id: card.id.clone(),
                        index,
                    })
                    .with_status("Deleted history entry".to_string())
                }
                _ => DialogOutcome::stay(),
            };
        }

        match key.code {
            KeyCode::Down | KeyCode::Char('j') => {
                self.select((self.selected + 1).min(last));
                DialogOutcome::stay()
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.select(self.selected.saturating_sub(1));
                DialogOutcome::stay()
            }
            KeyCode::Home | KeyCode::Char('g') => {
                self.select(0);
                DialogOutcome::stay()
            }
            KeyCode::End | KeyCode::Char('G') => {
                self.select(last);
                DialogOutcome::stay()
            }
            KeyCode::PageDown => {
                self.detail_scroll = self.detail_scroll.saturating_add(5);
                DialogOutcome::stay()
            }
            KeyCode::PageUp => {
                self.detail_scroll = self.detail_scroll.saturating_sub(5);
                DialogOutcome::stay()
            }
            KeyCode::Char('x') | KeyCode::Char('d') | KeyCode::Delete => {
                if self.history_index(card).is_some() {
                    self.confirming_delete = true;
                }
                DialogOutcome::stay()
            }
            KeyCode::Char('?') => DialogOutcome::help(),
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('h') => DialogOutcome::close(),
            _ => DialogOutcome::stay(),
        }
    }

    fn help(&self) -> Option<super::DialogHelp> {
        Some(super::DialogHelp {
            title: " Help — Card History ",
            rows: vec![
                ("Up / Down / j / k", "Select entry"),
                ("g / Home", "Newest entry"),
                ("G / End", "Oldest entry"),
                ("PgUp / PgDn", "Scroll the detail pane"),
                ("x / d / Del", "Delete entry (asks to confirm)"),
                ("Esc / q / h", "Close"),
            ],
        })
    }
}

impl CardHistory {
    fn render_summary(&self, frame: &mut Frame, area: Rect, card: &Card, accent: Color) {
        let block = Block::default().borders(Borders::RIGHT);
        let inner = block.inner(area);
        frame.render_widget(block, area);

        let rows = inner.height as usize;
        let offset = (self.selected + 1).saturating_sub(rows);
        let text_width = (inner.width as usize).saturating_sub(2);

        let lines: Vec<Line> = (0..Self::row_count(card))
            .skip(offset)
            .take(rows)
            .map(|row| {
                let (at, action) = match card.history.len().checked_sub(row + 1) {
                    Some(i) => (card.history[i].at, card.history[i].action.as_str()),
                    None => (card.created_at, "Created"),
                };
                let selected = row == self.selected;
                let marker = if selected { "» " } else { "  " };
                let style = if selected {
                    Style::default().fg(accent).add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };
                let text = truncate(&format!("{} {action}", short_stamp(at)), text_width);
                Line::from(Span::styled(format!("{marker}{text}"), style))
            })
            .collect();
        frame.render_widget(Paragraph::new(lines), inner);
    }

    fn render_detail(&self, frame: &mut Frame, area: Rect, card: &Card) {
        let area = Rect { x: area.x + 1, width: area.width.saturating_sub(1), ..area };
        let [body, footer] =
            Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).areas(area);

        let dim = Style::default().fg(Color::DarkGray);
        let mut lines: Vec<Line> = Vec::new();

        let (at, action, field, old, new) = match self.history_index(card) {
            Some(i) => {
                let e = &card.history[i];
                (e.at, e.action.as_str(), e.field, e.old.as_deref(), e.new.as_deref())
            }
            None => (card.created_at, "Created", HistoryField::General, None, None),
        };

        lines.push(Line::from(vec![
            Span::styled("At      ", dim),
            Span::raw(at.with_timezone(&Local).format("%Y-%m-%d %H:%M:%S").to_string()),
        ]));
        lines.push(Line::from(vec![Span::styled("Field   ", dim), Span::raw(field.label())]));
        lines.push(Line::from(vec![Span::styled("Action  ", dim), Span::raw(action.to_string())]));
        lines.push(Line::raw(""));

        if old.is_none() && new.is_none() {
            lines.push(Line::from(Span::styled("No field values recorded.", dim)));
        } else {
            for (kind, text) in diff_lines(old.unwrap_or(""), new.unwrap_or("")) {
                let (prefix, style) = match kind {
                    DiffKind::Same => ("  ", dim),
                    DiffKind::Added => ("+ ", Style::default().fg(Color::Green)),
                    DiffKind::Removed => ("- ", Style::default().fg(Color::Red)),
                };
                lines.push(Line::from(Span::styled(format!("{prefix}{text}"), style)));
            }
        }

        let scroll = self.detail_scroll.min(lines.len().saturating_sub(1) as u16);
        frame.render_widget(
            Paragraph::new(lines).wrap(Wrap { trim: false }).scroll((scroll, 0)),
            body,
        );

        if self.confirming_delete {
            frame.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::styled("Delete this entry? ", Style::default().fg(Color::Red)),
                    Span::styled(" y ", Style::default().fg(Color::Black).bg(Color::Red)),
                    Span::raw(" Yes  "),
                    Span::styled(" n ", Style::default().fg(Color::Black).bg(Color::Green)),
                    Span::raw(" No"),
                ])),
                footer,
            );
        }
    }
}

fn short_stamp(at: DateTime<Utc>) -> String {
    at.with_timezone(&Local).format("%Y-%m-%d %H:%M").to_string()
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
        out.push('…');
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::LoadedBoard;
    use crossterm::event::KeyModifiers;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn key(c: KeyCode) -> KeyEvent {
        KeyEvent::new(c, KeyModifiers::empty())
    }

    fn with_history_board(f: impl FnOnce(&LoadedBoard)) {
        crate::test_support::with_temp_dir(|| {
            let mut meta = crate::model::board::BoardMeta::new("Test".into());
            let list = crate::test_support::add_list(&mut meta, "L");
            let mut card = Card::new("Task".into());
            card.log_change(HistoryField::Title, "Edited title", Some("old".into()), Some("new".into()));
            card.log_change(HistoryField::Labels, "Added label 'foo'", Some("".into()), Some("foo".into()));
            crate::test_support::put_cards(&meta.id, &list, std::slice::from_mut(&mut card));
            let editor = crate::board_editor::BoardEditor::load(&meta.id).unwrap();
            f(editor.board());
        });
    }

    fn render_text(dialog: &CardHistory, board: &LoadedBoard) -> String {
        let mut terminal = Terminal::new(TestBackend::new(120, 34)).unwrap();
        terminal
            .draw(|f| dialog.render(f, f.area(), Some(board), Color::Cyan))
            .unwrap();
        terminal.backend().to_string()
    }

    #[test]
    fn navigation_moves_selection_and_clamps() {
        with_history_board(|board| {
            let mut d = CardHistory::new();
            for _ in 0..3 {
                d.handle_key(key(KeyCode::Down), Some(board));
            }
            assert_eq!(d.selected, 2);
            d.handle_key(key(KeyCode::Char('g')), Some(board));
            assert_eq!(d.selected, 0);
        });
    }

    #[test]
    fn detail_pane_shows_diff_of_selected_entry() {
        with_history_board(|board| {
            let mut d = CardHistory::new();
            let text = render_text(&d, board);
            assert!(text.contains("+ foo"), "{text}");
            d.handle_key(key(KeyCode::Down), Some(board));
            let text = render_text(&d, board);
            assert!(text.contains("- old") && text.contains("+ new"), "{text}");
        });
    }

    #[test]
    fn delete_needs_confirmation_and_targets_selected_entry() {
        with_history_board(|board| {
            let mut d = CardHistory::new();
            d.handle_key(key(KeyCode::Down), Some(board));
            let out = d.handle_key(key(KeyCode::Char('x')), Some(board));
            assert!(out.apply.is_none());
            let out = d.handle_key(key(KeyCode::Char('n')), Some(board));
            assert!(out.apply.is_none());
            d.handle_key(key(KeyCode::Char('x')), Some(board));
            let out = d.handle_key(key(KeyCode::Char('y')), Some(board));
            assert!(matches!(out.apply, Some(Command::DeleteHistoryEntry { index: 0, .. })));
        });
    }

    #[test]
    fn created_row_cannot_be_deleted() {
        with_history_board(|board| {
            let mut d = CardHistory::new();
            d.handle_key(key(KeyCode::Char('G')), Some(board));
            d.handle_key(key(KeyCode::Char('x')), Some(board));
            let out = d.handle_key(key(KeyCode::Char('y')), Some(board));
            assert!(out.apply.is_none());
        });
    }

    #[test]
    fn created_row_shows_no_recorded_values() {
        with_history_board(|board| {
            let mut d = CardHistory::new();
            d.handle_key(key(KeyCode::Char('G')), Some(board));
            let text = render_text(&d, board);
            assert!(text.contains("No field values recorded."), "{text}");
            assert!(text.contains("Created"), "{text}");
        });
    }

    #[test]
    fn summary_lists_every_action_newest_first() {
        with_history_board(|board| {
            let text = render_text(&CardHistory::new(), board);
            let labels = text.find("Added label 'foo'").expect("label row");
            let title = text.find("Edited title").expect("title row");
            assert!(labels < title, "{text}");
        });
    }

    #[test]
    fn page_keys_scroll_detail_and_selection_resets_it() {
        with_history_board(|board| {
            let mut d = CardHistory::new();
            d.handle_key(key(KeyCode::PageDown), Some(board));
            assert_eq!(d.detail_scroll, 5);
            d.handle_key(key(KeyCode::PageUp), Some(board));
            assert_eq!(d.detail_scroll, 0);
            d.handle_key(key(KeyCode::PageDown), Some(board));
            d.handle_key(key(KeyCode::Down), Some(board));
            assert_eq!(d.detail_scroll, 0);
        });
    }

    #[test]
    fn deleting_oldest_entry_moves_selection_onto_created_row() {
        with_history_board(|board| {
            let mut d = CardHistory::new();
            d.handle_key(key(KeyCode::Down), Some(board));
            d.handle_key(key(KeyCode::Char('d')), Some(board));
            let out = d.handle_key(key(KeyCode::Char('y')), Some(board));
            assert!(matches!(out.apply, Some(Command::DeleteHistoryEntry { index: 0, .. })));
            assert_eq!(out.status.as_deref(), Some("Deleted history entry"));
            assert_eq!(d.selected, 1);
        });
    }

    #[test]
    fn any_other_key_cancels_pending_delete() {
        with_history_board(|board| {
            let mut d = CardHistory::new();
            d.handle_key(key(KeyCode::Delete), Some(board));
            d.handle_key(key(KeyCode::Char('z')), Some(board));
            let out = d.handle_key(key(KeyCode::Char('y')), Some(board));
            assert!(out.apply.is_none());
        });
    }

    #[test]
    fn close_and_help_keys() {
        with_history_board(|board| {
            let mut d = CardHistory::new();
            assert!(matches!(d.handle_key(key(KeyCode::Esc), Some(board)).follow, super::super::Follow::Close));
            assert!(matches!(d.handle_key(key(KeyCode::Char('?')), Some(board)).follow, super::super::Follow::Help));
            assert!(d.help().unwrap().rows.iter().any(|(k, _)| k.contains("Del")));
        });
    }
}
