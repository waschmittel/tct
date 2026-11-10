use super::*;
use crate::export::blocks::Style;

impl<'f> Flow<'f> {
    /// Height of a footnote area: gap, lines, and the space between notes.
    pub(super) fn notes_height(items: &[NoteItem]) -> f32 {
        if items.is_empty() {
            return 0.0;
        }
        let lines: usize = items.iter().map(|i| i.lines.len()).sum();
        NOTE_GAP + lines as f32 * NOTE_LINE + (items.len() - 1) as f32 * NOTE_BETWEEN
    }

    /// Space a note takes above its first line: the gap to the body for the
    /// first note of a page, the space between notes for the others.
    pub(super) fn note_head(first: bool) -> f32 {
        if first { NOTE_GAP } else { NOTE_BETWEEN }
    }

    pub(super) fn lines_fitting(height: f32) -> usize {
        (height / NOTE_LINE).floor().max(0.0) as usize
    }

    /// Wrapped lines of footnote `num`; every source line starts a new line.
    pub(super) fn note_lines(&self, num: usize) -> Vec<Vec<Seg>> {
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
    pub(super) fn refs(&self, line: &[Seg]) -> Vec<usize> {
        line.iter()
            .filter(|s| s.style.sup)
            .filter_map(|s| {
                let label = s.text.strip_prefix('[')?.strip_suffix(']')?;
                self.notes.iter().position(|n| n.label == label)
            })
            .map(|p| p + 1)
            .collect()
    }

    pub(super) fn row_refs(&self, (wrapped, _): &(Vec<Vec<Vec<Seg>>>, f32)) -> Vec<usize> {
        wrapped
            .iter()
            .flatten()
            .flat_map(|l| self.refs(l))
            .collect()
    }

    /// The notes of `nums` the current page does not show yet.
    pub(super) fn new_notes(&self, nums: &[usize]) -> Vec<usize> {
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
    pub(super) fn fits(&self, height: f32, nums: &[usize]) -> bool {
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
    pub(super) fn register(&mut self, height: f32, nums: &[usize]) {
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
    pub(super) fn need_notes(&mut self, height: f32, nums: &[usize]) {
        while !self.fits(height, nums) && self.y > MARGIN {
            self.new_page();
        }
        self.register(height, nums);
    }

    /// Finish the document: flush carried note lines onto pages of their own
    /// and draw each page's footnote area above the footer.
    pub(super) fn finish(mut self) -> Vec<Vec<Op>> {
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
}
