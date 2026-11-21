//! In-editor Find and Find & Replace for [`MarkdownEditor`].
//!
//! Uses `tui-textarea-2`'s built-in `search` feature for match highlighting
//! across soft-wrapped lines and hanging indents, while owning match scanning,
//! match counting, Tab-focusable option/action chips, and undo-grouped
//! replacements here.

use regex::Regex;
use tui_textarea::{CursorMove, TextArea};

use super::lists::renumber_all;
use super::normalize::fenced_rows;
use crate::insert::line_input::LineInput;
use crate::insert::table_edit::{self, grouped};

/// Which element inside the Find / Find & Replace bar currently has keyboard focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FindFocus {
    Query,
    Replace,
    ReplaceOne,
    ReplaceAll,
    CaseSensitive,
    Regex,
}

/// Byte span of a single match on a logical line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MatchSpan {
    pub row: usize,
    pub start_byte: usize,
    pub end_byte: usize,
}

/// Persistent search preferences remembered across bar open/close within an
/// editor session (so `F3` / `Shift+F3` and reopening `Ctrl+F` reuse them).
#[derive(Debug, Clone)]
pub struct FindHistory {
    pub query: String,
    pub replace: String,
    pub case_sensitive: bool,
    pub use_regex: bool,
}

impl Default for FindHistory {
    fn default() -> Self {
        Self {
            query: String::new(),
            replace: String::new(),
            case_sensitive: false,
            // Regex search is the default (ADR-0025).
            use_regex: true,
        }
    }
}

/// Active Find / Find & Replace bar state owned by [`super::MarkdownEditor`].
pub struct FindState {
    pub query: LineInput,
    pub replace: LineInput,
    pub replace_mode: bool,
    pub focus: FindFocus,
    pub case_sensitive: bool,
    pub use_regex: bool,
    /// Cursor `(row, char_col)` used as the starting anchor when refreshing matches.
    pub anchor: (usize, usize),
    pub matches: Vec<MatchSpan>,
    pub current: Option<usize>,
    pub regex_error: bool,
}

impl FindState {
    pub fn open(
        initial_query: &str,
        initial_replace: &str,
        replace_mode: bool,
        focus_replace: bool,
        case_sensitive: bool,
        use_regex: bool,
        anchor: (usize, usize),
    ) -> Self {
        let focus = if replace_mode && focus_replace && !initial_query.is_empty() {
            FindFocus::Replace
        } else {
            FindFocus::Query
        };
        Self {
            query: LineInput::with_initial(initial_query),
            replace: LineInput::with_initial(initial_replace),
            replace_mode,
            focus,
            case_sensitive,
            use_regex,
            anchor,
            matches: Vec::new(),
            current: None,
            regex_error: false,
        }
    }

