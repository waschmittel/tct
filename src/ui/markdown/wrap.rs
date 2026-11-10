use ratatui::text::{Line, Span};

pub(super) fn wrap_spans(spans: Vec<Span<'static>>, max_width: usize) -> Vec<Line<'static>> {
    wrap_spans_with_indent(spans, max_width, 0)
}

pub(in crate::ui) fn wrap_spans_with_indent(
    spans: Vec<Span<'static>>,
    max_width: usize,
    indent: usize,
) -> Vec<Line<'static>> {
    if max_width == 0 {
        return vec![Line::from(spans)];
    }

    // Cap the hanging indent so continuation lines always keep room for text;
    // an indent >= the width would otherwise never fit and overflow the view.
    let indent = indent.min(max_width / 2);
    let width_for = |first_line: bool| {
        if first_line {
            max_width
        } else {
            max_width - indent
        }
    };

    let mut result: Vec<Line<'static>> = Vec::new();
    let mut current_spans: Vec<Span<'static>> = Vec::new();
    let mut current_width: usize = 0;
    let mut is_first_line = true;

    for span in spans {
        let style = span.style;
        let text = span.content.to_string();
        let span_width = text.chars().count();

        if current_width + span_width <= width_for(is_first_line) {
            current_width += span_width;
            current_spans.push(Span::styled(text, style));
            continue;
        }

        let mut remaining = text.as_str();
        while !remaining.is_empty() {
            // Recomputed every iteration: the first break inside a span moves
            // us onto an indented continuation line with a narrower budget.
            let budget = width_for(is_first_line).saturating_sub(current_width);
            if budget == 0 {
                result.push(Line::from(std::mem::take(&mut current_spans)));
                is_first_line = false;
                current_width = 0;
                continue;
            }

            let rem_width = remaining.chars().count();
            if rem_width <= budget {
                current_width += rem_width;
                current_spans.push(Span::styled(remaining.to_string(), style));
                break;
            }

            // Find the character boundary for the budget
            let mut split_idx = 0;
            for (count, (idx, c)) in remaining.char_indices().enumerate() {
                if count >= budget {
                    break;
                }
                split_idx = idx + c.len_utf8();
            }

            let slice = &remaining[..split_idx];
            let break_at = match slice.rfind(' ') {
                Some(pos) if pos > 0 => pos,
                _ => {
                    // No space to break at. If the line already has text and the
                    // next word fits on a fresh line, move it there whole instead
                    // of splitting it mid-word.
                    let next_word = remaining.split(' ').next().unwrap_or("");
                    if current_width > 0
                        && !remaining.starts_with(' ')
                        && next_word.chars().count() <= width_for(false)
                    {
                        result.push(Line::from(std::mem::take(&mut current_spans)));
                        is_first_line = false;
                        current_width = 0;
                        continue;
                    }
                    split_idx
                }
            };

            let (chunk, rest) = remaining.split_at(break_at);
            let rest = rest.strip_prefix(' ').unwrap_or(rest);

            if !chunk.is_empty() {
                current_spans.push(Span::styled(chunk.to_string(), style));
            }
            result.push(Line::from(std::mem::take(&mut current_spans)));
            is_first_line = false;
            current_width = 0;
            remaining = rest;
        }
    }

    if !current_spans.is_empty() {
        result.push(Line::from(current_spans));
    }

    if result.is_empty() {
        result.push(Line::from(Vec::<Span<'static>>::new()));
    }

    // Apply indentation to all lines except the first one
    if indent > 0 && result.len() > 1 {
        let indent_str = " ".repeat(indent);
        for line in result.iter_mut().skip(1) {
            line.spans.insert(0, Span::raw(indent_str.clone()));
        }
    }

    result
}
