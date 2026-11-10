//! GitHub/GitLab-style footnotes: a reference `text[^label]` and a definition
//! `[^label]: note` anywhere in the text. Shared by the editor (`Ctrl+N`), the
//! read view (styling) and the PDF export (numbering, same-page placement).
//!
//! A definition is a line starting with `[^label]:` plus the lines that
//! continue it: following non-blank lines that start no other block (lazy
//! continuation), and paragraphs after a blank line that are indented by four
//! spaces. Fenced code and inline code are never scanned.

/// Wrap a footnote number in the text handed to the inline parser (private-use
/// characters, stripped from user text first so they cannot be forged).
pub const OPEN: char = '\u{E000}';
pub const CLOSE: char = '\u{E001}';

fn valid_label(label: &str) -> bool {
    !label.is_empty()
        && !label
            .chars()
            .any(|c| c.is_whitespace() || matches!(c, '[' | ']' | '^'))
}

/// `[^label]` at the start of `chars`: its length in chars and the label.
pub fn ref_at(chars: &[char]) -> Option<(usize, String)> {
    if chars.first() != Some(&'[') || chars.get(1) != Some(&'^') {
        return None;
    }
    let end = chars[2..].iter().position(|&c| c == ']')? + 2;
    let label: String = chars[2..end].iter().collect();
    valid_label(&label).then_some((end + 1, label))
}

/// A definition line: the label, the length in chars of the `[^label]:` marker
/// including its indent, and the note text after it.
pub fn def_start(line: &str) -> Option<(String, usize, String)> {
    let indent = line.chars().take_while(|&c| c == ' ').count();
    if indent > 3 {
        return None;
    }
    let chars: Vec<char> = line.chars().skip(indent).collect();
    let (len, label) = ref_at(&chars)?;
    if chars.get(len) != Some(&':') {
        return None;
    }
    let rest: String = chars[len + 1..].iter().collect();
    Some((label, indent + len + 1, rest.trim().to_string()))
}

/// References in a line as (start col in chars, length in chars, label),
/// skipping inline code and the label of a definition line.
pub fn refs(line: &str) -> Vec<(usize, usize, String)> {
    let chars: Vec<char> = line.chars().collect();
    let mut i = def_start(line).map_or(0, |(_, marker, _)| marker);
    let mut in_code = false;
    let mut found = Vec::new();
    while i < chars.len() {
        if chars[i] == '`' {
            in_code = !in_code;
        } else if !in_code
            && chars[i] == '['
            && let Some((len, label)) = ref_at(&chars[i..])
        {
            found.push((i, len, label));
            i += len;
            continue;
        }
        i += 1;
    }
    found
}

/// Rows that are fenced code (the fence lines included).
pub fn fenced_rows<S: AsRef<str>>(lines: &[S]) -> Vec<bool> {
    let mut open = false;
    lines
        .iter()
        .map(|l| {
            let fence = l.as_ref().starts_with("```");
            let inside = open || fence;
            if fence {
                open = !open;
            }
            inside
        })
        .collect()
}

/// A definition and the rows it spans (inclusive).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Def {
    pub label: String,
    pub start: usize,
    pub end: usize,
    /// Note text, one entry per source line (empty for a paragraph break).
    pub lines: Vec<String>,
}

fn indented(line: &str) -> bool {
    line.starts_with("    ") || line.starts_with('\t')
}

/// Does this unindented line start a block of its own (so it ends a note)?
fn starts_block(line: &str) -> bool {
    let numbered = line
        .split_once(". ")
        .is_some_and(|(n, _)| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()));
    line.starts_with('#')
        || line.starts_with("```")
        || line.starts_with('>')
        || line.starts_with("- ")
        || line.starts_with("* ")
        || numbered
}

