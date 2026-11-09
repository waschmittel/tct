//! Card-description markdown, parsed into the blocks the PDF layout draws.
//!
//! Mirrors the dialect the TUI preview understands (`ui/markdown.rs`): `#`
//! headings, `-`/`*`/`1.` list items, `>` citations, fenced code, pipe
//! tables, and `**bold**`, `*italic*`, `***both***`, `~~strike~~` and `` `code` `` inline.
//! Like the preview, every source line stays its own visual line.

use crate::insert::table::{Table, is_table_line};
use crate::ui::markdown::{citation_marker_len, is_quote_fence, table_block_len};

#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct Style {
    pub bold: bool,
    pub italic: bool,
    pub strike: bool,
    pub code: bool,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Run {
    pub text: String,
    pub style: Style,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Block {
    Blank,
    Heading { level: u8, runs: Vec<Run> },
    Text {
        /// Nesting depth of a list item (two source spaces per level).
        level: usize,
        marker: Option<String>,
        quote_depth: usize,
        runs: Vec<Run>,
    },
    Code(Vec<String>),
    Table(Table),
}

pub fn parse(markdown: &str) -> Vec<Block> {
    let lines: Vec<&str> = markdown.lines().collect();
    let mut blocks = Vec::new();
    let mut idx = 0;
    let mut in_quote = false;
    while idx < lines.len() {
        let line = lines[idx];
        if !line.starts_with("```") && is_quote_fence(line) {
            in_quote = !in_quote;
            idx += 1;
            continue;
        }
        if in_quote && !line.starts_with("```") {
            let mut block = parse_line(line);
            if let Block::Text { quote_depth, .. } = &mut block {
                *quote_depth += 1;
            }
            blocks.push(block);
            idx += 1;
            continue;
        }
        if line.starts_with("```") {
            let body: Vec<String> = lines[idx + 1..]
                .iter()
                .take_while(|l| !l.starts_with("```"))
                .map(|l| l.replace('\t', "    "))
                .collect();
            idx += body.len() + 2;
            blocks.push(Block::Code(body));
            continue;
        }
        if is_table_line(line)
            && let Some(len) = table_block_len(&lines[idx..])
        {
            let owned: Vec<String> = lines[idx..idx + len].iter().map(|l| l.to_string()).collect();
            if let Some(table) = Table::parse(&owned) {
                blocks.push(Block::Table(table));
                idx += len;
                continue;
            }
        }
        blocks.push(parse_line(line));
        idx += 1;
    }
    blocks
}

fn parse_line(line: &str) -> Block {
    if line.trim().is_empty() {
        return Block::Blank;
    }
    if let Some((level, rest)) = heading(line) {
        return Block::Heading { level, runs: inline(rest, Style::default()) };
    }

    let trimmed = line.trim_start();
    let indent = line[..line.len() - trimmed.len()].replace('\t', "    ").len();

    if let Some(marker_len) = citation_marker_len(trimmed) {
        let quote_depth = trimmed[..marker_len].matches('>').count();
        return Block::Text {
            level: 0,
            marker: None,
            quote_depth,
            runs: inline(&trimmed[marker_len..], Style::default()),
        };
    }
    if let Some(rest) = trimmed.strip_prefix("- ").or_else(|| trimmed.strip_prefix("* ")) {
        let marker = if indent >= 2 { "◦" } else { "•" };
        return list_item(indent, marker.to_string(), rest);
    }
    if let Some(dot) = trimmed.find(". ")
        && trimmed[..dot].parse::<u64>().is_ok()
    {
        return list_item(indent, format!("{}.", &trimmed[..dot]), &trimmed[dot + 2..]);
    }
    Block::Text { level: 0, marker: None, quote_depth: 0, runs: inline(line, Style::default()) }
}

fn list_item(indent: usize, marker: String, text: &str) -> Block {
    Block::Text {
        level: indent / 2,
        marker: Some(marker),
        quote_depth: 0,
        runs: inline(text, Style::default()),
    }
}

fn heading(line: &str) -> Option<(u8, &str)> {
    let hashes = line.chars().take_while(|&c| c == '#').count();
    if !(1..=6).contains(&hashes) {
        return None;
    }
    line[hashes..].strip_prefix(' ').map(|rest| (hashes as u8, rest))
}

/// Split `text` into styled runs; `base` is the style of unmarked text.
pub fn inline(text: &str, base: Style) -> Vec<Run> {
    let chars: Vec<char> = text.chars().collect();
    let mut runs = Vec::new();
    let mut plain = String::new();
    let mut i = 0;

    let flush = |plain: &mut String, runs: &mut Vec<Run>| {
        if !plain.is_empty() {
            runs.push(Run { text: std::mem::take(plain), style: base });
        }
    };

    while i < chars.len() {
        let marker = [("***", 3), ("**", 2), ("~~", 2), ("*", 1), ("`", 1)]
            .into_iter()
            .find(|(m, _)| chars[i..].starts_with(&m.chars().collect::<Vec<_>>()));
        if let Some((m, len)) = marker
            && let Some(end) = find_close(&chars, i + len, m, m == "*")
        {
            flush(&mut plain, &mut runs);
            let mut style = base;
            match m {
                "***" => {
                    style.bold = true;
                    style.italic = true;
                }
                "**" => style.bold = true,
                "~~" => style.strike = true,
                "*" => style.italic = true,
                _ => style.code = true,
            }
            runs.push(Run { text: chars[i + len..end].iter().collect(), style });
            i = end + len;
            continue;
        }
        plain.push(chars[i]);
        i += 1;
    }
    flush(&mut plain, &mut runs);
    runs
}

fn find_close(chars: &[char], from: usize, marker: &str, single_star: bool) -> Option<usize> {
    let pat: Vec<char> = marker.chars().collect();
    (from..chars.len()).find(|&i| {
        chars[i..].starts_with(&pat)
            && i > from
            && !(single_star && chars.get(i + 1) == Some(&'*'))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plain(runs: &[Run]) -> String {
        runs.iter().map(|r| r.text.as_str()).collect()
    }

    #[test]
    fn inline_markers_become_styled_runs() {
        let runs = inline("a **b** *c* ~~d~~ `e` f", Style::default());
        assert_eq!(plain(&runs), "a b c d e f");
        assert!(runs.iter().any(|r| r.text == "b" && r.style.bold));
        assert!(runs.iter().any(|r| r.text == "c" && r.style.italic));
        assert!(runs.iter().any(|r| r.text == "d" && r.style.strike));
        assert!(runs.iter().any(|r| r.text == "e" && r.style.code));
    }

    #[test]
    fn triple_star_is_bold_italic() {
        let runs = inline("a ***b*** c", Style::default());
        assert_eq!(plain(&runs), "a b c");
        assert!(runs.iter().any(|r| r.text == "b" && r.style.bold && r.style.italic));
    }

    #[test]
    fn unclosed_markers_stay_literal() {
        let runs = inline("2 * 3 and **open", Style::default());
        assert!(runs.iter().all(|r| !r.style.bold));
        assert_eq!(plain(&inline("a ** b", Style::default())), "a ** b");
    }

    #[test]
    fn lines_classify_into_blocks() {
        let blocks = parse("# Title\n\n- one\n  - two\n3. three\n> quoted\n```\nlet x;\n```\ntext");
        assert!(matches!(blocks[0], Block::Heading { level: 1, .. }));
        assert_eq!(blocks[1], Block::Blank);
        assert!(matches!(&blocks[2], Block::Text { level: 0, marker: Some(m), .. } if m == "•"));
        assert!(matches!(&blocks[3], Block::Text { level: 1, marker: Some(m), .. } if m == "◦"));
        assert!(matches!(&blocks[4], Block::Text { marker: Some(m), .. } if m == "3."));
        assert!(matches!(&blocks[5], Block::Text { quote_depth: 1, .. }));
        assert_eq!(blocks[6], Block::Code(vec!["let x;".into()]));
        assert!(matches!(&blocks[7], Block::Text { marker: None, .. }));
    }

    #[test]
    fn multiline_quote_fences_quote_the_lines_between() {
        let blocks = parse("a\n>>>\none\n\ntwo\n>>>\nb");
        assert!(matches!(&blocks[0], Block::Text { quote_depth: 0, .. }));
        assert!(matches!(&blocks[1], Block::Text { quote_depth: 1, .. }));
        assert!(matches!(&blocks[2], Block::Blank));
        assert!(matches!(&blocks[3], Block::Text { quote_depth: 1, .. }));
        assert!(matches!(&blocks[4], Block::Text { quote_depth: 0, .. }));
        assert_eq!(blocks.len(), 5);
    }

    #[test]
    fn pipe_tables_are_one_block() {
        let blocks = parse("| a | b |\n|---|--:|\n| 1 | 2 |\nafter");
        assert!(matches!(&blocks[0], Block::Table(t) if t.rows.len() == 2 && t.cols() == 2));
        assert_eq!(blocks.len(), 2);
    }

    #[test]
    fn unterminated_fence_runs_to_the_end() {
        assert_eq!(parse("```\na\nb"), vec![Block::Code(vec!["a".into(), "b".into()])]);
    }
}
