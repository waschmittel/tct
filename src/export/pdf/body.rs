use super::*;
use crate::export::blocks::{Block, InAlert, Run, Style};

impl<'f> Flow<'f> {
    /// The description under its section title: the title is kept with the
    /// first heading's section, and a title is never left at a page bottom.
    pub(super) fn description(&mut self, blocks: &[Block]) {
        let first = &blocks[..section_end(blocks, 0).max(1).min(blocks.len())];
        self.keep_together(&|f| {
            f.section_title(f.lang.label(Label::Description));
            f.blocks_inner(first, true);
        });
        self.section_title(self.lang.label(Label::Description));
        self.blocks_inner(blocks, true);
    }

    /// Headings plan page breaks hierarchically: a heading's whole section
    /// starts on a fresh page if it would otherwise be split and fits one
    /// page; sections longer than a page are not broken early, but the
    /// sub-sections inside them get the same treatment one level down.
    pub(super) fn blocks_inner(&mut self, blocks: &[Block], title_above: bool) {
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
    pub(super) fn quoted(&self, runs: &[Run], quote_depth: usize) -> Vec<Run> {
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

    pub(super) fn text_blocks(&mut self, blocks: &[Block]) {
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
    pub(super) fn text_geometry(
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
    pub(super) fn text_block(
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
    pub(super) fn lines_to_keep(i: usize, n: usize) -> usize {
        if i == 0 || (n >= 4 && i + 2 == n) {
            n.min(2)
        } else {
            1
        }
    }

    /// Code keeps its line structure; lines wider than the block are broken
    /// at the margin rather than clipped, so no content is lost. The block is
    /// tinted gray in both themes.
    pub(super) fn code_block(&mut self, lines: &[String]) {
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
}
