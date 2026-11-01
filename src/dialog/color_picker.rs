//! Color Picker / Board Configuration dialog — free HSL selection of a color,
//! plus board-level configuration (new item insertion position).
//!
//! Two targets: board configuration (opened from the Board Selector
//! with Shift+C, applied via `ConfigureSelectedBoard`) and a label color
//! (opened from the Label Manager with Shift+C, applied via
//! `Command::SetLabelColor`; returns to the Label Manager afterwards).

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

use super::{Dialog, DialogBackground, DialogOutcome, DialogSideEffect};
use crate::app::LoadedBoard;
use crate::command::Command;
use crate::model::board::NewCardPosition;
use crate::model::id::LabelId;
use crate::model::label::{self, LabelColor};

#[allow(dead_code)]
pub type BoardConfiguration = ColorPicker;

const HUE_STEP: f64 = 5.0;
const SL_STEP: f64 = 0.02;
const BAR_WIDTH: usize = 24;

/// What the picked color is applied to.
pub enum ColorTarget {
    /// Accent color and configuration of the board selected in the Board Selector.
    BoardAccent,
    /// A label in the open board's palette. Carries the Label Manager
    /// return state (`label_idx` selection, `from_picker` origin).
    Label {
        label_id: LabelId,
        label_idx: usize,
        from_picker: bool,
    },
}

pub struct ColorPicker {
    h: f64,
    s: f64,
    l: f64,
    field: usize,
    target: ColorTarget,
    new_card_position: Option<NewCardPosition>,
}

impl ColorPicker {
    fn seeded(
        color: LabelColor,
        target: ColorTarget,
        new_card_position: Option<NewCardPosition>,
    ) -> Self {
        let (r, g, b) = color.to_rgb();
        let (h, s, l) = label::rgb_to_hsl(r, g, b);
        Self {
            h,
            s,
            l,
            field: 0,
            target,
            new_card_position,
        }
    }

    pub fn for_board(color: LabelColor, new_card_position: NewCardPosition) -> Self {
        Self::seeded(color, ColorTarget::BoardAccent, Some(new_card_position))
    }

    pub fn for_label(
        color: LabelColor,
        label_id: LabelId,
        label_idx: usize,
        from_picker: bool,
    ) -> Self {
        Self::seeded(
            color,
            ColorTarget::Label {
                label_id,
                label_idx,
                from_picker,
            },
            None,
        )
    }

    /// The Label Manager to return to (label target only).
    fn back_to_manager(&self) -> Option<Box<dyn Dialog>> {
        match &self.target {
            ColorTarget::BoardAccent => None,
            ColorTarget::Label { label_idx, from_picker, .. } => {
                Some(Box::new(super::label_manager::LabelManager {
                    selected_idx: *label_idx,
                    from_picker: *from_picker,
                }))
            }
        }
    }

    fn current_color(&self) -> LabelColor {
        let (r, g, b) = label::hsl_to_rgb(self.h, self.s, self.l);
        LabelColor::Custom { r, g, b }
    }

    fn adjust(&mut self, dir: f64) {
        match self.field {
            0 => self.h = (self.h + dir * HUE_STEP).rem_euclid(360.0),
            1 => self.s = (self.s + dir * SL_STEP).clamp(0.0, 1.0),
            _ => self.l = (self.l + dir * SL_STEP).clamp(0.0, 1.0),
        }
    }

