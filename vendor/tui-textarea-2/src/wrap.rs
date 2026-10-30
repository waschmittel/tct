#[cfg(feature = "arbitrary")]
use arbitrary::Arbitrary;
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthChar;

/// Specify how logical lines are soft-wrapped at render time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "arbitrary", derive(Arbitrary))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum WrapMode {
    /// Disable soft wrapping and keep horizontal scrolling behavior.
    None,
    /// Prefer word boundaries and split oversized words at grapheme boundaries.
    Word,
    /// Wrap at grapheme boundaries.
    Glyph,
    /// Behave like [`Word`](Self::Word).
    ///
    /// This compatibility variant retains the original name for existing source code and serialized values. Both word modes guarantee that oversized words are split at grapheme boundaries instead of being clipped.
    WordOrGlyph,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct WrappedLine {
    pub row: usize,
    pub start_byte: usize,
    pub end_byte: usize,
    pub start_col: usize,
    pub end_col: usize,
    pub first_in_row: bool,
    pub last_in_row: bool,
    /// TCT patch: display columns of hanging indent for continuation rows (0 on the first row).
    pub indent: usize,
}

/// TCT patch: computes the hanging indent (display columns) of a logical line's continuation rows.
pub type ContinuationIndent = fn(&str) -> usize;

