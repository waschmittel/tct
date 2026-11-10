//! Export of a card to a file. Pure of UI state: the TUI dialog and the
//! `tct cards --export` command both hand over a board, a list name and a
//! card.

mod blocks;
pub mod lang;
pub mod path;
mod pdf;
pub mod web;

pub use pdf::render as card_pdf;

/// How the PDF looks: black text on white with gray accents for monochrome
/// printers (the default), or the same layout in color.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Theme {
    #[default]
    Mono,
    Color,
}

impl Theme {
    pub fn parse(name: &str) -> Option<Theme> {
        match name.to_lowercase().as_str() {
            "mono" | "monochrome" => Some(Theme::Mono),
            "color" | "colour" => Some(Theme::Color),
            _ => None,
        }
    }
}
