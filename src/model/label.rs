use ratatui::style::Color;
use serde::{Deserialize, Serialize};

use super::ids::{self, ShortId};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Label {
    pub id: ShortId,
    pub name: String,
    pub color: LabelColor,
}

impl Label {
    pub fn new(name: String, color: LabelColor) -> Self {
        Self {
            id: ids::new_id(),
            name,
            color,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum LabelColor {
    Red,
    Orange,
    Yellow,
    Green,
    Blue,
    Purple,
    Pink,
    #[default]
    Cyan,
    #[serde(rename = "custom")]
    Custom { r: u8, g: u8, b: u8 },
}


impl LabelColor {
    pub fn to_ratatui_color(self) -> Color {
        let (r, g, b) = self.to_rgb();
        Color::Rgb(r, g, b)
    }

    /// Human-readable name for status messages, e.g. "green" or "#aabbcc".
    pub fn display_name(self) -> String {
        match self {
            Self::Red => "red".into(),
            Self::Orange => "orange".into(),
            Self::Yellow => "yellow".into(),
            Self::Green => "green".into(),
            Self::Blue => "blue".into(),
            Self::Purple => "purple".into(),
            Self::Pink => "pink".into(),
            Self::Cyan => "cyan".into(),
            Self::Custom { r, g, b } => format!("#{r:02x}{g:02x}{b:02x}"),
        }
    }

    pub fn to_rgb(self) -> (u8, u8, u8) {
        match self {
            Self::Red => (255, 179, 186),
            Self::Orange => (255, 209, 170),
            Self::Yellow => (255, 250, 170),
            Self::Green => (186, 255, 186),
            Self::Blue => (170, 200, 255),
            Self::Purple => (215, 180, 255),
            Self::Pink => (255, 200, 220),
            Self::Cyan => (170, 240, 240),
            Self::Custom { r, g, b } => (r, g, b),
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::Red => Self::Orange,
            Self::Orange => Self::Yellow,
            Self::Yellow => Self::Green,
            Self::Green => Self::Blue,
            Self::Blue => Self::Purple,
            Self::Purple => Self::Pink,
            Self::Pink => Self::Cyan,
            Self::Cyan => Self::Red,
            Self::Custom { r, g, b } => {
                let (h, s, l) = rgb_to_hsl(r, g, b);
                let new_h = (h + 37.0) % 360.0;
                let (nr, ng, nb) = hsl_to_rgb(new_h, s, l);
                Self::Custom { r: nr, g: ng, b: nb }
            }
        }
    }

    fn hue(self) -> f64 {
        match self {
            Self::Red => 0.0,
            Self::Orange => 30.0,
            Self::Yellow => 60.0,
            Self::Green => 120.0,
            Self::Blue => 220.0,
            Self::Purple => 280.0,
            Self::Pink => 340.0,
            Self::Cyan => 180.0,
            Self::Custom { r, g, b } => {
                let (h, _, _) = rgb_to_hsl(r, g, b);
                h
            }
        }
    }

    pub fn generate_pastel(existing: &[LabelColor]) -> Self {
        let mut hues: Vec<f64> = existing.iter().map(|c| c.hue()).collect();
        hues.sort_by(|a, b| a.partial_cmp(b).unwrap());

        let new_hue = if hues.is_empty() {
            210.0
        } else if hues.len() == 1 {
            (hues[0] + 180.0) % 360.0
        } else {
            let mut best_gap = 0.0f64;
            let mut best_mid = 0.0f64;
            for i in 0..hues.len() {
                let next = if i + 1 < hues.len() {
                    hues[i + 1]
                } else {
                    hues[0] + 360.0
                };
                let gap = next - hues[i];
                if gap > best_gap {
                    best_gap = gap;
                    best_mid = hues[i] + gap / 2.0;
                }
            }
            best_mid % 360.0
        };

        let (r, g, b) = hsl_to_rgb(new_hue, 0.55, 0.78);
        Self::Custom { r, g, b }
    }
}

pub(crate) fn hsl_to_rgb(h: f64, s: f64, l: f64) -> (u8, u8, u8) {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let h2 = h / 60.0;
    let x = c * (1.0 - (h2 % 2.0 - 1.0).abs());
    let (r1, g1, b1) = if h2 < 1.0 {
        (c, x, 0.0)
    } else if h2 < 2.0 {
        (x, c, 0.0)
    } else if h2 < 3.0 {
        (0.0, c, x)
    } else if h2 < 4.0 {
        (0.0, x, c)
    } else if h2 < 5.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };
    let m = l - c / 2.0;
    (
        ((r1 + m) * 255.0).round() as u8,
        ((g1 + m) * 255.0).round() as u8,
        ((b1 + m) * 255.0).round() as u8,
    )
}

pub(crate) fn rgb_to_hsl(r: u8, g: u8, b: u8) -> (f64, f64, f64) {
    let r = r as f64 / 255.0;
    let g = g as f64 / 255.0;
    let b = b as f64 / 255.0;
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0;
    if (max - min).abs() < 1e-10 {
        return (0.0, 0.0, l);
    }
    let d = max - min;
    let s = if l > 0.5 {
        d / (2.0 - max - min)
    } else {
        d / (max + min)
    };
    let h = if (max - r).abs() < 1e-10 {
        let mut h = (g - b) / d;
        if g < b {
            h += 6.0;
        }
        h * 60.0
    } else if (max - g).abs() < 1e-10 {
        ((b - r) / d + 2.0) * 60.0
    } else {
        ((r - g) / d + 4.0) * 60.0
    };
    (h, s, l)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pastel_color_is_rgb() {
        let color = LabelColor::generate_pastel(&[]);
        assert!(matches!(color, LabelColor::Custom { .. }));
    }

    #[test]
    fn pastel_colors_differentiate() {
        let first = LabelColor::generate_pastel(&[]);
        let second = LabelColor::generate_pastel(&[first]);
        let (h1, _, _) = match first {
            LabelColor::Custom { r, g, b } => rgb_to_hsl(r, g, b),
            _ => unreachable!(),
        };
        let (h2, _, _) = match second {
            LabelColor::Custom { r, g, b } => rgb_to_hsl(r, g, b),
            _ => unreachable!(),
        };
        let diff = (h1 - h2).abs();
        let diff = diff.min(360.0 - diff);
        assert!(diff > 90.0, "hue difference {diff} too small");
    }

    #[test]
    fn next_cycles_custom() {
        let c = LabelColor::Custom { r: 170, g: 200, b: 255 };
        let n = c.next();
        assert!(matches!(n, LabelColor::Custom { .. }));
        assert_ne!(c, n);
    }

    #[test]
    fn to_rgb_matches_ratatui_color() {
        let c = LabelColor::Cyan;
        let (r, g, b) = c.to_rgb();
        assert_eq!(c.to_ratatui_color(), Color::Rgb(r, g, b));
    }

    #[test]
    fn custom_color_serde_roundtrip() {
        let c = LabelColor::Custom { r: 12, g: 34, b: 250 };
        let json = serde_json::to_string(&c).unwrap();
        let back: LabelColor = serde_json::from_str(&json).unwrap();
        assert_eq!(c, back);
    }

    #[test]
    fn named_color_serde_roundtrip() {
        for c in [
            LabelColor::Red, LabelColor::Orange, LabelColor::Yellow,
            LabelColor::Green, LabelColor::Blue, LabelColor::Purple,
            LabelColor::Pink, LabelColor::Cyan,
        ] {
            let json = serde_json::to_string(&c).unwrap();
            let back: LabelColor = serde_json::from_str(&json).unwrap();
            assert_eq!(c, back);
        }
    }

    #[test]
    fn named_color_serializes_snake_case() {
        let json = serde_json::to_string(&LabelColor::Red).unwrap();
        assert_eq!(json, "\"red\"");
        let json = serde_json::to_string(&LabelColor::Cyan).unwrap();
        assert_eq!(json, "\"cyan\"");
    }

    #[test]
    fn default_is_cyan() {
        assert_eq!(LabelColor::default(), LabelColor::Cyan);
    }

    #[test]
    fn pastel_collision_avoids_existing_customs() {
        // Pre-existing custom colors at fixed hues
        let existing = vec![
            LabelColor::Custom { r: 255, g: 100, b: 100 }, // reddish
            LabelColor::Custom { r: 100, g: 255, b: 100 }, // greenish
            LabelColor::Custom { r: 100, g: 100, b: 255 }, // bluish
        ];
        let new = LabelColor::generate_pastel(&existing);
        let new_h = match new {
            LabelColor::Custom { r, g, b } => rgb_to_hsl(r, g, b).0,
            _ => unreachable!(),
        };
        // New hue must be at least some distance from each existing hue
        for e in &existing {
            let eh = e.hue();
            let diff = (new_h - eh).abs();
            let diff = diff.min(360.0 - diff);
            assert!(diff > 30.0, "new hue {new_h} too close to existing {eh}");
        }
    }

    #[test]
    fn pastel_seven_existing_returns_distinct() {
        let existing = vec![
            LabelColor::Red, LabelColor::Orange, LabelColor::Yellow,
            LabelColor::Green, LabelColor::Blue, LabelColor::Purple,
            LabelColor::Pink,
        ];
        let new = LabelColor::generate_pastel(&existing);
        assert!(matches!(new, LabelColor::Custom { .. }));
    }
}
