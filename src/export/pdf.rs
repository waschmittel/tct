//! Card → PDF/A-2b: lay the card out into pages of draw operations, then
//! emit them with krilla, which validates the result against the standard.
//!
//! PDF/A requires every font to be embedded, so text is set in the bundled
//! DejaVu faces; characters they lack are replaced with `?` instead of
//! drawing `.notdef`.

use std::rc::Rc;

use anyhow::{Context, anyhow};
use chrono::{Datelike, Local, Timelike};
use krilla::configure::{Archival, ConfigurationBuilder};
use krilla::geom::{PathBuilder, Point, Rect};
use krilla::metadata::Metadata;
use krilla::page::PageSettings;
use krilla::paint::{Fill, Stroke};
use krilla::text::{Font, TextDirection};
use krilla::{Document, SerializeSettings};
use skrifa::charmap::Charmap;
use skrifa::instance::{LocationRef, Size};
use skrifa::metrics::GlyphMetrics;
use skrifa::{FontRef, MetadataProvider};

use super::Theme;
use super::blocks::{self, Block, InAlert, Note, Run, Style};
use crate::alert::Alert;
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
/// Footnote text size, gap between the longest marker and the text, gap above
/// the footnote area, and the most a page's footnote area may take before the
/// rest of a long note continues on the next page.
const NOTE: f32 = 8.5;
const NOTE_MARKER_GAP: f32 = 6.0;
/// Line height of footnote text and the extra space between two notes.
const NOTE_LINE: f32 = NOTE * 1.5;
const NOTE_BETWEEN: f32 = 2.5;
const NOTE_GAP: f32 = 8.0;
const NOTE_BUDGET: f32 = KEEP_MAX;
/// Footnote references are set at this fraction of the text size, raised.
const SUP_SCALE: f32 = 0.7;
/// Room taken by an alert's icon before its title.
const ICON_SLOT: f32 = 15.0;
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
            units_per_em: f32::from(
                font.metrics(Size::unscaled(), LocationRef::default())
                    .units_per_em,
            ),
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
    Text {
        x: f32,
        y: f32,
        text: String,
        style: Style,
        size: f32,
        color: Rgb,
    },
    Rect {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        fill: Option<Rgb>,
        stroke: Option<Rgb>,
    },
    Line {
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
        color: Rgb,
        width: f32,
    },
    /// GitHub's Octicon for an alert, in the `size` square at (`x`, `y`).
    Icon {
        kind: Alert,
        x: f32,
        y: f32,
        size: f32,
        color: Rgb,
    },
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

/// The laid-out lines of footnote `num` on a page; `cont` marks the rest of a
/// note that began on an earlier page (no number is drawn).
struct NoteItem {
    num: usize,
    lines: Vec<Vec<Seg>>,
    cont: bool,
}

/// Size a run is drawn at: footnote references are small.
fn eff(style: Style, size: f32) -> f32 {
    if style.sup { size * SUP_SCALE } else { size }
}