pub fn definitions<S: AsRef<str>>(lines: &[S]) -> Vec<Def> {
    let fenced = fenced_rows(lines);
    let line = |i: usize| lines[i].as_ref();
    let n = lines.len();
    let mut defs = Vec::new();
    let mut i = 0;
    while i < n {
        let Some((label, _, first)) = (!fenced[i]).then(|| def_start(line(i))).flatten() else {
            i += 1;
            continue;
        };
        let mut body = vec![first];
        let mut end = i;
        let mut j = i + 1;
        while j < n && !fenced[j] {
            if line(j).trim().is_empty() {
                let next = (j..n).find(|&k| !line(k).trim().is_empty());
                match next {
                    Some(k) if !fenced[k] && indented(line(k)) && def_start(line(k)).is_none() => {
                        body.extend((j..k).map(|_| String::new()));
                        j = k;
                        continue;
                    }
                    _ => break,
                }
            }
            let text = line(j);
            if def_start(text).is_some() || (!indented(text) && starts_block(text)) {
                break;
            }
            body.push(text.trim().to_string());
            end = j;
            j += 1;
        }
        defs.push(Def {
            label,
            start: i,
            end,
            lines: body,
        });
        i = end + 1;
    }
    defs
}

/// The definition containing `row`.
pub fn definition_at<S: AsRef<str>>(lines: &[S], row: usize) -> Option<Def> {
    definitions(lines)
        .into_iter()
        .find(|d| (d.start..=d.end).contains(&row))
}

/// Position of the first reference to `label`, preferring text outside
/// definitions.
pub fn first_ref<S: AsRef<str>>(lines: &[S], label: &str) -> Option<(usize, usize)> {
    let fenced = fenced_rows(lines);
    let defs = definitions(lines);
    let in_def = |row: usize| defs.iter().any(|d| (d.start..=d.end).contains(&row));
    let find = |outside: bool| {
        (0..lines.len())
            .filter(|&r| !fenced[r] && in_def(r) != outside)
            .find_map(|r| {
                refs(lines[r].as_ref())
                    .into_iter()
                    .find(|(_, _, l)| l == label)
                    .map(|(col, _, _)| (r, col))
            })
    };
    find(true).or_else(|| find(false))
}

/// The number a new footnote gets: one above the highest numeric label in use.
pub fn next_number<S: AsRef<str>>(lines: &[S]) -> u64 {
    let fenced = fenced_rows(lines);
    lines
        .iter()
        .enumerate()
        .filter(|(r, _)| !fenced[*r])
        .flat_map(|(_, l)| {
            let l = l.as_ref();
            refs(l)
                .into_iter()
                .map(|(_, _, label)| label)
                .chain(def_start(l).map(|(label, _, _)| label))
        })
        .filter_map(|label| label.parse::<u64>().ok())
        .max()
        .map_or(1, |m| m + 1)
}

/// Numbered footnotes have an all-digit label; others are named.
pub fn is_numbered(label: &str) -> bool {
    !label.is_empty() && label.chars().all(|c| c.is_ascii_digit())
}

/// The result of [`renumbered`]: the new lines and the label mapping.
pub struct Renumbered {
    pub lines: Vec<String>,
    map: Vec<(String, String)>,
}

impl Renumbered {
    /// The new label for `old` (named and unknown labels stay).
    pub fn label(&self, old: &str) -> String {
        self.map
            .iter()
            .find(|(o, _)| o == old)
            .map_or_else(|| old.to_string(), |(_, n)| n.clone())
    }

    /// Relabel the references (and the definition label) of one line.
    pub fn relabel(&self, line: &str) -> String {
        let chars: Vec<char> = line.chars().collect();
        let mut spans = refs(line);
        if let Some((label, marker, _)) = def_start(line) {
            // The marker includes the colon; the label span does not.
            let indent = line.chars().take_while(|&c| c == ' ').count();
            spans.insert(0, (indent, marker - indent - 1, label));
        }
        let mut out = String::new();
        let mut at = 0;
        for (col, len, label) in spans {
            out.extend(&chars[at..col]);
            out.push_str(&format!("[^{}]", self.label(&label)));
            at = col + len;
        }
        out.extend(&chars[at..]);
        out
    }
}

