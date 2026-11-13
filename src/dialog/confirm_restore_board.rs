//! Confirmation dialog before restoring an archived board from the
//! Board Selector. Uses the **Board Selector** as background layer.

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Color;

use super::{Dialog, DialogBackground, DialogOutcome, DialogSideEffect};
use crate::app::LoadedBoard;
use crate::model::board::BoardMeta;

pub struct ConfirmRestoreBoard {
    pub board: BoardMeta,
    pub boards: Vec<BoardMeta>,
    pub selected: usize,
}

impl Dialog for ConfirmRestoreBoard {
    fn render(&self, frame: &mut Frame, area: Rect, _board: Option<&LoadedBoard>, _accent: Color) {
        super::common::render_confirm(
            frame,
            area,
            "Restore Board",
            &format!("Restore board '{}'?", self.board.name),
        );
    }

    fn handle_key(&mut self, key: KeyEvent, _board: Option<&LoadedBoard>) -> DialogOutcome {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                let board_id = self.board.id.clone();
                let name = self.board.name.clone();
                let mut boards = self.boards.clone();
                if self.selected < boards.len() {
                    boards.remove(self.selected);
                }
                let selected = if self.selected > 0 && self.selected >= boards.len() {
                    boards.len().saturating_sub(1)
                } else {
                    self.selected
                };
                let mut out =
                    DialogOutcome::side_effect(DialogSideEffect::RestoreArchivedBoard { board_id })
                        .with_status(format!("Restored board '{name}'"));
                if boards.is_empty() {
                    out = out.with_close_to(crate::app::AppMode::BoardSelector);
                } else {
                    out.follow =
                        super::Follow::Open(Box::new(super::archived_boards::ArchivedBoards {
                            boards,
                            selected,
                        }));
                }
                out
            }
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                if self.boards.is_empty() {
                    DialogOutcome::close_to(crate::app::AppMode::BoardSelector)
                } else {
                    DialogOutcome::open(Box::new(super::archived_boards::ArchivedBoards {
                        boards: self.boards.clone(),
                        selected: self.selected,
                    }))
                }
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
            title: " Help — Restore Board ",
            rows: vec![("y", "restore board"), ("n / esc", "cancel")],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyModifiers;

    fn key(c: KeyCode) -> KeyEvent {
        KeyEvent::new(c, KeyModifiers::empty())
    }

    #[test]
    fn n_and_esc_reopen_archived_boards_without_restoring() {
        let board = BoardMeta::new("Board A".into());
        let boards = vec![board.clone()];
        for code in [KeyCode::Char('n'), KeyCode::Esc] {
            let mut dialog = ConfirmRestoreBoard {
                board: board.clone(),
                boards: boards.clone(),
                selected: 0,
            };
            let out = dialog.handle_key(key(code), None);
            assert!(out.side_effect.is_none());
            assert!(matches!(out.follow, super::super::Follow::Open(_)));
        }
    }

    #[test]
    fn y_applies_restore_side_effect_and_closes_to_board_selector_when_empty() {
        let board = BoardMeta::new("Board A".into());
        let boards = vec![board.clone()];
        let mut dialog = ConfirmRestoreBoard {
            board: board.clone(),
            boards,
            selected: 0,
        };
        let out = dialog.handle_key(key(KeyCode::Char('y')), None);
        assert!(matches!(
            out.side_effect,
            Some(DialogSideEffect::RestoreArchivedBoard { .. })
        ));
        assert!(matches!(
            out.follow,
            super::super::Follow::CloseTo(crate::app::AppMode::BoardSelector)
        ));
        assert_eq!(out.status, Some("Restored board 'Board A'".into()));
    }

    #[test]
    fn y_reopens_archived_boards_when_more_boards_remain() {
        let b1 = BoardMeta::new("Board 1".into());
        let b2 = BoardMeta::new("Board 2".into());
        let boards = vec![b1.clone(), b2.clone()];
        let mut dialog = ConfirmRestoreBoard {
            board: b1,
            boards,
            selected: 0,
        };
        let out = dialog.handle_key(key(KeyCode::Char('y')), None);
        assert!(matches!(
            out.side_effect,
            Some(DialogSideEffect::RestoreArchivedBoard { .. })
        ));
        assert!(matches!(out.follow, super::super::Follow::Open(_)));
        assert_eq!(out.status, Some("Restored board 'Board 1'".into()));
    }

    #[test]
    fn background_is_board_selector() {
        let board = BoardMeta::new("Board A".into());
        let dialog = ConfirmRestoreBoard {
            board: board.clone(),
            boards: vec![board],
            selected: 0,
        };
        assert_eq!(dialog.background(), DialogBackground::BoardSelector);
    }

    #[test]
    fn help_and_unrelated_keys() {
        let board = BoardMeta::new("Board A".into());
        let mut dialog = ConfirmRestoreBoard {
            board: board.clone(),
            boards: vec![board],
            selected: 0,
        };
        let out = dialog.handle_key(key(KeyCode::Char('?')), None);
        assert!(matches!(out.follow, super::super::Follow::Help));
        let out = dialog.handle_key(key(KeyCode::Char('z')), None);
        assert!(matches!(out.follow, super::super::Follow::Stay));
        assert!(dialog.help().is_some());
    }
}