    pub fn tab_order(&self) -> &'static [FindFocus] {
        if self.replace_mode {
            &[
                FindFocus::Query,
                FindFocus::Replace,
                FindFocus::ReplaceOne,
                FindFocus::ReplaceAll,
                FindFocus::CaseSensitive,
                FindFocus::Regex,
            ]
        } else {
            &[FindFocus::Query, FindFocus::CaseSensitive, FindFocus::Regex]
        }
    }

    pub fn focus_next(&mut self) {
        let order = self.tab_order();
        let idx = order.iter().position(|&f| f == self.focus).unwrap_or(0);
        self.focus = order[(idx + 1) % order.len()];
    }

    pub fn focus_prev(&mut self) {
        let order = self.tab_order();
        let idx = order.iter().position(|&f| f == self.focus).unwrap_or(0);
        self.focus = order[(idx + order.len() - 1) % order.len()];
    }

    /// Build the regex pattern string for the current query + toggles.
    pub fn pattern_string(&self) -> Option<String> {
        build_pattern(&self.query.buffer, self.case_sensitive, self.use_regex)
    }

    /// Recompile the pattern, update textarea highlights, recompute all matches,
    /// and jump to the first match at or after `self.anchor` (wrapping around).
    pub fn refresh(&mut self, ta: &mut TextArea<'static>) {
        let Some(pattern) = self.pattern_string() else {
            let _ = ta.set_search_pattern("");
            self.matches.clear();
            self.current = None;
            self.regex_error = false;
            return;
        };

        let re = match Regex::new(&pattern) {
            Ok(re) => re,
            Err(_) => {
                let _ = ta.set_search_pattern("");
                self.matches.clear();
                self.current = None;
                self.regex_error = true;
                return;
            }
        };

        self.regex_error = false;
        let _ = ta.set_search_pattern(&pattern);
        self.matches = collect_matches(ta.lines(), &re);

        if self.matches.is_empty() {
            self.current = None;
            return;
        }

        let (ar, ac) = self.anchor;
        let anchor_byte = ta.lines().get(ar).map(|l| char_to_byte(l, ac)).unwrap_or(0);
        let idx = self
            .matches
            .iter()
            .position(|m| (m.row, m.start_byte) >= (ar, anchor_byte))
            .unwrap_or(0);
        self.current = Some(idx);
        jump_to_match(ta, self.matches[idx]);
    }

    /// Jump to the next match (wrapping around). Returns whether a match exists.
    pub fn next_match(&mut self, ta: &mut TextArea<'static>) -> bool {
        if self.matches.is_empty() {
            return false;
        }
        let next = match self.current {
            Some(i) => (i + 1) % self.matches.len(),
            None => 0,
        };
        self.current = Some(next);
        let span = self.matches[next];
        jump_to_match(ta, span);
        self.anchor = ta.cursor();
        true
    }

    /// Jump to the previous match (wrapping around). Returns whether a match exists.
    pub fn prev_match(&mut self, ta: &mut TextArea<'static>) -> bool {
        if self.matches.is_empty() {
            return false;
        }
        let prev = match self.current {
            Some(i) => (i + self.matches.len() - 1) % self.matches.len(),
            None => self.matches.len() - 1,
        };
        self.current = Some(prev);
        let span = self.matches[prev];
        jump_to_match(ta, span);
        self.anchor = ta.cursor();
        true
    }

    /// Replace the currently focused match as one undo step, then advance to
    /// the next match. Returns `true` if a replacement was made.
    pub fn replace_current(&mut self, ta: &mut TextArea<'static>, paste_mode: bool) -> bool {
        let Some(idx) = self.current else {
            return false;
        };
        let Some(&span) = self.matches.get(idx) else {
            return false;
        };
        let Some(pattern) = self.pattern_string() else {
            return false;
        };
        let Ok(re) = Regex::new(&pattern) else {
            return false;
        };
        let Some(line) = ta.lines().get(span.row) else {
            return false;
        };
        let replacement = expand_replacement(&re, line, span, &self.replace.buffer, self.use_regex);
        let start_char = line[..span.start_byte].chars().count();
        let del_chars = line[span.start_byte..span.end_byte].chars().count();

        grouped(ta, None, |ta| {
            ta.cancel_selection();
            ta.move_cursor(CursorMove::Jump(clamp_u16(span.row), clamp_u16(start_char)));
            if del_chars > 0 {
                ta.delete_str(del_chars);
            }
            if !replacement.is_empty() {
                ta.insert_str(&replacement);
            }
            if !paste_mode {
                table_edit::reformat_at_cursor(ta);
                renumber_all(ta);
            }
        });

        // Anchor immediately after the inserted replacement so self-matching
        // replacements (e.g. "a" -> "aa") advance instead of re-matching.
        let (cr, cc) = ta.cursor();
        let next_col = if del_chars == 0 && replacement.is_empty() {
            cc.saturating_add(1)
        } else {
            cc
        };
        self.anchor = (cr, next_col);
        self.refresh(ta);
        true
    }

    /// Replace every match in the document as one single undo step.
    /// Returns the number of occurrences replaced.
    pub fn replace_all(&mut self, ta: &mut TextArea<'static>, paste_mode: bool) -> usize {
        if self.matches.is_empty() {
            return 0;
        }
        let Some(pattern) = self.pattern_string() else {
            return 0;
        };
        let Ok(re) = Regex::new(&pattern) else {
            return 0;
        };

        let lines = ta.lines();
        let edits: Vec<(MatchSpan, String)> = self
            .matches
            .iter()
            .filter_map(|&span| {
                let line = lines.get(span.row)?;
                let rep = expand_replacement(&re, line, span, &self.replace.buffer, self.use_regex);
                Some((span, rep))
            })
            .collect();
        let count = edits.len();
        if count == 0 {
            return 0;
        }

        grouped(ta, None, |ta| {
            ta.cancel_selection();
            // Apply in reverse order (bottom-to-top, right-to-left) so earlier
            // coordinates never shift and self-matching replacements cannot loop.
            for (span, replacement) in edits.into_iter().rev() {
                let line = &ta.lines()[span.row];
                let start_char = line[..span.start_byte].chars().count();
                let del_chars = line[span.start_byte..span.end_byte].chars().count();
                ta.move_cursor(CursorMove::Jump(clamp_u16(span.row), clamp_u16(start_char)));
                if del_chars > 0 {
                    ta.delete_str(del_chars);
                }
                if !replacement.is_empty() {
                    ta.insert_str(&replacement);
                }
            }
            if !paste_mode {
                let fenced = fenced_rows(ta.lines());
                table_edit::reformat_all(ta, &fenced);
                renumber_all(ta);
            }
        });

        self.anchor = ta.cursor();
        self.refresh(ta);
        count
    }

    /// Status text shown right-aligned on the Find row: `1/3`, `0/0`, `invalid`, or empty.
    pub fn status_label(&self) -> Option<String> {
        if self.query.buffer.is_empty() {
            return None;
        }
        if self.regex_error {
            return Some("invalid".into());
        }
        let total = self.matches.len();
        if total == 0 {
            Some("0/0".into())
        } else {
            let cur = self.current.map(|i| i + 1).unwrap_or(0);
            Some(format!("{cur}/{total}"))
        }
    }
}

