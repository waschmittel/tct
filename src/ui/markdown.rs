use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use unicode_width::UnicodeWidthStr;

pub fn render_markdown(text: &str) -> Vec<Line<'static>> {
    let options = Options::ENABLE_TABLES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS;
    let parser = Parser::new_ext(text, options);

    let mut renderer = MdRenderer::new();
    for event in parser {
        renderer.process(event);
    }
    renderer.flush_line();
    renderer.lines
}

struct MdRenderer {
    lines: Vec<Line<'static>>,
    current_spans: Vec<Span<'static>>,
    style_stack: Vec<Style>,
    list_stack: Vec<ListCtx>,
    in_code_block: bool,
    in_heading: bool,
    heading_level: HeadingLevel,
    in_blockquote: bool,
    // Table state
    in_table: bool,
    in_table_head: bool,
    table_alignments: Vec<pulldown_cmark::Alignment>,
    table_rows: Vec<Vec<String>>,
    current_row: Vec<String>,
    current_cell: String,
}

#[derive(Clone)]
struct ListCtx {
    ordered: bool,
    counter: u64,
}

impl MdRenderer {
    fn new() -> Self {
        Self {
            lines: Vec::new(),
            current_spans: Vec::new(),
            style_stack: vec![Style::default()],
            list_stack: Vec::new(),
            in_code_block: false,
            in_heading: false,
            heading_level: HeadingLevel::H1,
            in_blockquote: false,
            in_table: false,
            in_table_head: false,
            table_alignments: Vec::new(),
            table_rows: Vec::new(),
            current_row: Vec::new(),
            current_cell: String::new(),
        }
    }

    fn current_style(&self) -> Style {
        self.style_stack.last().copied().unwrap_or_default()
    }

    fn push_modifier(&mut self, modifier: Modifier) {
        let new_style = self.current_style().add_modifier(modifier);
        self.style_stack.push(new_style);
    }

    fn pop_style(&mut self) {
        if self.style_stack.len() > 1 {
            self.style_stack.pop();
        }
    }

    fn flush_line(&mut self) {
        if !self.current_spans.is_empty() {
            let spans = std::mem::take(&mut self.current_spans);
            self.lines.push(Line::from(spans));
        }
    }

    fn list_indent(&self) -> String {
        let depth = self.list_stack.len().saturating_sub(1);
        "  ".repeat(depth)
    }

    fn process(&mut self, event: Event<'_>) {
        match event {
            Event::Start(tag) => self.start_tag(tag),
            Event::End(tag) => self.end_tag(tag),
            Event::Text(text) => self.text(&text),
            Event::Code(code) => self.inline_code(&code),
            Event::SoftBreak => {
                if self.in_table {
                    self.current_cell.push(' ');
                } else {
                    self.current_spans.push(Span::raw(" ".to_string()));
                }
            }
            Event::HardBreak => {
                if !self.in_table {
                    self.flush_line();
                }
            }
            Event::Rule => {
                self.flush_line();
                self.lines.push(Line::from(Span::styled(
                    "────────────────────".to_string(),
                    Style::default().fg(Color::DarkGray),
                )));
            }
            Event::TaskListMarker(checked) => {
                let marker = if checked { "[x] " } else { "[ ] " };
                let indent = self.list_indent();
                self.current_spans
                    .push(Span::raw(format!("{indent}  {marker}")));
            }
            _ => {}
        }
    }

