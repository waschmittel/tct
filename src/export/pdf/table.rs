use super::*;
use crate::export::blocks::{self, Run, Style};
use crate::insert::table::{Align, Table};

impl<'f> Flow<'f> {
    /// Column widths: the full text where it fits; otherwise every column
    /// keeps its longest word and the rest of the width is shared out in
    /// proportion to how much more each column would like. The flag says
    /// whether every column got at least its longest word.
    pub(super) fn table_widths(
        &self,
        cells: &[Vec<Vec<Run>>],
        cols: usize,
        size: f32,
        pad: f32,
    ) -> (Vec<f32>, bool) {
        let measure = |c: usize, f: &dyn Fn(&Self, &[Run]) -> f32| {
            cells.iter().map(|row| f(self, &row[c])).fold(0.0, f32::max) + 2.0 * pad
        };
        let natural: Vec<f32> = (0..cols)
            .map(|c| {
                measure(c, &|s, runs| {
                    s.wrap(runs, size, f32::MAX)
                        .iter()
                        .map(|l| l.last().map_or(0.0, |g| g.x + g.w))
                        .fold(0.0, f32::max)
                })
            })
            .collect();
        if natural.iter().sum::<f32>() <= CONTENT_W {
            return (natural, true);
        }
        let longest_word = |s: &Self, runs: &[Run]| {
            s.atoms(runs, size)
                .iter()
                .map(|a| if let Atom::Word(_, _, w) = a { *w } else { 0.0 })
                .fold(0.0, f32::max)
        };
        let minimum: Vec<f32> = (0..cols)
            .map(|c| measure(c, &longest_word).min(natural[c]))
            .collect();
        let min_total: f32 = minimum.iter().sum();
        if min_total >= CONTENT_W {
            return (
                minimum.iter().map(|w| w * CONTENT_W / min_total).collect(),
                false,
            );
        }
        let wanted: f32 = natural.iter().zip(&minimum).map(|(n, m)| n - m).sum();
        let spare = CONTENT_W - min_total;
        (
            minimum
                .iter()
                .zip(&natural)
                .map(|(m, n)| m + (n - m) * spare / wanted)
                .collect(),
            true,
        )
    }

    pub(super) fn rule_across(&mut self, y: f32, x: f32, width: f32, thickness: f32) {
        self.push(Op::Line {
            x1: x,
            y1: y,
            x2: x + width,
            y2: y,
            color: self.ink(),
            width: thickness,
        });
    }

