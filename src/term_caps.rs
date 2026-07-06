//! Terminal capability detection and degradation.
//!
//! The UI is authored for a modern truecolor emulator; limited terminals —
//! chiefly the Linux console (`TERM=linux`) — get a degraded rendering:
//! RGB colors are quantized to a palette the terminal can show, and glyphs
//! missing from console fonts (heavy box drawing, check marks) are swapped
//! for ones that exist. Detection runs once at startup (`TermCaps::detect`);
//! everything else keys off the resulting value, so tests can pin either
//! tier explicitly.

use ratatui::buffer::Buffer;
use ratatui::style::Color;
use ratatui::widgets::BorderType;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorSupport {
    /// 24-bit RGB passes through unchanged.
    TrueColor,
    /// RGB is quantized to the xterm 256-color palette (emulators that
    /// don't advertise truecolor via `COLORTERM`).
    Ansi256,
    /// Everything is quantized to the 16 ANSI colors (Linux console — the
    /// kernel VT maps richer SGR sequences to its 16-color palette itself,
    /// collapsing pastels to white and dark backgrounds to black).
    Ansi16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TermCaps {
    pub color: ColorSupport,
    /// Whether the terminal font has glyphs beyond the CP437-ish console
    /// repertoire (heavy box drawing, ✓). False on the Linux console.
    pub rich_glyphs: bool,
}

impl TermCaps {
    /// Modern emulator: truecolor, full glyph repertoire.
    pub fn full() -> Self {
        Self { color: ColorSupport::TrueColor, rich_glyphs: true }
    }

    /// Linux console: 16 colors, console-font glyphs only.
    pub fn linux_tty() -> Self {
        Self { color: ColorSupport::Ansi16, rich_glyphs: false }
    }

    /// Detect from the environment. `TERM=linux` wins (the console cannot
    /// be upgraded by `COLORTERM`); otherwise `COLORTERM` advertising
    /// truecolor selects full support, and anything else gets the safe
    /// 256-color middle tier.
    pub fn detect() -> Self {
        Self::from_env(
            std::env::var("TERM").ok().as_deref(),
            std::env::var("COLORTERM").ok().as_deref(),
        )
    }

    fn from_env(term: Option<&str>, colorterm: Option<&str>) -> Self {
        if term == Some("linux") {
            return Self::linux_tty();
        }
        let truecolor = colorterm
            .map(|c| c.contains("truecolor") || c.contains("24bit"))
            .unwrap_or(false);
        Self {
            color: if truecolor { ColorSupport::TrueColor } else { ColorSupport::Ansi256 },
            rich_glyphs: true,
        }
    }

    /// Background for the selected card. On the bottom tier the highlight
    /// is pinned to a palette color explicitly instead of trusting the
    /// quantizer to keep the subtle truecolor tone distinguishable from
    /// the plain black background.
    pub fn selection_bg(&self) -> Color {
        match self.color {
            ColorSupport::Ansi16 => Color::DarkGray,
            _ => Color::Rgb(40, 40, 55),
        }
    }

    /// Border of the selected card. Heavy box-drawing glyphs are missing
    /// from console fonts; double-line glyphs are in every VGA font.
    pub fn selected_border_type(&self) -> BorderType {
        if self.rich_glyphs { BorderType::Thick } else { BorderType::Double }
    }

    /// Checklist "done" marker.
    pub fn check_mark(&self) -> &'static str {
        if self.rich_glyphs { "✓" } else { "x" }
    }

    /// Reduce a single color to what the terminal can display.
    pub fn adapt_color(&self, color: Color) -> Color {
        match (self.color, color) {
            (ColorSupport::TrueColor, c) => c,
            (ColorSupport::Ansi256, Color::Rgb(r, g, b)) => {
                Color::Indexed(nearest_indexed(r, g, b))
            }
            (ColorSupport::Ansi256, c) => c,
            (ColorSupport::Ansi16, Color::Rgb(r, g, b)) => nearest_ansi16(r, g, b),
            (ColorSupport::Ansi16, Color::Indexed(i)) => {
                let (r, g, b) = indexed_to_rgb(i);
                nearest_ansi16(r, g, b)
            }
            (ColorSupport::Ansi16, c) => c,
        }
    }

    /// Post-process a rendered frame: quantize every cell's colors. Doing
    /// this on the finished buffer keeps the hundreds of style sites in the
    /// UI unaware of terminal tiers (only styles whose *quantized* form is
    /// unusable — see `selection_bg` — need site-level handling).
    pub fn adapt_buffer(&self, buf: &mut Buffer) {
        if self.color == ColorSupport::TrueColor {
            return;
        }
        for cell in buf.content.iter_mut() {
            cell.fg = self.adapt_color(cell.fg);
            cell.bg = self.adapt_color(cell.bg);
        }
    }
}

