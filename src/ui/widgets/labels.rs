use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};

use crate::model::label::Label;

/// Builds `[name]` label chips packed left-to-right, wrapped at `width`
/// columns. A chip is atomic: it never breaks across lines — not even at
/// spaces inside the label name. A chip wider than the full line is cut
/// off at `width` instead of being split. Used by the card widget, the
/// card-height calculation, and the card-detail Labels section — all
/// three must agree on the line count.
pub fn label_lines(labels: &[&Label], width: usize, dimmed: bool) -> Vec<Line<'static>> {
    let mut lines: Vec<Line<'static>> = Vec::new();
    if labels.is_empty() {
        return lines;
    }
    let width = width.max(1);
    let mut current: Vec<Span<'static>> = Vec::new();
    let mut current_w = 0usize;

    for label in labels {
        let style = if dimmed {
            Style::default().fg(Color::DarkGray)
        } else {
            Style::default()
                .fg(Color::Black)
                .bg(label.color.to_ratatui_color())
        };
        let mut text = format!("[{}]", label.name);
        let mut chip_w = text.chars().count();
        if chip_w > width {
            text = text.chars().take(width).collect();
            chip_w = width;
        }
        if current_w + chip_w > width && !current.is_empty() {
            lines.push(Line::from(std::mem::take(&mut current)));
            current_w = 0;
        }
        current_w += chip_w;
        current.push(Span::styled(text, style));
    }
    if !current.is_empty() {
        lines.push(Line::from(current));
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::label::LabelColor;

    fn labels(names: &[&str]) -> Vec<Label> {
        names
            .iter()
            .map(|n| Label::new((*n).into(), LabelColor::Red))
            .collect()
    }

    fn line_text(line: &Line) -> String {
        line.spans.iter().map(|s| s.content.as_ref()).collect()
    }

    #[test]
    fn empty_labels_no_lines() {
        assert!(label_lines(&[], 20, false).is_empty());
    }

    #[test]
    fn chips_pack_onto_one_line() {
        let owned = labels(&["a", "bb"]);
        let refs: Vec<&Label> = owned.iter().collect();
        let lines = label_lines(&refs, 20, false);
        assert_eq!(lines.len(), 1);
        assert_eq!(line_text(&lines[0]), "[a][bb]");
    }

    #[test]
    fn wraps_at_chip_boundary() {
        // "[aa][bb]" = 8 wide; width 7 forces "[bb]" onto line 2 intact.
        let owned = labels(&["aa", "bb"]);
        let refs: Vec<&Label> = owned.iter().collect();
        let lines = label_lines(&refs, 7, false);
        assert_eq!(lines.len(), 2);
        assert_eq!(line_text(&lines[0]), "[aa]");
        assert_eq!(line_text(&lines[1]), "[bb]");
    }

    #[test]
    fn oversized_chip_cut_off_never_split() {
        let owned = labels(&["abcdefghij"]);
        let refs: Vec<&Label> = owned.iter().collect();
        let lines = label_lines(&refs, 5, false);
        // "[abcdefghij]" = 12 chars at width 5 → cut off, no continuation
        assert_eq!(lines.len(), 1);
        assert_eq!(line_text(&lines[0]), "[abcd");
    }

    #[test]
    fn oversized_chip_goes_on_own_line() {
        let owned = labels(&["a", "abcdefghij", "b"]);
        let refs: Vec<&Label> = owned.iter().collect();
        let lines = label_lines(&refs, 5, false);
        assert_eq!(lines.len(), 3);
        assert_eq!(line_text(&lines[0]), "[a]");
        assert_eq!(line_text(&lines[1]), "[abcd");
        assert_eq!(line_text(&lines[2]), "[b]");
    }

    #[test]
    fn chip_with_spaces_wraps_as_one_unit() {
        // "[good first issue]" = 18 wide: doesn't fit after "[bug]" at
        // width 20, moves to the next line whole — no break at spaces.
        let owned = labels(&["bug", "good first issue"]);
        let refs: Vec<&Label> = owned.iter().collect();
        let lines = label_lines(&refs, 20, false);
        assert_eq!(lines.len(), 2);
        assert_eq!(line_text(&lines[0]), "[bug]");
        assert_eq!(line_text(&lines[1]), "[good first issue]");
    }

    #[test]
    fn oversized_chip_with_spaces_cut_off_not_word_wrapped() {
        let owned = labels(&["good first"]);
        let refs: Vec<&Label> = owned.iter().collect();
        // "[good first]" = 12 wide at width 10 → cut, not broken at the space
        let lines = label_lines(&refs, 10, false);
        assert_eq!(lines.len(), 1);
        assert_eq!(line_text(&lines[0]), "[good firs");
    }
}
