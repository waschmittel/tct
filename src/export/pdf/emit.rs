use anyhow::{Context, anyhow};
use chrono::{Datelike, Timelike};
use krilla::configure::{Archival, ConfigurationBuilder};
use krilla::geom::{PathBuilder, Point, Rect};
use krilla::metadata::Metadata;
use krilla::page::PageSettings;
use krilla::paint::{Fill, Stroke};
use krilla::text::TextDirection;
use krilla::{Document, SerializeSettings};

use super::*;
use crate::model::board::BoardMeta;
use crate::model::card::Card;

pub(super) fn emit(
    pages: &[Vec<Op>],
    fonts: &Fonts,
    board: &BoardMeta,
    list: &str,
    card: &Card,
    labels: &[String],
    (now, version): (chrono::DateTime<chrono::Utc>, &str),
) -> anyhow::Result<Vec<u8>> {
    let configuration = ConfigurationBuilder::new()
        .with_archival_validator(Archival::A2_B)
        .finish()
        .map_err(|e| anyhow!("PDF/A configuration: {e:?}"))?;
    let mut document = Document::new_with(SerializeSettings {
        compress_content_streams: true,
        configuration,
        ..SerializeSettings::default()
    });
    let mut metadata = Metadata::new()
        .creation_date(
            krilla::metadata::DateTime::new(now.year() as u16)
                .month(now.month() as u8)
                .day(now.day() as u8)
                .hour(now.hour() as u8)
                .minute(now.minute() as u8)
                .second(now.second() as u8),
        )
        .title(card.title.clone())
        .creator("tct".to_string())
        .producer(format!("tct {version}"))
        .description(format!("{} / {}", board.name, list));
    if !labels.is_empty() {
        metadata = metadata.keywords(labels.to_vec());
    }
    document.set_metadata(metadata);

    let total = pages.len();
    let created = format!("{} {}", Lang::of(card).label(Label::Status), stamp(now));
    let location = format!("{} › {}", board.name, list);
    for (n, ops) in pages.iter().enumerate() {
        let mut page = document.start_page_with(
            PageSettings::from_wh(PAGE_W, PAGE_H).ok_or_else(|| anyhow!("invalid page size"))?,
        );
        let mut surface = page.surface();
        for op in ops {
            draw_op(&mut surface, fonts, op)?;
        }
        let chrome = page_chrome(fonts, card, &location, &created, n, total);
        for op in &chrome {
            draw_op(&mut surface, fonts, op)?;
        }
        surface.finish();
        page.finish();
    }
    document
        .finish()
        .map_err(|e| anyhow!("PDF generation failed: {e:?}"))
}

pub(super) fn paint((r, g, b): Rgb) -> krilla::paint::Paint {
    krilla::color::rgb::Color::new(r, g, b).into()
}

pub(super) fn draw_op(
    surface: &mut krilla::surface::Surface,
    fonts: &Fonts,
    op: &Op,
) -> anyhow::Result<()> {
    match op {
        Op::Text {
            x,
            y,
            text,
            style,
            size,
            color,
        } => {
            surface.set_stroke(None);
            surface.set_fill(Some(Fill {
                paint: paint(*color),
                ..Fill::default()
            }));
            surface.draw_text(
                Point::from_xy(*x, *y),
                fonts.face(*style).pdf.clone(),
                *size,
                text,
                false,
                TextDirection::Auto,
            );
        }
        Op::Rect {
            x,
            y,
            w,
            h,
            fill,
            stroke,
        } => {
            let rect =
                Rect::from_xywh(*x, *y, *w, *h).ok_or_else(|| anyhow!("degenerate rectangle"))?;
            let mut path = PathBuilder::new();
            path.push_rect(rect);
            let path = path
                .finish()
                .ok_or_else(|| anyhow!("degenerate rectangle"))?;
            surface.set_fill(fill.map(|c| Fill {
                paint: paint(c),
                ..Fill::default()
            }));
            surface.set_stroke(stroke.map(|c| Stroke {
                paint: paint(c),
                width: 0.6,
                ..Stroke::default()
            }));
            surface.draw_path(&path);
        }
        Op::Icon {
            kind,
            x,
            y,
            size,
            color,
        } => {
            let svg = kurbo::BezPath::from_svg(kind.octicon_path())
                .context("bundled icon is unusable")?;
            let k = f64::from(*size) / 16.0;
            let (ox, oy) = (f64::from(*x), f64::from(*y));
            let p = |pt: kurbo::Point| ((ox + pt.x * k) as f32, (oy + pt.y * k) as f32);
            let mut path = PathBuilder::new();
            for el in svg.elements() {
                match *el {
                    kurbo::PathEl::MoveTo(a) => path.move_to(p(a).0, p(a).1),
                    kurbo::PathEl::LineTo(a) => path.line_to(p(a).0, p(a).1),
                    kurbo::PathEl::QuadTo(a, b) => path.quad_to(p(a).0, p(a).1, p(b).0, p(b).1),
                    kurbo::PathEl::CurveTo(a, b, c) => {
                        path.cubic_to(p(a).0, p(a).1, p(b).0, p(b).1, p(c).0, p(c).1)
                    }
                    kurbo::PathEl::ClosePath => path.close(),
                }
            }
            let path = path.finish().ok_or_else(|| anyhow!("degenerate icon"))?;
            surface.set_stroke(None);
            surface.set_fill(Some(Fill {
                paint: paint(*color),
                ..Fill::default()
            }));
            surface.draw_path(&path);
        }
        Op::Line {
            x1,
            y1,
            x2,
            y2,
            color,
            width,
        } => {
            let mut path = PathBuilder::new();
            path.move_to(*x1, *y1);
            path.line_to(*x2, *y2);
            let path = path.finish().ok_or_else(|| anyhow!("degenerate line"))?;
            surface.set_fill(None);
            surface.set_stroke(Some(Stroke {
                paint: paint(*color),
                width: *width,
                ..Stroke::default()
            }));
            surface.draw_path(&path);
        }
    }
    Ok(())
}