/// The 16 ANSI colors with their conventional VGA palette values, used as
/// quantization targets on 16-color terminals.
const ANSI16: [(Color, (u8, u8, u8)); 16] = [
    (Color::Black, (0, 0, 0)),
    (Color::Red, (170, 0, 0)),
    (Color::Green, (0, 170, 0)),
    (Color::Yellow, (170, 85, 0)),
    (Color::Blue, (0, 0, 170)),
    (Color::Magenta, (170, 0, 170)),
    (Color::Cyan, (0, 170, 170)),
    (Color::Gray, (170, 170, 170)),
    (Color::DarkGray, (85, 85, 85)),
    (Color::LightRed, (255, 85, 85)),
    (Color::LightGreen, (85, 255, 85)),
    (Color::LightYellow, (255, 255, 85)),
    (Color::LightBlue, (85, 85, 255)),
    (Color::LightMagenta, (255, 85, 255)),
    (Color::LightCyan, (85, 255, 255)),
    (Color::White, (255, 255, 255)),
];

fn dist2(a: (u8, u8, u8), b: (u8, u8, u8)) -> u32 {
    let dr = a.0 as i32 - b.0 as i32;
    let dg = a.1 as i32 - b.1 as i32;
    let db = a.2 as i32 - b.2 as i32;
    (dr * dr + dg * dg + db * db) as u32
}

fn nearest_ansi16(r: u8, g: u8, b: u8) -> Color {
    ANSI16
        .iter()
        .min_by_key(|(_, rgb)| dist2((r, g, b), *rgb))
        .map(|(c, _)| *c)
        .unwrap()
}

/// Channel levels of the xterm 6×6×6 color cube (indices 16–231).
const CUBE_LEVELS: [u8; 6] = [0, 95, 135, 175, 215, 255];

fn nearest_cube_level(v: u8) -> usize {
    CUBE_LEVELS
        .iter()
        .enumerate()
        .min_by_key(|&(_, &l)| (v as i32 - l as i32).abs())
        .map(|(i, _)| i)
        .unwrap()
}

/// Nearest xterm-256 index: best of the 6×6×6 cube and the grayscale ramp
/// (232–255, values 8..=238 in steps of 10).
fn nearest_indexed(r: u8, g: u8, b: u8) -> u8 {
    let (qr, qg, qb) = (nearest_cube_level(r), nearest_cube_level(g), nearest_cube_level(b));
    let cube_idx = (16 + 36 * qr + 6 * qg + qb) as u8;
    let cube_rgb = (CUBE_LEVELS[qr], CUBE_LEVELS[qg], CUBE_LEVELS[qb]);

    let gray = (r as u32 + g as u32 + b as u32) / 3;
    let gray_step = ((gray as i32 - 8).clamp(0, 230) + 5) / 10;
    let gray_idx = (232 + gray_step.min(23)) as u8;
    let gray_v = (8 + 10 * gray_step.min(23)) as u8;

    if dist2((r, g, b), (gray_v, gray_v, gray_v)) < dist2((r, g, b), cube_rgb) {
        gray_idx
    } else {
        cube_idx
    }
}