/// A label made from highlighted text: whitespace runs become `-`, brackets
/// and carets go, at most 32 chars. `None` if nothing is left or the result
/// would be a number (those are assigned, not chosen).
pub fn label_from(text: &str) -> Option<String> {
    let cleaned: String = text
        .split_whitespace()
        .map(|w| w.replace(['[', ']', '^'], ""))
        .filter(|w| !w.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    let label: String = cleaned.chars().take(32).collect();
    let label = label.trim_end_matches('-').to_string();
    (!label.is_empty() && !is_numbered(&label)).then_some(label)
}

/// Renumber the numbered footnotes of `lines` so that they ascend in the
/// order of their first reference. Only footnotes that form a pair — a label
/// with a reference *and* a definition (`new_label` counts as defined, for a
/// definition about to be added) — are renumbered, consistently in every
/// reference and definition of the label. Dangling references and unreferenced
/// definitions keep their label, and the new numbers skip those labels, so no
/// reference ever starts resolving to another footnote's definition. Named
/// labels and fenced code stay untouched.
pub fn renumbered(lines: &[String], new_label: Option<&str>) -> Renumbered {
    let fenced = fenced_rows(lines);
    let text_rows = || {
        lines
            .iter()
            .zip(&fenced)
            .filter(|(_, f)| !**f)
            .map(|(l, _)| l)
    };
    let mut referenced: Vec<String> = Vec::new();
    let mut defined: Vec<String> = new_label.map(String::from).into_iter().collect();
    let mut all: Vec<String> = Vec::new();
    for line in text_rows() {
        let found = refs(line).into_iter().map(|(_, _, l)| l);
        let labels = found.chain(def_start(line).map(|(l, _, _)| l));
        for label in labels.filter(|l| is_numbered(l)) {
            if !all.contains(&label) {
                all.push(label);
            }
        }
        for (_, _, label) in refs(line) {
            if is_numbered(&label) && !referenced.contains(&label) {
                referenced.push(label);
            }
        }
        if let Some((label, _, _)) = def_start(line) {
            defined.push(label);
        }
    }
    let paired: Vec<&String> = referenced.iter().filter(|l| defined.contains(l)).collect();
    // Labels left alone keep their meaning: new numbers must not land on them.
    let reserved: Vec<&String> = all.iter().filter(|l| !paired.contains(l)).collect();
    let mut next = 0_u64;
    let map = paired
        .into_iter()
        .map(|old| {
            loop {
                next += 1;
                if !reserved.iter().any(|r| **r == next.to_string()) {
                    break;
                }
            }
            (old.clone(), next.to_string())
        })
        .collect();
    let mut result = Renumbered {
        lines: Vec::new(),
        map,
    };
    result.lines = lines
        .iter()
        .zip(&fenced)
        .map(|(line, &fenced)| {
            if fenced {
                line.clone()
            } else {
                result.relabel(line)
            }
        })
        .collect();
    result
}

/// What each reference of `lines` resolves to: the row of the definition it
/// points at (`None` if dangling). Lets tests prove that an
/// edit left every reference pointing at the same footnote.
#[cfg(test)]
pub fn resolved(lines: &[String]) -> Vec<Option<usize>> {
    let fenced = fenced_rows(lines);
    let defs = definitions(lines);
    lines
        .iter()
        .enumerate()
        .filter(|(r, _)| !fenced[*r])
        .flat_map(|(r, l)| refs(l).into_iter().map(move |(c, _, label)| (r, c, label)))
        .map(|(_, _, label)| defs.iter().find(|d| d.label == label).map(|d| d.start))
        .collect()
}

/// A footnote as exported: its marker (the name of a named note, a running
/// count for numbered ones) and its source lines.
pub struct Note {
    pub label: String,
    pub lines: Vec<String>,
}

/// A description split for export: the body without definitions, with each
/// resolvable reference replaced by `OPEN marker CLOSE`, plus the notes in
/// order of first reference. Numbered notes count up in that order (as on
/// GitHub); named ones keep their name.
pub struct Extracted {
    pub body: Vec<String>,
    pub notes: Vec<Note>,
    /// Whether any definition line was removed from the body.
    pub removed: bool,
}

pub fn extract(markdown: &str) -> Extracted {
    let lines: Vec<String> = markdown
        .lines()
        .map(|l| l.replace([OPEN, CLOSE], ""))
        .collect();
    let defs = definitions(&lines);
    let fenced = fenced_rows(&lines);
    let mut dropped = vec![false; lines.len()];
    for d in &defs {
        dropped[d.start..=d.end].fill(true);
    }
    let mut order: Vec<&Def> = Vec::new();
    let (mut markers, mut counted) = (Vec::<String>::new(), 0);
    let mut body = Vec::new();
    for (row, line) in lines.iter().enumerate() {
        if dropped[row] {
            continue;
        }
        if fenced[row] {
            body.push(line.clone());
            continue;
        }
        let chars: Vec<char> = line.chars().collect();
        let mut out = String::new();
        let mut at = 0;
        for (col, len, label) in refs(line) {
            // The first definition of a label wins.
            let Some(def) = defs.iter().find(|d| d.label == label) else {
                continue;
            };
            let marker = match order.iter().position(|d| std::ptr::eq(*d, def)) {
                Some(p) => markers[p].clone(),
                None => {
                    order.push(def);
                    let marker = if is_numbered(&def.label) {
                        counted += 1;
                        counted.to_string()
                    } else {
                        def.label.clone()
                    };
                    markers.push(marker.clone());
                    marker
                }
            };
            out.extend(&chars[at..col]);
            out.push(OPEN);
            out.push_str(&marker);
            out.push(CLOSE);
            at = col + len;
        }
        out.extend(&chars[at..]);
        body.push(out);
    }
    Extracted {
        body,
        notes: order
            .iter()
            .zip(markers)
            .map(|(d, label)| Note {
                label,
                lines: d.lines.clone(),
            })
            .collect(),
        removed: !defs.is_empty(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(text: &str) -> Vec<String> {
        text.lines().map(String::from).collect()
    }

    #[test]
    fn references_need_a_valid_label_and_skip_code() {
        let found = |s: &str| -> Vec<String> { refs(s).into_iter().map(|r| r.2).collect() };
        assert_eq!(found("a[^1] b[^my-note]."), ["1", "my-note"]);
        assert_eq!(found("`x[^1]` y[^2]"), ["2"]);
        assert!(found("[^] [^a b] [^a[b] [^^] [ ^1]").is_empty());
        assert_eq!(refs("é[^é]")[0], (1, 4, "é".into()));
    }

    #[test]
    fn definition_lines() {
        assert_eq!(
            def_start("[^1]: Note"),
            Some(("1".into(), 5, "Note".into()))
        );
        assert_eq!(def_start("   [^a]:x").unwrap().2, "x");
        assert_eq!(def_start("[^a]:").unwrap().2, "");
        assert!(def_start("    [^a]: code").is_none());
        assert!(def_start("[^a] no colon").is_none());
        assert!(def_start("text [^a]: x").is_none());
        // The label of a definition is not a reference; refs in its text are.
        assert_eq!(refs("[^1]: see[^2]").len(), 1);
    }

    #[test]
    fn multi_line_definitions_end_at_blank_lines_and_other_blocks() {
        let lines = v("[^a]: one  \nsecond\n\nbody\n[^b]: two\n# Head\n[^c]: three\n- item");
        let d = definitions(&lines);
        assert_eq!(d.len(), 3);
        assert_eq!((d[0].start, d[0].end), (0, 1));
        assert_eq!(d[0].lines, ["one", "second"]);
        assert_eq!((d[1].start, d[1].end), (4, 4));
        assert_eq!((d[2].start, d[2].end), (6, 6));
    }

    #[test]
    fn indented_paragraphs_continue_a_definition_across_blank_lines() {
        let lines = v("[^a]: one\n\n    para two\n\nafter");
        let d = definitions(&lines);
        assert_eq!((d[0].start, d[0].end), (0, 2));
        assert_eq!(d[0].lines, ["one", "", "para two"]);
    }

    #[test]
    fn consecutive_definitions_and_fenced_code() {
        let lines = v("[^a]: x\n[^b]: y\n```\n[^c]: z\n```");
        let labels: Vec<_> = definitions(&lines).into_iter().map(|d| d.label).collect();
        assert_eq!(labels, ["a", "b"]);
    }

    #[test]
    fn definition_at_and_first_ref() {
        let lines = v("a[^1] b\n[^2]: n\ncont\nc[^2] d[^1]\n[^1]: m");
        assert_eq!(definition_at(&lines, 2).unwrap().label, "2");
        assert!(definition_at(&lines, 0).is_none());
        assert_eq!(first_ref(&lines, "1"), Some((0, 1)));
        assert_eq!(first_ref(&lines, "2"), Some((3, 1)));
        assert_eq!(first_ref(&lines, "9"), None);
        // Only referenced from another definition: fall back to it.
        assert_eq!(first_ref(&v("[^a]: x[^b]\n[^b]: y"), "b"), Some((0, 7)));
    }

    #[test]
    fn next_number_counts_up_from_numeric_labels_only() {
        assert_eq!(next_number(&v("plain")), 1);
        assert_eq!(next_number(&v("a[^1] b[^7]\n[^3]: x\n[^name]: y")), 8);
        assert_eq!(next_number(&v("[^note]: only names")), 1);
        assert_eq!(next_number(&v("```\n[^9]\n```")), 1);
    }

    #[test]
    fn extraction_orders_by_first_reference_and_drops_definitions() {
        let e = extract("x[^b] y[^a] z[^b]\n\n[^a]: A\n[^b]: B  \nmore\n[^unused]: U\n");
        assert_eq!(
            e.body[0],
            format!("x{OPEN}b{CLOSE} y{OPEN}a{CLOSE} z{OPEN}b{CLOSE}")
        );
        assert_eq!(e.body.len(), 2);
        assert_eq!(labels(&e), ["b", "a"]);
        assert_eq!(e.notes[0].lines, ["B", "more"]);
        assert!(e.removed);
    }

    fn labels(e: &Extracted) -> Vec<&str> {
        e.notes.iter().map(|n| n.label.as_str()).collect()
    }

    #[test]
    fn numbered_notes_count_up_by_first_reference_and_named_ones_keep_their_name() {
        let e = extract("a[^7] b[^x] c[^3] d[^7]\n\n[^3]: three\n[^7]: seven\n[^x]: ex");
        assert_eq!(labels(&e), ["1", "x", "2"]);
        assert_eq!(e.notes[0].lines, ["seven"]);
        assert_eq!(e.notes[2].lines, ["three"]);
        assert_eq!(
            e.body[0],
            format!("a{OPEN}1{CLOSE} b{OPEN}x{CLOSE} c{OPEN}2{CLOSE} d{OPEN}1{CLOSE}")
        );
    }

    #[test]
    fn labels_from_highlighted_text() {
        assert_eq!(label_from("hello"), Some("hello".into()));
        assert_eq!(
            label_from("  two  words\n next "),
            Some("two-words-next".into())
        );
        assert_eq!(label_from("a[b]^c"), Some("abc".into()));
        assert_eq!(label_from("[^]"), None);
        assert_eq!(label_from("   "), None);
        assert_eq!(label_from("42"), None);
        let long = label_from(&"word ".repeat(20)).unwrap();
        assert_eq!(long.chars().count(), 32);
        assert!(!long.ends_with('-'));
        assert!(refs(&format!("[^{long}]")).len() == 1);
        assert_eq!(label_from("äöü ß"), Some("äöü-ß".into()));
    }

    #[test]
    fn renumbering_follows_first_reference_and_spares_names_and_code() {
        let r = |t: &str| renumbered(&v(t), None).lines.join("\n");
        assert_eq!(
            r("a[^5] b[^name] c[^2]\n[^2]: two\n[^5]: five\n[^name]: n"),
            "a[^1] b[^name] c[^2]\n[^2]: two\n[^1]: five\n[^name]: n"
        );
        assert_eq!(
            r("x[^9]\n[^9]: n\n```\n[^9]\n```"),
            "x[^1]\n[^1]: n\n```\n[^9]\n```"
        );
        assert_eq!(r("plain"), "plain");
        assert_eq!(r("  [^3]: x[^3]"), "  [^1]: x[^1]");
    }

    fn renumbered_text(text: &str, new_label: Option<&str>) -> String {
        renumbered(&v(text), new_label).lines.join("\n")
    }

    #[test]
    fn dangling_references_and_orphan_definitions_keep_their_label() {
        assert_eq!(
            renumbered_text("a[^5] b[^9]\n[^5]: five\n[^3]: orphan", None),
            "a[^1] b[^9]\n[^1]: five\n[^3]: orphan"
        );
        // Nothing paired, nothing to do.
        let t = "a[^4] b[^2]\n[^7]: x";
        assert_eq!(renumbered_text(t, None), t);
    }

    #[test]
    fn new_numbers_skip_labels_that_were_left_alone() {
        // [^1] dangles: [^5] must not become [^1], or the dangling reference
        // would suddenly point at its definition.
        assert_eq!(
            renumbered_text("a[^5] b[^1]\n[^5]: five", None),
            "a[^2] b[^1]\n[^2]: five"
        );
        assert_eq!(
            renumbered_text("a[^9] b[^1] c[^3]\n[^9]: nine\n[^2]: orphan", None),
            "a[^4] b[^1] c[^3]\n[^4]: nine\n[^2]: orphan"
        );
    }

    #[test]
    fn a_label_about_to_be_defined_counts_as_paired() {
        assert_eq!(
            renumbered_text("a[^7] b[^1]\n[^1]: one", Some("7")),
            "a[^1] b[^2]\n[^2]: one"
        );
    }

    #[test]
    fn renumbering_never_changes_what_a_reference_resolves_to() {
        let cases = [
            // More references than definitions, and the other way round.
            "a[^3] b[^1] c[^2]\n\n[^2]: two",
            "a[^2]\n\n[^1]: one\n[^2]: two\n[^3]: three\n[^4]: four",
            // Several references to one footnote, repeated and nested ones.
            "a[^2] b[^2] c[^1] d[^2]\n\n[^1]: one[^2]\n[^2]: two",
            // Duplicate definitions, leading zeros, names and code mixed in.
            "a[^02] b[^2] c[^n] `d[^2]`\n```\n[^2]: code\n```\n[^02]: zz\n[^2]: two\n[^2]: dup\n[^n]: name",
            // Collisions: the new numbers would land on dangling ones.
            "a[^4] b[^1] c[^2] d[^3]\n[^4]: four\n[^3]: three",
            "a[^9] b[^8] c[^7]\n[^7]: seven\n[^9]: nine",
            // Multi-line definitions.
            "a[^6] b[^5]\n\n[^5]: five  \nmore\n\n    para\n[^6]: six",
        ];
        for case in cases {
            let before = v(case);
            for new_label in [None, Some("99")] {
                let after = renumbered(&before, new_label).lines;
                assert_eq!(
                    resolved(&before),
                    resolved(&after),
                    "{case:?} {new_label:?}"
                );
            }
        }
    }

    #[test]
    fn renumbering_is_stable_when_applied_twice() {
        for case in [
            "a[^3] b[^1] c[^2]\n\n[^2]: two",
            "a[^4] b[^1]\n[^4]: four\n[^1]: one\n[^6]: o",
        ] {
            let once = renumbered(&v(case), None).lines;
            let twice = renumbered(&once, None).lines;
            assert_eq!(once, twice, "{case:?}");
        }
    }

    #[test]
    fn unresolved_references_and_code_stay_literal() {
        let e = extract("a[^1] `b[^2]`\n```\nc[^2]\n```\n[^2]: n");
        assert_eq!(e.body[0], "a[^1] `b[^2]`");
        assert_eq!(e.body[2], "c[^2]");
        assert!(e.notes.is_empty());
    }

    #[test]
    fn duplicate_definitions_first_wins_and_forged_markers_are_stripped() {
        let e = extract(&format!("a[^x]{OPEN}9{CLOSE}\n[^x]: first\n[^x]: second"));
        assert_eq!(e.notes[0].lines, ["first"]);
        assert_eq!(e.body[0], format!("a{OPEN}x{CLOSE}9"));
    }

    #[test]
    fn text_without_footnotes_is_unchanged() {
        let e = extract("a\n\nb");
        assert_eq!(e.body, ["a", "", "b"]);
        assert!(!e.removed);
    }
}
