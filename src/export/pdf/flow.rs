use std::rc::Rc;

use super::*;
use crate::export::blocks::{Note, Style};
use crate::export::lang::Lang;

impl<'f> Flow<'f> {
    pub(super) fn mono(&self) -> bool {
        self.fonts.mono_theme()
    }

    pub(super) fn ink(&self) -> Rgb {
        if self.mono() { BLACK } else { INK }
    }

    /// Grays (secondary text, light rules, code tint) are shared by both
    /// themes; only chromatic things differ.
    pub(super) fn muted(&self) -> Rgb {
        MUTED
    }

    pub(super) fn rule_color(&self) -> Rgb {
        RULE
    }

    pub(super) fn new(fonts: &'f Fonts) -> Self {
        Self::with_notes(fonts, Rc::default())
    }

    pub(super) fn with_notes(fonts: &'f Fonts, notes: Rc<Vec<Note>>) -> Self {
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
            lang: Lang::default(),
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
    pub(super) fn keep_together(&mut self, build: &dyn Fn(&mut Flow<'f>)) {
        self.keep_together_up_to(f32::MAX, build);
    }

    /// Like [`keep_together`](Self::keep_together), but only for output at
    /// most `max` tall: big blocks split instead of leaving a large gap.
    pub(super) fn keep_together_up_to(&mut self, max: f32, build: &dyn Fn(&mut Flow<'f>)) {
        if self.measuring {
            return;
        }
        let mut scratch = Flow {
            measuring: true,
            lang: self.lang,
            ..Flow::with_notes(self.fonts, self.notes.clone())
        };
        build(&mut scratch);
        let height = scratch.y - MARGIN + scratch.reserved();
        if scratch.pages.len() == 1 && height <= max {
            self.need(height);
        }
    }

    pub(super) fn push(&mut self, op: Op) {
        self.pages
            .last_mut()
            .expect("a page always exists")
            .push(op);
    }

    pub(super) fn need(&mut self, height: f32) {
        if self.y + height > self.bottom() && self.y > MARGIN {
            self.new_page();
        }
    }

    /// Where content must end on this page: above its footnotes.
    pub(super) fn bottom(&self) -> f32 {
        BOTTOM - self.reserved()
    }

    pub(super) fn reserved(&self) -> f32 {
        Self::notes_height(self.page_notes.last().expect("a page always exists"))
    }

    pub(super) fn new_page(&mut self) {
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
}