/// RGB value of an xterm-256 palette index (for further reduction to 16).
fn indexed_to_rgb(i: u8) -> (u8, u8, u8) {
    match i {
        0..=15 => ANSI16[i as usize].1,
        16..=231 => {
            let c = i as usize - 16;
            (CUBE_LEVELS[c / 36], CUBE_LEVELS[c / 6 % 6], CUBE_LEVELS[c % 6])
        }
        232..=255 => {
            let v = 8 + 10 * (i - 232);
            (v, v, v)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_linux_term_is_bottom_tier() {
        let caps = TermCaps::from_env(Some("linux"), None);
        assert_eq!(caps, TermCaps::linux_tty());
        // COLORTERM cannot upgrade the console.
        let caps = TermCaps::from_env(Some("linux"), Some("truecolor"));
        assert_eq!(caps, TermCaps::linux_tty());
    }

    #[test]
    fn detect_colorterm_selects_truecolor() {
        for adv in ["truecolor", "24bit"] {
            let caps = TermCaps::from_env(Some("xterm-256color"), Some(adv));
            assert_eq!(caps, TermCaps::full());
        }
    }

    #[test]
    fn detect_without_colorterm_is_middle_tier() {
        let caps = TermCaps::from_env(Some("xterm-256color"), None);
        assert_eq!(caps.color, ColorSupport::Ansi256);
        assert!(caps.rich_glyphs);
    }

    #[test]
    fn ansi16_quantizes_to_nearest_palette_entry() {
        assert_eq!(nearest_ansi16(0, 0, 0), Color::Black);
        assert_eq!(nearest_ansi16(255, 85, 85), Color::LightRed);
        assert_eq!(nearest_ansi16(200, 0, 10), Color::Red);
        assert_eq!(nearest_ansi16(250, 250, 250), Color::White);
    }

    /// The selection highlight must never collapse into the plain black
    /// background on the bottom tier — pinned explicitly to DarkGray.
    #[test]
    fn selection_bg_stays_visible_on_bottom_tier() {
        assert_eq!(TermCaps::linux_tty().selection_bg(), Color::DarkGray);
        assert_ne!(
            TermCaps::linux_tty().adapt_color(TermCaps::linux_tty().selection_bg()),
            Color::Black
        );
    }

    #[test]
    fn ansi256_quantizes_rgb_to_indexed() {
        // Exact cube color maps onto the cube…
        assert_eq!(
            TermCaps { color: ColorSupport::Ansi256, rich_glyphs: true }
                .adapt_color(Color::Rgb(95, 135, 175)),
            Color::Indexed(16 + 36 + 6 * 2 + 3)
        );
        // …and near-grays land on the grayscale ramp.
        let c = TermCaps { color: ColorSupport::Ansi256, rich_glyphs: true }
            .adapt_color(Color::Rgb(40, 40, 42));
        match c {
            Color::Indexed(i) => assert!((232..=255).contains(&i), "expected gray ramp, got {i}"),
            other => panic!("expected Indexed, got {other:?}"),
        }
    }

    #[test]
    fn ansi16_reduces_indexed_via_rgb() {
        let caps = TermCaps::linux_tty();
        // 196 is pure red in the cube (255,0,0) — nearest VGA color is
        // the dark red (170,0,0).
        assert_eq!(caps.adapt_color(Color::Indexed(196)), Color::Red);
        assert_eq!(caps.adapt_color(Color::Indexed(255)), Color::White);
    }

    #[test]
    fn truecolor_passes_rgb_through() {
        let c = Color::Rgb(1, 2, 3);
        assert_eq!(TermCaps::full().adapt_color(c), c);
    }

    #[test]
    fn named_colors_untouched_on_all_tiers() {
        for caps in [TermCaps::full(), TermCaps::linux_tty()] {
            assert_eq!(caps.adapt_color(Color::Cyan), Color::Cyan);
        }
    }

    #[test]
    fn glyph_fallbacks() {
        assert_eq!(TermCaps::full().check_mark(), "✓");
        assert_eq!(TermCaps::linux_tty().check_mark(), "x");
        assert_eq!(TermCaps::full().selected_border_type(), BorderType::Thick);
        assert_eq!(TermCaps::linux_tty().selected_border_type(), BorderType::Double);
    }

    #[test]
    fn adapt_buffer_leaves_no_rgb_behind() {
        use ratatui::style::Style;
        let mut buf = Buffer::empty(ratatui::layout::Rect::new(0, 0, 4, 1));
        buf.set_string(0, 0, "test", Style::default().fg(Color::Rgb(1, 2, 3)).bg(Color::Rgb(4, 5, 6)));
        TermCaps::linux_tty().adapt_buffer(&mut buf);
        for cell in buf.content.iter() {
            assert!(!matches!(cell.fg, Color::Rgb(..)));
            assert!(!matches!(cell.bg, Color::Rgb(..)));
        }
    }
}