    fn slider_line(&self, idx: usize, label_text: &str, frac: f64, value: String, accent: Color) -> Line<'static> {
        let filled = ((frac * BAR_WIDTH as f64).round() as usize).min(BAR_WIDTH);
        let bar: String = "█".repeat(filled) + &"░".repeat(BAR_WIDTH - filled);
        let selected = idx == self.field;
        let marker = if selected { "» " } else { "  " };
        let name_style = if selected {
            Style::default().fg(accent).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };
        Line::from(vec![
            Span::styled(marker.to_string(), name_style),
            Span::styled(format!("{label_text:<11}"), name_style),
            Span::styled(bar, Style::default().fg(accent)),
            Span::styled(format!("  {value}"), Style::default().fg(Color::DarkGray)),
        ])
    }

    fn new_items_line(&self, pos: NewCardPosition, accent: Color) -> Line<'static> {
        let selected = self.field == 3;
        let marker = if selected { "» " } else { "  " };
        let name_style = if selected {
            Style::default().fg(accent).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };

        let active_style = if selected {
            Style::default().fg(accent).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
        };
        let inactive_style = Style::default().fg(Color::DarkGray);

        let (top_symbol, top_style, bottom_symbol, bottom_style) = match pos {
            NewCardPosition::Top => ("● Top", active_style, "○ Bottom", inactive_style),
            NewCardPosition::Bottom => ("○ Top", inactive_style, "● Bottom", active_style),
        };

        Line::from(vec![
            Span::styled(marker.to_string(), name_style),
            Span::styled(format!("{:<11}", "New items"), name_style),
            Span::styled(top_symbol, top_style),
            Span::raw("    "),
            Span::styled(bottom_symbol, bottom_style),
        ])
    }
}

impl Dialog for ColorPicker {
    fn render(&self, frame: &mut Frame, area: Rect, _board: Option<&LoadedBoard>, accent: Color) {
        let is_board = self.new_card_position.is_some();
        let height = if is_board { 10u16 } else { 9u16 };
        let width = 48u16.min(area.width.saturating_sub(4));
        let x = (area.width.saturating_sub(width)) / 2;
        let y = (area.height.saturating_sub(height)) / 2;
        let popup = Rect::new(x, y, width, height);

        frame.render_widget(Clear, popup);

        let title = match self.target {
            ColorTarget::BoardAccent => " Board Configuration ",
            ColorTarget::Label { .. } => " Label Color ",
        };
        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(accent));
        let inner = block.inner(popup);
        frame.render_widget(block, popup);

        let (r, g, b) = label::hsl_to_rgb(self.h, self.s, self.l);
        let swatch_bg = Color::Rgb(r, g, b);

        let mut lines = vec![
            self.slider_line(0, "Hue", self.h / 360.0, format!("{:>3}°", self.h.round() as i32), accent),
            self.slider_line(1, "Saturation", self.s, format!("{:>3}%", (self.s * 100.0).round() as i32), accent),
            self.slider_line(2, "Lightness", self.l, format!("{:>3}%", (self.l * 100.0).round() as i32), accent),
            Line::raw(""),
            Line::from(vec![
                Span::styled("  Preview   ", Style::default().fg(Color::White)),
                Span::styled(" ".repeat(BAR_WIDTH), Style::default().bg(swatch_bg)),
                Span::styled(format!("  #{r:02X}{g:02X}{b:02X}"), Style::default().fg(Color::DarkGray)),
            ]),
        ];

        if let Some(pos) = self.new_card_position {
            lines.push(Line::raw(""));
            lines.push(self.new_items_line(pos, accent));
        }

