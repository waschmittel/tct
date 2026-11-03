//! Where to save the PDF of a card: a path input with a directory listing
//! beneath it. Type a path, or browse — Up/Down pick an entry, Tab or Enter
//! opens a folder / takes a file name, `..` goes up.

use std::path::{Path, PathBuf};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

use super::{Dialog, DialogHelp, DialogOutcome, DialogSideEffect};
use crate::app::LoadedBoard;
use crate::export::path::{self, Entry};
use crate::insert::line_input::{LineInput, LineKey};
use crate::model::id::CardId;

const LISTING_ROWS: usize = 8;
const WIDTH: u16 = 72;

pub struct ExportPdf {
    card_id: CardId,
    card_title: String,
    file_name: String,
    /// Relative input is taken relative to this directory.
    cwd: PathBuf,
    input: LineInput,
    entries: Vec<Entry>,
    selected: Option<usize>,
    /// The typed path names an existing file; Enter again overwrites it.
    overwrite_armed: bool,
    notice: Option<String>,
}

impl ExportPdf {
    /// `start_dir` is where the input begins (last export folder, else `cwd`).
    pub fn new(card_id: CardId, card_title: String, start_dir: &Path, cwd: PathBuf) -> Self {
        let file_name = path::default_file_name(&card_title);
        let mut dialog = Self {
            card_id,
            card_title,
            file_name,
            cwd,
            input: LineInput::with_initial(&path::display_dir(start_dir)),
            entries: Vec::new(),
            selected: None,
            overwrite_armed: false,
            notice: None,
        };
        dialog.refresh();
        dialog
    }

    fn refresh(&mut self) {
        self.entries = path::suggestions(&self.input.buffer, &self.cwd);
        self.selected = None;
        self.overwrite_armed = false;
    }

    fn set_input(&mut self, text: String) {
        self.input = LineInput::with_initial(&text);
        self.refresh();
    }

    fn take_entry(&mut self, entry: &Entry) {
        let (dir, _) = path::split_dir(&self.input.buffer);
        let text = if entry.name == ".." {
            path::parent_dir(dir)
        } else if entry.is_dir {
            format!("{dir}{}/", entry.name)
        } else {
            format!("{dir}{}", entry.name)
        };
        self.set_input(text);
    }

    /// Complete the name being typed as far as the listing agrees.
    fn complete(&mut self) {
        let (dir, typed) = path::split_dir(&self.input.buffer);
        let names: Vec<&Entry> = self.entries.iter().filter(|e| e.name != "..").collect();
        let Some(first) = names.first() else { return };
        let mut common: String = first.name.clone();
        for entry in &names[1..] {
            let shared = common.chars().zip(entry.name.chars()).take_while(|(a, b)| a == b).count();
            common = common.chars().take(shared).collect();
        }
        let typed_len = typed.chars().count();
        if names.len() == 1 {
            let entry = (*first).clone();
            self.take_entry(&entry);
        } else if common.chars().count() > typed_len {
            let text = format!("{dir}{common}");
            self.set_input(text);
        }
    }

    fn save(&mut self) -> DialogOutcome {
        let target = path::resolve(&self.input.buffer, &self.cwd, &self.file_name);
        if target.parent().is_some_and(|dir| !dir.is_dir()) {
            self.notice = Some("Folder does not exist".into());
            return DialogOutcome::stay();
        }
        if target.is_dir() {
            self.notice = Some("A folder with that name exists".into());
            return DialogOutcome::stay();
        }
        if target.exists() && !self.overwrite_armed {
            self.overwrite_armed = true;
            self.notice = Some("File exists — Enter again to overwrite".into());
            return DialogOutcome::stay();
        }
        DialogOutcome::side_effect(DialogSideEffect::ExportCardPdf {
            card_id: self.card_id.clone(),
            path: target,
        })
        .with_close()
    }

