use super::*;
use crate::export::blocks::{self, Run, Style};
use crate::model::card::Card;

impl<'f> Flow<'f> {
    pub(super) fn chips(&mut self, chips: &[(String, Rgb)]) {
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

    pub(super) fn checklist(&mut self, card: &Card) {
        let height = BODY * LEADING;
        let x = MARGIN + 18.0;
        for item in &card.checklist {
            let runs = blocks::inline(&item.text, Style::default(), self.lang);
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
