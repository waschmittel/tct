//! Card → PDF/A-2b: lay the card out into pages of draw operations, then
//! emit them with krilla, which validates the result against the standard.
//!
//! PDF/A requires every font to be embedded, so text is set in the bundled
//! DejaVu faces; characters they lack are replaced with `?` instead of
//! drawing `.notdef`.

use anyhow::{Context, anyhow};
use chrono::{Datelike, Local, Timelike};
use krilla::{Document, SerializeSettings};
use krilla::configure::{Archival, ConfigurationBuilder};
use krilla::geom::{PathBuilder, Point, Rect};
use krilla::metadata::Metadata;
use krilla::page::PageSettings;
use krilla::paint::{Fill, Stroke};
use krilla::text::{Font, TextDirection};
use skrifa::charmap::Charmap;
use skrifa::instance::{LocationRef, Size};
use skrifa::metrics::GlyphMetrics;
use skrifa::{FontRef, MetadataProvider};

use super::Theme;
use super::blocks::{self, Block, Run, Style};
use crate::insert::table::{Align, Table};
use crate::model::board::BoardMeta;
use crate::model::card::Card;

const PAGE_W: f32 = 595.28;
const PAGE_H: f32 = 841.89;
const MARGIN: f32 = 54.0;
const CONTENT_W: f32 = PAGE_W - 2.0 * MARGIN;
const BOTTOM: f32 = PAGE_H - MARGIN - 6.0;
const FOOTER_BASELINE: f32 = PAGE_H - MARGIN + 12.0;
const HEADER_BASELINE: f32 = MARGIN - 14.0;

const KEEP_MAX: f32 = (BOTTOM - MARGIN) / 2.0;
const BODY: f32 = 10.0;
const CODE: f32 = 9.0;
const LEADING: f32 = 1.4;

type Rgb = (u8, u8, u8);
const INK: Rgb = (33, 37, 41);
const MUTED: Rgb = (110, 117, 125);
const RULE: Rgb = (206, 212, 218);
const TINT: Rgb = (241, 243, 245);
const BLACK: Rgb = (0, 0, 0);
const HEAVY_RULE: f32 = 1.0;
const MIDRULE: f32 = 0.5;

static SANS: &[u8] = include_bytes!("../../assets/fonts/DejaVuSans.ttf");
static BOLD: &[u8] = include_bytes!("../../assets/fonts/DejaVuSans-Bold.ttf");
static BOLD_OBLIQUE: &[u8] = include_bytes!("../../assets/fonts/DejaVuSans-BoldOblique.ttf");
static OBLIQUE: &[u8] = include_bytes!("../../assets/fonts/DejaVuSans-Oblique.ttf");
static MONO: &[u8] = include_bytes!("../../assets/fonts/DejaVuSansMono.ttf");

struct Face {
    pdf: Font,
    charmap: Charmap<'static>,
    metrics: GlyphMetrics<'static>,
    units_per_em: f32,
}

impl Face {
    fn new(data: &'static [u8]) -> anyhow::Result<Self> {
        let font = FontRef::new(data).context("bundled font is unreadable")?;
        Ok(Self {
            pdf: Font::new(data.into(), 0).ok_or_else(|| anyhow!("bundled font is unusable"))?,
            charmap: font.charmap(),
            metrics: font.glyph_metrics(Size::unscaled(), LocationRef::default()),
            units_per_em: f32::from(font.metrics(Size::unscaled(), LocationRef::default()).units_per_em),
        })
    }

    fn advance(&self, c: char, size: f32) -> f32 {
        self.charmap
            .map(c)
            .and_then(|g| self.metrics.advance_width(g))
            .map_or(0.0, |w| w / self.units_per_em * size)
    }

    fn width(&self, text: &str, size: f32) -> f32 {
        text.chars().map(|c| self.advance(c, size)).sum()
    }

    fn clean(&self, text: &str) -> String {
        text.chars()
            .flat_map(|c| match c {
                '\t' => "    ".chars().collect::<Vec<_>>(),
                '\u{200B}' | '\u{200D}' | '\u{FE0E}' | '\u{FE0F}' => Vec::new(),
                c if c.is_control() => Vec::new(),
                c if self.charmap.map(c).is_some() => vec![c],
                _ => vec!['?'],
            })
            .collect()
    }
}

struct Fonts {
    theme: Theme,
    sans: Face,
    bold: Face,
    oblique: Face,
    bold_oblique: Face,
    mono: Face,
}

impl Fonts {
    #[cfg(test)]
    fn load() -> anyhow::Result<Self> {
        Self::load_for(Theme::Color)
    }

    fn load_for(theme: Theme) -> anyhow::Result<Self> {
        Ok(Self {
            theme,
            sans: Face::new(SANS)?,
            bold: Face::new(BOLD)?,
            oblique: Face::new(OBLIQUE)?,
            bold_oblique: Face::new(BOLD_OBLIQUE)?,
            mono: Face::new(MONO)?,
        })
    }

    fn mono_theme(&self) -> bool {
        self.theme == Theme::Mono
    }

    fn face(&self, style: Style) -> &Face {
        if style.code {
            &self.mono
        } else if style.bold && style.italic {
            &self.bold_oblique
        } else if style.bold {
            &self.bold
        } else if style.italic {
            &self.oblique
        } else {
            &self.sans
        }
    }
}

enum Op {
    Text { x: f32, y: f32, text: String, style: Style, size: f32, color: Rgb },
    Rect { x: f32, y: f32, w: f32, h: f32, fill: Option<Rgb>, stroke: Option<Rgb> },
    Line { x1: f32, y1: f32, x2: f32, y2: f32, color: Rgb, width: f32 },
}

struct Seg {
    text: String,
    style: Style,
    x: f32,
    w: f32,
}

enum Atom {
    Word(String, Style, f32),
    Space(Style, f32),
}

struct TableLayout<'a> {
    widths: &'a [f32],
    aligns: &'a [Align],
    left: f32,
    size: f32,
    pad: (f32, f32),
    /// Thin vertical lines between columns and at both edges.
    col_lines: bool,
    /// Thin rules between body rows (matrices only).
    row_lines: bool,
    /// Matrix (empty top-left cell): the corner cell is open above and to the
    /// left, and a heavier line separates the labels from the contents.
    matrix: bool,
}

impl TableLayout<'_> {
    /// Top and bottom rule: heavy, unless row separators make them one of many.
    fn outer_rule(&self) -> f32 {
        if self.row_lines { MIDRULE } else { HEAVY_RULE }
    }

    /// A matrix without row separators has no top and bottom border at all.
    fn open_ends(&self) -> bool {
        self.matrix && !self.row_lines
    }
}

struct Flow<'f> {
    fonts: &'f Fonts,
    pages: Vec<Vec<Op>>,
    y: f32,
    /// A throw-away flow used to measure content; it skips page-break planning.
    measuring: bool,
}

impl<'f> Flow<'f> {
    fn mono(&self) -> bool {
        self.fonts.mono_theme()
    }

    fn ink(&self) -> Rgb {
        if self.mono() { BLACK } else { INK }
    }

    /// Grays (secondary text, light rules, code tint) are shared by both
    /// themes; only chromatic things differ.
    fn muted(&self) -> Rgb {
        MUTED
    }

    fn rule_color(&self) -> Rgb {
        RULE
    }

    fn new(fonts: &'f Fonts) -> Self {
        Self { fonts, pages: vec![Vec::new()], y: MARGIN, measuring: false }
    }

