use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;
use unicode_width::UnicodeWidthStr;

use crate::alert::Alert;

/// Detect leading list-marker (bullet or numbered) or citation indent so wrapped
/// continuation lines line up under the first character of the item text.
pub(crate) fn detect_list_indent(line: &str) -> usize {
    let trimmed = line.trim_start();
    let base_indent = line.len() - trimmed.len();
    if trimmed.starts_with("- ") || trimmed.starts_with("* ") {
        return base_indent + 2;
    }
    if let Some(dot_pos) = trimmed.find(". ") {
        let num_part = &trimmed[..dot_pos];
        if num_part.parse::<u64>().is_ok() {
            return base_indent + dot_pos + 2;
        }
    }
    if let Some(marker_len) = citation_marker_len(trimmed) {
        return base_indent + marker_len;
    }
    0
}

/// A line that is just `>>>` closes a multiline blockquote (GitLab style).
pub(crate) fn closes_quote(line: &str) -> bool {
    line.trim() == ">>>"
}

/// A line that opens or closes a multiline blockquote: `>>>`, or `>>> [!kind]`
/// (which opens an alert).
pub(crate) fn is_quote_fence(line: &str) -> bool {
    closes_quote(line) || fence_alert(line).is_some()
}

/// The alert a `>>> [!kind]` line opens.
pub(crate) fn fence_alert(line: &str) -> Option<Alert> {
    Alert::parse(line.trim().strip_prefix(">>> ")?)
}

/// The alert a `> [!kind]` line opens (single `>`; the caller checks that it
/// starts a quote).
pub(crate) fn quote_alert(line: &str) -> Option<Alert> {
    let trimmed = line.trim_start();
    let marker = citation_marker_len(trimmed)?;
    if trimmed[..marker].matches('>').count() != 1 {
        return None;
    }
    Alert::parse(&trimmed[marker..])
}

/// The tag line of an alert, shown as the kind in capitals, in the width of the tag.
pub(super) fn alert_header(alert: Alert, tag: &str, color: Color) -> Span<'static> {
    Span::styled(
        alert.header(tag.trim_end().width()),
        Style::default().fg(color).add_modifier(Modifier::BOLD),
    )
}

/// Parse citation/blockquote marker length in `trimmed` (e.g. `> `, `>> `, `> > `).
pub(crate) fn citation_marker_len(trimmed: &str) -> Option<usize> {
    if !trimmed.starts_with('>') || is_quote_fence(trimmed) {
        return None;
    }
    let mut len = 0;
    let mut chars = trimmed.char_indices().peekable();
    while let Some(&(i, c)) = chars.peek() {
        if c == '>' {
            chars.next();
            len = i + 1;
            while let Some(&(sp_i, ' ')) = chars.peek() {
                chars.next();
                len = sp_i + 1;
            }
        } else {
            break;
        }
    }
    if len > 0 && (len == trimmed.len() || trimmed[..len].ends_with(' ')) {
        Some(len)
    } else {
        None
    }
}
