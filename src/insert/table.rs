//! Pipe-table model for the description editor: parse the lines of a table,
//! edit it (rows, columns, alignment) and render it back, padded to its
//! widest cells with the alignment hints of the delimiter row respected.
//!
//! Pure functions over strings — the glue that applies this to the textarea
//! lives in `table_edit.rs`. Only pipe tables with a leading `|` are
//! recognised; pipes escaped as `\|` do not split cells.

use unicode_width::UnicodeWidthStr;

/// Column alignment, as hinted by the delimiter row (`---`, `:--`, `:-:`, `--:`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Align {
    None,
    Left,
    Center,
    Right,
}

impl Align {
    fn next(self) -> Self {
        match self {
            Align::None => Align::Left,
            Align::Left => Align::Center,
            Align::Center => Align::Right,
            Align::Right => Align::None,
        }
    }

    fn from_hint(text: &str) -> Self {
        match (text.starts_with(':'), text.ends_with(':')) {
            (true, true) if text.len() > 1 => Align::Center,
            (true, _) => Align::Left,
            (false, true) => Align::Right,
            _ => Align::None,
        }
    }
}

/// Narrowest column: fits the `:-:` hint.
const MIN_WIDTH: usize = 3;

/// A cell of a raw line: chars `start..end` sit between two pipes (`end` is the
/// closing pipe, or the line length); `text_start..text_end` is the trimmed text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RawCell {
    pub start: usize,
    pub end: usize,
    pub text_start: usize,
    pub text_end: usize,
}

/// A line is part of a pipe table when its first non-blank character is `|`.
pub fn is_table_line(line: &str) -> bool {
    line.trim_start().starts_with('|')
}

/// Split a line into cells on unescaped pipes.
pub fn split_cells(line: &str) -> Vec<RawCell> {
    let chars: Vec<char> = line.chars().collect();
    let mut i = chars.iter().take_while(|c| c.is_whitespace()).count();
    if chars.get(i) == Some(&'|') {
        i += 1;
    }
    let cell = |start: usize, end: usize| {
        let inner = &chars[start..end];
        let lead = inner.iter().take_while(|c| c.is_whitespace()).count();
        if lead == inner.len() {
            return RawCell { start, end, text_start: start, text_end: start };
        }
        let trail = inner.iter().rev().take_while(|c| c.is_whitespace()).count();
        RawCell { start, end, text_start: start + lead, text_end: end - trail }
    };
    let mut cells = Vec::new();
    let mut start = i;
    let mut j = i;
    while j < chars.len() {
        match chars[j] {
            '\\' => j += 1,
            '|' => {
                cells.push(cell(start, j));
                start = j + 1;
            }
            _ => {}
        }
        j += 1;
    }
    if chars[start.min(chars.len())..].iter().any(|c| !c.is_whitespace()) {
        cells.push(cell(start, chars.len()));
    }
    cells
}

fn cell_text(line: &str, cell: &RawCell) -> String {
    line.chars().skip(cell.text_start).take(cell.text_end - cell.text_start).collect()
}

/// A delimiter row consists of `-` and `:` cells with at least one `-`. An
/// emptied cell is tolerated so the table survives while its hint is retyped.
pub fn delimiter_aligns(line: &str) -> Option<Vec<Align>> {
    if !is_table_line(line) {
        return None;
    }
    let hints: Vec<String> = split_cells(line).iter().map(|c| cell_text(line, c)).collect();
    let valid = !hints.is_empty()
        && hints.iter().all(|h| h.chars().all(|c| c == '-' || c == ':'))
        && hints.iter().any(|h| h.contains('-'));
    valid.then(|| hints.iter().map(|h| Align::from_hint(h)).collect())
}

/// Where the cursor sits in a raw table line: the cell and the char offset into
/// that cell's text. An offset beyond the text length counts spaces typed after it.
pub fn locate(line: &str, col: usize) -> (usize, usize) {
    let cells = split_cells(line);
    let Some(first) = cells.first() else {
        return (0, 0);
    };
    if col < first.start {
        return (0, 0);
    }
    for (i, c) in cells.iter().enumerate() {
        if col <= c.end {
            return (i, col.saturating_sub(c.text_start));
        }
    }
    let last = &cells[cells.len() - 1];
    (cells.len() - 1, last.text_end - last.text_start)
}