    /// Start a new page unless `build`'s output fits on one page but not on
    /// what is left of this one; output longer than a page just flows.
    fn keep_together(&mut self, build: &dyn Fn(&mut Flow<'f>)) {
        self.keep_together_up_to(f32::MAX, build);
    }

    /// Like [`keep_together`](Self::keep_together), but only for output at
    /// most `max` tall: big blocks split instead of leaving a large gap.
    fn keep_together_up_to(&mut self, max: f32, build: &dyn Fn(&mut Flow<'f>)) {
        if self.measuring {
            return;
        }
        let mut scratch = Flow { measuring: true, ..Flow::new(self.fonts) };
        build(&mut scratch);
        let height = scratch.y - MARGIN;
        if scratch.pages.len() == 1 && height <= max {
            self.need(height);
        }
    }

    fn push(&mut self, op: Op) {
        self.pages.last_mut().expect("a page always exists").push(op);
    }

    fn need(&mut self, height: f32) {
        if self.y + height > BOTTOM && self.y > MARGIN {
            self.pages.push(Vec::new());
            self.y = MARGIN;
        }
    }

    fn atoms(&self, runs: &[Run], size: f32) -> Vec<Atom> {
        let mut atoms = Vec::new();
        for run in runs {
            let face = self.fonts.face(run.style);
            let text = face.clean(&run.text);
            let mut word = String::new();
            let flush = |word: &mut String, atoms: &mut Vec<Atom>| {
                if !word.is_empty() {
                    let w = face.width(word, size);
                    atoms.push(Atom::Word(std::mem::take(word), run.style, w));
                }
            };
            for c in text.chars() {
                if c == ' ' {
                    flush(&mut word, &mut atoms);
                    atoms.push(Atom::Space(run.style, face.advance(' ', size)));
                } else {
                    word.push(c);
                }
            }
            flush(&mut word, &mut atoms);
        }
        atoms
    }

    fn wrap(&self, runs: &[Run], size: f32, width: f32) -> Vec<Vec<Seg>> {
        let mut lines: Vec<Vec<Seg>> = vec![Vec::new()];
        let mut x = 0.0_f32;
        let mut pending: Vec<(Style, f32)> = Vec::new();

        let place = |lines: &mut Vec<Vec<Seg>>, x: &mut f32, text: &str, style: Style, w: f32| {
            let line = lines.last_mut().expect("a line always exists");
            match line.last_mut() {
                Some(seg) if seg.style == style => {
                    seg.text.push_str(text);
                    seg.w += w;
                }
                _ => line.push(Seg { text: text.to_string(), style, x: *x, w }),
            }
            *x += w;
        };

        for atom in self.atoms(runs, size) {
            match atom {
                Atom::Space(style, w) => {
                    if x > 0.0 {
                        pending.push((style, w));
                    }
                }
                Atom::Word(word, style, w) => {
                    let gap: f32 = pending.iter().map(|(_, w)| w).sum();
                    if x > 0.0 && x + gap + w > width {
                        pending.clear();
                        lines.push(Vec::new());
                        x = 0.0;
                    }
                    for (s, sw) in pending.drain(..) {
                        place(&mut lines, &mut x, " ", s, sw);
                    }
                    if w <= width {
                        place(&mut lines, &mut x, &word, style, w);
                        continue;
                    }
                    let face = self.fonts.face(style);
                    for c in word.chars() {
                        let cw = face.advance(c, size);
                        if x > 0.0 && x + cw > width {
                            lines.push(Vec::new());
                            x = 0.0;
                        }
                        place(&mut lines, &mut x, &c.to_string(), style, cw);
                    }
                }
            }
        }
        lines
    }

    fn draw_line(&mut self, segs: &[Seg], x: f32, top: f32, size: f32, color: Rgb, shift: f32) {
        let height = size * LEADING;
        let baseline = top + size * 1.05;
        for seg in segs {
            let sx = x + shift + seg.x;
            if seg.style.code {
                self.push(Op::Rect {
                    x: sx - 1.0,
                    y: top + height * 0.08,
                    w: seg.w + 2.0,
                    h: height * 0.86,
                    fill: Some(TINT),
                    stroke: None,
                });
            }
            self.push(Op::Text {
                x: sx,
                y: baseline,
                text: seg.text.clone(),
                style: seg.style,
                size,
                color,
            });
            if seg.style.strike {
                let y = baseline - size * 0.3;
                self.push(Op::Line { x1: sx, y1: y, x2: sx + seg.w, y2: y, color, width: 0.6 });
            }
        }
    }

    fn paragraph(&mut self, runs: &[Run], size: f32, x: f32, width: f32, color: Rgb) {
        let height = size * LEADING;
        for line in self.wrap(runs, size, width) {
            self.need(height);
            self.draw_line(&line, x, self.y, size, color, 0.0);
            self.y += height;
        }
    }

    fn rule(&mut self, color: Rgb, width: f32) {
        let y = self.y;
        self.push(Op::Line { x1: MARGIN, y1: y, x2: PAGE_W - MARGIN, y2: y, color, width });
    }

    fn section_title(&mut self, title: &str) {
        self.need(40.0);
        self.y += 14.0;
        let runs = [Run { text: title.to_uppercase(), style: Style { bold: true, ..Style::default() } }];
        let thickness = 0.6;
        self.paragraph(&runs, 9.0, MARGIN, CONTENT_W, self.muted());
        self.y += 1.0;
        self.rule(self.rule_color(), thickness);
        self.y += 8.0;
    }

    /// The description under its section title: the title is kept with the
    /// first heading's section, and a title is never left at a page bottom.
    fn description(&mut self, blocks: &[Block]) {
        let first = &blocks[..section_end(blocks, 0).max(1).min(blocks.len())];
        self.keep_together(&|f| {
            f.section_title("Description");
            f.blocks_inner(first, true);
        });
        self.section_title("Description");
        self.blocks_inner(blocks, true);
    }

    /// Headings plan page breaks hierarchically: a heading's whole section
    /// starts on a fresh page if it would otherwise be split and fits one
    /// page; sections longer than a page are not broken early, but the
    /// sub-sections inside them get the same treatment one level down.
    fn blocks_inner(&mut self, blocks: &[Block], title_above: bool) {
        let mut i = 0;
        while i < blocks.len() {
            let mut next = i + 1;
            match &blocks[i] {
                Block::Blank => self.y += BODY * 0.6,
                Block::Heading { level, runs } => {
                    let size = [16.0, 14.0, 12.5, 11.0, 10.5, 10.0][usize::from(*level - 1)];
                    let bold: Vec<Run> = runs
                        .iter()
                        .map(|r| Run { style: Style { bold: true, ..r.style }, ..r.clone() })
                        .collect();
                    if !(title_above && i == 0) {
                        let section = &blocks[i..section_end(blocks, i)];
                        self.keep_together(&|f| f.blocks_inner(section, true));
                    }
                    self.need(size * LEADING + 2.0 * BODY * LEADING);
                    self.y += size * 0.4;
                    self.paragraph(&bold, size, MARGIN, CONTENT_W, self.ink());
                    self.y += size * 0.15;
                }
                Block::Text { .. } => {
                    next = i + blocks[i..].iter().take_while(|b| matches!(b, Block::Text { .. })).count();
                    let paragraph = &blocks[i..next];
                    self.keep_together_up_to(KEEP_MAX, &|f| f.text_blocks(paragraph));
                    self.text_blocks(paragraph);
                }
                Block::Code(lines) => {
                    self.keep_together_up_to(KEEP_MAX, &|f| f.code_block(lines));
                    self.code_block(lines);
                }
                Block::Table(table) => {
                    self.keep_together_up_to(KEEP_MAX, &|f| f.table(table));
                    self.table(table);
                }
            }
            i = next;
        }
    }

    /// Quotes are set in italics (the terminal style shows `>` instead).
    fn quoted(&self, runs: &[Run], quote_depth: usize) -> Vec<Run> {
        let italic = quote_depth > 0;
        runs.iter()
            .map(|r| Run { style: Style { italic: italic || r.style.italic, ..r.style }, ..r.clone() })
            .collect()
    }

    fn text_blocks(&mut self, blocks: &[Block]) {
        let counts: Vec<usize> = blocks
            .iter()
            .map(|b| match b {
                Block::Text { level, marker, quote_depth, runs } => {
                    let (_, _, width) = self.text_geometry(*level, marker.as_deref(), *quote_depth);
                    self.wrap(&self.quoted(runs, *quote_depth), BODY, width).len()
                }
                _ => 0,
            })
            .collect();
        let total: usize = counts.iter().sum();
        let mut before = 0;
        for (block, count) in blocks.iter().zip(counts) {
            if let Block::Text { level, marker, quote_depth, runs } = block {
                self.text_block(*level, marker.as_deref(), *quote_depth, runs, (before, total));
            }
            before += count;
        }
    }

    /// Indent, text x and text width of a text block.
    fn text_geometry(&self, level: usize, marker: Option<&str>, quote_depth: usize) -> (f32, f32, f32) {
        let indent = level as f32 * 14.0 + quote_depth as f32 * 12.0;
        let marker_w = marker.map_or(0.0, |m| (self.fonts.sans.width(m, BODY) + 5.0).max(16.0));
        (indent, MARGIN + indent + marker_w, (CONTENT_W - indent - marker_w).max(40.0))
    }

    /// `place` is (lines of the paragraph before this block, lines in the paragraph).
    fn text_block(
        &mut self,
        level: usize,
        marker: Option<&str>,
        quote_depth: usize,
        runs: &[Run],
        place: (usize, usize),
    ) {
        let (indent, x, width) = self.text_geometry(level, marker, quote_depth);
        let color = if quote_depth > 0 { self.muted() } else { self.ink() };
        let height = BODY * LEADING;
        let lines = self.wrap(&self.quoted(runs, quote_depth), BODY, width);
        for (i, line) in lines.iter().enumerate() {
            self.need(Self::lines_to_keep(place.0 + i, place.1) as f32 * height);
            if quote_depth > 0 {
                for d in 0..quote_depth {
                    let bx = MARGIN + d as f32 * 12.0 + 3.0;
                    let y = self.y;
                    self.push(Op::Line { x1: bx, y1: y, x2: bx, y2: y + height, color: self.rule_color(), width: 1.5 });
                }
            }
            if i == 0
                && let Some(marker) = marker
            {
                let face_style = Style::default();
                let w = self.fonts.face(face_style).width(marker, BODY);
                let seg = Seg { text: marker.to_string(), style: face_style, x: 0.0, w };
                let y = self.y;
                self.draw_line(&[seg], MARGIN + indent, y, BODY, self.muted(), 0.0);
            }
            self.draw_line(line, x, self.y, BODY, color, 0.0);
            self.y += height;
        }
    }

    /// Lines that must fit before line `i` of `n` may be drawn: no single
    /// line is left alone at the top or bottom of a page.
    fn lines_to_keep(i: usize, n: usize) -> usize {
        if i == 0 || (n >= 4 && i + 2 == n) { n.min(2) } else { 1 }
    }

    /// Code keeps its line structure; lines wider than the block are broken
    /// at the margin rather than clipped, so no content is lost. The block is
    /// tinted gray in both themes.
    fn code_block(&mut self, lines: &[String]) {
        let size = CODE;
        let height = size * LEADING;
        let mono = &self.fonts.mono;
        let room = CONTENT_W - 12.0;
        let mut rows: Vec<String> = Vec::new();
        for line in lines {
            let mut row = String::new();
            for c in mono.clean(line).chars() {
                if mono.width(&row, size) + mono.advance(c, size) > room {
                    rows.push(std::mem::take(&mut row));
                }
                row.push(c);
            }
            rows.push(row);
        }
        self.y += 2.0;
        for (i, text) in rows.iter().enumerate() {
            self.need(Self::lines_to_keep(i, rows.len()) as f32 * height);
            let y = self.y;
            self.push(Op::Rect { x: MARGIN, y, w: CONTENT_W, h: height, fill: Some(TINT), stroke: None });
            self.push(Op::Text {
                x: MARGIN + 6.0,
                y: y + size * 1.05,
                text: text.clone(),
                style: Style { code: true, ..Style::default() },
                size,
                color: self.ink(),
            });
            self.y += height;
        }
        self.y += 2.0;
    }

    /// Column widths: the full text where it fits; otherwise every column
    /// keeps its longest word and the rest of the width is shared out in
    /// proportion to how much more each column would like. The flag says
    /// whether every column got at least its longest word.
    fn table_widths(&self, cells: &[Vec<Vec<Run>>], cols: usize, size: f32, pad: f32) -> (Vec<f32>, bool) {
        let measure = |c: usize, f: &dyn Fn(&Self, &[Run]) -> f32| {
            cells.iter().map(|row| f(self, &row[c])).fold(0.0, f32::max) + 2.0 * pad
        };
        let natural: Vec<f32> = (0..cols)
            .map(|c| {
                measure(c, &|s, runs| {
                    s.wrap(runs, size, f32::MAX).iter().map(|l| l.last().map_or(0.0, |g| g.x + g.w)).fold(0.0, f32::max)
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
        let minimum: Vec<f32> = (0..cols).map(|c| measure(c, &longest_word).min(natural[c])).collect();
        let min_total: f32 = minimum.iter().sum();
        if min_total >= CONTENT_W {
            return (minimum.iter().map(|w| w * CONTENT_W / min_total).collect(), false);
        }
        let wanted: f32 = natural.iter().zip(&minimum).map(|(n, m)| n - m).sum();
        let spare = CONTENT_W - min_total;
        (minimum.iter().zip(&natural).map(|(m, n)| m + (n - m) * spare / wanted).collect(), true)
    }

    fn rule_across(&mut self, y: f32, x: f32, width: f32, thickness: f32) {
        self.push(Op::Line { x1: x, y1: y, x2: x + width, y2: y, color: self.ink(), width: thickness });
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
    fn table(&mut self, table: &Table) {
        const PAD_Y: f32 = 2.5;
        let header_style = Style { bold: true, ..Style::default() };
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
                        blocks::inline(text, if bold { header_style } else { Style::default() })
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
        let layout = TableLayout { widths: &widths, aligns: &table.aligns, left, size, pad: (pad_x, PAD_Y), col_lines, row_lines, matrix };

        let first_rows = rows.iter().take(2).map(|(_, h)| h).sum::<f32>();
        self.need(first_rows + 2.0);
        self.y += 2.0;
        self.top_rule(&layout, total);
        self.table_row(&rows[0], true, &layout);
        self.header_rule(&layout, total);
        for (i, row) in rows[1..].iter().enumerate() {
            if layout.row_lines && i > 0 {
                self.rule_across(self.y, left, total, MIDRULE);
            }
            if self.y + row.1 > BOTTOM && self.y > MARGIN {
                if !layout.open_ends() {
                    self.rule_across(self.y, left, total, MIDRULE);
                }
                self.pages.push(Vec::new());
                self.y = MARGIN + 2.0;
                self.top_rule(&layout, total);
                self.table_row(&rows[0], true, &layout);
                self.header_rule(&layout, total);
            }
            self.table_row(row, false, &layout);
        }
        if !layout.open_ends() {
            self.rule_across(self.y, left, total, layout.outer_rule());
        }
        self.y += 6.0;
    }

    /// Rule above the header; a matrix leaves the empty corner cell open.
    fn top_rule(&mut self, t: &TableLayout, total: f32) {
        if t.open_ends() {
            return;
        }
        let skip = if t.matrix { t.widths[0] } else { 0.0 };
        self.rule_across(self.y, t.left + skip, total - skip, t.outer_rule());
    }

    /// Rule under the header: light, but heavy in a matrix where it separates
    /// the column labels from the contents.
    fn header_rule(&mut self, t: &TableLayout, total: f32) {
        self.rule_across(self.y, t.left, total, if t.matrix { HEAVY_RULE } else { MIDRULE });
    }

    fn table_row(&mut self, (wrapped, row_h): &(Vec<Vec<Vec<Seg>>>, f32), header: bool, t: &TableLayout) {
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
            if !(t.col_lines || divider) || (t.matrix && header && i == 0) {
                continue;
            }
            let width = if divider { HEAVY_RULE } else { MIDRULE };
            self.push(Op::Line { x1: vx, y1: top, x2: vx, y2: top + row_h, color: self.ink(), width });
        }
        self.y += row_h;
    }

    fn chips(&mut self, chips: &[(String, Rgb)]) {
        const SIZE: f32 = 8.5;
        const H: f32 = 14.0;
        let mut x = MARGIN;
        let plain = |text: &str| [Run { text: text.to_string(), style: Style::default() }].to_vec();
        for (name, color) in chips {
            let text = self.fonts.sans.clean(name);
            let w = self.fonts.sans.width(&text, SIZE) + 10.0;
            if x > MARGIN && x + w > PAGE_W - MARGIN {
                x = MARGIN;
                self.y += H + 4.0;
            }
            self.need(H);
            let y = self.y;
            let (fill, stroke) = if self.mono() { (None, Some(BLACK)) } else { (Some(*color), None) };
            self.push(Op::Rect { x, y, w, h: H, fill, stroke });
            let segs = self.wrap(&plain(&text), SIZE, w);
            if let Some(line) = segs.first() {
                self.draw_line(line, x + 5.0, y + (H - SIZE * LEADING) / 2.0, SIZE, self.ink(), 0.0);
            }
            x += w + 4.0;
        }
        self.y += H + 6.0;
    }

    fn checklist(&mut self, card: &Card) {
        let height = BODY * LEADING;
        let x = MARGIN + 18.0;
        for item in &card.checklist {
            let runs = blocks::inline(&item.text, Style::default());
            let color = if item.completed { self.muted() } else { self.ink() };
            for (i, line) in self.wrap(&runs, BODY, CONTENT_W - 18.0).iter().enumerate() {
                self.need(height);
                let top = self.y;
                if i == 0 {
                    let (bx, by) = (MARGIN + 2.0, top + 2.5);
                    self.push(Op::Rect { x: bx, y: by, w: 8.0, h: 8.0, fill: None, stroke: Some(self.muted()) });
                    if item.completed {
                        self.push(Op::Line { x1: bx + 1.8, y1: by + 4.2, x2: bx + 3.4, y2: by + 6.0, color: self.ink(), width: 1.1 });
                        self.push(Op::Line { x1: bx + 3.4, y1: by + 6.0, x2: bx + 6.4, y2: by + 1.6, color: self.ink(), width: 1.1 });
                    }
                }
                self.draw_line(line, x, top, BODY, color, 0.0);
                self.y += height;
            }
            self.y += 1.5;
        }
    }
}

/// End (exclusive) of the section starting at `start`: up to the next
/// heading of the same or a higher level; for non-heading content, up to the
/// next heading of any level.
fn section_end(blocks: &[Block], start: usize) -> usize {
    let level = match blocks.get(start) {
        Some(Block::Heading { level, .. }) => *level,
        _ => 6,
    };
    blocks[start + 1..]
        .iter()
        .position(|b| matches!(b, Block::Heading { level: l, .. } if *l <= level))
        .map_or(blocks.len(), |p| start + 1 + p)
}

fn fit(face: &Face, text: &str, size: f32, max: f32) -> String {
    let mut text = face.clean(text);
    if face.width(&text, size) <= max {
        return text;
    }
    while face.width(&text, size) + face.width("…", size) > max && text.pop().is_some() {}
    format!("{}…", text.trim_end())
}

fn stamp(t: chrono::DateTime<chrono::Utc>) -> String {
    t.with_timezone(&Local).format("%Y-%m-%d %H:%M").to_string()
}

/// Running header (from the second page on) and footer of page `n` of `total`.
fn page_chrome(fonts: &Fonts, card: &Card, location: &str, created: &str, n: usize, total: usize) -> Vec<Op> {
    let (muted, rule) = (MUTED, RULE);
    let small = |x: f32, y: f32, text: String| Op::Text {
        x,
        y,
        text,
        style: Style::default(),
        size: 8.0,
        color: muted,
    };
    let right_aligned = |text: &str| PAGE_W - MARGIN - fonts.sans.width(text, 8.0);
    let mut chrome = Vec::new();
    if n > 0 {
        let created_x = right_aligned(created);
        let room = created_x - MARGIN - 16.0;
        chrome.push(small(MARGIN, HEADER_BASELINE, fit(&fonts.sans, &card.title, 8.0, room)));
        chrome.push(small(created_x, HEADER_BASELINE, created.to_string()));
        chrome.push(Op::Line {
            x1: MARGIN,
            y1: HEADER_BASELINE + 5.0,
            x2: PAGE_W - MARGIN,
            y2: HEADER_BASELINE + 5.0,
            color: rule,
            width: 0.6,
        });
    }
    let counter = format!("{} / {}", n + 1, total);
    let room = CONTENT_W - fonts.sans.width(&counter, 8.0) - 16.0;
    chrome.push(small(MARGIN, FOOTER_BASELINE, fit(&fonts.sans, location, 8.0, room)));
    chrome.push(small(right_aligned(&counter), FOOTER_BASELINE, counter));
    chrome
}

fn emit(
    pages: &[Vec<Op>],
    fonts: &Fonts,
    board: &BoardMeta,
    list: &str,
    card: &Card,
    labels: &[String],
    now: chrono::DateTime<chrono::Utc>,
) -> anyhow::Result<Vec<u8>> {
    let configuration = ConfigurationBuilder::new()
        .with_archival_validator(Archival::A2_B)
        .finish()
        .map_err(|e| anyhow!("PDF/A configuration: {e:?}"))?;
    let mut document = Document::new_with(SerializeSettings {
        compress_content_streams: true,
        configuration,
        ..SerializeSettings::default()
    });
    let mut metadata = Metadata::new()
        .creation_date(
            krilla::metadata::DateTime::new(now.year() as u16)
                .month(now.month() as u8)
                .day(now.day() as u8)
                .hour(now.hour() as u8)
                .minute(now.minute() as u8)
                .second(now.second() as u8),
        )
        .title(card.title.clone())
        .creator("tct".to_string())
        .producer(format!("tct {}", crate::VERSION))
        .description(format!("{} / {}", board.name, list));
    if !labels.is_empty() {
        metadata = metadata.keywords(labels.to_vec());
    }
    document.set_metadata(metadata);

    let total = pages.len();
    let created = format!("Status {}", stamp(now));
    let location = format!("{} › {}", board.name, list);
    for (n, ops) in pages.iter().enumerate() {
        let mut page = document.start_page_with(
            PageSettings::from_wh(PAGE_W, PAGE_H).ok_or_else(|| anyhow!("invalid page size"))?,
        );
        let mut surface = page.surface();
        for op in ops {
            draw_op(&mut surface, fonts, op)?;
        }
        let chrome = page_chrome(fonts, card, &location, &created, n, total);
        for op in &chrome {
            draw_op(&mut surface, fonts, op)?;
        }
        surface.finish();
        page.finish();
    }
    document.finish().map_err(|e| anyhow!("PDF generation failed: {e:?}"))
}

fn paint((r, g, b): Rgb) -> krilla::paint::Paint {
    krilla::color::rgb::Color::new(r, g, b).into()
}

fn draw_op(surface: &mut krilla::surface::Surface, fonts: &Fonts, op: &Op) -> anyhow::Result<()> {
    match op {
        Op::Text { x, y, text, style, size, color } => {
            surface.set_stroke(None);
            surface.set_fill(Some(Fill { paint: paint(*color), ..Fill::default() }));
            surface.draw_text(
                Point::from_xy(*x, *y),
                fonts.face(*style).pdf.clone(),
                *size,
                text,
                false,
                TextDirection::Auto,
            );
        }
        Op::Rect { x, y, w, h, fill, stroke } => {
            let rect = Rect::from_xywh(*x, *y, *w, *h).ok_or_else(|| anyhow!("degenerate rectangle"))?;
            let mut path = PathBuilder::new();
            path.push_rect(rect);
            let path = path.finish().ok_or_else(|| anyhow!("degenerate rectangle"))?;
            surface.set_fill(fill.map(|c| Fill { paint: paint(c), ..Fill::default() }));
            surface.set_stroke(stroke.map(|c| Stroke { paint: paint(c), width: 0.6, ..Stroke::default() }));
            surface.draw_path(&path);
        }
        Op::Line { x1, y1, x2, y2, color, width } => {
            let mut path = PathBuilder::new();
            path.move_to(*x1, *y1);
            path.line_to(*x2, *y2);
            let path = path.finish().ok_or_else(|| anyhow!("degenerate line"))?;
            surface.set_fill(None);
            surface.set_stroke(Some(Stroke { paint: paint(*color), width: *width, ..Stroke::default() }));
            surface.draw_path(&path);
        }
    }
    Ok(())
}

fn layout<'f>(
    fonts: &'f Fonts,
    board: &BoardMeta,
    list: &str,
    card: &Card,
    now: chrono::DateTime<chrono::Utc>,
) -> (Vec<Vec<Op>>, Vec<String>) {
    let mut flow = Flow::new(fonts);

    let title = [Run { text: card.title.clone(), style: Style { bold: true, ..Style::default() } }];
    flow.paragraph(&title, 20.0, MARGIN, CONTENT_W, flow.ink());
    flow.y += 4.0;
    let accent = if flow.mono() { BLACK } else { board.accent_color.to_rgb() };
    flow.rule(accent, 2.0);
    flow.y += 8.0;

    let muted = |text: String| [Run { text, style: Style::default() }];
    let mut facts = vec![format!("{} › {}", board.name, list)];
    if let Some(due) = card.due_date {
        facts.push(format!("Due {}", due.format("%Y-%m-%d")));
    }
    if let Some((done, total)) = card.checklist_progress() {
        facts.push(format!("Checklist {done}/{total}"));
    }
    flow.paragraph(&muted(facts.join("   ·   ")), 9.0, MARGIN, CONTENT_W, flow.muted());
    let dates = format!(
        "Created {}   ·   Updated {}   ·   Status {}",
        stamp(card.created_at),
        stamp(card.updated_at),
        stamp(now)
    );
    flow.paragraph(&muted(dates), 9.0, MARGIN, CONTENT_W, flow.muted());
    flow.y += 6.0;

    let chips: Vec<(String, Rgb)> = card
        .label_ids
        .iter()
        .filter_map(|id| board.labels.iter().find(|l| &l.id == id))
        .map(|l| (l.name.clone(), l.color.to_rgb()))
        .collect();
    if !chips.is_empty() {
        flow.chips(&chips);
    }

    if !card.description.trim().is_empty() {
        flow.description(&blocks::parse(&card.description));
    }
    if !card.checklist.is_empty() {
        let checklist = |f: &mut Flow<'_>| {
            f.section_title("Checklist");
            f.checklist(card);
        };
        flow.keep_together(&checklist);
        checklist(&mut flow);
    }

    let label_names: Vec<String> = chips.into_iter().map(|(name, _)| name).collect();
    (flow.pages, label_names)
}

/// Render `card` (from `list` of `board`) as a PDF/A-2b document in the given theme.
pub fn render(board: &BoardMeta, list: &str, card: &Card, theme: Theme) -> anyhow::Result<Vec<u8>> {
    render_at(board, list, card, theme, chrono::Utc::now())
}

/// Like [`render`] with an explicit "made at" time (creation date and the "Status" stamp).
pub fn render_at(
    board: &BoardMeta,
    list: &str,
    card: &Card,
    theme: Theme,
    now: chrono::DateTime<chrono::Utc>,
) -> anyhow::Result<Vec<u8>> {
    let fonts = Fonts::load_for(theme)?;
    let (pages, labels) = layout(&fonts, board, list, card, now);
    emit(&pages, &fonts, board, list, card, &labels, now)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::card::ChecklistItem;

    const THEMES: [Theme; 2] = [Theme::Color, Theme::Mono];

    fn board() -> BoardMeta {
        BoardMeta::new("Projekt Ä".into())
    }

    #[test]
    fn output_is_a_pdf_a_2b_document() {
        let mut card = Card::new("Größe prüfen ✓ 😀".into());
        card.description = "# Plan\n\n- eins\n  - zwei\n> zitat\n\n```\ncode\n```\n\n| a | b |\n|---|--:|\n| 1 | 2 |".into();
        card.checklist = vec![
            ChecklistItem { text: "offen".into(), completed: false },
            ChecklistItem { text: "erledigt".into(), completed: true },
        ];
        let pdf = render(&board(), "To Do", &card, Theme::Color).unwrap();
        assert!(pdf.starts_with(b"%PDF-"));
        let text = String::from_utf8_lossy(&pdf);
        assert!(text.contains("pdfaid:part"), "XMP must declare the PDF/A part");
        assert!(text.contains("<pdfaid:conformance>B"), "PDF/A level B");
    }

    #[test]
    fn long_cards_paginate() {
        let mut card = Card::new("Long".into());
        card.description = (0..300).map(|i| format!("line {i}")).collect::<Vec<_>>().join("\n");
        let fonts = Fonts::load().unwrap();
        let (pages, _) = layout(&fonts, &board(), "L", &card, chrono::Utc::now());
        assert!(pages.len() >= 5, "got {} pages", pages.len());
    }

    #[test]
    fn unbreakable_words_and_wide_tables_do_not_overflow() {
        let mut card = Card::new("x".repeat(400));
        card.description = format!("{}\n\n| {} | b |\n|---|---|\n| c | d |", "y".repeat(500), "z".repeat(300));
        render(&board(), "L", &card, Theme::Color).unwrap();
    }

    #[test]
    fn wide_tables_keep_short_headers_whole_and_honour_alignment() {
        let mut card = Card::new("T".into());
        card.description = "| Left | Center | Right | Default |\n|:--|:-:|--:|---|\n| a | b | c | d |\n| x | a very long centered cell text that must wrap inside a narrow column because the table is wide | 12345 | y |".into();
        let fonts = Fonts::load().unwrap();
        let (pages, _) = layout(&fonts, &board(), "L", &card, chrono::Utc::now());
        let x_of = |needle: &str| {
            pages[0].iter().find_map(|op| match op {
                Op::Text { text, x, .. } if text == needle => Some(*x),
                _ => None,
            })
        };
        for header in ["Right", "Default"] {
            assert!(x_of(header).is_some(), "{header} was split across lines");
        }
        let (c, five) = (x_of("c").unwrap(), x_of("12345").unwrap());
        let width_c = fonts.sans.width("c", 9.5);
        let width_five = fonts.sans.width("12345", 9.5);
        assert!((c + width_c - (five + width_five)).abs() < 0.5, "right-aligned cells share a right edge");
    }

    fn texts(page: &[Op]) -> Vec<&str> {
        page.iter()
            .filter_map(|op| if let Op::Text { text, .. } = op { Some(text.as_str()) } else { None })
            .collect()
    }

    fn page_of(pages: &[Vec<Op>], needle: &str) -> usize {
        pages
            .iter()
            .position(|p| texts(p).iter().any(|t| t.contains(needle)))
            .unwrap_or_else(|| panic!("{needle} not drawn"))
    }

    fn lines(prefix: &str, n: usize) -> String {
        (0..n).map(|i| format!("{prefix} {i}")).collect::<Vec<_>>().join("\n")
    }

    fn laid_out(card: &Card) -> Vec<Vec<Op>> {
        let fonts = Fonts::load().unwrap();
        layout(&fonts, &board(), "L", card, chrono::Utc::now()).0
    }

    #[test]
    fn a_level_one_section_that_fits_a_page_is_not_split() {
        let mut card = Card::new("T".into());
        card.description = format!("{}\n\n# Section\n\n{}", lines("filler", 38), lines("body", 30));
        let pages = laid_out(&card);
        let heading = page_of(&pages, "Section");
        assert_eq!(heading, 1, "section should start on a fresh page");
        assert_eq!(page_of(&pages, "body 29"), heading);
    }

    #[test]
    fn oversized_sections_fall_back_to_keeping_their_subsections_whole() {
        let mut card = Card::new("T".into());
        let subs: Vec<String> = (0..6).map(|n| format!("## Sub {n}\n\n{}", lines(&format!("s{n}"), 22))).collect();
        card.description = format!("# Huge\n\n{}", subs.join("\n\n"));
        let pages = laid_out(&card);
        assert_eq!(page_of(&pages, "Huge"), 0, "an oversized section starts where it is");
        for n in 0..6 {
            assert_eq!(page_of(&pages, &format!("Sub {n}")), page_of(&pages, &format!("s{n} 21")), "Sub {n} is split");
        }
    }

    #[test]
    fn checklist_moves_to_a_fresh_page_when_it_would_not_fit() {
        let mut card = Card::new("T".into());
        card.description = lines("filler", 30);
        card.checklist = (0..12).map(|i| ChecklistItem { text: format!("item {i}"), completed: false }).collect();
        let pages = laid_out(&card);
        assert_eq!(page_of(&pages, "CHECKLIST"), 1);
        assert_eq!(page_of(&pages, "item 11"), 1);
    }

    #[test]
    fn checklist_stays_when_it_fits() {
        let mut card = Card::new("T".into());
        card.description = lines("filler", 5);
        card.checklist = vec![ChecklistItem { text: "one".into(), completed: false }];
        assert_eq!(laid_out(&card).len(), 1);
    }

    #[test]
    fn footer_shows_the_location_and_later_pages_get_a_titled_header_with_the_status_date() {
        let fonts = Fonts::load().unwrap();
        let mut card = Card::new("My Card".into());
        card.description = "x".into();
        let created = "Status 2026-01-02 03:04";
        let first = page_chrome(&fonts, &card, "Board › List", created, 0, 2);
        let second = page_chrome(&fonts, &card, "Board › List", created, 1, 2);
        assert!(texts(&first).contains(&"Board › List"));
        assert!(!texts(&first).iter().any(|t| t.contains("My Card")), "title is not repeated in the footer");
        assert!(!texts(&first).iter().any(|t| t.contains("Status")), "no header on page 1");
        assert!(texts(&second).contains(&"My Card"));
        assert!(texts(&second).contains(&created));
        assert!(texts(&second).contains(&"2 / 2"));
    }

    #[test]
    fn first_page_intro_states_the_status_date() {
        let mut card = Card::new("T".into());
        card.description = "x".into();
        let pages = laid_out(&card);
        assert!(texts(&pages[0]).iter().any(|t| t.contains("Status")));
    }

    fn count(page: &[Op], needle: &str) -> usize {
        texts(page).iter().filter(|t| **t == needle).count()
    }

    /// Vertical lines of the given weight, as (x, y1, y2).
    fn verticals(page: &[Op], weight: f32) -> Vec<(f32, f32, f32)> {
        page.iter()
            .filter_map(|op| match op {
                Op::Line { x1, y1, x2, y2, width, .. } if x1 == x2 && *width == weight => Some((*x1, *y1, *y2)),
                _ => None,
            })
            .collect()
    }

    /// Table rules of the given weight, as (y, x1, x2) in drawing order.
    fn rules(page: &[Op], weight: f32) -> Vec<(f32, f32, f32)> {
        page.iter()
            .filter_map(|op| match op {
                Op::Line { x1, y1, x2, y2, width, .. } if y1 == y2 && *width == weight => Some((*y1, *x1, *x2)),
                _ => None,
            })
            .collect()
    }

    fn table_page(table: &str) -> Vec<Op> {
        let mut card = Card::new("T".into());
        card.description = table.into();
        laid_out(&card).remove(0)
    }

    #[test]
    fn matrix_first_column_is_bold_and_normal_tables_are_not() {
        let bold = |page: &[Op], t: &str| page.iter().any(|op| matches!(op, Op::Text { text, style, .. } if text == t && style.bold));
        let m = table_page("|  | A |\n|---|---|\n| row | 1 |");
        assert!(bold(&m, "row") && !bold(&m, "1") && bold(&m, "A"));
        let n = table_page("| H | A |\n|---|---|\n| row | 1 |");
        assert!(!bold(&n, "row"));
    }

    fn matrix(cols: usize, body_rows: usize) -> String {
        let head: Vec<String> = (1..cols).map(|c| format!("C{c}")).collect();
        let delim = vec!["---"; cols].join("|");
        let mut t = format!("|  | {} |\n|{delim}|\n", head.join(" | "));
        for r in 0..body_rows {
            t.push_str(&format!("| r{r} |{}\n", " x |".repeat(cols - 1)));
        }
        t
    }

    #[test]
    fn matrix_corner_has_no_rule_above_and_no_line_left() {
        // Few rows: no row separators, so no top or bottom border either; only
        // the heavy rule under the header remains.
        let m = table_page(&matrix(3, 2));
        let hm = rules(&m, HEAVY_RULE);
        assert_eq!(hm.len(), 1, "just the header rule");
        // With row separators the top rule starts right of the open corner.
        let r = table_page(&matrix(3, 8));
        let (thin, header) = (rules(&r, MIDRULE), rules(&r, HEAVY_RULE)[0]);
        assert!(thin[0].1 > header.1 + 10.0, "top rule must start right of the corner");
        assert_eq!(thin[0].2, header.2);
        assert!(thin.last().unwrap().1 <= header.1 + 0.1, "the bottom rule stays whole");
        // Wide matrix: left edge line is dropped on the header row only.
        let g = table_page(&matrix(5, 2));
        let thin = verticals(&g, MIDRULE);
        let left = thin.iter().map(|l| l.0).fold(f32::MAX, f32::min);
        assert_eq!(thin.iter().filter(|l| l.0 == left).count(), 2, "left edge on the two body rows only");
    }

    #[test]
    fn matrix_labels_are_separated_from_contents_by_heavy_lines() {
        let m = table_page(&matrix(3, 2));
        // Heavy divider right of the labels, over header + body rows; no thin
        // column lines with only 2 content columns.
        assert_eq!(verticals(&m, HEAVY_RULE).len(), 3);
        assert!(verticals(&m, MIDRULE).is_empty());
        assert_eq!(rules(&m, MIDRULE).len(), 0, "no row separators and no light header rule");
        assert_eq!(rules(&m, HEAVY_RULE).len(), 1, "header rule only; no top or bottom border");
    }

    #[test]
    fn matrix_column_lines_start_at_four_content_columns() {
        let thin = |cols| verticals(&table_page(&matrix(cols, 2)), MIDRULE).len();
        assert_eq!(thin(4), 0, "3 content columns");
        // Header: 4 (no left edge); each of the 2 body rows: 5. The divider after the labels is heavy.
        assert_eq!(thin(5), 4 + 2 * 5, "4 content columns");
    }

    #[test]
    fn matrix_row_separators_start_above_eight_rows_regardless_of_columns() {
        let seps = |cols, body| rules(&table_page(&matrix(cols, body)), MIDRULE).len();
        assert_eq!(seps(3, 6), 0, "7 rows in all");
        // 6 between the 7 body rows + the (now thin) top and bottom rules
        assert_eq!(seps(3, 7), 8, "8 rows");
        assert_eq!(seps(6, 7), 8, "column count is irrelevant");
        assert_eq!(seps(6, 3), 0, "few rows, many columns: no row separators");
    }

    #[test]
    fn matrix_outer_rules_are_as_thin_as_row_separators_when_there_are_any() {
        let many = table_page(&matrix(3, 7));
        let few = table_page(&matrix(3, 3));
        // Heavy: only the header rule remains in the long one.
        assert_eq!(rules(&many, HEAVY_RULE).len(), 1);
        assert_eq!(rules(&few, HEAVY_RULE).len(), 1, "only the header rule: no outer borders");
        assert!(rules(&few, MIDRULE).is_empty());
        // A normal table keeps its outer borders.
        let plain = table_page(&matrix(3, 3).replacen("|  |", "| H |", 1));
        assert_eq!(rules(&plain, HEAVY_RULE).len(), 2);
    }

    #[test]
    fn long_tables_repeat_their_header_on_every_page() {
        let mut card = Card::new("T".into());
        let rows: Vec<String> = (0..120).map(|i| format!("| r{i} | {i} |")).collect();
        card.description = format!("| Name | Qty |\n|---|--:|\n{}", rows.join("\n"));
        let pages = laid_out(&card);
        assert!(pages.len() >= 3);
        for (n, page) in pages.iter().enumerate() {
            assert_eq!(count(page, "Name"), 1, "page {n} lacks the repeated header");
        }
        assert_eq!(pages.iter().map(|p| count(p, "r119")).sum::<usize>(), 1);
    }

    #[test]
    fn a_short_table_is_not_split_across_pages() {
        let mut card = Card::new("T".into());
        let rows: Vec<String> = (0..10).map(|i| format!("| r{i} | {i} |")).collect();
        card.description = format!("{}\n\n| Name | Qty |\n|---|--:|\n{}", lines("filler", 38), rows.join("\n"));
        let pages = laid_out(&card);
        assert_eq!(page_of(&pages, "Name"), page_of(&pages, "r9"));
        assert_eq!(page_of(&pages, "Name"), 1);
    }

    #[test]
    fn short_code_blocks_and_paragraphs_are_kept_whole() {
        let mut card = Card::new("T".into());
        card.description = format!("{}\n\n```\n{}\n```\n\n{}", lines("filler", 38), lines("code", 8), lines("para", 6));
        let pages = laid_out(&card);
        assert_eq!(page_of(&pages, "code 0"), page_of(&pages, "code 7"));
        assert_eq!(page_of(&pages, "para 0"), page_of(&pages, "para 5"));
    }

    #[test]
    fn long_paragraphs_never_leave_a_single_line_behind() {
        let mut card = Card::new("T".into());
        for filler in 30..60 {
            card.description = format!("{}\n\n{}", lines("filler", filler), lines("para", 30));
            let pages = laid_out(&card);
            let on_first = texts(&pages[0]).iter().filter(|t| t.starts_with("para")).count();
            assert_ne!(on_first, 1, "orphaned line with {filler} filler lines");
            assert_ne!(on_first, 29, "widow line with {filler} filler lines");
        }
    }

    #[test]
    fn long_code_lines_wrap_instead_of_being_clipped() {
        let mut card = Card::new("T".into());
        card.description = format!("```\n{}END\n```", "abcdefghij".repeat(40));
        let pages = laid_out(&card);
        let drawn: String = texts(&pages[0]).concat();
        assert!(drawn.contains(&"abcdefghij".repeat(40)) && drawn.contains("END"), "code text was lost");
    }

    #[test]
    fn crowded_tables_shrink_padding_and_type_before_breaking_words() {
        let (_, meta, card) = corner_cases().into_iter().find(|(n, _, _)| n.contains("wide-table")).unwrap();
        let fonts = Fonts::load().unwrap();
        let (pages, _) = layout(&fonts, &meta, "L", &card, chrono::Utc::now());
        assert_eq!(count(&pages[0], "Column"), 10, "a header word was split");
    }

    #[test]
    fn wide_list_markers_do_not_touch_the_text() {
        let mut card = Card::new("T".into());
        card.description = "99. ninety-nine\n100. hundred".into();
        let pages = laid_out(&card);
        let x_of = |needle: &str| {
            pages[0].iter().find_map(|op| match op {
                Op::Text { text, x, .. } if text == needle => Some(*x),
                _ => None,
            })
        };
        let fonts = Fonts::load().unwrap();
        let marker_end = x_of("100.").unwrap() + fonts.sans.width("100.", BODY);
        assert!(x_of("hundred").unwrap() >= marker_end + 3.0);
    }

    #[test]
    fn mono_uses_only_black_and_grays() {
        let fonts = Fonts::load_for(Theme::Mono).unwrap();
        let gray = |c: Rgb| c == BLACK || c == INK || c == MUTED || c == RULE || c == TINT;
        for (name, meta, card) in corner_cases() {
            let (pages, _) = layout(&fonts, &meta, "L", &card, chrono::Utc::now());
            for (n, page) in pages.iter().enumerate() {
                let chrome = page_chrome(&fonts, &card, "B › L", "Status 2026-01-01 00:00", n, pages.len());
                for op in page.iter().chain(&chrome) {
                    match op {
                        Op::Text { color, .. } | Op::Line { color, .. } => assert!(gray(*color), "{name} p{n}: {color:?}"),
                        Op::Rect { fill, stroke, .. } => {
                            assert!(fill.is_none_or(|c| c == TINT), "{name} p{n}: colored fill");
                            assert!(stroke.is_none_or(gray), "{name} p{n}: colored outline");
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn mono_keeps_the_modern_layout_so_only_colour_differs() {
        let modern = Fonts::load_for(Theme::Color).unwrap();
        let mono = Fonts::load_for(Theme::Mono).unwrap();
        for (name, meta, card) in corner_cases() {
            let now = chrono::Utc::now();
            let place = |fonts: &Fonts| -> Vec<(usize, String, i32, i32)> {
                layout(fonts, &meta, "L", &card, now)
                    .0
                    .iter()
                    .enumerate()
                    .flat_map(|(n, page)| {
                        page.iter().filter_map(move |op| match op {
                            Op::Text { text, x, y, .. } => Some((n, text.clone(), (*x * 10.0) as i32, (*y * 10.0) as i32)),
                            _ => None,
                        })
                    })
                    .collect()
            };
            assert_eq!(place(&modern), place(&mono), "{name}: text moved");
        }
    }

    #[test]
    fn mono_outlines_labels_but_tints_code_gray() {
        let mut card = Card::new("T".into());
        card.description = "`inline`\n\n```\none\n```\n\n> quoted".into();
        let mut meta = board();
        meta.labels.push(crate::model::label::Label::new("urgent".into(), crate::model::label::LabelColor::Red));
        card.label_ids.push(meta.labels[0].id.clone());
        let fonts = Fonts::load_for(Theme::Mono).unwrap();
        let (pages, _) = layout(&fonts, &meta, "L", &card, chrono::Utc::now());
        let outlined = pages[0].iter().filter(|op| matches!(op, Op::Rect { stroke: Some(_), fill: None, .. })).count();
        assert_eq!(outlined, 1, "only the label chip is outlined");
        let tinted = pages[0].iter().filter(|op| matches!(op, Op::Rect { fill: Some(TINT), .. })).count();
        assert_eq!(tinted, 2, "inline code and the code block");
        let gray_quote = pages[0].iter().any(|op| matches!(op, Op::Text { text, color, .. } if text == "quoted" && *color == MUTED));
        assert!(gray_quote, "quotes are gray");
    }

    #[test]
    fn empty_card_renders() {
        render(&board(), "L", &Card::new(String::new()), Theme::Color).unwrap();
    }

    /// Awkward inputs, each rendered as an example PDF and checked for
    /// validity and layout invariants. Names become the example file names.
    fn corner_cases() -> Vec<(&'static str, BoardMeta, Card)> {
        use crate::model::label::{Label, LabelColor};

        let plain = |title: &str, description: String| {
            let mut card = Card::new(title.into());
            card.description = description;
            card
        };
        let mut meta = board();
        let colors = [LabelColor::Red, LabelColor::Blue, LabelColor::Green, LabelColor::Purple];
        for i in 0..24 {
            meta.labels.push(Label::new(format!("label number {i}"), colors[i % colors.len()]));
        }

        let mut cases = Vec::new();
        let mut showcase = plain("Release 2.0 — planning overview", SHOWCASE.into());
        showcase.label_ids = meta.labels.iter().take(3).map(|l| l.id.clone()).collect();
        showcase.due_date = chrono::NaiveDate::from_ymd_opt(2030, 6, 30);
        showcase.checklist = [("Freeze the feature list", true), ("Write the migration guide", true), ("Run the load test", false), ("Tag the release", false)]
            .into_iter()
            .map(|(text, completed)| ChecklistItem { text: text.into(), completed })
            .collect();
        cases.push(("00-showcase", meta.clone(), showcase));
        cases.push(("01-empty-card", meta.clone(), Card::new(String::new())));
        cases.push((
            "02-very-long-title",
            meta.clone(),
            plain(&"An exceedingly long card title that has to wrap over several lines ".repeat(6), "Short.".into()),
        ));
        cases.push((
            "03-unicode-and-missing-glyphs",
            meta.clone(),
            plain(
                "Größe, 日本語 and emoji 😀",
                "Latin: äöüß ÄÖÜ € — “quotes” … ✓ → ★\n\nMissing from the fonts (shown as ?): 日本語 العربية 🧑\u{200d}💻\n\ntab\there and a control\u{7}char".into(),
            ),
        ));
        cases.push((
            "04-unbreakable-words",
            meta.clone(),
            plain(
                "Unbreakable",
                format!("{}\n\n- {}\n\n> {}\n\n`{}`", "x".repeat(300), "y".repeat(260), "z".repeat(260), "c".repeat(200)),
            ),
        ));
        let wide_header: Vec<String> = (0..10).map(|i| format!("Column {i}")).collect();
        let wide_row: Vec<String> = (0..10).map(|i| format!("cell {i}")).collect();
        cases.push((
            "05-wide-table",
            meta.clone(),
            plain(
                "Wide table",
                format!(
                    "| {} |\n|{}|\n| {} |",
                    wide_header.join(" | "),
                    vec!["---"; 10].join("|"),
                    wide_row.join(" | ")
                ),
            ),
        ));
        let rows: Vec<String> = (0..150).map(|i| format!("| item {i} | {} | {i}.50 |", "note ".repeat(i % 7 + 1))).collect();
        cases.push((
            "06-long-table-repeats-header",
            meta.clone(),
            plain("Long table", format!("| Item | Note | Price |\n|:--|:-:|--:|\n{}", rows.join("\n"))),
        ));
        let mut hierarchy = String::from("# Overview\n\n");
        hierarchy.push_str(&lines("Overview line", 20));
        for chapter in 1..=3 {
            hierarchy.push_str(&format!("\n\n# Chapter {chapter}\n\n{}", lines("Intro", 4)));
            for part in 1..=3 {
                hierarchy.push_str(&format!("\n\n## Part {chapter}.{part}\n\n{}", lines("Text", 14)));
                hierarchy.push_str(&format!("\n\n### Detail {chapter}.{part}\n\n{}", lines("Detail", 6)));
            }
        }
        cases.push(("07-heading-hierarchy", meta.clone(), plain("Heading hierarchy", hierarchy)));
        cases.push((
            "08-lists-and-quotes",
            meta.clone(),
            plain(
                "Lists and quotes",
                "- one\n  - two\n    - three\n      - four\n1. first\n2. second with a very long line that wraps around the page because it just keeps going and going and going\n> quote\n>> nested quote\n> > spaced nested quote\n\n* star item\n10. ten".into(),
            ),
        ));
        cases.push((
            "09-code-blocks",
            meta.clone(),
            plain(
                "Code blocks",
                format!(
                    "```\n{}\n```\n\n```\n{}\n```\n\n```\nunterminated",
                    "a very long line of code that goes far beyond the right margin of the page and must be clipped".repeat(2),
                    lines("line of a block that is taller than half a page", 60)
                ),
            ),
        ));
        let mut checklist = plain("Checklist page break", lines("filler", 30));
        checklist.checklist = (0..14)
            .map(|i| ChecklistItem {
                text: if i == 3 { "a long checklist item ".repeat(12) } else { format!("item {i}") },
                completed: i % 3 == 0,
            })
            .collect();
        cases.push(("10-checklist-page-break", meta.clone(), checklist));
        let mut labelled = plain("Many labels", "Labels wrap onto several rows.".into());
        labelled.label_ids = meta.labels.iter().map(|l| l.id.clone()).collect();
        labelled.due_date = chrono::NaiveDate::from_ymd_opt(2030, 1, 31);
        cases.push(("11-many-labels", meta.clone(), labelled));
        cases.push((
            "12-markdown-oddities",
            meta.clone(),
            plain(
                "Markdown oddities",
                "Unclosed **bold and `code and *italic\n\n2 * 3 * 4 = 24\n\n**\n\n####### seven hashes\n\n#no space\n\n| only | header |\n|---|---|\n\n| ragged | row |\n|---|---|\n| a |\n| a | b | c |\n| esc \\| pipe | `x` |\n|  |  |\n\n---\n\n<b>html</b> &amp; stuff".into(),
            ),
        ));
        cases.push((
            "13-emphasis",
            meta.clone(),
            plain(
                "Emphasis",
                "**bold** *italic* ***bold italic*** and ***unclosed\n\n- ***item*** with `code`\n\n> ***quoted*** text\n\n| ***head*** | **b** |\n|---|---|\n| *i* | ***both*** |\n\n***\n\n******".into(),
            ),
        ));
        let mut matrix = String::from("A matrix has an empty top-left cell; its first column labels the rows.\n\n");
        matrix.push_str("|  | A | B |\n|---|:-:|--:|\n| **x** | 1 | 2 |\n| y | 3 | 4 |\n\n");
        matrix.push_str("Whitespace-only corner and emphasis in the labels:\n\n|   | north | south | east | west |\n|---|---|---|---|---|\n| *italic* | 1 | 2 | 3 | 4 |\n| ***both*** | 5 | 6 | 7 | 8 |\n| `code` | 9 | 10 | 11 | 12 |\n\n");
        matrix.push_str("Wrapping labels and cells, and empty row labels:\n\n|  | Short | Long |\n|---|---|---|\n| a row label that is long enough to wrap inside its column | x | a cell that is long enough to wrap as well, over several lines |\n|  | no label | still fine |\n\n");
        matrix.push_str("Header-only matrix and a one-column matrix:\n\n|  | only header |\n|---|---|\n\n|  |\n|---|\n| lonely |\n\n");
        matrix.push_str("Many rows: a grid with row separators, repeating its header across pages:\n\n|  | Q1 | Q2 | Q3 | Q4 |\n|---|--:|--:|--:|--:|\n");
        for i in 0..90 {
            matrix.push_str(&format!("| region {i} | {} | {} | {} | {} |\n", i, i * 2, i * 3, i * 4));
        }
        cases.push(("14-matrix", meta.clone(), plain("Matrix tables", matrix)));
        cases
    }

    const SHOWCASE: &str = "\
Short, readable summary of the release plan with **bold**, *italic* and `inline code`.

# Goals

- Ship the new **export** pipeline
- Cut start-up time in half
  - measured on the reference laptop
  - reported in the release notes
1. Freeze
2. Test
3. Release

> Scope is fixed; dates are not.

# Timeline

| Phase | Owner | Weeks |
|:--|:-:|--:|
| Design | Ada | 2 |
| Build | Grace | 6 |
| Verify | Linus | 3 |

# Responsibilities

A matrix labels its rows as well as its columns (empty top-left cell):

|  | Design | Build | Verify |
|---|:-:|:-:|:-:|
| Ada | owner | review | — |
| Grace | review | owner | support |
| Linus | — | support | owner |

# Notes

```
cargo build --release
./scripts/package.sh
```
";

    #[test]
    fn corner_cases_render_as_valid_pdf_a() {
        for theme in THEMES {
            for (name, meta, card) in corner_cases() {
                let pdf = render(&meta, "List", &card, theme).unwrap_or_else(|e| panic!("{name} {theme:?}: {e:#}"));
                assert!(pdf.starts_with(b"%PDF-"), "{name} {theme:?}");
            }
        }
    }

    #[test]
    fn corner_cases_stay_inside_the_page() {
        for theme in THEMES {
        let fonts = Fonts::load_for(theme).unwrap();
        for (name, meta, card) in corner_cases() {
            let name = format!("{name} {theme:?}");
            let (pages, _) = layout(&fonts, &meta, "List", &card, chrono::Utc::now());
            for (n, page) in pages.iter().enumerate() {
                for op in page {
                    match op {
                        Op::Text { x, y, text, style, size, .. } => {
                            let right = x + fonts.face(*style).width(text, *size);
                            assert!(*x >= MARGIN - 1.0 && right <= PAGE_W - MARGIN + 1.0, "{name} p{n}: '{text}' leaves the margins ({x}..{right})");
                            assert!(*y <= BOTTOM + 1.0, "{name} p{n}: '{text}' below the content area ({y})");
                        }
                        Op::Rect { x, w, y, h, .. } => {
                            assert!(*x >= MARGIN - 1.0 && x + w <= PAGE_W - MARGIN + 1.0, "{name} p{n}: rect leaves the margins");
                            assert!(y + h <= BOTTOM + 1.0, "{name} p{n}: rect below the content area");
                        }
                        Op::Line { x1, x2, y1, y2, .. } => {
                            assert!(x1.min(*x2) >= MARGIN - 1.0 && x1.max(*x2) <= PAGE_W - MARGIN + 1.0, "{name} p{n}: line leaves the margins");
                            assert!(y1.max(*y2) <= BOTTOM + 1.0, "{name} p{n}: line below the content area");
                        }
                    }
                }
            }
        }
        }
    }

    #[test]
    fn corner_cases_never_produce_blank_pages() {
        for theme in THEMES {
        let fonts = Fonts::load_for(theme).unwrap();
        for (name, meta, card) in corner_cases() {
            let (pages, _) = layout(&fonts, &meta, "List", &card, chrono::Utc::now());
            for (n, page) in pages.iter().enumerate() {
                assert!(!page.is_empty(), "{name}: page {n} is blank");
            }
        }
        }
    }

    /// Fixed "made at" time, so regenerated examples only change with their content.
    const EXAMPLE_TIME: chrono::DateTime<chrono::Utc> =
        chrono::DateTime::from_timestamp(1_767_225_600, 0).unwrap();

    #[test]
    fn rendering_at_a_fixed_time_is_deterministic() {
        for (_, meta, card) in corner_cases().into_iter().take(3) {
            let a = render_at(&meta, "To Do", &card, Theme::Color, EXAMPLE_TIME).unwrap();
            let b = render_at(&meta, "To Do", &card, Theme::Color, EXAMPLE_TIME).unwrap();
            assert!(a == b, "PDF output must not depend on anything but its input");
        }
    }

    /// Regenerates `docs/export-examples/*.pdf`:
    /// `cargo test write_example_pdfs -- --ignored`.
    #[test]
    #[ignore = "writes files into docs/"]
    fn write_example_pdfs() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/export-examples");
        for (theme, dir) in [(Theme::Mono, root.clone()), (Theme::Color, root.join("color"))] {
            std::fs::create_dir_all(&dir).unwrap();
            for (name, meta, card) in corner_cases() {
                // Color is shown by the showcase only; the rest are monochrome.
                if !name.starts_with("00-") && theme == Theme::Color {
                    continue;
                }
                let pdf = render_at(&meta, "To Do", &card, theme, EXAMPLE_TIME).unwrap();
                std::fs::write(dir.join(format!("{name}.pdf")), pdf).unwrap();
            }
        }
    }
}
