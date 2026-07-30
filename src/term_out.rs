//! Terminal output for the non-TUI surfaces (the `tct <command>` CLI and the
//! warnings the stores print while loading).
//!
//! Card titles, list names, file names and parse errors are all attacker-
//! controlled when the data directory came from a checked-out repository, and
//! writing them to a terminal verbatim hands them an escape-sequence channel:
//! `ESC ]0;…BEL` retitles the window, OSC 52 writes the clipboard, and cursor
//! moves let one line overwrite another. The TUI is already safe — ratatui
//! drops control characters when it fills its cell buffer — so this module is
//! the equivalent choke point for everything printed outside it. Nothing in
//! `cli/` or `storage/` may call `println!`/`eprintln!` directly; the
//! [`outln!`](crate::outln) / [`errln!`](crate::errln) macros sanitize the
//! whole formatted line, so no interpolated value can escape by being added to
//! a format string later.

/// Strip the characters a terminal would act on rather than show. `\n` and
/// `\t` survive — multi-line descriptions and column alignment need them, and
/// neither can move the cursor backwards or start an escape sequence.
pub fn sanitize(text: &str) -> String {
    text.chars()
        .filter(|c| *c == '\n' || *c == '\t' || !is_terminal_control(*c))
        .collect()
}

fn is_terminal_control(c: char) -> bool {
    c.is_control() || matches!(c, '\u{80}'..='\u{9f}')
}

#[macro_export]
macro_rules! outln {
    () => { println!() };
    ($($arg:tt)*) => { println!("{}", $crate::term_out::sanitize(&format!($($arg)*))) };
}

#[macro_export]
macro_rules! errln {
    () => { eprintln!() };
    ($($arg:tt)*) => { eprintln!("{}", $crate::term_out::sanitize(&format!($($arg)*))) };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_escape_sequences() {
        assert_eq!(sanitize("a\u{1b}]0;PWNED\u{7}b"), "a]0;PWNEDb");
        assert_eq!(sanitize("red\u{1b}[31m"), "red[31m");
    }

    #[test]
    fn strips_carriage_return_and_backspace() {
        assert_eq!(sanitize("visible\rhidden"), "visiblehidden");
        assert_eq!(sanitize("ab\u{8}\u{8}cd"), "abcd");
    }

    #[test]
    fn strips_eight_bit_control_introducers() {
        assert_eq!(sanitize("a\u{9b}31mb"), "a31mb");
        assert_eq!(sanitize("a\u{9d}0;xb"), "a0;xb");
    }

    #[test]
    fn keeps_newline_and_tab() {
        assert_eq!(sanitize("line1\nline2\tcol"), "line1\nline2\tcol");
    }

    #[test]
    fn keeps_ordinary_and_wide_text() {
        assert_eq!(sanitize("Fix login bug — 日本語 🙂"), "Fix login bug — 日本語 🙂");
    }
}