        frame.render_widget(Paragraph::new(lines), inner);
    }

    fn handle_key(&mut self, key: KeyEvent, _board: Option<&LoadedBoard>) -> DialogOutcome {
        let max_field = if self.new_card_position.is_some() { 3 } else { 2 };
        match key.code {
            KeyCode::Up if self.field > 0 => {
                self.field -= 1;
                DialogOutcome::stay()
            }
            KeyCode::Down if self.field < max_field => {
                self.field += 1;
                DialogOutcome::stay()
            }
            KeyCode::Left => {
                if self.field == 3 {
                    if let Some(pos) = &mut self.new_card_position {
                        *pos = NewCardPosition::Top;
                    }
                } else {
                    self.adjust(-1.0);
                }
                DialogOutcome::stay()
            }
            KeyCode::Right => {
                if self.field == 3 {
                    if let Some(pos) = &mut self.new_card_position {
                        *pos = NewCardPosition::Bottom;
                    }
                } else {
                    self.adjust(1.0);
                }
                DialogOutcome::stay()
            }
            KeyCode::Char(' ') if self.field == 3 => {
                if let Some(pos) = &mut self.new_card_position {
                    *pos = pos.toggle();
                }
                DialogOutcome::stay()
            }
            KeyCode::Char('t') | KeyCode::Char('T') if self.field == 3 => {
                if let Some(pos) = &mut self.new_card_position {
                    *pos = NewCardPosition::Top;
                }
                DialogOutcome::stay()
            }
            KeyCode::Char('b') | KeyCode::Char('B') if self.field == 3 => {
                if let Some(pos) = &mut self.new_card_position {
                    *pos = NewCardPosition::Bottom;
                }
                DialogOutcome::stay()
            }
            KeyCode::Enter => match &self.target {
                ColorTarget::BoardAccent => {
                    let new_card_position = self.new_card_position.unwrap_or_default();
                    DialogOutcome::side_effect(DialogSideEffect::ConfigureSelectedBoard {
                        color: self.current_color(),
                        new_card_position,
                    })
                    .with_close_to(crate::app::AppMode::BoardSelector)
                    .with_status("Board configuration saved".to_string())
                }
                ColorTarget::Label { label_id, .. } => {
                    let mut out = DialogOutcome::apply(Command::SetLabelColor {
                        label_id: label_id.clone(),
                        color: self.current_color(),
                    })
                    .with_status(format!(
                        "Label color set to {}",
                        self.current_color().display_name()
                    ));
                    out.follow = super::Follow::Open(self.back_to_manager().unwrap());
                    out
                }
            },
            KeyCode::Char('?') => DialogOutcome::help(),
            KeyCode::Esc => match self.back_to_manager() {
                Some(manager) => DialogOutcome::open(manager),
                None => DialogOutcome::close_to(crate::app::AppMode::BoardSelector),
            },
            _ => DialogOutcome::stay(),
        }
    }

    fn help(&self) -> Option<super::DialogHelp> {
        match self.target {
            ColorTarget::BoardAccent => Some(super::DialogHelp {
                title: " Help — Board Configuration ",
                rows: vec![
                    ("Up / Down", "Select field (hue, saturation, lightness, new items)"),
                    ("Left / Right", "Adjust selected field / change new items position"),
                    ("Space", "Toggle new items (when selected)"),
                    ("Enter", "Apply configuration"),
                    ("Esc", "Cancel"),
                ],
            }),
            ColorTarget::Label { .. } => Some(super::DialogHelp {
                title: " Help — Color Picker ",
                rows: vec![
                    ("Up / Down", "Select field (hue, saturation, lightness)"),
                    ("Left / Right", "Adjust selected field"),
                    ("Enter", "Apply color"),
                    ("Esc", "Cancel"),
                ],
            }),
        }
    }

    fn background(&self) -> DialogBackground {
        match self.target {
            ColorTarget::BoardAccent => DialogBackground::BoardSelector,
            ColorTarget::Label { .. } => DialogBackground::Auto,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyModifiers;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::empty())
    }

    #[test]
    fn seeds_from_color_and_roundtrips() {
        let picker = ColorPicker::for_board(
            LabelColor::Custom { r: 170, g: 200, b: 255 },
            NewCardPosition::Bottom,
        );
        match picker.current_color() {
            LabelColor::Custom { r, g, b } => {
                // HSL roundtrip is lossy by at most 1 per channel.
                assert!((r as i32 - 170).abs() <= 1);
                assert!((g as i32 - 200).abs() <= 1);
                assert!((b as i32 - 255).abs() <= 1);
            }
            _ => unreachable!(),
        }
    }

    #[test]
    fn down_up_moves_field_including_new_items() {
        let mut p = ColorPicker::for_board(LabelColor::Cyan, NewCardPosition::Bottom);
        assert_eq!(p.field, 0);
        p.handle_key(key(KeyCode::Down), None);
        assert_eq!(p.field, 1);
        p.handle_key(key(KeyCode::Down), None);
        assert_eq!(p.field, 2);
        p.handle_key(key(KeyCode::Down), None);
        assert_eq!(p.field, 3);
        // Clamp at bottom.
        p.handle_key(key(KeyCode::Down), None);
        assert_eq!(p.field, 3);
        p.handle_key(key(KeyCode::Up), None);
        assert_eq!(p.field, 2);
    }

    #[test]
    fn new_items_position_controls() {
        let mut p = ColorPicker::for_board(LabelColor::Cyan, NewCardPosition::Bottom);
        p.field = 3;
        assert_eq!(p.new_card_position, Some(NewCardPosition::Bottom));

        // Left sets Top
        p.handle_key(key(KeyCode::Left), None);
        assert_eq!(p.new_card_position, Some(NewCardPosition::Top));

        // Right sets Bottom
        p.handle_key(key(KeyCode::Right), None);
        assert_eq!(p.new_card_position, Some(NewCardPosition::Bottom));

        // Space toggles
        p.handle_key(key(KeyCode::Char(' ')), None);
        assert_eq!(p.new_card_position, Some(NewCardPosition::Top));
        p.handle_key(key(KeyCode::Char(' ')), None);
        assert_eq!(p.new_card_position, Some(NewCardPosition::Bottom));

        // 't' sets Top, 'b' sets Bottom
        p.handle_key(key(KeyCode::Char('t')), None);
        assert_eq!(p.new_card_position, Some(NewCardPosition::Top));
        p.handle_key(key(KeyCode::Char('b')), None);
        assert_eq!(p.new_card_position, Some(NewCardPosition::Bottom));
    }

    #[test]
    fn right_left_adjusts_selected_field() {
        let mut p = ColorPicker::for_board(LabelColor::Cyan, NewCardPosition::Bottom);
        let h0 = p.h;
        p.handle_key(key(KeyCode::Right), None);
        assert_eq!(p.h, (h0 + HUE_STEP).rem_euclid(360.0));
        p.handle_key(key(KeyCode::Left), None);
        assert!((p.h - h0).abs() < 1e-9);
    }

    #[test]
    fn saturation_clamps_at_bounds() {
        let mut p = ColorPicker::for_board(LabelColor::Cyan, NewCardPosition::Bottom);
        p.field = 1;
        for _ in 0..100 {
            p.handle_key(key(KeyCode::Left), None);
        }
        assert_eq!(p.s, 0.0);
        for _ in 0..100 {
            p.handle_key(key(KeyCode::Right), None);
        }
        assert_eq!(p.s, 1.0);
    }

    #[test]
    fn enter_emits_configure_selected_board_side_effect() {
        let mut p = ColorPicker::for_board(LabelColor::Cyan, NewCardPosition::Top);
        let out = p.handle_key(key(KeyCode::Enter), None);
        match out.side_effect {
            Some(DialogSideEffect::ConfigureSelectedBoard { color, new_card_position }) => {
                assert_eq!(color, p.current_color());
                assert_eq!(new_card_position, NewCardPosition::Top);
            }
            _ => panic!("Expected ConfigureSelectedBoard side effect"),
        }
        assert_eq!(out.status.as_deref(), Some("Board configuration saved"));
    }

    #[test]
    fn board_dialog_has_board_configuration_help() {
        let p = ColorPicker::for_board(LabelColor::Cyan, NewCardPosition::Bottom);
        let help = p.help().unwrap();
        assert_eq!(help.title, " Help — Board Configuration ");
    }

    #[test]
    fn label_picker_clamps_at_field_two() {
        let mut p = ColorPicker::for_label(LabelColor::Cyan, LabelId::generate(), 0, false);
        p.handle_key(key(KeyCode::Down), None);
        p.handle_key(key(KeyCode::Down), None);
        assert_eq!(p.field, 2);
        p.handle_key(key(KeyCode::Down), None);
        assert_eq!(p.field, 2);
        let help = p.help().unwrap();
        assert_eq!(help.title, " Help — Color Picker ");
    }
}