/// Build a regex pattern string from a raw query and flags.
pub fn build_pattern(query: &str, case_sensitive: bool, use_regex: bool) -> Option<String> {
    if query.is_empty() {
        return None;
    }
    let body = if use_regex {
        query.to_string()
    } else {
        regex::escape(query)
    };
    if case_sensitive {
        Some(body)
    } else {
        Some(format!("(?i){body}"))
    }
}

/// Repeat the last search (used by `F3` / `Shift+F3` when the Find bar is closed).
/// Moves the cursor to the next (or previous) match and returns `true` if found.
pub fn repeat_search(ta: &mut TextArea<'static>, history: &FindHistory, forward: bool) -> bool {
    let Some(pattern) = build_pattern(&history.query, history.case_sensitive, history.use_regex)
    else {
        return false;
    };
    let Ok(re) = Regex::new(&pattern) else {
        return false;
    };
    let matches = collect_matches(ta.lines(), &re);
    if matches.is_empty() {
        return false;
    }
    let (cr, cc) = ta.cursor();
    let cur_byte = ta.lines().get(cr).map(|l| char_to_byte(l, cc)).unwrap_or(0);

    let target = if forward {
        matches
            .iter()
            .find(|m| (m.row, m.start_byte) > (cr, cur_byte))
            .or_else(|| matches.first())
    } else {
        matches
            .iter()
            .rev()
            .find(|m| (m.row, m.start_byte) < (cr, cur_byte))
            .or_else(|| matches.last())
    };

    if let Some(&span) = target {
        jump_to_match(ta, span);
        true
    } else {
        false
    }
}

/// Extract the current single-line selection text, if any, to pre-fill Find.
pub fn selected_single_line(ta: &TextArea<'static>) -> Option<String> {
    let ((r1, c1), (r2, c2)) = ta.selection_range()?;
    if r1 != r2 || c1 == c2 {
        return None;
    }
    let line = ta.lines().get(r1)?;
    let start = char_to_byte(line, c1);
    let end = char_to_byte(line, c2);
    Some(line[start..end].to_string())
}

fn collect_matches(lines: &[String], re: &Regex) -> Vec<MatchSpan> {
    let mut out = Vec::new();
    for (row, line) in lines.iter().enumerate() {
        for m in re.find_iter(line) {
            out.push(MatchSpan {
                row,
                start_byte: m.start(),
                end_byte: m.end(),
            });
        }
    }
    out
}

fn expand_replacement(
    re: &Regex,
    line: &str,
    span: MatchSpan,
    template: &str,
    use_regex: bool,
) -> String {
    if !use_regex {
        return template.to_string();
    }
    if let Some(caps) = re.captures_at(line, span.start_byte) {
        let mut out = String::new();
        caps.expand(template, &mut out);
        out
    } else {
        template.to_string()
    }
}

fn jump_to_match(ta: &mut TextArea<'static>, span: MatchSpan) {
    let start_char = ta
        .lines()
        .get(span.row)
        .map(|l| l[..span.start_byte].chars().count())
        .unwrap_or(0);
    ta.cancel_selection();
    ta.move_cursor(CursorMove::Jump(clamp_u16(span.row), clamp_u16(start_char)));
}

fn char_to_byte(line: &str, char_col: usize) -> usize {
    line.char_indices()
        .nth(char_col)
        .map(|(b, _)| b)
        .unwrap_or(line.len())
}

fn clamp_u16(n: usize) -> u16 {
    n.min(u16::MAX as usize) as u16
}