/// Model row of a line within the table (the delimiter line counts as the header).
pub fn row_of_line(line: usize) -> usize {
    line.saturating_sub(1)
}

/// Line within the table of a model row.
pub fn line_of_row(row: usize) -> usize {
    if row == 0 { 0 } else { row + 1 }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Table {
    pub indent: String,
    pub aligns: Vec<Align>,
    /// Header first, then the body rows; every row has `aligns.len()` cells.
    pub rows: Vec<Vec<String>>,
}

/// Result of a structural edit.
#[derive(Debug, PartialEq, Eq)]
pub enum Edit {
    /// Done; the cursor goes to this row (or column, for column edits).
    Place(usize),
    /// The edit removes the table altogether.
    Gone,
    /// Not possible, with the reason.
    Refused(&'static str),
}

/// A table rendered back to lines.
pub struct Rendered {
    pub lines: Vec<String>,
    /// Per line, the char index where each cell's text starts.
    pub text_starts: Vec<Vec<usize>>,
}

impl Table {
    /// `lines` are the table's lines: header, delimiter, body rows.
    pub fn parse(lines: &[String]) -> Option<Table> {
        let mut aligns = delimiter_aligns(lines.get(1)?)?;
        let indent: String = lines[0].chars().take_while(|c| c.is_whitespace()).collect();
        let mut rows: Vec<Vec<String>> = lines
            .iter()
            .enumerate()
            .filter(|(i, _)| *i != 1)
            .map(|(_, l)| split_cells(l).iter().map(|c| cell_text(l, c)).collect())
            .collect();
        let cols = rows.iter().map(Vec::len).chain([aligns.len()]).max().unwrap_or(1).max(1);
        aligns.resize(cols, Align::None);
        for r in &mut rows {
            r.resize(cols, String::new());
        }
        Some(Table { indent, aligns, rows })
    }

    /// A table of empty cells, ready to type into.
    pub fn blank(cols: usize, body_rows: usize) -> Table {
        Table {
            indent: String::new(),
            aligns: vec![Align::None; cols],
            rows: vec![vec![String::new(); cols]; body_rows + 1],
        }
    }

    /// A table with the given header cells and one empty body row.
    pub fn from_header(indent: &str, mut header: Vec<String>) -> Table {
        let cols = header.len().max(1);
        header.resize(cols, String::new());
        Table {
            indent: indent.to_string(),
            aligns: vec![Align::None; cols],
            rows: vec![header, vec![String::new(); cols]],
        }
    }

    pub fn cols(&self) -> usize {
        self.aligns.len()
    }

    pub fn render(&self) -> Rendered {
        let widths: Vec<usize> = (0..self.cols())
            .map(|c| {
                self.rows
                    .iter()
                    .map(|r| UnicodeWidthStr::width(r[c].as_str()))
                    .max()
                    .unwrap_or(0)
                    .max(MIN_WIDTH)
            })
            .collect();
        let mut lines = Vec::new();
        let mut text_starts = Vec::new();
        for (r, row) in self.rows.iter().enumerate() {
            let mut line = format!("{}|", self.indent);
            let mut starts = Vec::new();
            for (c, text) in row.iter().enumerate() {
                let gap = widths[c] - UnicodeWidthStr::width(text.as_str());
                let (left, right) = match self.aligns[c] {
                    Align::Right => (gap, 0),
                    Align::Center => (gap / 2, gap - gap / 2),
                    _ => (0, gap),
                };
                line.push(' ');
                line.push_str(&" ".repeat(left));
                starts.push(line.chars().count());
                line.push_str(text);
                line.push_str(&" ".repeat(right));
                line.push_str(" |");
            }
            lines.push(line);
            text_starts.push(starts);
            if r == 0 {
                let mut line = format!("{}|", self.indent);
                let mut starts = Vec::new();
                for (align, width) in self.aligns.iter().zip(&widths) {
                    line.push(' ');
                    starts.push(line.chars().count());
                    line.push_str(&delimiter_cell(*align, *width));
                    line.push_str(" |");
                }
                lines.push(line);
                text_starts.push(starts);
            }
        }
        Rendered { lines, text_starts }
    }

    /// Insert an empty body row next to `row`. Nothing goes above the header.
    pub fn insert_row(&mut self, row: usize, after: bool) -> Edit {
        if row == 0 && !after {
            return Edit::Refused("Nothing can go above the header row");
        }
        let at = if after { row + 1 } else { row };
        self.rows.insert(at, vec![String::new(); self.cols()]);
        Edit::Place(at)
    }

    pub fn insert_col(&mut self, col: usize, after: bool) -> Edit {
        let at = if after { col + 1 } else { col };
        self.aligns.insert(at, Align::None);
        for r in &mut self.rows {
            r.insert(at, String::new());
        }
        Edit::Place(at)
    }

    /// Deleting the header row deletes the table.
    pub fn delete_row(&mut self, row: usize) -> Edit {
        if row == 0 {
            return Edit::Gone;
        }
        self.rows.remove(row);
        Edit::Place(row.min(self.rows.len() - 1))
    }

    /// Deleting the last column deletes the table.
    pub fn delete_col(&mut self, col: usize) -> Edit {
        if self.cols() == 1 {
            return Edit::Gone;
        }
        self.aligns.remove(col);
        for r in &mut self.rows {
            r.remove(col);
        }
        Edit::Place(col.min(self.cols() - 1))
    }

    /// Swap a body row with its neighbour; the header stays on top.
    pub fn swap_row(&mut self, row: usize, down: bool) -> Edit {
        let other = if down { row + 1 } else { row.wrapping_sub(1) };
        if row == 0 || other == 0 {
            return Edit::Refused("The header row stays on top");
        }
        if other >= self.rows.len() {
            return Edit::Refused("Already the last row");
        }
        self.rows.swap(row, other);
        Edit::Place(other)
    }

    pub fn swap_col(&mut self, col: usize, right: bool) -> Edit {
        let other = if right { col + 1 } else { col.wrapping_sub(1) };
        if other >= self.cols() {
            return Edit::Refused(if right { "Already the last column" } else { "Already the first column" });
        }
        self.aligns.swap(col, other);
        for r in &mut self.rows {
            r.swap(col, other);
        }
        Edit::Place(other)
    }

    /// Cycle a column through no hint → left → center → right.
    pub fn cycle_align(&mut self, col: usize) {
        self.aligns[col] = self.aligns[col].next();
    }
}

fn delimiter_cell(align: Align, width: usize) -> String {
    match align {
        Align::None => "-".repeat(width),
        Align::Left => format!(":{}", "-".repeat(width - 1)),
        Align::Right => format!("{}:", "-".repeat(width - 1)),
        Align::Center => format!(":{}:", "-".repeat(width - 2)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(s: &[&str]) -> Vec<String> {
        s.iter().map(|l| l.to_string()).collect()
    }

    fn render(s: &[&str]) -> Vec<String> {
        Table::parse(&lines(s)).unwrap().render().lines
    }

    #[test]
    fn splits_on_unescaped_pipes_only() {
        let line = r"| a \| b | c |";
        let cells = split_cells(line);
        assert_eq!(cells.len(), 2);
        assert_eq!(cell_text(line, &cells[0]), r"a \| b");
    }

    #[test]
    fn rows_without_a_trailing_pipe_still_split() {
        assert_eq!(split_cells("| a | b").len(), 2);
        assert_eq!(split_cells("| a | b |").len(), 2);
    }

    #[test]
    fn pads_columns_to_the_widest_cell() {
        assert_eq!(
            render(&["| a | bb |", "|-|-|", "| ccccc | d |"]),
            ["| a     | bb  |", "| ----- | --- |", "| ccccc | d   |"]
        );
    }

    #[test]
    fn alignment_hints_are_kept_and_applied() {
        assert_eq!(
            render(&["| l | c | r | n |", "| :- | :-: | -: | - |", "| 1 | 2 | 3 | 4 |"]),
            ["| l   |  c  |   r | n   |", "| :-- | :-: | --: | --- |", "| 1   |  2  |   3 | 4   |"]
        );
    }

    #[test]
    fn center_puts_the_odd_space_on_the_right() {
        let r = render(&["| abcd |", "| :-: |", "| x |"]);
        assert_eq!(r[2], "|  x   |");
    }

    #[test]
    fn display_width_is_used_for_wide_characters() {
        let r = render(&["| 日本 | b |", "| - | - |", "| abcd | c |"]);
        assert_eq!(r[0], "| 日本 | b   |");
        assert_eq!(r[2], "| abcd | c   |");
    }

    #[test]
    fn ragged_rows_are_padded_and_extra_cells_widen_the_table() {
        let r = render(&["| a |", "| - |", "| 1 | 2 |"]);
        assert_eq!(r, ["| a   |     |", "| --- | --- |", "| 1   | 2   |"]);
    }

    #[test]
    fn a_table_needs_a_delimiter_row() {
        assert!(Table::parse(&lines(&["| a |", "| b |"])).is_none());
        assert!(delimiter_aligns("| --- | :-: |").is_some());
        assert!(delimiter_aligns("| a | - |").is_none());
        assert!(delimiter_aligns("|   |   |").is_none());
        assert!(delimiter_aligns("| --- |   |").is_some());
    }

    #[test]
    fn locate_maps_columns_to_cells() {
        let l = "| ab | cd |";
        assert_eq!(locate(l, 0), (0, 0));
        assert_eq!(locate(l, 2), (0, 0));
        assert_eq!(locate(l, 4), (0, 2));
        assert_eq!(locate(l, 5), (0, 3));
        assert_eq!(locate(l, 7), (1, 0));
        assert_eq!(locate(l, 12), (1, 2));
    }

    #[test]
    fn insert_and_delete_rows() {
        let mut t = Table::parse(&lines(&["| a |", "| - |", "| 1 |"])).unwrap();
        assert_eq!(t.insert_row(0, true), Edit::Place(1));
        assert_eq!(t.rows.len(), 3);
        assert!(matches!(t.insert_row(0, false), Edit::Refused(_)));
        assert_eq!(t.delete_row(2), Edit::Place(1));
        assert_eq!(t.delete_row(0), Edit::Gone);
    }

    #[test]
    fn deleting_the_last_column_deletes_the_table() {
        let mut t = Table::parse(&lines(&["| a | b |", "| - | - |"])).unwrap();
        assert_eq!(t.delete_col(1), Edit::Place(0));
        assert_eq!(t.delete_col(0), Edit::Gone);
    }

    #[test]
    fn swapping_moves_alignment_with_the_column_and_keeps_the_header_on_top() {
        let mut t =
            Table::parse(&lines(&["| a | b |", "| :- | -: |", "| 1 | 2 |", "| 3 | 4 |"])).unwrap();
        assert_eq!(t.swap_col(0, true), Edit::Place(1));
        assert_eq!(t.aligns, [Align::Right, Align::Left]);
        assert_eq!(t.rows[1], ["2", "1"]);
        assert!(matches!(t.swap_row(1, false), Edit::Refused(_)));
        assert_eq!(t.swap_row(1, true), Edit::Place(2));
        assert_eq!(t.rows[1], ["4", "3"]);
    }

    #[test]
    fn cycling_alignment_goes_around() {
        let mut t = Table::blank(1, 0);
        let seen: Vec<Align> = (0..4)
            .map(|_| {
                t.cycle_align(0);
                t.aligns[0]
            })
            .collect();
        assert_eq!(seen, [Align::Left, Align::Center, Align::Right, Align::None]);
    }

    #[test]
    fn indent_is_preserved() {
        assert_eq!(render(&["  | a |", "  | - |"]), ["  | a   |", "  | --- |"]);
    }

    #[test]
    fn text_starts_point_at_the_cell_text() {
        let r = Table::parse(&lines(&["| a | bb |", "| - | -: |"])).unwrap().render();
        assert_eq!(r.text_starts[0], [2, 9]);
        assert_eq!(r.lines[0].chars().nth(2), Some('a'));
        assert_eq!(r.lines[0].chars().nth(9), Some('b'));
    }
}
