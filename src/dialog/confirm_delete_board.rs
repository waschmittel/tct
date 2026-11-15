//! Confirmation dialog before permanently deleting an archived board
//! (more impactful than deleting a card: the board's cards go with it).
//! Uses the **Board Selector** as background layer.

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Color;

use super::{Dialog, DialogBackground, DialogOutcome, DialogSideEffect};
use crate::app::LoadedBoard;
use crate::model::board::BoardMeta;

pub struct ConfirmDeleteBoard {
    pub board: BoardMeta,
    pub boards: Vec<BoardMeta>,
    pub selected: usize,
}

impl ConfirmDeleteBoard {
    fn reopen(boards: Vec<BoardMeta>, selected: usize) -> DialogOutcome {
        if boards.is_empty() {
            DialogOutcome::close_to(crate::app::AppMode::BoardSelector)
        } else {
            DialogOutcome::open(Box::new(super::archived_boards::ArchivedBoards {
                boards,
                selected,
            }))
        }
    }
}

impl Dialog for ConfirmDeleteBoard {
    fn render(&self, frame: &mut Frame, area: Rect, _board: Option<&LoadedBoard>, _accent: Color) {
        super::common::render_confirm(
            frame,
            area,
            "Delete Board",
            &format!("Permanently delete board '{}'?", self.board.name),
        );
    }

    fn handle_key(&mut self, key: KeyEvent, _board: Option<&LoadedBoard>) -> DialogOutcome {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                let mut boards = self.boards.clone();
                if self.selected < boards.len() {
                    boards.remove(self.selected);
                }
                let selected = self.selected.min(boards.len().saturating_sub(1));
                let mut out = Self::reopen(boards, selected);
                out.side_effect = Some(DialogSideEffect::DeleteArchivedBoard {
                    board_id: self.board.id.clone(),
                });
                out.status = Some(format!("Deleted board '{}'", self.board.name));
                out
            }
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                Self::reopen(self.boards.clone(), self.selected)
            }
            KeyCode::Char('?') => DialogOutcome::help(),
            _ => DialogOutcome::stay(),
        }
    }

    fn background(&self) -> DialogBackground {
        DialogBackground::BoardSelector
    }

    fn help(&self) -> Option<super::DialogHelp> {
        Some(super::DialogHelp {
            title: " Help — Delete Board ",
            rows: vec![("y", "delete permanently"), ("n / esc", "cancel")],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dialog::Follow;
    use crossterm::event::KeyModifiers;

    fn key(c: KeyCode) -> KeyEvent {
        KeyEvent::new(c, KeyModifiers::empty())
    }

    fn dialog(n: usize, selected: usize) -> ConfirmDeleteBoard {
        let boards: Vec<_> = (0..n)
            .map(|i| BoardMeta::new(format!("Board {i}")))
            .collect();
        ConfirmDeleteBoard {
            board: boards[selected].clone(),
            boards,
            selected,
        }
    }

    #[test]
    fn n_and_esc_reopen_without_deleting() {
        for code in [KeyCode::Char('n'), KeyCode::Esc] {
            let out = dialog(2, 1).handle_key(key(code), None);
            assert!(out.side_effect.is_none());
            assert!(out.status.is_none());
            assert!(matches!(out.follow, Follow::Open(_)));
        }
    }

    #[test]
    fn y_deletes_and_closes_when_last() {
        let out = dialog(1, 0).handle_key(key(KeyCode::Char('y')), None);
        assert!(matches!(
            out.side_effect,
            Some(DialogSideEffect::DeleteArchivedBoard { .. })
        ));
        assert!(matches!(
            out.follow,
            Follow::CloseTo(crate::app::AppMode::BoardSelector)
        ));
        assert_eq!(out.status, Some("Deleted board 'Board 0'".into()));
    }

    #[test]
    fn y_reopens_list_without_deleted_board() {
        let out = dialog(3, 2).handle_key(key(KeyCode::Char('y')), None);
        assert!(out.side_effect.is_some());
        let Follow::Open(_) = out.follow else {
            panic!("expected reopen")
        };
    }

    #[test]
    fn help_and_unrelated_keys() {
        let mut d = dialog(1, 0);
        assert!(matches!(
            d.handle_key(key(KeyCode::Char('?')), None).follow,
            Follow::Help
        ));
        assert!(matches!(
            d.handle_key(key(KeyCode::Char('z')), None).follow,
            Follow::Stay
        ));
        assert!(d.help().is_some());
        assert_eq!(d.background(), DialogBackground::BoardSelector);
    }
}
