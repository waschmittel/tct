//! Single-line text buffer with cursor — the editing primitive shared by
//! all line-editor insert handlers (card titles, list names, checklist
//! items, board names, label names).
//!
//! UTF-8 aware: `cursor` is a byte offset that always lands on a char
//! boundary. `Backspace` removes the prior char (1–4 bytes), `Left`/
//! `Right` walk char-by-char.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// Single-line text buffer with cursor (byte offset on a char boundary).
pub struct LineInput {
    pub buffer: String,
    pub cursor: usize,
}

/// Higher-level event a [`LineInput`] handler should react to after the
/// generic key has been consumed by [`LineInput::handle_key`].
pub enum LineKey {
    /// Buffer was modified; otherwise no action needed.
    Edited,
    /// `Enter` pressed — caller should confirm.
    Confirm,
    /// `Esc` pressed — caller should cancel.
    Cancel,
    /// Key was unhandled at this level.
    Ignored,
}

impl LineInput {
    pub fn new() -> Self {
        Self {
            buffer: String::new(),
            cursor: 0,
        }
    }

    pub fn with_initial(initial: &str) -> Self {
        Self {
            buffer: initial.to_string(),
            cursor: initial.len(),
        }
    }

    /// Process a single key event. Returns [`LineKey`] indicating whether
    /// the caller should confirm, cancel, or just keep editing.
    pub fn handle_key(&mut self, key: KeyEvent) -> LineKey {
        match key.code {
            KeyCode::Esc => LineKey::Cancel,
            KeyCode::Enter => LineKey::Confirm,
            KeyCode::Backspace => {
                if self.cursor > 0
                    && let Some((idx, _)) = self.buffer[..self.cursor].char_indices().last()
                {
                    self.buffer.remove(idx);
                    self.cursor = idx;
                }
                LineKey::Edited
            }
            KeyCode::Delete if self.cursor < self.buffer.len() => {
                self.buffer.remove(self.cursor);
                LineKey::Edited
            }
            KeyCode::Left => {
                if self.cursor > 0
                    && let Some((idx, _)) = self.buffer[..self.cursor].char_indices().last()
                {
                    self.cursor = idx;
                }
                LineKey::Edited
            }
            KeyCode::Right => {
                if self.cursor < self.buffer.len()
                    && let Some(c) = self.buffer[self.cursor..].chars().next()
                {
                    self.cursor += c.len_utf8();
                }
                LineKey::Edited
            }
            KeyCode::Home => {
                self.cursor = 0;
                LineKey::Edited
            }
            KeyCode::End => {
                self.cursor = self.buffer.len();
                LineKey::Edited
            }
            KeyCode::Char('u') if has_ctrl_or_cmd(key.modifiers) => {
                self.buffer.clear();
                self.cursor = 0;
                LineKey::Edited
            }
            KeyCode::Char('a') if has_ctrl_or_cmd(key.modifiers) => {
                self.cursor = 0;
                LineKey::Edited
            }
            KeyCode::Char('e') if has_ctrl_or_cmd(key.modifiers) => {
                self.cursor = self.buffer.len();
                LineKey::Edited
            }
            KeyCode::Char(c) => {
                self.buffer.insert(self.cursor, c);
                self.cursor += c.len_utf8();
                LineKey::Edited
            }
            _ => LineKey::Ignored,
        }
    }

    pub fn trimmed(&self) -> String {
        self.buffer.trim().to_string()
    }
}

/// Returns true if either Ctrl or macOS Cmd (Super) is held.
pub fn has_ctrl_or_cmd(modifiers: KeyModifiers) -> bool {
    modifiers.contains(KeyModifiers::CONTROL) || modifiers.contains(KeyModifiers::SUPER)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    fn k(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::empty())
    }

    #[test]
    fn umlaut_insertion_keeps_cursor_on_char_boundary() {
        let mut l = LineInput::new();
        l.handle_key(k(KeyCode::Char('ä')));
        l.handle_key(k(KeyCode::Char('b')));
        assert_eq!(l.buffer, "äb");
        // 2 bytes for 'ä' + 1 byte for 'b'.
        assert_eq!(l.cursor, 3);
    }

    #[test]
    fn utf8_backspace_left_right_delete() {
        let mut l = LineInput::with_initial("äöü");
        assert_eq!(l.cursor, 6);

        l.handle_key(k(KeyCode::Backspace));
        assert_eq!(l.buffer, "äö");
        assert_eq!(l.cursor, 4);

        l.handle_key(k(KeyCode::Left));
        assert_eq!(l.cursor, 2);

        l.handle_key(k(KeyCode::Delete));
        assert_eq!(l.buffer, "ä");
        assert_eq!(l.cursor, 2);

        l.handle_key(k(KeyCode::Left));
        assert_eq!(l.cursor, 0);

        l.handle_key(k(KeyCode::Right));
        assert_eq!(l.cursor, 2);

        l.handle_key(k(KeyCode::Backspace));
        assert_eq!(l.buffer, "");
        assert_eq!(l.cursor, 0);
    }
}
