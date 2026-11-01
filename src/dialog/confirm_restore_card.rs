//! Confirmation dialog before restoring an archived card.

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Color;

use super::{Dialog, DialogOutcome, DialogSideEffect};
use crate::app::LoadedBoard;
use crate::model::card::Card;

pub struct ConfirmRestoreCard {
    pub card: Card,
    pub cards: Vec<Card>,
    pub selected: usize,
}

impl Dialog for ConfirmRestoreCard {
    fn render(
        &self,
        frame: &mut Frame,
        area: Rect,
        _board: Option<&LoadedBoard>,
        _accent: Color,
    ) {
        let name = self.card.title.as_str();
        super::common::render_confirm(frame, area, "Restore Card", &format!("Restore '{name}'?"));
    }

    fn handle_key(&mut self, key: KeyEvent, _board: Option<&LoadedBoard>) -> DialogOutcome {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                let card = self.card.clone();
                let title = card.title.clone();
                let mut cards = self.cards.clone();
                if self.selected < cards.len() {
                    cards.remove(self.selected);
                }
                let selected = if self.selected > 0 && self.selected >= cards.len() {
                    cards.len().saturating_sub(1)
                } else {
                    self.selected
                };
                let mut out =
                    DialogOutcome::side_effect(DialogSideEffect::StageAndRestoreCard { card })
                        .with_status(format!("Restored card '{title}'"));
                if cards.is_empty() {
                    out = out.with_close();
                } else {
                    out.follow = super::Follow::Open(Box::new(super::archived_cards::ArchivedCards {
                        cards,
                        selected,
                    }));
                }
                out
            }
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                if self.cards.is_empty() {
                    DialogOutcome::close()
                } else {
                    DialogOutcome::open(Box::new(super::archived_cards::ArchivedCards {
                        cards: self.cards.clone(),
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
            title: " Help — Restore Card ",
            rows: vec![("y", "Restore card"), ("n / Esc", "Cancel")],
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
    fn n_and_esc_reopen_archived_cards_without_restoring() {
        let card = Card::new("Test".into());
        let cards = vec![card.clone()];
        for code in [KeyCode::Char('n'), KeyCode::Esc] {
            let mut dialog = ConfirmRestoreCard {
                card: card.clone(),
                cards: cards.clone(),
                selected: 0,
            };
            let out = dialog.handle_key(key(code), None);
            assert!(out.side_effect.is_none());
            assert!(matches!(out.follow, super::super::Follow::Open(_)));
        }
    }

    #[test]
    fn y_applies_stage_and_restore_and_closes_when_empty() {
        let card = Card::new("Test".into());
        let cards = vec![card.clone()];
        let mut dialog = ConfirmRestoreCard {
            card: card.clone(),
            cards,
            selected: 0,
        };
        let out = dialog.handle_key(key(KeyCode::Char('y')), None);
        assert!(matches!(
            out.side_effect,
            Some(DialogSideEffect::StageAndRestoreCard { .. })
        ));
        assert!(matches!(out.follow, super::super::Follow::Close));
        assert_eq!(out.status, Some("Restored card 'Test'".into()));
    }

    #[test]
    fn y_reopens_archived_cards_when_more_cards_remain() {
        let card1 = Card::new("Test 1".into());
        let card2 = Card::new("Test 2".into());
        let cards = vec![card1.clone(), card2.clone()];
        let mut dialog = ConfirmRestoreCard {
            card: card1,
            cards,
            selected: 0,
        };
        let out = dialog.handle_key(key(KeyCode::Char('y')), None);
        assert!(matches!(
            out.side_effect,
            Some(DialogSideEffect::StageAndRestoreCard { .. })
        ));
        assert!(matches!(out.follow, super::super::Follow::Open(_)));
        assert_eq!(out.status, Some("Restored card 'Test 1'".into()));
    }

    #[test]
    fn help_and_unrelated_keys() {
        let card = Card::new("Test".into());
        let mut dialog = ConfirmRestoreCard {
            card: card.clone(),
            cards: vec![card],
            selected: 0,
        };
        let out = dialog.handle_key(key(KeyCode::Char('?')), None);
        assert!(matches!(out.follow, super::super::Follow::Help));
        let out = dialog.handle_key(key(KeyCode::Char('z')), None);
        assert!(matches!(out.follow, super::super::Follow::Stay));
        assert!(dialog.help().is_some());
    }
}
