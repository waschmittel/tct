//! GitHub/GitLab-style alerts: a quote whose first line is `[!note]`, `[!tip]`,
//! `[!important]`, `[!caution]` or `[!warning]` (`> [!note]`, or `>>> [!note]`
//! opening a multiline quote). Shared by the editor, the read view and the PDF.

use ratatui::style::Color;
use unicode_width::UnicodeWidthStr;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Alert {
    Note,
    Tip,
    Important,
    Caution,
    Warning,
}

impl Alert {
    pub const ALL: [Alert; 5] = [
        Alert::Note,
        Alert::Tip,
        Alert::Important,
        Alert::Caution,
        Alert::Warning,
    ];

    /// `[!kind]` (any case, surrounding whitespace ignored).
    pub fn parse(text: &str) -> Option<Alert> {
        let name = text.trim().strip_prefix("[!")?.strip_suffix(']')?;
        Self::ALL
            .into_iter()
            .find(|a| a.name().eq_ignore_ascii_case(name))
    }

    pub fn name(self) -> &'static str {
        match self {
            Alert::Note => "note",
            Alert::Tip => "tip",
            Alert::Important => "important",
            Alert::Caution => "caution",
            Alert::Warning => "warning",
        }
    }

    pub fn tag(self) -> String {
        format!("[!{}]", self.name())
    }

    /// Path data of GitHub's Primer Octicon (16×16, MIT, `assets/octicons/`)
    /// that marks the alert; used by the PDF only.
    pub fn octicon_path(self) -> &'static str {
        let svg = match self {
            Alert::Note => include_str!("../assets/octicons/info-16.svg"),
            Alert::Tip => include_str!("../assets/octicons/light-bulb-16.svg"),
            Alert::Important => include_str!("../assets/octicons/report-16.svg"),
            Alert::Warning => include_str!("../assets/octicons/alert-16.svg"),
            Alert::Caution => include_str!("../assets/octicons/stop-16.svg"),
        };
        svg.split("d=\"")
            .nth(1)
            .and_then(|s| s.split('"').next())
            .expect("octicon svg has a path")
    }

    /// Colors of GitHub's light theme (PDF).
    pub fn rgb(self) -> (u8, u8, u8) {
        match self {
            Alert::Note => (9, 105, 218),
            Alert::Tip => (26, 127, 55),
            Alert::Important => (130, 80, 223),
            Alert::Caution => (207, 34, 46),
            Alert::Warning => (154, 103, 0),
        }
    }

    /// Colors of GitHub's dark theme (terminal).
    pub fn tui_color(self) -> Color {
        let (r, g, b) = match self {
            Alert::Note => (88, 166, 255),
            Alert::Tip => (63, 185, 80),
            Alert::Important => (163, 113, 247),
            Alert::Caution => (248, 81, 73),
            Alert::Warning => (210, 153, 34),
        };
        Color::Rgb(r, g, b)
    }

    /// Read-view stand-in for the tag (the kind in capitals), padded to the tag's display width so
    /// that no character after it moves.
    pub fn header(self, tag_width: usize) -> String {
        let text = self.name().to_uppercase();
        let pad = tag_width.saturating_sub(text.width());
        format!("{text}{}", " ".repeat(pad))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_tags_case_insensitively() {
        assert_eq!(Alert::parse("[!NOTE]"), Some(Alert::Note));
        assert_eq!(Alert::parse("  [!Tip]  "), Some(Alert::Tip));
        assert_eq!(Alert::parse("[!bogus]"), None);
        assert_eq!(Alert::parse("[! note]"), None);
        assert_eq!(Alert::parse("note"), None);
    }

    #[test]
    fn octicons_parse_as_paths() {
        for a in Alert::ALL {
            let path = kurbo::BezPath::from_svg(a.octicon_path()).unwrap();
            let b = kurbo::Shape::bounding_box(&path);
            assert!(
                b.x0 >= -0.01 && b.y0 >= -0.01 && b.x1 <= 16.01 && b.y1 <= 16.01,
                "{a:?}: {b:?}"
            );
        }
    }

    #[test]
    fn header_never_exceeds_the_tag_width() {
        for a in Alert::ALL {
            let w = a.tag().width();
            assert_eq!(a.header(w).width(), w, "{a:?}");
        }
    }
}
