//! Confirmation dialog before restoring an archived list.

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Color;

use super::{Dialog, DialogOutcome};
use crate::app::LoadedBoard;
use crate::command::Command;
use crate::model::list::CardList;

pub struct ConfirmRestoreList {
    pub list: CardList,
    pub lists: Vec<CardList>,
    pub selected: usize,
}

impl Dialog for ConfirmRestoreList {
    fn render(&self, frame: &mut Frame, area: Rect, _board: Option<&LoadedBoard>, _accent: Color) {
        let name = self.list.name.as_str();
        super::common::render_confirm(
            frame,
            area,
            "Restore List",
            &format!("Restore list '{name}'?"),
        );
    }

    fn handle_key(&mut self, key: KeyEvent, _board: Option<&LoadedBoard>) -> DialogOutcome {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                let list_id = self.list.id.clone();
                let name = self.list.name.clone();
                let mut lists = self.lists.clone();
                if self.selected < lists.len() {
                    lists.remove(self.selected);
                }
                let selected = if self.selected > 0 && self.selected >= lists.len() {
                    lists.len().saturating_sub(1)
                } else {
                    self.selected
                };
                let mut out = DialogOutcome::apply(Command::RestoreList { list_id })
                    .with_status(format!("Restored list '{name}'"));
                if lists.is_empty() {
                    out = out.with_close();
                } else {
                    out.follow =
                        super::Follow::Open(Box::new(super::archived_lists::ArchivedLists {
                            lists,
                            selected,
                        }));
                }
                out
            }
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                if self.lists.is_empty() {
                    DialogOutcome::close()
                } else {
                    DialogOutcome::open(Box::new(super::archived_lists::ArchivedLists {
                        lists: self.lists.clone(),
                        selected: self.selected,
                    }))
                }
            }
            KeyCode::Char('?') => DialogOutcome::help(),
            _ => DialogOutcome::stay(),
        }
    }

    fn help(&self) -> Option<super::DialogHelp> {
        Some(super::DialogHelp {
            title: " Help — Restore List ",
            rows: vec![("y", "restore list"), ("n / esc", "cancel")],
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
    fn n_and_esc_reopen_archived_lists_without_restoring() {
        let list = CardList::new("List A".into());
        let lists = vec![list.clone()];
        for code in [KeyCode::Char('n'), KeyCode::Esc] {
            let mut dialog = ConfirmRestoreList {
                list: list.clone(),
                lists: lists.clone(),
                selected: 0,
            };
            let out = dialog.handle_key(key(code), None);
            assert!(out.apply.is_none());
            assert!(matches!(out.follow, super::super::Follow::Open(_)));
        }
    }

    #[test]
    fn y_applies_restore_command_and_closes_when_empty() {
        let list = CardList::new("List A".into());
        let lists = vec![list.clone()];
        let mut dialog = ConfirmRestoreList {
            list: list.clone(),
            lists,
            selected: 0,
        };
        let out = dialog.handle_key(key(KeyCode::Char('y')), None);
        assert!(matches!(out.apply, Some(Command::RestoreList { .. })));
        assert!(matches!(out.follow, super::super::Follow::Close));
        assert_eq!(out.status, Some("Restored list 'List A'".into()));
    }

    #[test]
    fn y_reopens_archived_lists_when_more_lists_remain() {
        let l1 = CardList::new("List 1".into());
        let l2 = CardList::new("List 2".into());
        let lists = vec![l1.clone(), l2.clone()];
        let mut dialog = ConfirmRestoreList {
            list: l1,
            lists,
            selected: 0,
        };
        let out = dialog.handle_key(key(KeyCode::Char('y')), None);
        assert!(matches!(out.apply, Some(Command::RestoreList { .. })));
        assert!(matches!(out.follow, super::super::Follow::Open(_)));
        assert_eq!(out.status, Some("Restored list 'List 1'".into()));
    }

    #[test]
    fn help_and_unrelated_keys() {
        let list = CardList::new("List A".into());
        let mut dialog = ConfirmRestoreList {
            list: list.clone(),
            lists: vec![list],
            selected: 0,
        };
        let out = dialog.handle_key(key(KeyCode::Char('?')), None);
        assert!(matches!(out.follow, super::super::Follow::Help));
        let out = dialog.handle_key(key(KeyCode::Char('z')), None);
        assert!(matches!(out.follow, super::super::Follow::Stay));
        assert!(dialog.help().is_some());
    }
}
