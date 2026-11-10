//! Card → PDF/A-2b: lay the card out into pages of draw operations, then
//! emit them with krilla, which validates the result against the standard.
//!
//! PDF/A requires every font to be embedded, so text is set in the bundled
//! DejaVu faces; characters they lack are replaced with `?` instead of
//! drawing `.notdef`.

use std::rc::Rc;

use super::Theme;
use super::blocks::{Note, Style};
use super::lang::Lang;
use crate::alert::Alert;
use crate::insert::table::Align;
use crate::model::board::BoardMeta;
use crate::model::card::Card;
use chrome::*;
use emit::*;
use fonts::*;
use layout::*;

mod body;
mod card;
mod chrome;
mod emit;
mod flow;
mod fonts;
mod layout;
mod notes;
mod table;
#[cfg(test)]
mod tests;
mod text;

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
    /// Language of the document: typographic quotes in table cells and checklist.
    lang: Lang,
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
    emit(&pages, &fonts, board, list, card, &labels, (now, version))
}
