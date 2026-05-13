use ratatui::style::Color;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Label {
    pub name: String,
    pub color: LabelColor,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LabelColor {
    Red,
    Orange,
    Yellow,
    Green,
    Blue,
    Purple,
    Pink,
    Cyan,
}

impl LabelColor {
    pub fn to_ratatui_color(self) -> Color {
        match self {
            Self::Red => Color::Red,
            Self::Orange => Color::Rgb(255, 165, 0),
            Self::Yellow => Color::Yellow,
            Self::Green => Color::Green,
            Self::Blue => Color::Blue,
            Self::Purple => Color::Magenta,
            Self::Pink => Color::Rgb(255, 182, 193),
            Self::Cyan => Color::Cyan,
        }
    }

    pub fn all() -> &'static [LabelColor] {
        &[
            Self::Red,
            Self::Orange,
            Self::Yellow,
            Self::Green,
            Self::Blue,
            Self::Purple,
            Self::Pink,
            Self::Cyan,
        ]
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Red => "Red",
            Self::Orange => "Orange",
            Self::Yellow => "Yellow",
            Self::Green => "Green",
            Self::Blue => "Blue",
            Self::Purple => "Purple",
            Self::Pink => "Pink",
            Self::Cyan => "Cyan",
        }
    }
}