#[derive(Clone, Copy)]
struct WordWrapUnit {
    start: usize,
    end: usize,
    kind: WordWrapUnitKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WordWrapUnitKind {
    Content,
    Whitespace,
}

pub(crate) fn wrapped_rows(
    lines: &[String],
    mode: WrapMode,
    width: usize,
    tab_len: u8,
    continuation_indent: Option<ContinuationIndent>,
) -> Vec<WrappedLine> {
    let mut rows = Vec::new();

    for (row, line) in lines.iter().enumerate() {
        let ranges = indented_line_ranges(line, mode, width, tab_len, continuation_indent);
        let mut start_col = 0usize;
        for (i, (start_byte, end_byte, indent)) in ranges.iter().copied().enumerate() {
            let end_col = start_col + line[start_byte..end_byte].chars().count();
            rows.push(WrappedLine {
                row,
                start_byte,
                end_byte,
                start_col,
                end_col,
                first_in_row: i == 0,
                last_in_row: i + 1 == ranges.len(),
                indent,
            });
            start_col = end_col;
        }
    }

    rows
}

/// TCT patch: like [`line_ranges`], but continuation rows are wrapped to `width - indent`
/// and carry that indent as the third tuple element.
fn indented_line_ranges(
    line: &str,
    mode: WrapMode,
    width: usize,
    tab_len: u8,
    continuation_indent: Option<ContinuationIndent>,
) -> Vec<(usize, usize, usize)> {
    let first = line_ranges(line, mode, width, tab_len);
    let plain = |ranges: Vec<(usize, usize)>| ranges.into_iter().map(|(s, e)| (s, e, 0)).collect();

    let width = width.max(1);
    let indent = continuation_indent.map_or(0, |f| f(line));
    // Keep at least two cells for text on continuation rows; otherwise don't indent.
    if first.len() < 2 || mode == WrapMode::None || width.saturating_sub(indent) < 2 {
        return plain(first);
    }

    let head_end = first[0].1;
    let mut out = vec![(first[0].0, head_end, 0)];
    for (s, e) in line_ranges(&line[head_end..], mode, width - indent, tab_len) {
        out.push((head_end + s, head_end + e, indent));
    }
    out
}

pub(crate) fn line_ranges(
    line: &str,
    mode: WrapMode,
    width: usize,
    tab_len: u8,
) -> Vec<(usize, usize)> {
    if mode == WrapMode::None {
        return vec![(0, line.len())];
    }

    let width = width.max(1);
    let mut out = match mode {
        WrapMode::None => vec![(0, line.len())],
        WrapMode::Glyph => {
            let mut chunks = Vec::new();
            split_range_by_grapheme_width(line, 0, line.len(), width, tab_len, &mut chunks);
            chunks
        }
        WrapMode::Word | WrapMode::WordOrGlyph => wrap_words(line, width, tab_len),
    };

    if out.is_empty() {
        out.push((0, 0));
    }
    out
}

fn wrap_words(line: &str, width: usize, tab_len: u8) -> Vec<(usize, usize)> {
    let units = word_wrap_units(line);

    if units.is_empty() {
        return vec![(0, 0)];
    }

    let mut out = Vec::new();
    let mut i = 0usize;
    let mut seg_start = units[0].start;
    let mut seg_end = seg_start;
    let mut seg_width = 0usize;

    while i < units.len() {
        let unit = units[i];
        if seg_end == seg_start {
            seg_start = unit.start;
        }

        let unit_width = display_width_from(word_wrap_unit_text(line, unit), seg_width, tab_len);
        // The final separator before a word may occupy the reserved caret cell because the cursor after it belongs to the following visual row. Trailing whitespace remains grapheme-sized so it fills the current row before wrapping.
        let uses_reserved_cell = unit.kind == WordWrapUnitKind::Whitespace
            && units
                .get(i + 1)
                .is_some_and(|next| next.kind == WordWrapUnitKind::Content)
            && seg_end > seg_start
            && seg_width + unit_width == width.saturating_add(1);
        if seg_width + unit_width <= width || uses_reserved_cell {
            seg_end = unit.end;
            seg_width += unit_width;
            i += 1;
            continue;
        }

        if seg_end > seg_start {
            out.push((seg_start, seg_end));
            seg_start = seg_end;
            seg_width = 0;
            continue;
        }

        split_range_by_grapheme_width(line, unit.start, unit.end, width, tab_len, &mut out);

        i += 1;
        seg_start = unit.end;
        seg_end = unit.end;
        seg_width = 0;
    }

    if seg_end > seg_start {
        out.push((seg_start, seg_end));
    }

    out
}

fn word_wrap_units(line: &str) -> Vec<WordWrapUnit> {
    let mut units = Vec::new();
    for (start, text) in UnicodeSegmentation::split_word_bound_indices(line) {
        if text.chars().all(char::is_whitespace) {
            units.extend(UnicodeSegmentation::grapheme_indices(text, true).map(
                |(grapheme_start, grapheme)| WordWrapUnit {
                    start: start + grapheme_start,
                    end: start + grapheme_start + grapheme.len(),
                    kind: WordWrapUnitKind::Whitespace,
                },
            ));
        } else {
            units.push(WordWrapUnit {
                start,
                end: start + text.len(),
                kind: WordWrapUnitKind::Content,
            });
        }
    }
    units
}

fn split_range_by_grapheme_width(
    line: &str,
    start: usize,
    end: usize,
    width: usize,
    tab_len: u8,
    out: &mut Vec<(usize, usize)>,
) {
    let mut segment_start = start;
    while segment_start < end {
        let mut segment_end = segment_start;
        let mut segment_width = 0usize;

        for (offset, grapheme) in
            UnicodeSegmentation::grapheme_indices(&line[segment_start..end], true)
        {
            let grapheme_start = segment_start + offset;
            let grapheme_end = grapheme_start + grapheme.len();
            let next_width = display_width_to(grapheme, segment_width, tab_len);
            let grapheme_width = next_width.saturating_sub(segment_width);

            if segment_end != segment_start && segment_width + grapheme_width > width {
                break;
            }

            segment_end = grapheme_end;
            segment_width = next_width;
            if segment_width > width {
                break;
            }
        }

        if segment_end == segment_start {
            if let Some(ch) = line[segment_start..end].chars().next() {
                segment_end = segment_start + ch.len_utf8();
            } else {
                break;
            }
        }

        out.push((segment_start, segment_end));
        segment_start = segment_end;
    }
}

#[inline]
fn word_wrap_unit_text(line: &str, unit: WordWrapUnit) -> &str {
    &line[unit.start..unit.end]
}

fn display_width_from(text: &str, start_width: usize, tab_len: u8) -> usize {
    display_width_to(text, start_width, tab_len).saturating_sub(start_width)
}

fn display_width_to(text: &str, mut width: usize, tab_len: u8) -> usize {
    for c in text.chars() {
        if c == '\t' {
            if tab_len > 0 {
                let tab = tab_len as usize;
                let pad = tab - (width % tab);
                width += pad;
            }
        } else {
            width += c.width().unwrap_or(0);
        }
    }
    width
}

#[cfg(test)]
mod tests {
    use super::*;