    fn move_selection(&mut self, down: bool) {
        self.selected = match (self.selected, down) {
            (None, true) if !self.entries.is_empty() => Some(0),
            (Some(i), true) => Some((i + 1).min(self.entries.len() - 1)),
            (Some(0), false) => None,
            (Some(i), false) => Some(i - 1),
            (current, _) => current,
        };
        self.overwrite_armed = false;
    }
}

impl Dialog for ExportPdf {
    fn render(&self, frame: &mut Frame, area: Rect, _board: Option<&LoadedBoard>, accent: Color) {
        let rows = self.entries.len().clamp(1, LISTING_ROWS);
        let width = WIDTH.min(area.width.saturating_sub(4));
        let height = (rows as u16 + 5).min(area.height);
        let popup = Rect::new(
            area.x + area.width.saturating_sub(width) / 2,
            area.y + area.height.saturating_sub(height) / 2,
            width,
            height,
        );
        frame.render_widget(Clear, popup);

        let title: String = self.card_title.chars().take(width.saturating_sub(18) as usize).collect();
        let block = Block::default()
            .title(format!(" Export PDF — {title} "))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(accent));
        let inner = block.inner(popup);
        frame.render_widget(block, popup);

        let dim = Style::default().fg(Color::DarkGray);
        let buffer = &self.input.buffer;
        let cursor_col = buffer[..self.input.cursor].chars().count();
        let visible_w = inner.width as usize;
        let scroll = (cursor_col + 1).saturating_sub(visible_w);
        let (_, typed_name) = path::split_dir(buffer);
        let ghost = if typed_name.is_empty() { self.file_name.as_str() } else { "" };
        let mut input_line = vec![Span::raw(buffer.chars().skip(scroll).collect::<String>())];
        input_line.push(Span::styled(ghost.to_string(), dim));
        frame.render_widget(
            Paragraph::new(Line::from(input_line)),
            Rect::new(inner.x, inner.y, inner.width, 1),
        );
        frame.set_cursor_position((inner.x + (cursor_col - scroll) as u16, inner.y));

        let mut divider = "─".repeat(inner.width as usize);
        if self.entries.len() > LISTING_ROWS {
            let counter = format!(" {}/{} ", self.selected.map_or(0, |i| i + 1), self.entries.len());
            let keep = divider.chars().count().saturating_sub(counter.chars().count() + 1);
            divider = format!("{}{counter}─", "─".repeat(keep));
        }
        frame.render_widget(
            Paragraph::new(Span::styled(divider, dim)),
            Rect::new(inner.x, inner.y + 1, inner.width, 1),
        );

        let first = self.selected.map_or(0, |i| i.saturating_sub(LISTING_ROWS - 1));
        let mut lines: Vec<Line> = self
            .entries
            .iter()
            .enumerate()
            .skip(first)
            .take(LISTING_ROWS)
            .map(|(i, entry)| {
                let label = if entry.is_dir { format!("{}/", entry.name) } else { entry.name.clone() };
                let style = if entry.is_dir {
                    Style::default().fg(accent)
                } else {
                    Style::default()
                };
                if Some(i) == self.selected {
                    Line::from(vec![
                        Span::styled("» ", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
                        Span::styled(label, style.add_modifier(Modifier::BOLD)),
                    ])
                } else {
                    Line::from(Span::styled(format!("  {label}"), style))
                }
            })
            .collect();
        if lines.is_empty() {
            lines.push(Line::from(Span::styled("  (nothing here)", dim)));
        }
        frame.render_widget(
            Paragraph::new(lines),
            Rect::new(inner.x, inner.y + 2, inner.width, rows as u16),
        );

        if let Some(notice) = &self.notice {
            frame.render_widget(
                Paragraph::new(Span::styled(notice.clone(), Style::default().fg(Color::Yellow))),
                Rect::new(inner.x, inner.y + 2 + rows as u16, inner.width, 1),
            );
        }
    }

