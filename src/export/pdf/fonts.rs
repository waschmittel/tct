use anyhow::{Context, anyhow};
use krilla::text::Font;
use skrifa::charmap::Charmap;
use skrifa::instance::{LocationRef, Size};
use skrifa::metrics::GlyphMetrics;
use skrifa::{FontRef, MetadataProvider};

use crate::export::Theme;
use crate::export::blocks::Style;

pub(super) static SANS: &[u8] = include_bytes!("../../../assets/fonts/DejaVuSans.ttf");
pub(super) static BOLD: &[u8] = include_bytes!("../../../assets/fonts/DejaVuSans-Bold.ttf");
pub(super) static BOLD_OBLIQUE: &[u8] =
    include_bytes!("../../../assets/fonts/DejaVuSans-BoldOblique.ttf");
pub(super) static OBLIQUE: &[u8] = include_bytes!("../../../assets/fonts/DejaVuSans-Oblique.ttf");
pub(super) static MONO: &[u8] = include_bytes!("../../../assets/fonts/DejaVuSansMono.ttf");

pub(super) struct Face {
    pub(super) pdf: Font,
    pub(super) charmap: Charmap<'static>,
    pub(super) metrics: GlyphMetrics<'static>,
    pub(super) units_per_em: f32,
}

impl Face {
    pub(super) fn new(data: &'static [u8]) -> anyhow::Result<Self> {
        let font = FontRef::new(data).context("bundled font is unreadable")?;
        Ok(Self {
            pdf: Font::new(data.into(), 0).ok_or_else(|| anyhow!("bundled font is unusable"))?,
            charmap: font.charmap(),
            metrics: font.glyph_metrics(Size::unscaled(), LocationRef::default()),
            units_per_em: f32::from(
                font.metrics(Size::unscaled(), LocationRef::default())
                    .units_per_em,
            ),
        })
    }

    pub(super) fn advance(&self, c: char, size: f32) -> f32 {
        self.charmap
            .map(c)
            .and_then(|g| self.metrics.advance_width(g))
            .map_or(0.0, |w| w / self.units_per_em * size)
    }

    pub(super) fn width(&self, text: &str, size: f32) -> f32 {
        text.chars().map(|c| self.advance(c, size)).sum()
    }

    pub(super) fn clean(&self, text: &str) -> String {
        text.chars()
            .flat_map(|c| match c {
                '\t' => "    ".chars().collect::<Vec<_>>(),
                '\u{200B}' | '\u{200D}' | '\u{FE0E}' | '\u{FE0F}' => Vec::new(),
                c if c.is_control() => Vec::new(),
                c if self.charmap.map(c).is_some() => vec![c],
                _ => vec!['?'],
            })
            .collect()
    }
}

pub(super) struct Fonts {
    pub(super) theme: Theme,
    pub(super) sans: Face,
    pub(super) bold: Face,
    pub(super) oblique: Face,
    pub(super) bold_oblique: Face,
    pub(super) mono: Face,
}

impl Fonts {
    #[cfg(test)]
    pub(super) fn load() -> anyhow::Result<Self> {
        Self::load_for(Theme::Color)
    }

    pub(super) fn load_for(theme: Theme) -> anyhow::Result<Self> {
        Ok(Self {
            theme,
            sans: Face::new(SANS)?,
            bold: Face::new(BOLD)?,
            oblique: Face::new(OBLIQUE)?,
            bold_oblique: Face::new(BOLD_OBLIQUE)?,
            mono: Face::new(MONO)?,
        })
    }

    pub(super) fn mono_theme(&self) -> bool {
        self.theme == Theme::Mono
    }

    pub(super) fn face(&self, style: Style) -> &Face {
        if style.code {
            &self.mono
        } else if style.bold && style.italic {
            &self.bold_oblique
        } else if style.bold {
            &self.bold
        } else if style.italic {
            &self.oblique
        } else {
            &self.sans
        }
    }
}