    fn segments(line: &str, mode: WrapMode, width: usize) -> Vec<&str> {
        line_ranges(line, mode, width, 4)
            .into_iter()
            .map(|(s, e)| &line[s..e])
            .collect()
    }

    #[test]
    fn word_wrap_splits_long_word() {
        let have = segments("helloworld", WrapMode::Word, 4);
        assert_eq!(have, vec!["hell", "owor", "ld"]);
    }

    #[test]
    fn word_or_glyph_wrap_splits_long_word() {
        let have = segments("helloworld", WrapMode::WordOrGlyph, 4);
        assert_eq!(have, vec!["hell", "owor", "ld"]);
    }

    #[test]
    fn word_modes_have_compatible_wrapping() {
        let word = segments("hello supercalifragilistic world", WrapMode::Word, 8);
        let compatibility = segments("hello supercalifragilistic world", WrapMode::WordOrGlyph, 8);
        assert_eq!(word, compatibility);
    }

    #[test]
    fn word_modes_consume_whitespace_one_grapheme_at_a_time() {
        for mode in [WrapMode::Word, WrapMode::WordOrGlyph] {
            assert_eq!(
                segments("abcd    ", mode, 6),
                vec!["abcd  ", "  "],
                "{mode:?} trailing whitespace"
            );
            assert_eq!(
                segments("abcd    x", mode, 6),
                vec!["abcd  ", "  x"],
                "{mode:?} separator whitespace"
            );
        }
    }

    #[test]
    fn glyph_wrap_handles_wide_chars() {
        let have = segments("ab犬猫", WrapMode::Glyph, 4);
        assert_eq!(have, vec!["ab犬", "猫"]);
    }

    #[test]
    fn glyph_wrap_keeps_combining_grapheme_cluster() {
        let have = segments("e\u{301}x", WrapMode::Glyph, 1);
        assert_eq!(have, vec!["e\u{301}", "x"]);
    }

    #[test]
    fn tab_width_is_accounted_for_in_wrap() {
        let have = segments("\tX", WrapMode::WordOrGlyph, 2);
        assert_eq!(have, vec!["\t", "X"]);
    }

    fn list_indent(line: &str) -> usize {
        line.len() - line.trim_start_matches(' ').len() + 2
    }

    fn indented(line: &str, width: usize) -> Vec<(&str, usize)> {
        indented_line_ranges(line, WrapMode::WordOrGlyph, width, 4, Some(list_indent))
            .into_iter()
            .map(|(s, e, i)| (&line[s..e], i))
            .collect()
    }

    #[test]
    fn continuation_rows_wrap_to_reduced_width_and_carry_indent() {
        let have = indented("- aaa bbb ccc ddd", 8);
        assert_eq!(have, vec![("- aaa ", 0), ("bbb ", 2), ("ccc ", 2), ("ddd", 2)]);
    }

    #[test]
    fn continuation_indent_is_skipped_when_it_leaves_no_room() {
        let have = indented("- aaa bbb", 3);
        assert!(have.iter().all(|(_, i)| *i == 0));
    }

    #[test]
    fn unwrapped_line_has_no_indent() {
        assert_eq!(indented("- short", 20), vec![("- short", 0)]);
    }

    #[test]
    fn no_indent_fn_matches_plain_wrapping() {
        let plain = segments("- aaa bbb ccc ddd", WrapMode::WordOrGlyph, 8);
        let none: Vec<&str> =
            indented_line_ranges("- aaa bbb ccc ddd", WrapMode::WordOrGlyph, 8, 4, None)
                .into_iter()
                .map(|(s, e, _)| &"- aaa bbb ccc ddd"[s..e])
                .collect();
        assert_eq!(plain, none);
    }
}
