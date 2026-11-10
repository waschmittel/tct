use super::*;
use crate::export::blocks::{Run, Style};

impl<'f> Flow<'f> {
    pub(super) fn atoms(&self, runs: &[Run], size: f32) -> Vec<Atom> {
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

    pub(super) fn wrap(&self, runs: &[Run], size: f32, width: f32) -> Vec<Vec<Seg>> {
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

    pub(super) fn draw_line(
        &mut self,
        segs: &[Seg],
        x: f32,
        top: f32,
        size: f32,
        color: Rgb,
        shift: f32,
    ) {
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

    pub(super) fn paragraph(&mut self, runs: &[Run], size: f32, x: f32, width: f32, color: Rgb) {
        let height = size * LEADING;
        for line in self.wrap(runs, size, width) {
            self.need_notes(height, &self.refs(&line));
            self.draw_line(&line, x, self.y, size, color, 0.0);
            self.y += height;
        }
    }

    pub(super) fn rule(&mut self, color: Rgb, width: f32) {
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

    pub(super) fn section_title(&mut self, title: &str) {
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
}