struct Flow<'f> {
    fonts: &'f Fonts,
    pages: Vec<Vec<Op>>,
    y: f32,
    /// A throw-away flow used to measure content; it skips page-break planning.
    measuring: bool,
    /// Footnotes in order of first reference; `num` below is index + 1.
    notes: Rc<Vec<Note>>,
    /// Footnotes shown at the bottom of each page: those referenced on it.
    page_notes: Vec<Vec<NoteItem>>,
    /// Lines of notes that did not fit their page; they open the next one.
    carry: Vec<NoteItem>,
    /// The widest indent of note text (behind the longest marker of the document).
    note_wrap_indent: f32,
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
        Self::with_notes(fonts, Rc::default())
    }

    fn with_notes(fonts: &'f Fonts, notes: Rc<Vec<Note>>) -> Self {
        // Note text is wrapped for the longest marker of the document; each
        // page then indents by its own longest one.
        let marker_style = Style {
            sup: true,
            ..Style::default()
        };
        let longest = notes
            .iter()
            .map(|n| {
                let marker = format!("[{}]", n.label);
                fonts.sans.width(&marker, eff(marker_style, BODY))
            })
            .fold(0.0, f32::max);
        Self {
            note_wrap_indent: (longest + NOTE_MARKER_GAP).min(CONTENT_W / 3.0),
            fonts,
            pages: vec![Vec::new()],
            y: MARGIN,
            measuring: false,
            notes,
            page_notes: vec![Vec::new()],
            carry: Vec::new(),
        }
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
        let mut scratch = Flow {
            measuring: true,
            ..Flow::with_notes(self.fonts, self.notes.clone())
        };
        build(&mut scratch);
        let height = scratch.y - MARGIN + scratch.reserved();
        if scratch.pages.len() == 1 && height <= max {
            self.need(height);
        }
    }

    fn push(&mut self, op: Op) {
        self.pages
            .last_mut()
            .expect("a page always exists")
            .push(op);
    }

    fn need(&mut self, height: f32) {
        if self.y + height > self.bottom() && self.y > MARGIN {
            self.new_page();
        }
    }

    /// Where content must end on this page: above its footnotes.
    fn bottom(&self) -> f32 {
        BOTTOM - self.reserved()
    }

    fn reserved(&self) -> f32 {
        Self::notes_height(self.page_notes.last().expect("a page always exists"))
    }

    /// Height of a footnote area: gap, lines, and the space between notes.
    fn notes_height(items: &[NoteItem]) -> f32 {
        if items.is_empty() {
            return 0.0;
        }
        let lines: usize = items.iter().map(|i| i.lines.len()).sum();
        NOTE_GAP + lines as f32 * NOTE_LINE + (items.len() - 1) as f32 * NOTE_BETWEEN
    }

    /// Space a note takes above its first line: the gap to the body for the
    /// first note of a page, the space between notes for the others.
    fn note_head(first: bool) -> f32 {
        if first { NOTE_GAP } else { NOTE_BETWEEN }
    }

    fn lines_fitting(height: f32) -> usize {
        (height / NOTE_LINE).floor().max(0.0) as usize
    }

    fn new_page(&mut self) {
        self.pages.push(Vec::new());
        self.page_notes.push(Vec::new());
        self.y = MARGIN;
        // Carried note lines open the page, up to the budget.
        let page = self.page_notes.last_mut().expect("just pushed");
        while !self.carry.is_empty() {
            let used = Self::notes_height(page);
            let room = Self::lines_fitting(NOTE_BUDGET - used - Self::note_head(page.is_empty()));
            if room == 0 {
                break;
            }
            let mut item = self.carry.remove(0);
            let rest = item.lines.split_off(room.min(item.lines.len()));
            if !rest.is_empty() {
                self.carry.insert(
                    0,
                    NoteItem {
                        num: item.num,
                        lines: rest,
                        cont: true,
                    },
                );
            }
            page.push(item);
        }
    }

    /// Wrapped lines of footnote `num`; every source line starts a new line.
    fn note_lines(&self, num: usize) -> Vec<Vec<Seg>> {
        // Wrapped for the widest indent any page may use, so text never
        // outgrows a page whose area turns out to need a wider one.
        let width = CONTENT_W - self.note_wrap_indent;
        self.notes[num - 1]
            .lines
            .iter()
            .flat_map(|runs| self.wrap(runs, NOTE, width))
            .collect()
    }

    /// Footnote numbers referenced in a laid-out line (references carry the
    /// marker as their text).
    fn refs(&self, line: &[Seg]) -> Vec<usize> {
        line.iter()
            .filter(|s| s.style.sup)
            .filter_map(|s| {
                let label = s.text.strip_prefix('[')?.strip_suffix(']')?;
                self.notes.iter().position(|n| n.label == label)
            })
            .map(|p| p + 1)
            .collect()
    }

    fn row_refs(&self, (wrapped, _): &(Vec<Vec<Vec<Seg>>>, f32)) -> Vec<usize> {
        wrapped
            .iter()
            .flatten()
            .flat_map(|l| self.refs(l))
            .collect()
    }

    /// The notes of `nums` the current page does not show yet.
    fn new_notes(&self, nums: &[usize]) -> Vec<usize> {
        let shown = self.page_notes.last().expect("a page always exists");
        let mut new = Vec::new();
        for &n in nums {
            if !new.contains(&n) && !shown.iter().any(|i| i.num == n) {
                new.push(n);
            }
        }
        new
    }

    /// Does a block of `height` at the cursor fit with its footnotes?
    fn fits(&self, height: f32, nums: &[usize]) -> bool {
        let new = self.new_notes(nums);
        let mut extra = 0.0;
        if !new.is_empty() {
            let mut first = self.page_notes.last().is_some_and(Vec::is_empty);
            for &n in &new {
                extra += Self::note_head(first) + self.note_lines(n).len() as f32 * NOTE_LINE;
                first = false;
            }
            if self.reserved() + extra > NOTE_BUDGET {
                return false;
            }
        }
        self.y + height + self.reserved() + extra <= BOTTOM
    }

    /// Show the notes of `nums` on this page, for a block of `height` at the
    /// cursor. What cannot fit even a fresh page continues on the next one.
    fn register(&mut self, height: f32, nums: &[usize]) {
        for num in self.new_notes(nums) {
            let page = self.page_notes.last().expect("a page always exists");
            let used = Self::notes_height(page);
            let free = (BOTTOM - self.y - height - used).min(NOTE_BUDGET - used);
            let room = Self::lines_fitting(free - Self::note_head(page.is_empty()));
            let mut lines = self.note_lines(num);
            let rest = lines.split_off(room.min(lines.len()));
            let placed = !lines.is_empty();
            if placed {
                self.page_notes
                    .last_mut()
                    .expect("a page always exists")
                    .push(NoteItem {
                        num,
                        lines,
                        cont: false,
                    });
            }
            if !rest.is_empty() {
                self.carry.push(NoteItem {
                    num,
                    lines: rest,
                    cont: placed,
                });
            }
        }
    }

    /// Break the page until a block of `height` fits together with the
    /// footnotes it references (a fresh page takes it regardless), then show
    /// those footnotes on the page.
    fn need_notes(&mut self, height: f32, nums: &[usize]) {
        while !self.fits(height, nums) && self.y > MARGIN {
            self.new_page();
        }
        self.register(height, nums);
    }

    /// Finish the document: flush carried note lines onto pages of their own
    /// and draw each page's footnote area above the footer.
    fn finish(mut self) -> Vec<Vec<Op>> {
        while !self.carry.is_empty() {
            self.new_page();
        }
        let page_notes = std::mem::take(&mut self.page_notes);
        for (page, mut items) in page_notes.into_iter().enumerate() {
            if items.is_empty() {
                continue;
            }
            items.sort_by_key(|i| (!i.cont, i.num));
            let top = BOTTOM - Self::notes_height(&items);
            // Text starts behind the longest marker of this page.
            let marker_style = Style {
                sup: true,
                ..Style::default()
            };
            let longest = items
                .iter()
                .map(|i| {
                    let marker = format!("[{}]", self.notes[i.num - 1].label);
                    self.fonts.sans.width(&marker, eff(marker_style, BODY))
                })
                .fold(0.0, f32::max);
            let indent = (longest + NOTE_MARKER_GAP).min(CONTENT_W / 3.0);
            let mut canvas = Flow::new(self.fonts);
            canvas.push(Op::Line {
                x1: MARGIN,
                y1: top + 2.0,
                x2: MARGIN + CONTENT_W / 4.0,
                y2: top + 2.0,
                color: self.muted(),
                width: 0.6,
            });
            let mut y = top + NOTE_GAP;
            for (k, item) in items.iter().enumerate() {
                if k > 0 {
                    y += NOTE_BETWEEN;
                }
                for (i, line) in item.lines.iter().enumerate() {
                    if i == 0 && !item.cont {
                        // The marker: raised, as large as a reference in the
                        // text, no dot, always black; on the text's baseline.
                        let label = format!("[{}]", self.notes[item.num - 1].label);
                        let w = self.fonts.sans.width(&label, eff(marker_style, BODY));
                        let seg = Seg {
                            text: label,
                            style: marker_style,
                            x: 0.0,
                            w,
                        };
                        let top = y - (BODY - NOTE) * 1.05;
                        canvas.draw_line(&[seg], MARGIN, top, BODY, BLACK, 0.0);
                    }
                    canvas.draw_line(line, MARGIN + indent, y, NOTE, self.ink(), 0.0);
                    y += NOTE_LINE;
                }
            }
            let ops = canvas.pages.remove(0);
            self.pages[page].extend(ops);
        }
        self.pages
    }

    fn atoms(&self, runs: &[Run], size: f32) -> Vec<Atom> {
        let mut atoms = Vec::new();
        for run in runs {
            let face = self.fonts.face(run.style);
            let size = eff(run.style, size);
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
                Some(seg) if seg.style == style && !style.sup => {
                    seg.text.push_str(text);
                    seg.w += w;
                }
                _ => line.push(Seg {
                    text: text.to_string(),
                    style,
                    x: *x,
                    w,
                }),
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
                        let cw = face.advance(c, eff(style, size));
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
            let lift = if seg.style.sup { size * 0.35 } else { 0.0 };
            self.push(Op::Text {
                x: sx,
                y: baseline - lift,
                text: seg.text.clone(),
                style: seg.style,
                size: eff(seg.style, size),
                color,
            });
            if seg.style.strike {
                let y = baseline - size * 0.3;
                self.push(Op::Line {
                    x1: sx,
                    y1: y,
                    x2: sx + seg.w,
                    y2: y,
                    color,
                    width: 0.6,
                });
            }
        }
    }

    fn paragraph(&mut self, runs: &[Run], size: f32, x: f32, width: f32, color: Rgb) {
        let height = size * LEADING;
        for line in self.wrap(runs, size, width) {
            self.need_notes(height, &self.refs(&line));
            self.draw_line(&line, x, self.y, size, color, 0.0);
            self.y += height;
        }
    }

    fn rule(&mut self, color: Rgb, width: f32) {
        let y = self.y;
        self.push(Op::Line {
            x1: MARGIN,
            y1: y,
            x2: PAGE_W - MARGIN,
            y2: y,
            color,
            width,
        });
    }

    fn section_title(&mut self, title: &str) {
        self.need(40.0);
        self.y += 14.0;
        let runs = [Run {
            text: title.to_uppercase(),
            style: Style {
                bold: true,
                ..Style::default()
            },
        }];
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
                Block::Blank => {
                    let side = |b: Option<&Block>| -> f32 {
                        match b {
                            Some(Block::Text { alert: Some(_), .. }) => 0.35,
                            Some(Block::Text { quote_depth, .. }) if *quote_depth > 0 => 0.35,
                            Some(Block::Text {
                                marker: Some(_), ..
                            }) => 0.2,
                            _ => 0.0,
                        }
                    };
                    let (before, after) = (i.checked_sub(1).map(|j| &blocks[j]), blocks.get(i + 1));
                    self.y += BODY * (0.6 + side(before).max(side(after)));
                }
                Block::Heading { level, runs } => {
                    let size = [16.0, 14.0, 12.5, 11.0, 10.5, 10.0][usize::from(*level - 1)];
                    let bold: Vec<Run> = runs
                        .iter()
                        .map(|r| Run {
                            style: Style {
                                bold: true,
                                ..r.style
                            },
                            ..r.clone()
                        })
                        .collect();
                    if !(title_above && i == 0) {
                        let section = &blocks[i..section_end(blocks, i)];
                        self.keep_together(&|f| f.blocks_inner(section, true));
                    }
                    self.need(size * LEADING + 2.0 * BODY * LEADING);
                    self.y += size * 0.8;
                    self.paragraph(&bold, size, MARGIN, CONTENT_W, self.ink());
                    self.y += size * 0.15;
                }
                Block::Text { .. } => {
                    next = i + blocks[i..]
                        .iter()
                        .take_while(|b| matches!(b, Block::Text { .. }))
                        .count();
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
            .map(|r| Run {
                style: Style {
                    italic: italic || r.style.italic,
                    ..r.style
                },
                ..r.clone()
            })
            .collect()
    }

    fn text_blocks(&mut self, blocks: &[Block]) {
        let counts: Vec<usize> = blocks
            .iter()
            .map(|b| match b {
                Block::Text {
                    level,
                    marker,
                    quote_depth,
                    alert,
                    runs,
                } => {
                    let (_, _, width) =
                        self.text_geometry(*level, marker.as_deref(), *quote_depth, *alert);
                    self.wrap(&self.quoted(runs, *quote_depth), BODY, width)
                        .len()
                }
                _ => 0,
            })
            .collect();
        let total: usize = counts.iter().sum();
        let mut before = 0;
        for (block, count) in blocks.iter().zip(counts) {
            if let Block::Text {
                level,
                marker,
                quote_depth,
                alert,
                runs,
            } = block
            {
                self.text_block(
                    *level,
                    marker.as_deref(),
                    *quote_depth,
                    *alert,
                    runs,
                    (before, total),
                );
            }
            before += count;
        }
    }

    /// Indent, text x and text width of a text block.
    fn text_geometry(
        &self,
        level: usize,
        marker: Option<&str>,
        quote_depth: usize,
        alert: Option<InAlert>,
    ) -> (f32, f32, f32) {
        let indent = level as f32 * 14.0
            + quote_depth as f32 * 12.0
            + if alert.is_some() { 12.0 } else { 0.0 };
        let icon_w = if alert.is_some_and(|a| a.title) {
            ICON_SLOT
        } else {
            0.0
        };
        let marker_w =
            icon_w + marker.map_or(0.0, |m| (self.fonts.sans.width(m, BODY) + 5.0).max(16.0));
        (
            indent,
            MARGIN + indent + marker_w,
            (CONTENT_W - indent - marker_w).max(40.0),
        )
    }

    /// `place` is (lines of the paragraph before this block, lines in the paragraph).
    fn text_block(
        &mut self,
        level: usize,
        marker: Option<&str>,
        quote_depth: usize,
        alert: Option<InAlert>,
        runs: &[Run],
        place: (usize, usize),
    ) {
        let (indent, x, width) = self.text_geometry(level, marker, quote_depth, alert);
        let alert_color = alert.map(|a| if self.mono() { BLACK } else { a.kind.rgb() });
        let color = match (alert, alert_color) {
            (Some(InAlert { title: true, .. }), Some(c)) => c,
            _ if quote_depth > 0 => self.muted(),
            _ => self.ink(),
        };
        let height = BODY * LEADING;
        let lines = self.wrap(&self.quoted(runs, quote_depth), BODY, width);
        for (i, line) in lines.iter().enumerate() {
            let keep = Self::lines_to_keep(place.0 + i, place.1);
            let nums: Vec<usize> = lines[i..(i + keep).min(lines.len())]
                .iter()
                .flat_map(|l| self.refs(l))
                .collect();
            self.need_notes(keep as f32 * height, &nums);
            if quote_depth > 0 {
                for d in 0..quote_depth {
                    let bx = MARGIN + d as f32 * 12.0 + 3.0;
                    let y = self.y;
                    self.push(Op::Line {
                        x1: bx,
                        y1: y,
                        x2: bx,
                        y2: y + height,
                        color: self.rule_color(),
                        width: 1.5,
                    });
                }
            }
            if let (Some(a), Some(bar)) = (alert, alert_color) {
                let y = self.y;
                self.push(Op::Line {
                    x1: MARGIN + 3.0,
                    y1: y,
                    x2: MARGIN + 3.0,
                    y2: y + height,
                    color: bar,
                    width: 2.5,
                });
                if a.title && i == 0 {
                    let size = BODY + 1.0;
                    self.push(Op::Icon {
                        kind: a.kind,
                        x: MARGIN + indent,
                        y: y + (height - size) / 2.0,
                        size,
                        color: bar,
                    });
                }
            }
            if i == 0
                && let Some(marker) = marker
            {
                let face_style = Style::default();
                let w = self.fonts.face(face_style).width(marker, BODY);
                let seg = Seg {
                    text: marker.to_string(),
                    style: face_style,
                    x: 0.0,
                    w,
                };
                let y = self.y;
                let marker_color = if quote_depth > 0 {
                    self.muted()
                } else {
                    self.ink()
                };
                self.draw_line(&[seg], MARGIN + indent, y, BODY, marker_color, 0.0);
            }
            self.draw_line(line, x, self.y, BODY, color, 0.0);
            self.y += height;
        }
    }

    /// Lines that must fit before line `i` of `n` may be drawn: no single
    /// line is left alone at the top or bottom of a page.
    fn lines_to_keep(i: usize, n: usize) -> usize {
        if i == 0 || (n >= 4 && i + 2 == n) {
            n.min(2)
        } else {
            1
        }
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
            self.push(Op::Rect {
                x: MARGIN,
                y,
                w: CONTENT_W,
                h: height,
                fill: Some(TINT),
                stroke: None,
            });
            self.push(Op::Text {
                x: MARGIN + 6.0,
                y: y + size * 1.05,
                text: text.clone(),
                style: Style {
                    code: true,
                    ..Style::default()
                },
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
    fn table_widths(
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

    fn rule_across(&mut self, y: f32, x: f32, width: f32, thickness: f32) {
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
    fn table(&mut self, table: &Table) {
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
        self.rule_across(
            self.y,
            t.left,
            total,
            if t.matrix { HEAVY_RULE } else { MIDRULE },
        );
    }

    fn table_row(
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
            if !(t.col_lines || divider) || (t.matrix && header && i == 0) {
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

    fn chips(&mut self, chips: &[(String, Rgb)]) {
        const SIZE: f32 = 8.5;
        const H: f32 = 14.0;
        let mut x = MARGIN;
        let plain = |text: &str| {
            [Run {
                text: text.to_string(),
                style: Style::default(),
            }]
            .to_vec()
        };
        for (name, color) in chips {
            let text = self.fonts.sans.clean(name);
            let w = self.fonts.sans.width(&text, SIZE) + 10.0;
            if x > MARGIN && x + w > PAGE_W - MARGIN {
                x = MARGIN;
                self.y += H + 4.0;
            }
            self.need(H);
            let y = self.y;
            let (fill, stroke) = if self.mono() {
                (None, Some(BLACK))
            } else {
                (Some(*color), None)
            };
            self.push(Op::Rect {
                x,
                y,
                w,
                h: H,
                fill,
                stroke,
            });
            let segs = self.wrap(&plain(&text), SIZE, w);
            if let Some(line) = segs.first() {
                self.draw_line(
                    line,
                    x + 5.0,
                    y + (H - SIZE * LEADING) / 2.0,
                    SIZE,
                    self.ink(),
                    0.0,
                );
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
            let color = if item.completed {
                self.muted()
            } else {
                self.ink()
            };
            for (i, line) in self.wrap(&runs, BODY, CONTENT_W - 18.0).iter().enumerate() {
                self.need(height);
                let top = self.y;
                if i == 0 {
                    let (bx, by) = (MARGIN + 2.0, top + 2.5);
                    self.push(Op::Rect {
                        x: bx,
                        y: by,
                        w: 8.0,
                        h: 8.0,
                        fill: None,
                        stroke: Some(self.muted()),
                    });
                    if item.completed {
                        self.push(Op::Line {
                            x1: bx + 1.8,
                            y1: by + 4.2,
                            x2: bx + 3.4,
                            y2: by + 6.0,
                            color: self.ink(),
                            width: 1.1,
                        });
                        self.push(Op::Line {
                            x1: bx + 3.4,
                            y1: by + 6.0,
                            x2: bx + 6.4,
                            y2: by + 1.6,
                            color: self.ink(),
                            width: 1.1,
                        });
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
fn page_chrome(
    fonts: &Fonts,
    card: &Card,
    location: &str,
    created: &str,
    n: usize,
    total: usize,
) -> Vec<Op> {
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
        chrome.push(small(
            MARGIN,
            HEADER_BASELINE,
            fit(&fonts.sans, &card.title, 8.0, room),
        ));
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
    chrome.push(small(
        MARGIN,
        FOOTER_BASELINE,
        fit(&fonts.sans, location, 8.0, room),
    ));
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
    version: &str,
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
        .producer(format!("tct {version}"))
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
    document
        .finish()
        .map_err(|e| anyhow!("PDF generation failed: {e:?}"))
}

fn paint((r, g, b): Rgb) -> krilla::paint::Paint {
    krilla::color::rgb::Color::new(r, g, b).into()
}

fn draw_op(surface: &mut krilla::surface::Surface, fonts: &Fonts, op: &Op) -> anyhow::Result<()> {
    match op {
        Op::Text {
            x,
            y,
            text,
            style,
            size,
            color,
        } => {
            surface.set_stroke(None);
            surface.set_fill(Some(Fill {
                paint: paint(*color),
                ..Fill::default()
            }));
            surface.draw_text(
                Point::from_xy(*x, *y),
                fonts.face(*style).pdf.clone(),
                *size,
                text,
                false,
                TextDirection::Auto,
            );
        }
        Op::Rect {
            x,
            y,
            w,
            h,
            fill,
            stroke,
        } => {
            let rect =
                Rect::from_xywh(*x, *y, *w, *h).ok_or_else(|| anyhow!("degenerate rectangle"))?;
            let mut path = PathBuilder::new();
            path.push_rect(rect);
            let path = path
                .finish()
                .ok_or_else(|| anyhow!("degenerate rectangle"))?;
            surface.set_fill(fill.map(|c| Fill {
                paint: paint(c),
                ..Fill::default()
            }));
            surface.set_stroke(stroke.map(|c| Stroke {
                paint: paint(c),
                width: 0.6,
                ..Stroke::default()
            }));
            surface.draw_path(&path);
        }
        Op::Icon {
            kind,
            x,
            y,
            size,
            color,
        } => {
            let svg = kurbo::BezPath::from_svg(kind.octicon_path())
                .context("bundled icon is unusable")?;
            let k = f64::from(*size) / 16.0;
            let (ox, oy) = (f64::from(*x), f64::from(*y));
            let p = |pt: kurbo::Point| ((ox + pt.x * k) as f32, (oy + pt.y * k) as f32);
            let mut path = PathBuilder::new();
            for el in svg.elements() {
                match *el {
                    kurbo::PathEl::MoveTo(a) => path.move_to(p(a).0, p(a).1),
                    kurbo::PathEl::LineTo(a) => path.line_to(p(a).0, p(a).1),
                    kurbo::PathEl::QuadTo(a, b) => path.quad_to(p(a).0, p(a).1, p(b).0, p(b).1),
                    kurbo::PathEl::CurveTo(a, b, c) => {
                        path.cubic_to(p(a).0, p(a).1, p(b).0, p(b).1, p(c).0, p(c).1)
                    }
                    kurbo::PathEl::ClosePath => path.close(),
                }
            }
            let path = path.finish().ok_or_else(|| anyhow!("degenerate icon"))?;
            surface.set_stroke(None);
            surface.set_fill(Some(Fill {
                paint: paint(*color),
                ..Fill::default()
            }));
            surface.draw_path(&path);
        }
        Op::Line {
            x1,
            y1,
            x2,
            y2,
            color,
            width,
        } => {
            let mut path = PathBuilder::new();
            path.move_to(*x1, *y1);
            path.line_to(*x2, *y2);
            let path = path.finish().ok_or_else(|| anyhow!("degenerate line"))?;
            surface.set_fill(None);
            surface.set_stroke(Some(Stroke {
                paint: paint(*color),
                width: *width,
                ..Stroke::default()
            }));
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
    let doc = blocks::parse_doc(&card.description);
    let mut flow = Flow::with_notes(fonts, Rc::new(doc.notes));

    let title = [Run {
        text: card.title.clone(),
        style: Style {
            bold: true,
            ..Style::default()
        },
    }];
    flow.paragraph(&title, 20.0, MARGIN, CONTENT_W, flow.ink());
    flow.y += 4.0;
    let accent = if flow.mono() {
        BLACK
    } else {
        board.accent_color.to_rgb()
    };
    flow.rule(accent, 2.0);
    flow.y += 8.0;

    let muted = |text: String| {
        [Run {
            text,
            style: Style::default(),
        }]
    };
    let mut facts = vec![format!("{} › {}", board.name, list)];
    if let Some(due) = card.due_date {
        facts.push(format!("Due {}", due.format("%Y-%m-%d")));
    }
    if let Some((done, total)) = card.checklist_progress() {
        facts.push(format!("Checklist {done}/{total}"));
    }
    flow.paragraph(
        &muted(facts.join("   ·   ")),
        9.0,
        MARGIN,
        CONTENT_W,
        flow.muted(),
    );
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

    if !doc.blocks.is_empty() {
        flow.description(&doc.blocks);
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
    (flow.finish(), label_names)
}

/// Render `card` (from `list` of `board`) as a PDF/A-2b document in the given theme.
pub fn render(board: &BoardMeta, list: &str, card: &Card, theme: Theme) -> anyhow::Result<Vec<u8>> {
    render_at(board, list, card, theme, chrono::Utc::now(), crate::VERSION)
}

/// Like [`render`] with an explicit "made at" time (creation date and the "Status" stamp)
/// and tool version, so output can be made independent of the build.
pub fn render_at(
    board: &BoardMeta,
    list: &str,
    card: &Card,
    theme: Theme,
    now: chrono::DateTime<chrono::Utc>,
    version: &str,
) -> anyhow::Result<Vec<u8>> {
    let fonts = Fonts::load_for(theme)?;
    let (pages, labels) = layout(&fonts, board, list, card, now);
    emit(&pages, &fonts, board, list, card, &labels, now, version)
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
            ChecklistItem {
                text: "offen".into(),
                completed: false,
            },
            ChecklistItem {
                text: "erledigt".into(),
                completed: true,
            },
        ];
        let pdf = render(&board(), "To Do", &card, Theme::Color).unwrap();
        assert!(pdf.starts_with(b"%PDF-"));
        let text = String::from_utf8_lossy(&pdf);
        assert!(
            text.contains("pdfaid:part"),
            "XMP must declare the PDF/A part"
        );
        assert!(text.contains("<pdfaid:conformance>B"), "PDF/A level B");
    }

    #[test]
    fn long_cards_paginate() {
        let mut card = Card::new("Long".into());
        card.description = (0..300)
            .map(|i| format!("line {i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let fonts = Fonts::load().unwrap();
        let (pages, _) = layout(&fonts, &board(), "L", &card, chrono::Utc::now());
        assert!(pages.len() >= 5, "got {} pages", pages.len());
    }

    #[test]
    fn unbreakable_words_and_wide_tables_do_not_overflow() {
        let mut card = Card::new("x".repeat(400));
        card.description = format!(
            "{}\n\n| {} | b |\n|---|---|\n| c | d |",
            "y".repeat(500),
            "z".repeat(300)
        );
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
        assert!(
            (c + width_c - (five + width_five)).abs() < 0.5,
            "right-aligned cells share a right edge"
        );
    }

    fn texts(page: &[Op]) -> Vec<&str> {
        page.iter()
            .filter_map(|op| {
                if let Op::Text { text, .. } = op {
                    Some(text.as_str())
                } else {
                    None
                }
            })
            .collect()
    }

    fn page_of(pages: &[Vec<Op>], needle: &str) -> usize {
        pages
            .iter()
            .position(|p| texts(p).iter().any(|t| t.contains(needle)))
            .unwrap_or_else(|| panic!("{needle} not drawn"))
    }

    fn lines(prefix: &str, n: usize) -> String {
        (0..n)
            .map(|i| format!("{prefix} {i}"))
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn laid_out(card: &Card) -> Vec<Vec<Op>> {
        let fonts = Fonts::load().unwrap();
        layout(&fonts, &board(), "L", card, chrono::Utc::now()).0
    }

    #[test]
    fn a_level_one_section_that_fits_a_page_is_not_split() {
        let mut card = Card::new("T".into());
        card.description = format!(
            "{}\n\n# Section\n\n{}",
            lines("filler", 38),
            lines("body", 30)
        );
        let pages = laid_out(&card);
        let heading = page_of(&pages, "Section");
        assert_eq!(heading, 1, "section should start on a fresh page");
        assert_eq!(page_of(&pages, "body 29"), heading);
    }

    #[test]
    fn oversized_sections_fall_back_to_keeping_their_subsections_whole() {
        let mut card = Card::new("T".into());
        let subs: Vec<String> = (0..6)
            .map(|n| format!("## Sub {n}\n\n{}", lines(&format!("s{n}"), 22)))
            .collect();
        card.description = format!("# Huge\n\n{}", subs.join("\n\n"));
        let pages = laid_out(&card);
        assert_eq!(
            page_of(&pages, "Huge"),
            0,
            "an oversized section starts where it is"
        );
        for n in 0..6 {
            assert_eq!(
                page_of(&pages, &format!("Sub {n}")),
                page_of(&pages, &format!("s{n} 21")),
                "Sub {n} is split"
            );
        }
    }

    #[test]
    fn checklist_moves_to_a_fresh_page_when_it_would_not_fit() {
        let mut card = Card::new("T".into());
        card.description = lines("filler", 30);
        card.checklist = (0..12)
            .map(|i| ChecklistItem {
                text: format!("item {i}"),
                completed: false,
            })
            .collect();
        let pages = laid_out(&card);
        assert_eq!(page_of(&pages, "CHECKLIST"), 1);
        assert_eq!(page_of(&pages, "item 11"), 1);
    }

    #[test]
    fn checklist_stays_when_it_fits() {
        let mut card = Card::new("T".into());
        card.description = lines("filler", 5);
        card.checklist = vec![ChecklistItem {
            text: "one".into(),
            completed: false,
        }];
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
        assert!(
            !texts(&first).iter().any(|t| t.contains("My Card")),
            "title is not repeated in the footer"
        );
        assert!(
            !texts(&first).iter().any(|t| t.contains("Status")),
            "no header on page 1"
        );
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
                Op::Line {
                    x1,
                    y1,
                    x2,
                    y2,
                    width,
                    ..
                } if x1 == x2 && *width == weight => Some((*x1, *y1, *y2)),
                _ => None,
            })
            .collect()
    }

    /// Table rules of the given weight, as (y, x1, x2) in drawing order.
    fn rules(page: &[Op], weight: f32) -> Vec<(f32, f32, f32)> {
        page.iter()
            .filter_map(|op| match op {
                Op::Line {
                    x1,
                    y1,
                    x2,
                    y2,
                    width,
                    ..
                } if y1 == y2 && *width == weight => Some((*y1, *x1, *x2)),
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
        let bold = |page: &[Op], t: &str| {
            page.iter()
                .any(|op| matches!(op, Op::Text { text, style, .. } if text == t && style.bold))
        };
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
        assert!(
            thin[0].1 > header.1 + 10.0,
            "top rule must start right of the corner"
        );
        assert_eq!(thin[0].2, header.2);
        assert!(
            thin.last().unwrap().1 <= header.1 + 0.1,
            "the bottom rule stays whole"
        );
        // Wide matrix: left edge line is dropped on the header row only.
        let g = table_page(&matrix(5, 2));
        let thin = verticals(&g, MIDRULE);
        let left = thin.iter().map(|l| l.0).fold(f32::MAX, f32::min);
        assert_eq!(
            thin.iter().filter(|l| l.0 == left).count(),
            2,
            "left edge on the two body rows only"
        );
    }

    #[test]
    fn matrix_labels_are_separated_from_contents_by_heavy_lines() {
        let m = table_page(&matrix(3, 2));
        // Heavy divider right of the labels, over header + body rows; no thin
        // column lines with only 2 content columns.
        assert_eq!(verticals(&m, HEAVY_RULE).len(), 3);
        assert!(verticals(&m, MIDRULE).is_empty());
        assert_eq!(
            rules(&m, MIDRULE).len(),
            0,
            "no row separators and no light header rule"
        );
        assert_eq!(
            rules(&m, HEAVY_RULE).len(),
            1,
            "header rule only; no top or bottom border"
        );
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
        assert_eq!(
            rules(&few, HEAVY_RULE).len(),
            1,
            "only the header rule: no outer borders"
        );
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
        card.description = format!(
            "{}\n\n| Name | Qty |\n|---|--:|\n{}",
            lines("filler", 38),
            rows.join("\n")
        );
        let pages = laid_out(&card);
        assert_eq!(page_of(&pages, "Name"), page_of(&pages, "r9"));
        assert_eq!(page_of(&pages, "Name"), 1);
    }

    #[test]
    fn short_code_blocks_and_paragraphs_are_kept_whole() {
        let mut card = Card::new("T".into());
        card.description = format!(
            "{}\n\n```\n{}\n```\n\n{}",
            lines("filler", 38),
            lines("code", 8),
            lines("para", 6)
        );
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
            let on_first = texts(&pages[0])
                .iter()
                .filter(|t| t.starts_with("para"))
                .count();
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
        assert!(
            drawn.contains(&"abcdefghij".repeat(40)) && drawn.contains("END"),
            "code text was lost"
        );
    }

    #[test]
    fn crowded_tables_shrink_padding_and_type_before_breaking_words() {
        let (_, meta, card) = corner_cases()
            .into_iter()
            .find(|(n, _, _)| n.contains("wide-table"))
            .unwrap();
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
                let chrome = page_chrome(
                    &fonts,
                    &card,
                    "B › L",
                    "Status 2026-01-01 00:00",
                    n,
                    pages.len(),
                );
                for op in page.iter().chain(&chrome) {
                    match op {
                        Op::Text { color, .. }
                        | Op::Line { color, .. }
                        | Op::Icon { color, .. } => assert!(gray(*color), "{name} p{n}: {color:?}"),
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
                            Op::Text { text, x, y, .. } => {
                                Some((n, text.clone(), (*x * 10.0) as i32, (*y * 10.0) as i32))
                            }
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
        meta.labels.push(crate::model::label::Label::new(
            "urgent".into(),
            crate::model::label::LabelColor::Red,
        ));
        card.label_ids.push(meta.labels[0].id.clone());
        let fonts = Fonts::load_for(Theme::Mono).unwrap();
        let (pages, _) = layout(&fonts, &meta, "L", &card, chrono::Utc::now());
        let outlined = pages[0]
            .iter()
            .filter(|op| {
                matches!(
                    op,
                    Op::Rect {
                        stroke: Some(_),
                        fill: None,
                        ..
                    }
                )
            })
            .count();
        assert_eq!(outlined, 1, "only the label chip is outlined");
        let tinted = pages[0]
            .iter()
            .filter(|op| {
                matches!(
                    op,
                    Op::Rect {
                        fill: Some(TINT),
                        ..
                    }
                )
            })
            .count();
        assert_eq!(tinted, 2, "inline code and the code block");
        let gray_quote = pages[0].iter().any(
            |op| matches!(op, Op::Text { text, color, .. } if text == "quoted" && *color == MUTED),
        );
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
        let colors = [
            LabelColor::Red,
            LabelColor::Blue,
            LabelColor::Green,
            LabelColor::Purple,
        ];
        for i in 0..24 {
            meta.labels.push(Label::new(
                format!("label number {i}"),
                colors[i % colors.len()],
            ));
        }

        let mut cases = Vec::new();
        let mut showcase = plain("Release 2.0 — planning overview", SHOWCASE.into());
        showcase.label_ids = meta.labels.iter().take(3).map(|l| l.id.clone()).collect();
        showcase.due_date = chrono::NaiveDate::from_ymd_opt(2030, 6, 30);
        showcase.checklist = [
            ("Freeze the feature list", true),
            ("Write the migration guide", true),
            ("Run the load test", false),
            ("Tag the release", false),
        ]
        .into_iter()
        .map(|(text, completed)| ChecklistItem {
            text: text.into(),
            completed,
        })
        .collect();
        cases.push(("00-showcase", meta.clone(), showcase));
        cases.push(("01-empty-card", meta.clone(), Card::new(String::new())));
        cases.push((
            "02-very-long-title",
            meta.clone(),
            plain(
                &"An exceedingly long card title that has to wrap over several lines ".repeat(6),
                "Short.".into(),
            ),
        ));
        cases.push((
            "03-unicode-and-missing-glyphs",
            meta.clone(),
            plain(
                "Größe, 日本語 and emoji 😀",
                "Latin: äöüß ÄÖÜ € — “quotes” … ✓ → ★\n\nThree dots... become an ellipsis, but `code...` and\n\n```\nfenced...\n```\n\nstay literal.\n\nMissing from the fonts (shown as ?): 日本語 العربية 🧑\u{200d}💻\n\ntab\there and a control\u{7}char".into(),
            ),
        ));
        cases.push((
            "04-unbreakable-words",
            meta.clone(),
            plain(
                "Unbreakable",
                format!(
                    "{}\n\n- {}\n\n> {}\n\n`{}`",
                    "x".repeat(300),
                    "y".repeat(260),
                    "z".repeat(260),
                    "c".repeat(200)
                ),
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
        let rows: Vec<String> = (0..150)
            .map(|i| format!("| item {i} | {} | {i}.50 |", "note ".repeat(i % 7 + 1)))
            .collect();
        cases.push((
            "06-long-table-repeats-header",
            meta.clone(),
            plain(
                "Long table",
                format!(
                    "| Item | Note | Price |\n|:--|:-:|--:|\n{}",
                    rows.join("\n")
                ),
            ),
        ));
        let mut hierarchy = String::from("# Overview\n\n");
        hierarchy.push_str(&lines("Overview line", 20));
        for chapter in 1..=3 {
            hierarchy.push_str(&format!("\n\n# Chapter {chapter}\n\n{}", lines("Intro", 4)));
            for part in 1..=3 {
                hierarchy.push_str(&format!(
                    "\n\n## Part {chapter}.{part}\n\n{}",
                    lines("Text", 14)
                ));
                hierarchy.push_str(&format!(
                    "\n\n### Detail {chapter}.{part}\n\n{}",
                    lines("Detail", 6)
                ));
            }
        }
        cases.push((
            "07-heading-hierarchy",
            meta.clone(),
            plain("Heading hierarchy", hierarchy),
        ));
        cases.push((
            "08-lists-and-quotes",
            meta.clone(),
            plain(
                "Lists and quotes",
                "- one\n  - two\n    - three\n      - four\n1. first\n2. second with a very long line that wraps around the page because it just keeps going and going and going\n> quote\n>> nested quote\n> > spaced nested quote\n\n>>>\nmultiline quote\n\nsecond paragraph of it\n>>>\n\n* star item\n10. ten".into(),
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
                text: if i == 3 {
                    "a long checklist item ".repeat(12)
                } else {
                    format!("item {i}")
                },
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
        let mut matrix = String::from(
            "A matrix has an empty top-left cell; its first column labels the rows.\n\n",
        );
        matrix.push_str("|  | A | B |\n|---|:-:|--:|\n| **x** | 1 | 2 |\n| y | 3 | 4 |\n\n");
        matrix.push_str("Whitespace-only corner and emphasis in the labels:\n\n|   | north | south | east | west |\n|---|---|---|---|---|\n| *italic* | 1 | 2 | 3 | 4 |\n| ***both*** | 5 | 6 | 7 | 8 |\n| `code` | 9 | 10 | 11 | 12 |\n\n");
        matrix.push_str("Wrapping labels and cells, and empty row labels:\n\n|  | Short | Long |\n|---|---|---|\n| a row label that is long enough to wrap inside its column | x | a cell that is long enough to wrap as well, over several lines |\n|  | no label | still fine |\n\n");
        matrix.push_str("Header-only matrix and a one-column matrix:\n\n|  | only header |\n|---|---|\n\n|  |\n|---|\n| lonely |\n\n");
        matrix.push_str("Many rows: a grid with row separators, repeating its header across pages:\n\n|  | Q1 | Q2 | Q3 | Q4 |\n|---|--:|--:|--:|--:|\n");
        for i in 0..90 {
            matrix.push_str(&format!(
                "| region {i} | {} | {} | {} | {} |\n",
                i,
                i * 2,
                i * 3,
                i * 4
            ));
        }
        cases.push(("14-matrix", meta.clone(), plain("Matrix tables", matrix)));
        cases.push((
            "15-alerts",
            meta.clone(),
            plain(
                "Alerts",
                "> [!note]\n> Useful information, with **bold** and `code`.\n\n> [!tip]\n> A tip that is long enough to wrap around the page because it just keeps going and going and going and going\n> - with a list\n\n>>> [!important]\nYou should consider the following ramifications:\n\n1. consideration 1\n2. consideration 2\n>>>\n\n> [!caution]\n> Careful.\n\n>>> [!warning]\nDanger!\n>>>\n\n> [!NOTE]\n\n> [!bogus]\n> stays an ordinary quote".into(),
            ),
        ));
        cases.push((
            "16-footnotes",
            meta.clone(),
            plain("Footnotes", footnote_example()),
        ));
        cases
    }

    /// Definitions first, references spread over several pages: a note is on
    /// the page of its reference, wherever it is defined.
    fn footnote_example() -> String {
        let mut text = String::from(
            "[^late]: Defined at the top, shown on the page of its reference.\n\
             [^1]: A numbered note.  \nWith a second line, after two trailing spaces.\n\
             [^fn-long]: A long note wraps over several lines inside the footnote area at the bottom of the page, set smaller than the body text.\n\
             [^unused]: Never referenced, so never printed.\n\n\
             A simple footnote[^1] and a named one[^late].\n\n",
        );
        for i in 0..40 {
            text.push_str(&format!(
                "Filler paragraph {i} pushes the next reference down.\n"
            ));
        }
        text.push_str("\nLater on[^fn-long], citing the first again[^1], and a missing one[^nope].\n\n| Cell | Note |\n|---|---|\n| a | with a note[^late] |\n\n- list item[^fn-long]\n\n> quote[^1]\n");
        text
    }

    const SHOWCASE: &str = "\
Short, readable summary of the release plan with **bold**, *italic* and `inline code`.

This second paragraph is deliberately a little longer, so that it wraps over several lines and the spacing between consecutive paragraphs can be judged: the gap should be clearly visible, yet smaller than the one above a heading.

A third paragraph follows directly. It is here to show that prose reads evenly, and that line spacing inside a paragraph differs from the space between paragraphs.

# Goals

The goals below are ordered by priority. This paragraph precedes a list, so the distance to the first bullet matters.

- Ship the new **export** pipeline
- Cut start-up time in half
  - measured on the reference laptop
  - reported in the release notes
1. Freeze
2. Test
3. Release

A paragraph directly after a list must not stick to its last item, and should not float away either.

> Scope is fixed; dates are not.

Prose after a quote, to judge the gap below it.

Footnotes[^scope] sit at the bottom of the page of their reference.

[^scope]: Defined anywhere in the text; set small below a rule.

> [!warning]
> The freeze date is firm.

Prose between two alerts and before the next block: it should neither touch the alert nor leave a gulf.

>>> [!tip]
A multiline alert may hold several paragraphs.

Like this one.
>>>

Text right below an alert, followed by a plain multiline quote.

>>>
A multiline quote spans several lines
without a `>` on each of them.

Even paragraphs.
>>>

# Timeline

The schedule has three phases. This paragraph introduces the table, so check the gap between the text and the table's top rule.

| Phase | Owner | Weeks |
|:--|:-:|--:|
| Design | Ada | 2 |
| Build | Grace | 6 |
| Verify | Linus | 3 |

The paragraph after a table follows the bottom rule at a comfortable distance, and is itself long enough to wrap onto a second line.

# Responsibilities

A matrix labels its rows as well as its columns (empty top-left cell):

|  | Design | Build | Verify |
|---|:-:|:-:|:-:|
| Ada | owner | review | — |
| Grace | review | owner | support |
| Linus | — | support | owner |

Closing prose after the matrix, before a heading and a code block.

# Notes

Run the following to package a release:

```
cargo build --release
./scripts/package.sh
```

That is all — a last paragraph after the code block.
";

    #[test]
    fn alerts_draw_a_bar_and_a_capitalized_title_in_their_color_or_black() {
        let (meta, card) = corner_cases()
            .into_iter()
            .find(|(n, ..)| *n == "15-alerts")
            .map(|(_, m, c)| (m, c))
            .unwrap();
        for theme in THEMES {
            let fonts = Fonts::load_for(theme).unwrap();
            let (pages, _) = layout(&fonts, &meta, "L", &card, chrono::Utc::now());
            let ops: Vec<&Op> = pages.iter().flatten().collect();
            let titles: Vec<(&str, Rgb)> = ops
                .iter()
                .filter_map(|op| match op {
                    Op::Text {
                        text, style, color, ..
                    } if style.bold
                        && text != "DESCRIPTION"
                        && text.chars().all(|c| c.is_ascii_uppercase()) =>
                    {
                        Some((text.as_str(), *color))
                    }
                    _ => None,
                })
                .collect();
            let names: Vec<&str> = titles.iter().map(|t| t.0).collect();
            assert_eq!(
                names,
                ["NOTE", "TIP", "IMPORTANT", "CAUTION", "WARNING", "NOTE"],
                "{theme:?}"
            );
            for ((_, color), kind) in titles.iter().zip([
                Alert::Note,
                Alert::Tip,
                Alert::Important,
                Alert::Caution,
                Alert::Warning,
                Alert::Note,
            ]) {
                assert_eq!(
                    *color,
                    if theme == Theme::Mono {
                        BLACK
                    } else {
                        kind.rgb()
                    }
                );
            }
            let icons: Vec<(Alert, Rgb)> = ops
                .iter()
                .filter_map(|op| {
                    if let Op::Icon { kind, color, .. } = op {
                        Some((*kind, *color))
                    } else {
                        None
                    }
                })
                .collect();
            assert_eq!(
                icons.iter().map(|i| i.0).collect::<Vec<_>>(),
                [
                    Alert::Note,
                    Alert::Tip,
                    Alert::Important,
                    Alert::Caution,
                    Alert::Warning,
                    Alert::Note
                ]
            );
            assert!(
                icons
                    .iter()
                    .all(|(k, c)| *c == if theme == Theme::Mono { BLACK } else { k.rgb() })
            );
            let bars = ops
                .iter()
                .filter(|op| matches!(op, Op::Line { width, .. } if *width == 2.5))
                .count();
            assert!(bars >= 12, "{theme:?}: {bars} bar segments");
            assert!(
                ops.iter()
                    .any(|op| matches!(op, Op::Text { text, .. } if text.contains("[!bogus]")))
            );
        }
    }

    #[test]
    fn list_markers_are_ink_except_in_quotes() {
        let meta = board();
        let mut card = Card::new("T".into());
        card.description = "- a\n1. b\n> alert\n\n> [!note]\n> - c\n> 2. d".into();
        for theme in THEMES {
            let fonts = Fonts::load_for(theme).unwrap();
            let ink = if theme == Theme::Mono { BLACK } else { INK };
            let (pages, _) = layout(&fonts, &meta, "L", &card, chrono::Utc::now());
            let color = |m: &str| {
                pages[0].iter().find_map(|op| match op {
                    Op::Text { text, color, .. } if text == m => Some(*color),
                    _ => None,
                })
            };
            for m in ["•", "1.", "2."] {
                assert_eq!(color(m), Some(ink), "{theme:?} {m}");
            }
        }
    }

    #[test]
    fn corner_cases_render_as_valid_pdf_a() {
        for theme in THEMES {
            for (name, meta, card) in corner_cases() {
                let pdf = render(&meta, "List", &card, theme)
                    .unwrap_or_else(|e| panic!("{name} {theme:?}: {e:#}"));
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
                            Op::Text {
                                x,
                                y,
                                text,
                                style,
                                size,
                                ..
                            } => {
                                let right = x + fonts.face(*style).width(text, *size);
                                assert!(
                                    *x >= MARGIN - 1.0 && right <= PAGE_W - MARGIN + 1.0,
                                    "{name} p{n}: '{text}' leaves the margins ({x}..{right})"
                                );
                                assert!(
                                    *y <= BOTTOM + 1.0,
                                    "{name} p{n}: '{text}' below the content area ({y})"
                                );
                            }
                            Op::Rect { x, w, y, h, .. } => {
                                assert!(
                                    *x >= MARGIN - 1.0 && x + w <= PAGE_W - MARGIN + 1.0,
                                    "{name} p{n}: rect leaves the margins"
                                );
                                assert!(
                                    y + h <= BOTTOM + 1.0,
                                    "{name} p{n}: rect below the content area"
                                );
                            }
                            Op::Icon { x, y, size, .. } => {
                                assert!(
                                    *x >= MARGIN - 1.0 && x + size <= PAGE_W - MARGIN + 1.0,
                                    "{name} p{n}: icon leaves the margins"
                                );
                                assert!(
                                    y + size <= BOTTOM + 1.0,
                                    "{name} p{n}: icon below the content area"
                                );
                            }
                            Op::Line { x1, x2, y1, y2, .. } => {
                                assert!(
                                    x1.min(*x2) >= MARGIN - 1.0
                                        && x1.max(*x2) <= PAGE_W - MARGIN + 1.0,
                                    "{name} p{n}: line leaves the margins"
                                );
                                assert!(
                                    y1.max(*y2) <= BOTTOM + 1.0,
                                    "{name} p{n}: line below the content area"
                                );
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

    /// Fixed "made at" time (2000-01-01 00:00 UTC) and fixed created/updated
    /// times of every example card, so regenerated examples only change with
    /// their content. Dates print in local time: regenerate with `TZ=UTC`.
    /// Fixed: the build's version is `snap-<today>` and would change every example daily.
    const EXAMPLE_VERSION: &str = "snap-2026-11-06";

    const EXAMPLE_TIME: chrono::DateTime<chrono::Utc> =
        chrono::DateTime::from_timestamp(946_684_800, 0).unwrap();

    fn at_example_time(mut card: Card) -> Card {
        card.created_at = chrono::DateTime::from_timestamp(946_022_400, 0).unwrap(); // 1999-12-24 08:00
        card.updated_at = chrono::DateTime::from_timestamp(946_684_740, 0).unwrap(); // 1999-12-31 23:59
        card
    }

    #[test]
    fn rendering_at_a_fixed_time_is_deterministic() {
        for (_, meta, card) in corner_cases().into_iter().take(3) {
            let card = at_example_time(card);
            let a = render_at(
                &meta,
                "To Do",
                &card,
                Theme::Color,
                EXAMPLE_TIME,
                EXAMPLE_VERSION,
            )
            .unwrap();
            let b = render_at(
                &meta,
                "To Do",
                &card,
                Theme::Color,
                EXAMPLE_TIME,
                EXAMPLE_VERSION,
            )
            .unwrap();
            assert!(
                a == b,
                "PDF output must not depend on anything but its input"
            );
        }
    }

    /// Regenerates `docs/export-examples/*.pdf`:
    /// `TZ=UTC cargo test write_example_pdfs -- --ignored`.
    #[test]
    #[ignore = "writes files into docs/"]
    fn write_example_pdfs() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/export-examples");
        for (theme, dir) in [
            (Theme::Mono, root.clone()),
            (Theme::Color, root.join("color")),
        ] {
            std::fs::create_dir_all(&dir).unwrap();
            for (name, meta, card) in corner_cases() {
                // Color is shown by the showcase and the alerts only; the rest are monochrome.
                if !name.starts_with("00-") && !name.starts_with("15-") && theme == Theme::Color {
                    continue;
                }
                let card = at_example_time(card);
                let pdf =
                    render_at(&meta, "To Do", &card, theme, EXAMPLE_TIME, EXAMPLE_VERSION).unwrap();
                std::fs::write(dir.join(format!("{name}.pdf")), pdf).unwrap();
            }
        }
    }

    // ── Footnotes ────────────────────────────────────────────────────

    /// Size of a reference in the text and of the marker in front of a note.
    const MARKER_SIZE: f32 = BODY * SUP_SCALE;

    /// Is this op the marker in front of a note (a raised run at the left margin
    /// inside the footnote area)?
    fn is_marker(page: &[Op], op: &Op) -> bool {
        matches!(op, Op::Text { x, y, style, .. }
            if style.sup && *x == MARGIN && note_area_top(page).is_some_and(|top| *y > top))
    }

    /// Baseline of the marker `label` of a note.
    fn marker_y(page: &[Op], label: &str) -> f32 {
        page.iter()
            .find_map(|op| match op {
                Op::Text { text, y, .. }
                    if is_marker(page, op) && *text == format!("[{label}]") =>
                {
                    Some(*y)
                }
                _ => None,
            })
            .unwrap_or_else(|| panic!("no marker {label}"))
    }

    /// Markers of the references drawn in the text of `page`.
    fn refs_on(page: &[Op]) -> Vec<String> {
        page.iter()
            .filter_map(|op| match op {
                Op::Text { text, style, .. } if style.sup && !is_marker(page, op) => {
                    // The reference in the text is bracketed: `[1]`.
                    Some(text.trim_matches(['[', ']']).to_string())
                }
                _ => None,
            })
            .collect()
    }

    /// Markers of the footnotes that start on `page`, in drawing order.
    fn markers_on(page: &[Op]) -> Vec<String> {
        page.iter()
            .filter_map(|op| match op {
                Op::Text { text, .. } if is_marker(page, op) => {
                    Some(text.trim_matches(['[', ']']).to_string())
                }
                _ => None,
            })
            .collect()
    }

    /// Baseline and x of the first text `text` drawn at `size`.
    fn text_at(page: &[Op], text: &str, size: f32) -> (f32, f32) {
        page.iter()
            .find_map(|op| match op {
                Op::Text {
                    text: t,
                    x,
                    y,
                    size: s,
                    ..
                } if t == text && *s == size => Some((*y, *x)),
                _ => None,
            })
            .unwrap_or_else(|| panic!("{text} not drawn at {size}"))
    }

    /// Top of the footnote area: the short rule above it.
    fn note_area_top(page: &[Op]) -> Option<f32> {
        page.iter().find_map(|op| match op {
            Op::Line { x1, x2, y1, .. } if (x2 - x1 - CONTENT_W / 4.0).abs() < 0.01 => Some(*y1),
            _ => None,
        })
    }

    /// The invariants of footnote layout, for every page: each reference has
    /// its note on the same page (`whole`: no note is long enough to carry
    /// over), notes are in order, and the footnote area never overlaps the body.
    fn assert_notes_consistent(pages: &[Vec<Op>], whole: bool, name: &str) {
        for (n, page) in pages.iter().enumerate() {
            let markers = markers_on(page);
            let top = note_area_top(page);
            assert_eq!(
                top.is_some(),
                !markers.is_empty() || {
                    // A page of continuation lines has an area but no marker.
                    top.is_some()
                },
                "{name} p{n}"
            );
            if whole {
                for r in refs_on(page) {
                    assert!(
                        markers.contains(&r),
                        "{name} p{n}: note {r} is not on its page"
                    );
                }
            }
            assert!(
                markers
                    .iter()
                    .all(|m| markers.iter().filter(|o| *o == m).count() == 1),
                "{name} p{n}: notes out of order"
            );
            if let Some(top) = top {
                for op in page {
                    if let Op::Text { y, size, style, .. } = op
                        && (*size == BODY || (style.sup && !is_marker(page, op)))
                    {
                        assert!(*y < top, "{name} p{n}: body text runs into the notes");
                    }
                }
            }
        }
    }

    fn note_card(description: String) -> Card {
        let mut card = Card::new("T".into());
        card.description = description;
        card
    }

    #[test]
    fn a_note_is_on_the_page_of_its_reference_wherever_it_is_defined() {
        for definition_first in [true, false] {
            let body = format!("{}\n\nsee[^a] here\n", lines("filler", 60));
            let def = "[^a]: The note text.";
            let text = if definition_first {
                format!("{def}\n\n{body}")
            } else {
                format!("{body}\n{def}")
            };
            let pages = laid_out(&note_card(text));
            assert!(pages.len() >= 2);
            let reference = page_of(&pages, "here");
            assert!(reference > 0);
            assert_eq!(page_of(&pages, "The note text."), reference);
            assert_eq!(refs_on(&pages[reference]), ["a"]);
            assert_notes_consistent(&pages, true, "placement");
        }
    }

    #[test]
    fn the_reference_never_ends_up_apart_from_its_note_at_a_page_boundary() {
        // Sweep the reference line across the bottom of page one.
        let (mut first, mut second) = (false, false);
        for filler in 38..=62 {
            let text = format!(
                "{}\n\nlast line[^1]\n\n[^1]: Boundary note.",
                lines("filler", filler)
            );
            let pages = laid_out(&note_card(text));
            assert_notes_consistent(&pages, true, &format!("filler {filler}"));
            let page = page_of(&pages, "last line");
            assert_eq!(page_of(&pages, "Boundary note."), page, "filler {filler}");
            first |= page == 0;
            second |= page == 1;
        }
        assert!(first && second, "the sweep must cover both outcomes");
    }

    #[test]
    fn named_notes_keep_their_name_and_are_drawn_in_order_of_first_reference() {
        let pages = laid_out(&note_card(
            "[^alpha]: Alpha\n[^b]: Beta\n\nx[^b] y[^alpha] z[^b]\n".into(),
        ));
        assert_eq!(refs_on(&pages[0]), ["b", "alpha", "b"]);
        assert_eq!(markers_on(&pages[0]), ["b", "alpha"]);
        let y = |t: &str| text_at(&pages[0], t, NOTE).0;
        let marker = |t: &str| marker_y(&pages[0], t);
        assert!((y("Beta") - marker("b") - BODY * 0.35).abs() < 0.01);
        assert!(y("Beta") < marker("alpha"));
        assert!((y("Alpha") - marker("alpha") - BODY * 0.35).abs() < 0.01);
    }

    #[test]
    fn numbered_notes_count_up_by_first_reference_whatever_their_labels() {
        let pages = laid_out(&note_card(
            "[^3]: Three\n[^7]: Seven\n[^x]: Named\n\na[^7] b[^x] c[^3] d[^7]\n".into(),
        ));
        assert_eq!(refs_on(&pages[0]), ["1", "x", "2", "1"]);
        assert_eq!(markers_on(&pages[0]), ["1", "x", "2"]);
        let shown = texts(&pages[0]);
        let at = |t: &str| shown.iter().position(|s| *s == t).unwrap();
        assert!(at("Seven") < at("Named") && at("Named") < at("Three"));
    }

    #[test]
    fn references_in_the_text_and_markers_in_front_of_notes_are_both_bracketed() {
        let pages = laid_out(&note_card("a[^1] b[^name]\n\n[^1]: A\n[^name]: B".into()));
        let page = &pages[0];
        let raised = |marker: bool| -> Vec<&str> {
            page.iter()
                .filter_map(|op| match op {
                    Op::Text { text, style, .. } if style.sup && is_marker(page, op) == marker => {
                        Some(text.as_str())
                    }
                    _ => None,
                })
                .collect()
        };
        assert_eq!(raised(false), ["[1]", "[name]"]);
        assert_eq!(raised(true), ["[1]", "[name]"]);
    }

    #[test]
    fn markers_are_raised_black_and_without_a_dot() {
        for theme in THEMES {
            let fonts = Fonts::load_for(theme).unwrap();
            let card = note_card("x[^a] y[^2]\n\n[^a]: A\n[^2]: B".into());
            let (pages, _) = layout(&fonts, &board(), "L", &card, chrono::Utc::now());
            let markers: Vec<(&str, Rgb)> = pages[0]
                .iter()
                .filter_map(|op| match op {
                    Op::Text {
                        text,
                        size,
                        style,
                        color,
                        ..
                    } if is_marker(&pages[0], op) && *size == MARKER_SIZE && style.sup => {
                        Some((text.as_str(), *color))
                    }
                    _ => None,
                })
                .collect();
            assert_eq!(markers, [("[a]", BLACK), ("[1]", BLACK)], "{theme:?}");
            assert!(
                !texts(&pages[0])
                    .iter()
                    .any(|t| t.ends_with('.') && t.len() <= 2)
            );
        }
    }

    #[test]
    fn all_note_text_shares_one_indent_set_by_the_longest_marker() {
        let long = "a-rather-long-footnote-name";
        let text = format!("a[^1] b[^{long}] c[^2]\n\n[^1]: One\n[^{long}]: Long\n[^2]: Two");
        let pages = laid_out(&note_card(text));
        let xs: Vec<f32> = ["One", "Long", "Two"]
            .iter()
            .map(|t| text_at(&pages[0], t, NOTE).1)
            .collect();
        assert!(xs.windows(2).all(|w| (w[0] - w[1]).abs() < 0.01), "{xs:?}");
        let fonts = Fonts::load().unwrap();
        let marker_end = MARGIN + fonts.sans.width(&format!("[{long}]"), MARKER_SIZE);
        assert!(xs[0] > marker_end, "text starts behind the longest marker");
        // Short markers alone need a much smaller indent.
        let pages = laid_out(&note_card("a[^1]\n\n[^1]: One".into()));
        assert!(text_at(&pages[0], "One", NOTE).1 < xs[0] - 50.0);
    }

    #[test]
    fn each_page_indents_by_its_own_longest_marker() {
        let long = "name-long-enough";
        let text = format!(
            "a[^{long}]\n\n{}\n\nb[^1]\n\n[^{long}]: Alpha\n[^1]: Beta",
            lines("filler", 60)
        );
        let pages = laid_out(&note_card(text));
        let (a, b) = (page_of(&pages, "Alpha"), page_of(&pages, "Beta"));
        assert_ne!(a, b);
        let (long_x, short_x) = (
            text_at(&pages[a], "Alpha", NOTE).1,
            text_at(&pages[b], "Beta", NOTE).1,
        );
        let fonts = Fonts::load().unwrap();
        assert!(long_x > MARGIN + fonts.sans.width(&format!("[{long}]"), MARKER_SIZE));
        assert!(short_x < long_x - 40.0, "{short_x} vs {long_x}");
    }

    #[test]
    fn notes_are_set_apart_and_the_rule_is_visible() {
        let pages = laid_out(&note_card("a[^1] b[^2]\n\n[^1]: A1  \nA2\n[^2]: B".into()));
        let y = |t: &str| text_at(&pages[0], t, NOTE).0;
        assert!((y("A2") - y("A1") - NOTE_LINE).abs() < 0.01);
        assert!((y("B") - y("A2") - NOTE_LINE - NOTE_BETWEEN).abs() < 0.01);
        let rule = pages[0].iter().find_map(|op| match op {
            Op::Line { color, x1, x2, .. } if (x2 - x1 - CONTENT_W / 4.0).abs() < 0.01 => {
                Some(*color)
            }
            _ => None,
        });
        assert_eq!(rule, Some(MUTED));
    }

    #[test]
    fn markers_are_as_large_as_references_and_sit_on_the_text_baseline() {
        let pages = laid_out(&note_card("a[^1]\n\n[^1]: Note".into()));
        let sizes: Vec<f32> = pages[0]
            .iter()
            .filter_map(|op| match op {
                Op::Text { style, size, .. } if style.sup => Some(*size),
                _ => None,
            })
            .collect();
        assert_eq!(sizes, [MARKER_SIZE, MARKER_SIZE]);
        // Raised by the same fraction as in the text (the baseline lift is 0.35 em).
        let lift = text_at(&pages[0], "Note", NOTE).0 - marker_y(&pages[0], "1");
        assert!((lift - BODY * 0.35).abs() < 0.01, "{lift}");
    }

    #[test]
    fn multi_line_notes_keep_their_lines_and_unreferenced_ones_are_dropped() {
        let pages = laid_out(&note_card(
            "a[^m]\n\n[^m]: First line  \nsecond line\n[^dead]: unreferenced".into(),
        ));
        let shown = texts(&pages[0]);
        assert!(shown.contains(&"First line") && shown.contains(&"second line"));
        assert!(!shown.iter().any(|t| t.contains("unreferenced")));
        assert!(!shown.iter().any(|t| t.contains("[^")));
        // The definitions left no gap or text in the body.
        assert_eq!(count(&pages[0], "a"), 1);
    }

    #[test]
    fn an_indented_paragraph_after_a_blank_line_belongs_to_the_note() {
        let pages = laid_out(&note_card("a[^1]\n\n[^1]: one\n\n    two\n\nafter".into()));
        let shown = texts(&pages[0]);
        assert!(shown.contains(&"two") && shown.contains(&"after"));
        let top = note_area_top(&pages[0]).unwrap();
        let y = |t: &str| {
            pages[0]
                .iter()
                .find_map(|op| match op {
                    Op::Text { text, y, .. } if text == t => Some(*y),
                    _ => None,
                })
                .unwrap()
        };
        assert!(y("two") > top && y("after") < top);
    }

    #[test]
    fn unresolved_references_and_code_stay_literal() {
        let pages = laid_out(&note_card(
            "missing[^zz] and `code[^1]`\n```\n[^1]: not a definition\n```\nreal[^1]\n\n[^1]: Real"
                .into(),
        ));
        let shown = texts(&pages[0]).join("|");
        assert!(shown.contains("missing[^zz]"));
        assert!(shown.contains("code[^1]"));
        assert!(shown.contains("[^1]: not a definition"));
        assert_eq!(refs_on(&pages[0]), ["1"]);
        assert_eq!(markers_on(&pages[0]), ["1"]);
    }

    #[test]
    fn a_note_referenced_on_several_pages_is_repeated_on_each() {
        let text = format!(
            "[^r]: Repeated\n\nfirst[^r]\n\n{}\n\nsecond[^r]\n",
            lines("filler", 60)
        );
        let pages = laid_out(&note_card(text));
        let with_ref: Vec<usize> = (0..pages.len())
            .filter(|&p| !refs_on(&pages[p]).is_empty())
            .collect();
        assert_eq!(with_ref.len(), 2);
        for p in with_ref {
            assert_eq!(markers_on(&pages[p]), ["r"]);
            assert_eq!(count(&pages[p], "Repeated"), 1);
        }
        assert_notes_consistent(&pages, true, "repeat");
    }

    #[test]
    fn references_in_headings_lists_quotes_alerts_and_tables_get_their_notes() {
        let text = "# Head[^1]\n- item[^2]\n> quote[^3]\n> [!note]\n> alert[^4]\n\n| H[^5] | x |\n|---|---|\n| c[^6] | y |\n\n\
                    [^1]: n1\n[^2]: n2\n[^3]: n3\n[^4]: n4\n[^5]: n5\n[^6]: n6\n";
        let pages = laid_out(&note_card(text.into()));
        let expected = ["1", "2", "3", "4", "5", "6"];
        assert_eq!(refs_on(&pages[0]), expected);
        assert_eq!(markers_on(&pages[0]), expected);
        assert_notes_consistent(&pages, true, "blocks");
    }

    #[test]
    fn a_long_table_repeats_header_references_and_keeps_row_notes_on_their_page() {
        let mut text = String::from("| H[^h] | x |\n|---|---|\n");
        for i in 0..90 {
            text.push_str(&format!("| row {i}[^{}] | v |\n", i % 7));
        }
        for label in std::iter::once("h".to_string()).chain((0..7).map(|i| i.to_string())) {
            text.push_str(&format!("\n[^{label}]: Note {label}"));
        }
        let pages = laid_out(&note_card(text));
        assert!(pages.len() >= 2);
        assert_notes_consistent(&pages, true, "table");
        for page in &pages {
            // The repeated header carries its reference, so its note is there too.
            assert!(refs_on(page).contains(&"h".to_string()));
        }
    }

    #[test]
    fn a_note_longer_than_its_page_budget_continues_on_the_next_pages() {
        let long: String = (0..90).map(|i| format!("\nnote line {i}")).collect();
        let text = format!(
            "[^1]: start{long}\n\nref[^1] and after\n\n{}",
            lines("body", 70)
        );
        let pages = laid_out(&note_card(text));
        assert_notes_consistent(&pages, false, "long");
        assert!(pages.len() >= 3);
        // Every line is drawn once, in order, and only the first part has the marker.
        let mut seen = Vec::new();
        for page in &pages {
            for t in texts(page) {
                if let Some(n) = t.strip_prefix("note line ") {
                    seen.push(n.parse::<usize>().unwrap());
                }
            }
        }
        assert_eq!(seen, (0..90).collect::<Vec<_>>());
        let marker_pages: Vec<usize> = (0..pages.len())
            .filter(|&p| markers_on(&pages[p]).contains(&"1".to_string()))
            .collect();
        assert_eq!(marker_pages, [page_of(&pages, "ref")]);
        // The area never takes more than its budget.
        for page in &pages {
            if let Some(top) = note_area_top(page) {
                assert!(BOTTOM - top <= NOTE_BUDGET + 1.0);
            }
        }
    }

    #[test]
    fn a_note_that_ends_the_document_on_a_carried_page_is_still_drawn() {
        let long: String = (0..60).map(|i| format!("\nend line {i}")).collect();
        let pages = laid_out(&note_card(format!("ref[^1]\n\n[^1]: head{long}")));
        let drawn: usize = pages
            .iter()
            .map(|p| {
                texts(p)
                    .iter()
                    .filter(|t| t.starts_with("end line"))
                    .count()
            })
            .sum();
        assert_eq!(drawn, 60);
        assert!(pages.iter().all(|p| !p.is_empty()));
    }

    #[test]
    fn many_references_in_one_paragraph_never_overflow_the_page() {
        let refs: String = (0..12).map(|i| format!("w{i}[^{i}] ")).collect();
        let defs: String = (0..12)
            .map(|i| format!("\n[^{i}]: Note {i} with a few words so that it takes a little room"))
            .collect();
        let pages = laid_out(&note_card(format!("{refs}\n{defs}")));
        assert_notes_consistent(&pages, true, "many");
        assert_eq!(notes_drawn(&pages, 12), 12);
    }

    fn notes_drawn(pages: &[Vec<Op>], n: usize) -> usize {
        (0..n)
            .filter(|i| {
                pages.iter().any(|p| {
                    texts(p)
                        .iter()
                        .any(|t| t.starts_with(&format!("Note {i} ")))
                })
            })
            .count()
    }

    #[test]
    fn more_notes_than_a_page_budget_spill_onto_following_pages_without_loss() {
        let refs: String = (0..40).map(|i| format!("w{i}[^{i}] ")).collect();
        let defs: String = (0..40)
            .map(|i| format!("\n[^{i}]: Note {i} with a few words so that it takes a little room"))
            .collect();
        let pages = laid_out(&note_card(format!("{refs}\n{defs}")));
        assert_notes_consistent(&pages, false, "spill");
        assert_eq!(notes_drawn(&pages, 40), 40);
        assert!(pages.len() >= 2);
    }

    #[test]
    fn a_page_of_only_definitions_leaves_no_description() {
        let pages = laid_out(&note_card("[^1]: lonely".into()));
        assert_eq!(pages.len(), 1);
        assert!(!texts(&pages[0]).iter().any(|t| t.contains("lonely")));
        assert!(
            !texts(&pages[0])
                .iter()
                .any(|t| t.eq_ignore_ascii_case("description"))
        );
    }

    #[test]
    fn references_are_raised_and_smaller_than_the_text() {
        let pages = laid_out(&note_card("word[^1]\n\n[^1]: n".into()));
        let (mut text_y, mut ref_y, mut ref_size) = (0.0, 0.0, 0.0);
        for op in &pages[0] {
            if let Op::Text {
                text,
                y,
                size,
                style,
                ..
            } = op
            {
                if text == "word" {
                    text_y = *y;
                } else if style.sup && text == "[1]" && !is_marker(&pages[0], op) {
                    (ref_y, ref_size) = (*y, *size);
                }
            }
        }
        assert!(ref_y < text_y && ref_size < BODY);
    }

    #[test]
    fn footnote_pdfs_stay_deterministic_across_themes() {
        let (meta, card) = corner_cases()
            .into_iter()
            .find(|(n, ..)| *n == "16-footnotes")
            .map(|(_, m, c)| (m, c))
            .unwrap();
        let positions = |theme| {
            let fonts = Fonts::load_for(theme).unwrap();
            let (pages, _) = layout(&fonts, &meta, "L", &card, chrono::Utc::now());
            assert_notes_consistent(&pages, true, "example");
            pages
                .iter()
                .map(|p| {
                    p.iter()
                        .filter_map(|op| match op {
                            Op::Text { text, x, y, .. } => {
                                Some((text.clone(), *x as i32, *y as i32))
                            }
                            _ => None,
                        })
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(positions(Theme::Mono), positions(Theme::Color));
    }
}