    fn start_tag(&mut self, tag: Tag<'_>) {
        match tag {
            Tag::Heading { level, .. } => {
                self.flush_line();
                self.in_heading = true;
                self.heading_level = level;
                let style = match level {
                    HeadingLevel::H1 | HeadingLevel::H2 => Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
                    _ => Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                };
                self.style_stack.push(style);
            }
            Tag::Paragraph => {
                if !self.in_table {
                    self.flush_line();
                }
            }
            Tag::BlockQuote(_) => {
                self.flush_line();
                self.in_blockquote = true;
                let style = self
                    .current_style()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::ITALIC);
                self.style_stack.push(style);
            }
            Tag::CodeBlock(_kind) => {
                self.flush_line();
                self.in_code_block = true;
                self.lines.push(Line::from(Span::styled(
                    "```".to_string(),
                    Style::default().fg(Color::DarkGray),
                )));
            }
            Tag::List(start) => {
                if self.list_stack.is_empty() {
                    self.flush_line();
                }
                self.list_stack.push(ListCtx {
                    ordered: start.is_some(),
                    counter: start.unwrap_or(1),
                });
            }
            Tag::Item => {
                self.flush_line();
                let indent = self.list_indent();
                if let Some(ctx) = self.list_stack.last() {
                    if ctx.ordered {
                        let num = ctx.counter;
                        self.current_spans.push(Span::styled(
                            format!("{indent}{num}. "),
                            Style::default().fg(Color::Cyan),
                        ));
                    } else {
                        self.current_spans.push(Span::styled(
                            format!("{indent}• "),
                            Style::default().fg(Color::Cyan),
                        ));
                    }
                }
            }
            Tag::Emphasis => {
                self.push_modifier(Modifier::ITALIC);
            }
            Tag::Strong => {
                self.push_modifier(Modifier::BOLD);
            }
            Tag::Strikethrough => {
                self.push_modifier(Modifier::CROSSED_OUT);
            }
            Tag::Table(alignments) => {
                self.flush_line();
                self.in_table = true;
                self.table_alignments = alignments;
                self.table_rows.clear();
            }
            Tag::TableHead => {
                self.in_table_head = true;
                self.current_row.clear();
            }
            Tag::TableRow => {
                self.current_row.clear();
            }
            Tag::TableCell => {
                self.current_cell.clear();
            }
            _ => {}
        }
    }

    fn end_tag(&mut self, tag: TagEnd) {
        match tag {
            TagEnd::Heading(_) => {
                self.pop_style();
                self.in_heading = false;
                self.flush_line();
            }
            TagEnd::Paragraph => {
                if !self.in_table {
                    self.flush_line();
                    if !self.in_blockquote && self.list_stack.is_empty() {
                        self.lines.push(Line::raw(""));
                    }
                }
            }
            TagEnd::BlockQuote(_) => {
                self.pop_style();
                self.in_blockquote = false;
                self.flush_line();
            }
            TagEnd::CodeBlock => {
                self.in_code_block = false;
                self.lines.push(Line::from(Span::styled(
                    "```".to_string(),
                    Style::default().fg(Color::DarkGray),
                )));
            }
            TagEnd::List(_) => {
                if let Some(ctx) = self.list_stack.pop() {
                    let _ = ctx;
                }
                if self.list_stack.is_empty() {
                    self.flush_line();
                }
            }
            TagEnd::Item => {
                self.flush_line();
                if let Some(ctx) = self.list_stack.last_mut() {
                    ctx.counter += 1;
                }
            }
            TagEnd::Emphasis | TagEnd::Strong | TagEnd::Strikethrough => {
                self.pop_style();
            }
            TagEnd::Table => {
                self.render_table();
                self.in_table = false;
                self.table_rows.clear();
            }
            TagEnd::TableHead => {
                self.in_table_head = false;
                self.table_rows.push(std::mem::take(&mut self.current_row));
            }
            TagEnd::TableRow => {
                self.table_rows.push(std::mem::take(&mut self.current_row));
            }
            TagEnd::TableCell => {
                self.current_row
                    .push(std::mem::take(&mut self.current_cell));
            }
            _ => {}
        }
    }

    fn text(&mut self, text: &str) {
        if self.in_table {
            self.current_cell.push_str(text);
            return;
        }

        if self.in_code_block {
            for line in text.split('\n') {
                if !line.is_empty() {
                    self.lines.push(Line::from(Span::styled(
                        format!("  {line}"),
                        Style::default().fg(Color::Green),
                    )));
                }
            }
            return;
        }

        if self.in_blockquote {
            let style = self.current_style();
            for (i, line) in text.split('\n').enumerate() {
                if i > 0 {
                    self.flush_line();
                }
                if self.current_spans.is_empty() || i > 0 {
                    self.current_spans.push(Span::styled(
                        "│ ".to_string(),
                        Style::default().fg(Color::Yellow),
                    ));
                }
                self.current_spans
                    .push(Span::styled(line.to_string(), style));
            }
            return;
        }

        let style = self.current_style();
        self.current_spans
            .push(Span::styled(text.to_string(), style));
    }

    fn inline_code(&mut self, code: &str) {
        if self.in_table {
            self.current_cell.push('`');
            self.current_cell.push_str(code);
            self.current_cell.push('`');
            return;
        }
        self.current_spans.push(Span::styled(
            code.to_string(),
            Style::default()
                .fg(Color::Green)
                .bg(Color::Rgb(40, 40, 40)),
        ));
    }

    fn render_table(&mut self) {
        if self.table_rows.is_empty() {
            return;
        }

        let num_cols = self
            .table_rows
            .iter()
            .map(|r| r.len())
            .max()
            .unwrap_or(0);
        if num_cols == 0 {
            return;
        }

        let mut col_widths = vec![3usize; num_cols];
        for row in &self.table_rows {
            for (i, cell) in row.iter().enumerate() {
                col_widths[i] = col_widths[i].max(UnicodeWidthStr::width(cell.as_str()));
            }
        }

        for (ri, row) in self.table_rows.iter().enumerate() {
            let mut spans = vec![Span::raw(" ".to_string())];
            for (ci, cell) in row.iter().enumerate() {
                if ci > 0 {
                    spans.push(Span::styled(
                        " │ ".to_string(),
                        Style::default().fg(Color::DarkGray),
                    ));
                }
                let w = col_widths[ci];
                let padded = format!("{:<width$}", cell, width = w);
                let style = if ri == 0 {
                    Style::default().add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };
                spans.push(Span::styled(padded, style));
            }
            self.lines.push(Line::from(spans));

            // Separator after header
            if ri == 0 {
                let mut sep_spans = vec![Span::raw(" ".to_string())];
                for (ci, &w) in col_widths.iter().enumerate() {
                    if ci > 0 {
                        sep_spans.push(Span::styled(
                            "─┼─".to_string(),
                            Style::default().fg(Color::DarkGray),
                        ));
                    }
                    sep_spans.push(Span::styled(
                        "─".repeat(w),
                        Style::default().fg(Color::DarkGray),
                    ));
                }
                self.lines.push(Line::from(sep_spans));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_header() {
        let lines = render_markdown("# Hello");
        assert!(!lines.is_empty());
    }

    #[test]
    fn test_code_block() {
        let lines = render_markdown("```\nfn main() {}\n```");
        assert!(lines.len() >= 3);
    }

    #[test]
    fn test_inline_code() {
        let lines = render_markdown("use `foo` here");
        assert!(!lines.is_empty());
    }

    #[test]
    fn test_bold() {
        let lines = render_markdown("this is **bold** text");
        assert!(!lines.is_empty());
    }

    #[test]
    fn test_list_items() {
        let lines = render_markdown("- item 1\n- item 2");
        assert!(lines.len() >= 2);
    }

    #[test]
    fn test_table() {
        let md = "| Name | Value |\n| --- | --- |\n| a | 1 |\n| b | 2 |";
        let lines = render_markdown(md);
        assert!(lines.len() >= 4);
    }

    #[test]
    fn test_blockquote() {
        let lines = render_markdown("> quoted text");
        assert!(!lines.is_empty());
    }

    #[test]
    fn test_ordered_list() {
        let lines = render_markdown("1. first\n2. second\n3. third");
        assert!(lines.len() >= 3);
    }

    #[test]
    fn test_strikethrough() {
        let lines = render_markdown("~~deleted~~ text");
        assert!(!lines.is_empty());
    }

    #[test]
    fn test_task_list() {
        let lines = render_markdown("- [x] done\n- [ ] todo");
        assert!(lines.len() >= 2);
    }

    #[test]
    fn test_nested_list() {
        let lines = render_markdown("- parent\n  - child\n  - child2");
        assert!(lines.len() >= 2);
    }
}