    fn handle_key(&mut self, key: KeyEvent, _board: Option<&LoadedBoard>) -> DialogOutcome {
        self.notice = None;
        let plain = !key.modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT);
        match key.code {
            KeyCode::F(1) => return DialogOutcome::help(),
            KeyCode::Esc if self.selected.is_some() => {
                self.selected = None;
                return DialogOutcome::stay();
            }
            KeyCode::Down if plain => self.move_selection(true),
            KeyCode::Up if plain => self.move_selection(false),
            KeyCode::Tab => match self.selected.and_then(|i| self.entries.get(i)).cloned() {
                Some(entry) => self.take_entry(&entry),
                None => self.complete(),
            },
            KeyCode::Enter => {
                if let Some(entry) = self.selected.and_then(|i| self.entries.get(i)).cloned() {
                    self.take_entry(&entry);
                } else {
                    return self.save();
                }
            }
            _ => match self.input.handle_key(key) {
                LineKey::Cancel => return DialogOutcome::close(),
                LineKey::Edited => self.refresh(),
                LineKey::Confirm | LineKey::Ignored => {}
            },
        }
        DialogOutcome::stay()
    }

    fn help(&self) -> Option<DialogHelp> {
        Some(DialogHelp {
            title: " Help — Export PDF ",
            rows: vec![
                ("Type", "Path of the file (empty name: card title)"),
                ("Up / Down", "Browse folders and PDFs below"),
                ("Tab", "Open folder / take name; complete typed name"),
                ("Enter", "Open selected folder, else save"),
                ("..", "Go up one folder"),
                ("Esc", "Deselect, then cancel"),
                ("F1", "This help"),
            ],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dialog::Follow;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::empty())
    }

    fn dialog(dir: &Path) -> ExportPdf {
        ExportPdf::new(CardId::generate(), "My Card".into(), dir, dir.to_path_buf())
    }

    fn type_text(d: &mut ExportPdf, text: &str) {
        for c in text.chars() {
            d.handle_key(key(KeyCode::Char(c)), None);
        }
    }

    fn exported(out: DialogOutcome) -> Option<PathBuf> {
        match out.side_effect {
            Some(DialogSideEffect::ExportCardPdf { path, .. }) => Some(path),
            _ => None,
        }
    }

    #[test]
    fn enter_on_the_untouched_input_saves_under_the_card_title() {
        let tmp = tempfile::tempdir().unwrap();
        let mut d = dialog(tmp.path());
        let out = d.handle_key(key(KeyCode::Enter), None);
        assert!(matches!(out.follow, Follow::Close));
        assert_eq!(exported(out), Some(tmp.path().join("my-card.pdf")));
    }

    #[test]
    fn typing_a_name_overrides_the_default() {
        let tmp = tempfile::tempdir().unwrap();
        let mut d = dialog(tmp.path());
        type_text(&mut d, "report");
        let out = d.handle_key(key(KeyCode::Enter), None);
        assert_eq!(exported(out), Some(tmp.path().join("report.pdf")));
    }

    #[test]
    fn browsing_into_a_folder_then_saving() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir(tmp.path().join("docs")).unwrap();
        let mut d = dialog(tmp.path());
        d.handle_key(key(KeyCode::Down), None);
        d.handle_key(key(KeyCode::Down), None);
        assert_eq!(d.entries[d.selected.unwrap()].name, "docs");
        d.handle_key(key(KeyCode::Enter), None);
        assert!(d.input.buffer.ends_with("/docs/"));
        let out = d.handle_key(key(KeyCode::Enter), None);
        assert_eq!(exported(out), Some(tmp.path().join("docs/my-card.pdf")));
    }

    #[test]
    fn dot_dot_goes_up_one_folder() {
        let tmp = tempfile::tempdir().unwrap();
        let sub = tmp.path().join("a");
        std::fs::create_dir(&sub).unwrap();
        let mut d = dialog(&sub);
        d.handle_key(key(KeyCode::Down), None);
        assert_eq!(d.entries[0].name, "..");
        d.handle_key(key(KeyCode::Tab), None);
        let out = d.handle_key(key(KeyCode::Enter), None);
        assert_eq!(exported(out).unwrap().parent(), Some(tmp.path()));
    }

    #[test]
    fn tab_completes_the_common_prefix_and_a_single_match() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir(tmp.path().join("projects")).unwrap();
        std::fs::create_dir(tmp.path().join("project-x")).unwrap();
        std::fs::create_dir(tmp.path().join("zeta")).unwrap();
        let mut d = dialog(tmp.path());
        type_text(&mut d, "pr");
        d.handle_key(key(KeyCode::Tab), None);
        assert!(d.input.buffer.ends_with("/project"));
        type_text(&mut d, "s");
        d.handle_key(key(KeyCode::Tab), None);
        assert!(d.input.buffer.ends_with("/projects/"));
    }

    #[test]
    fn existing_file_needs_a_second_enter() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("my-card.pdf"), b"old").unwrap();
        let mut d = dialog(tmp.path());
        let first = d.handle_key(key(KeyCode::Enter), None);
        assert!(exported(first).is_none());
        assert!(d.notice.is_some());
        let second = d.handle_key(key(KeyCode::Enter), None);
        assert!(exported(second).is_some());
    }

    #[test]
    fn missing_folder_is_refused_in_the_dialog() {
        let tmp = tempfile::tempdir().unwrap();
        let mut d = dialog(tmp.path());
        type_text(&mut d, "nope/x");
        let out = d.handle_key(key(KeyCode::Enter), None);
        assert!(exported(out).is_none());
        assert_eq!(d.notice.as_deref(), Some("Folder does not exist"));
    }

    #[test]
    fn esc_deselects_first_then_cancels() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir(tmp.path().join("d")).unwrap();
        let mut d = dialog(tmp.path());
        d.handle_key(key(KeyCode::Down), None);
        let out = d.handle_key(key(KeyCode::Esc), None);
        assert!(matches!(out.follow, Follow::Stay));
        let out = d.handle_key(key(KeyCode::Esc), None);
        assert!(matches!(out.follow, Follow::Close));
        assert!(out.side_effect.is_none());
    }

    #[test]
    fn renders_input_ghost_name_listing_and_selection() {
        use ratatui::Terminal;
        use ratatui::backend::TestBackend;

        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir(tmp.path().join("docs")).unwrap();
        std::fs::write(tmp.path().join("old.pdf"), b"").unwrap();
        let mut d = dialog(tmp.path());
        d.handle_key(key(KeyCode::Down), None);
        d.handle_key(key(KeyCode::Down), None);

        let mut terminal = Terminal::new(TestBackend::new(90, 20)).unwrap();
        terminal
            .draw(|f| d.render(f, f.area(), None, Color::Cyan))
            .unwrap();
        let text: String = terminal
            .backend()
            .buffer()
            .content()
            .chunks(90)
            .map(|row| row.iter().map(|c| c.symbol()).collect::<String>() + "\n")
            .collect();
        assert!(text.contains("Export PDF — My Card"), "{text}");
        assert!(text.contains("» docs/"), "selected folder missing:\n{text}");
        assert!(text.contains("old.pdf"), "{text}");
    }

    #[test]
    fn untouched_input_shows_the_default_name_as_ghost_text() {
        use ratatui::Terminal;
        use ratatui::backend::TestBackend;

        let d = ExportPdf::new(CardId::generate(), "My Card".into(), Path::new("/"), PathBuf::from("/"));
        let mut terminal = Terminal::new(TestBackend::new(90, 20)).unwrap();
        terminal
            .draw(|f| d.render(f, f.area(), None, Color::Cyan))
            .unwrap();
        let row: String = terminal.backend().buffer().content().iter().map(|c| c.symbol()).collect();
        assert!(row.contains("/my-card.pdf"), "{row}");
    }

    #[test]
    fn f1_opens_help() {
        let tmp = tempfile::tempdir().unwrap();
        let mut d = dialog(tmp.path());
        assert!(matches!(d.handle_key(key(KeyCode::F(1)), None).follow, Follow::Help));
        assert!(d.help().is_some());
    }
}
