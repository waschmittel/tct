//! Keymaps: one table per App Mode mapping a physical key to a mode action
//! plus the help-overlay text for that binding. Dispatch (`lookup`) and the
//! help overlay (`help_rows`) read the same table, so a binding is defined
//! exactly once.

use crossterm::event::KeyCode;

pub struct Binding<A: Copy> {
    pub code: KeyCode,
    /// `Some(_)` to require a shift state (arrow keys); `None` for char
    /// keys, where case already encodes shift.
    pub shift: Option<bool>,
    pub action: A,
    /// Key display in the help overlay, e.g. `"Shift+Up/Down"`. Bindings
    /// sharing a help row use the same string; `help_rows` dedupes.
    pub keys: &'static str,
    pub help: &'static str,
    pub section: &'static str,
}

pub fn lookup<A: Copy>(map: &[Binding<A>], code: KeyCode, shift: bool) -> Option<A> {
    map.iter()
        .find(|b| b.code == code && b.shift.map(|s| s == shift).unwrap_or(true))
        .map(|b| b.action)
}

/// Help rows of one section, in table order, deduped (e.g. `"Up / Down"`
/// appears once even though Up and Down are two bindings).
pub fn help_rows<A: Copy>(
    map: &[Binding<A>],
    section: &str,
) -> Vec<(&'static str, &'static str)> {
    let mut rows: Vec<(&'static str, &'static str)> = Vec::new();
    for b in map.iter().filter(|b| b.section == section) {
        if !rows.iter().any(|(k, h)| *k == b.keys && *h == b.help) {
            rows.push((b.keys, b.help));
        }
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Copy, PartialEq, Debug)]
    enum A {
        X,
        Y,
        Z,
    }

    const MAP: &[Binding<A>] = &[
        Binding { code: KeyCode::Up, shift: Some(false), action: A::X, keys: "Up / Down", help: "nav", section: "S" },
        Binding { code: KeyCode::Down, shift: Some(false), action: A::Y, keys: "Up / Down", help: "nav", section: "S" },
        Binding { code: KeyCode::Char('q'), shift: None, action: A::Z, keys: "q", help: "quit", section: "App" },
    ];

    #[test]
    fn lookup_respects_shift_requirement() {
        assert_eq!(lookup(MAP, KeyCode::Up, false), Some(A::X));
        assert_eq!(lookup(MAP, KeyCode::Up, true), None);
        assert_eq!(lookup(MAP, KeyCode::Char('q'), true), Some(A::Z));
    }

    #[test]
    fn help_rows_dedupes_shared_rows() {
        assert_eq!(help_rows(MAP, "S"), vec![("Up / Down", "nav")]);
        assert_eq!(help_rows(MAP, "App"), vec![("q", "quit")]);
    }
}
