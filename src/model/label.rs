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
        }
    }
}
