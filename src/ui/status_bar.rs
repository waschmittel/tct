use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::app::{App, AppMode};

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let mode_str = match &app.mode {
        AppMode::BoardSelector => "BOARDS",
        // Grab state lives inside Normal mode (input/normal.rs swallows
        // everything but arrows/drop keys while it's set).
        AppMode::Normal if app.grab_active => "MOVE",
        AppMode::Normal => "NORMAL",
        AppMode::CardDetail => "DETAIL",
        AppMode::Insert => "INSERT",
        AppMode::Command => "SEARCH",
        AppMode::Dialog => "DIALOG",
        AppMode::Help => "HELP",
    };

    // The status bar carries exactly one hint: `?` for the contextual help
    // overlay (the overlay itself documents everything else). Insert-mode
    // handlers consume plain text keys, so only the date picker — which
    // ignores `?` — offers it; the other handlers show just their title.
    let insert_hint = if matches!(app.mode, AppMode::Insert) {
        app.insert.as_ref().map(|h| {
            let title = h.title();
            if h.as_any()
                .downcast_ref::<crate::insert::date_picker::DatePicker>()
                .is_some()
            {
                format!("{title}  ?:help")
            } else {
                title.to_string()
            }
        })
    } else {
        None
    };

    let hints: &str = match &app.mode {
        AppMode::BoardSelector
        | AppMode::Normal
        | AppMode::CardDetail
        | AppMode::Dialog => "?:help",
        AppMode::Insert => insert_hint.as_deref().unwrap_or(""),
        AppMode::Command | AppMode::Help => "",
    };

    // Single row: mode chip + key hints left, data dir right. Transient
    // status messages render as a toast in the top-right corner instead
    // (`super::render_status_toast`), so hints stay visible.
    let line = Line::from(vec![
        Span::styled(
            format!(" {mode_str} "),
            Style::default().fg(Color::Black).bg(app.accent_color()),
        ),
        Span::raw(" "),
        Span::styled(hints, Style::default().fg(Color::DarkGray)),
    ]);
    frame.render_widget(Paragraph::new(line), area);

    let used = mode_str.chars().count() + 3 + hints.chars().count();
    render_data_dir(frame, area, app, used);
}

/// Data dir, right-aligned on the status row. Yields to the left content:
/// shortened to fit the remaining width, dropped entirely if even
/// `…/<last component>` doesn't fit.
fn render_data_dir(frame: &mut Frame, area: Rect, app: &App, used: usize) {
    if area.height < 1 {
        return;
    }
    // 2-col gap after the left content, 1 trailing col.
    let budget = (area.width as usize).saturating_sub(used + 3);
    let Some(dir) = shorten_path(&app.data_dir_display, budget) else {
        return;
    };
    let paragraph = Paragraph::new(Line::from(Span::styled(
        format!("{dir} "),
        Style::default().fg(Color::DarkGray),
    )))
    .right_aligned();
    frame.render_widget(paragraph, Rect::new(area.x, area.y, area.width, 1));
}

/// Fit `path` into `max` columns by dropping leading components behind `…`.
/// Returns `None` when even the last component doesn't fit.
fn shorten_path(path: &str, max: usize) -> Option<String> {
    if path.chars().count() <= max {
        return Some(path.to_string());
    }
    let parts: Vec<&str> = path.split('/').collect();
    for i in 1..parts.len() {
        let candidate = format!("…/{}", parts[i..].join("/"));
        if candidate.chars().count() <= max {
            return Some(candidate);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::shorten_path;

    #[test]
    fn fits_unchanged() {
        assert_eq!(shorten_path("~/.tct", 10), Some("~/.tct".into()));
    }

    #[test]
    fn drops_leading_components() {
        assert_eq!(
            shorten_path("~/gitlab-dm/tct/.tct", 12),
            Some("…/tct/.tct".into())
        );
    }

    #[test]
    fn none_when_last_component_too_long() {
        assert_eq!(shorten_path("~/some/very-long-component", 5), None);
    }

    #[test]
    fn none_at_zero_budget() {
        assert_eq!(shorten_path("~/.tct", 0), None);
    }
}