    /// Booktabs-like: heavy rules above and below, a light rule under the
    /// centered bold header; tables with more than 7 rows or 3 columns also get
    /// thin vertical lines between columns and at both edges. No row separators.
    ///
    /// A *matrix* (empty top-left cell, `Table::is_matrix`) has labels on the
    /// left too, so it differs in these ways:
    /// - the first column is bold like the header;
    /// - the corner cell has no rule above it and no line to its left;
    /// - heavy lines (`HEAVY_RULE`) always separate the labels from the
    ///   contents: under the header row and right of the first column;
    /// - column lines follow the *content* columns: only with more than 3 of
    ///   them (so more than 4 columns in all);
    /// - without row separators it has no top or bottom border at all;
    /// - thin row separators appear with more than 7 rows (as for column lines in normal tables), whatever the column
    ///   count; then the top and bottom rules are as thin as those separators.
    pub(super) fn table(&mut self, table: &Table) {
        const PAD_Y: f32 = 2.5;
        let header_style = Style {
            bold: true,
            ..Style::default()
        };
        let matrix = table.is_matrix();
        let cells: Vec<Vec<Vec<Run>>> = table
            .rows
            .iter()
            .enumerate()
            .map(|(r, row)| {
                row.iter()
                    .enumerate()
                    .map(|(c, text)| {
                        let bold = r == 0 || (matrix && c == 0);
                        blocks::inline(
                            text,
                            if bold { header_style } else { Style::default() },
                            self.lang,
                        )
                    })
                    .collect()
            })
            .collect();
        let (mut size, mut pad_x) = (9.5, 5.0);
        let mut widths = Vec::new();
        for (candidate_size, candidate_pad) in [(9.5, 5.0), (9.5, 3.0), (8.5, 2.5), (7.5, 2.0)] {
            (size, pad_x) = (candidate_size, candidate_pad);
            let (candidate, whole_words) = self.table_widths(&cells, table.cols(), size, pad_x);
            widths = candidate;
            if whole_words {
                break;
            }
        }
        let total: f32 = widths.iter().sum();
        let left = MARGIN + (CONTENT_W - total) / 2.0;

        let height = size * LEADING;
        let rows: Vec<(Vec<Vec<Vec<Seg>>>, f32)> = cells
            .iter()
            .map(|row| {
                let wrapped: Vec<Vec<Vec<Seg>>> = row
                    .iter()
                    .zip(&widths)
                    .map(|(runs, w)| self.wrap(runs, size, w - 2.0 * pad_x))
                    .collect();
                let lines = wrapped.iter().map(Vec::len).max().unwrap_or(1).max(1);
                (wrapped, lines as f32 * height + 2.0 * PAD_Y)
            })
            .collect();
        let (col_lines, row_lines) = if matrix {
            (table.cols() - 1 > 3, table.rows.len() > 7)
        } else {
            (table.rows.len() > 7 || table.cols() > 3, false)
        };
        let layout = TableLayout {
            widths: &widths,
            aligns: &table.aligns,
            left,
            size,
            pad: (pad_x, PAD_Y),
            col_lines,
            row_lines,
            matrix,
        };

        let first_rows = rows.iter().take(2).map(|(_, h)| h).sum::<f32>();
        let first_refs: Vec<usize> = rows.iter().take(2).flat_map(|r| self.row_refs(r)).collect();
        self.need_notes(first_rows + 2.0, &first_refs);
        self.y += 2.0;
        self.top_rule(&layout, total);
        self.table_row(&rows[0], true, &layout);
        self.header_rule(&layout, total);
        for (i, row) in rows[1..].iter().enumerate() {
            if layout.row_lines && i > 0 {
                self.rule_across(self.y, left, total, MIDRULE);
            }
            let nums = self.row_refs(row);
            if !self.fits(row.1, &nums) && self.y > MARGIN {
                if !layout.open_ends() {
                    self.rule_across(self.y, left, total, MIDRULE);
                }
                self.new_page();
                self.y = MARGIN + 2.0;
                let header_refs = self.row_refs(&rows[0]);
                self.register(rows[0].1, &header_refs);
                self.top_rule(&layout, total);
                self.table_row(&rows[0], true, &layout);
                self.header_rule(&layout, total);
            }
            self.register(row.1, &nums);
            self.table_row(row, false, &layout);
        }
        if !layout.open_ends() {
            self.rule_across(self.y, left, total, layout.outer_rule());
        }
        self.y += 6.0;
    }

    /// Rule above the header; a matrix leaves the empty corner cell open.
    pub(super) fn top_rule(&mut self, t: &TableLayout, total: f32) {
        if t.open_ends() {
            return;
        }
        let skip = if t.matrix { t.widths[0] } else { 0.0 };
        self.rule_across(self.y, t.left + skip, total - skip, t.outer_rule());
    }

    /// Rule under the header: light, but heavy in a matrix where it separates
    /// the column labels from the contents.
    pub(super) fn header_rule(&mut self, t: &TableLayout, total: f32) {
        self.rule_across(
            self.y,
            t.left,
            total,
            if t.matrix { HEAVY_RULE } else { MIDRULE },
        );
    }

    pub(super) fn table_row(
        &mut self,
        (wrapped, row_h): &(Vec<Vec<Vec<Seg>>>, f32),
        header: bool,
        t: &TableLayout,
    ) {
        let top = self.y;
        let mut x = t.left;
        for (c, segs) in wrapped.iter().enumerate() {
            for (l, line) in segs.iter().enumerate() {
                let used = line.last().map_or(0.0, |s| s.x + s.w);
                let room = t.widths[c] - 2.0 * t.pad.0 - used;
                let align = if header { Align::Center } else { t.aligns[c] };
                let shift = match align {
                    Align::Right => room,
                    Align::Center => room / 2.0,
                    _ => 0.0,
                };
                let ly = top + t.pad.1 + l as f32 * t.size * LEADING;
                self.draw_line(line, x + t.pad.0, ly, t.size, self.ink(), shift.max(0.0));
            }
            x += t.widths[c];
        }
        let mut vx = t.left;
        for (i, w) in std::iter::once(&0.0).chain(t.widths).enumerate() {
            vx += w;
            let divider = t.matrix && i == 1; // labels | contents
            let outer = i == 0 || i == t.widths.len();
            if !(t.col_lines || divider) || (t.matrix && outer) {
                continue;
            }
            let width = if divider { HEAVY_RULE } else { MIDRULE };
            self.push(Op::Line {
                x1: vx,
                y1: top,
                x2: vx,
                y2: top + row_h,
                color: self.ink(),
                width,
            });
        }
        self.y += row_h;
    }
}
