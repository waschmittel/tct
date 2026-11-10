//! Line-based diff for the history detail view.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffKind {
    Same,
    Added,
    Removed,
}

/// Diff `old` against `new` line by line (longest common subsequence).
/// Removed lines precede added lines within a changed run.
pub fn diff_lines<'a>(old: &'a str, new: &'a str) -> Vec<(DiffKind, &'a str)> {
    let a: Vec<&str> = lines(old);
    let b: Vec<&str> = lines(new);

    // lcs[i][j] = LCS length of a[i..] and b[j..]
    let mut lcs = vec![vec![0usize; b.len() + 1]; a.len() + 1];
    for i in (0..a.len()).rev() {
        for j in (0..b.len()).rev() {
            lcs[i][j] = if a[i] == b[j] {
                lcs[i + 1][j + 1] + 1
            } else {
                lcs[i + 1][j].max(lcs[i][j + 1])
            };
        }
    }

    let (mut i, mut j) = (0, 0);
    let mut out = Vec::with_capacity(a.len() + b.len());
    while i < a.len() || j < b.len() {
        if i < a.len() && j < b.len() && a[i] == b[j] {
            out.push((DiffKind::Same, a[i]));
            i += 1;
            j += 1;
        } else if j < b.len() && (i == a.len() || lcs[i][j + 1] > lcs[i + 1][j]) {
            out.push((DiffKind::Added, b[j]));
            j += 1;
        } else {
            out.push((DiffKind::Removed, a[i]));
            i += 1;
        }
    }
    out
}

/// Minimum consecutive unchanged lines required to collapse into `[first, "...", last]`.
pub const COLLAPSE_THRESHOLD: usize = 4;

/// Collapse long runs of unchanged lines (`DiffKind::Same`), leaving only the first
/// and last line of the collapsed content with `"..."` in between.
///
/// Runs of fewer than 4 unchanged lines are preserved as-is so short context is not obscured.
pub fn collapse_unchanged<'a>(diff: Vec<(DiffKind, &'a str)>) -> Vec<(DiffKind, &'a str)> {
    let mut out = Vec::with_capacity(diff.len());
    let mut same_run: Vec<&'a str> = Vec::new();

    for (kind, text) in diff {
        match kind {
            DiffKind::Same => {
                same_run.push(text);
            }
            DiffKind::Added | DiffKind::Removed => {
                flush_same_run(&mut same_run, &mut out);
                out.push((kind, text));
            }
        }
    }
    flush_same_run(&mut same_run, &mut out);

    out
}

fn flush_same_run<'a>(same_run: &mut Vec<&'a str>, out: &mut Vec<(DiffKind, &'a str)>) {
    if same_run.len() >= COLLAPSE_THRESHOLD {
        out.push((DiffKind::Same, same_run[0]));
        out.push((DiffKind::Same, "..."));
        out.push((DiffKind::Same, same_run[same_run.len() - 1]));
    } else {
        out.extend(same_run.drain(..).map(|t| (DiffKind::Same, t)));
    }
    same_run.clear();
}

/// Diff `old` against `new` line by line, collapsing long unchanged sections.
pub fn diff_lines_collapsed<'a>(old: &'a str, new: &'a str) -> Vec<(DiffKind, &'a str)> {
    collapse_unchanged(diff_lines(old, new))
}

fn lines(s: &str) -> Vec<&str> {
    if s.is_empty() {
        Vec::new()
    } else {
        s.split('\n').collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use DiffKind::*;

    #[test]
    fn identical_text_is_all_same() {
        assert_eq!(diff_lines("a\nb", "a\nb"), vec![(Same, "a"), (Same, "b")]);
    }

    #[test]
    fn added_and_removed_lines() {
        assert_eq!(
            diff_lines("a\nb\nc", "a\nx\nc\nd"),
            vec![
                (Same, "a"),
                (Removed, "b"),
                (Added, "x"),
                (Same, "c"),
                (Added, "d")
            ]
        );
    }

    #[test]
    fn empty_old_is_all_added() {
        assert_eq!(diff_lines("", "foo"), vec![(Added, "foo")]);
    }

    #[test]
    fn empty_new_is_all_removed() {
        assert_eq!(diff_lines("foo", ""), vec![(Removed, "foo")]);
    }

    #[test]
    fn both_empty_is_empty() {
        assert!(diff_lines("", "").is_empty());
    }

    #[test]
    fn duplicate_lines_diff_by_count() {
        assert_eq!(
            diff_lines("a\na", "a\na\na"),
            vec![(Same, "a"), (Same, "a"), (Added, "a")]
        );
    }

    #[test]
    fn reorder_shows_removal_and_addition() {
        let d = diff_lines("a\nb", "b\na");
        assert_eq!(d.iter().filter(|(k, _)| *k == Same).count(), 1);
        assert_eq!(d.iter().filter(|(k, _)| *k == Added).count(), 1);
        assert_eq!(d.iter().filter(|(k, _)| *k == Removed).count(), 1);
    }

    #[test]
    fn collapse_short_unchanged_runs_are_preserved() {
        // Runs of 1, 2, and 3 lines should not be collapsed.
        let one = vec![(Same, "1")];
        assert_eq!(collapse_unchanged(one.clone()), one);

        let two = vec![(Same, "1"), (Same, "2")];
        assert_eq!(collapse_unchanged(two.clone()), two);

        let three = vec![(Same, "1"), (Same, "2"), (Same, "3")];
        assert_eq!(collapse_unchanged(three.clone()), three);
    }

    #[test]
    fn collapse_four_or_more_unchanged_lines_collapses_middle() {
        // Exactly 4 lines: first line, "...", last line
        let four = vec![(Same, "1"), (Same, "2"), (Same, "3"), (Same, "4")];
        assert_eq!(
            collapse_unchanged(four),
            vec![(Same, "1"), (Same, "..."), (Same, "4")]
        );

        // 10 lines: first line, "...", last line
        let ten: Vec<(DiffKind, &str)> = (1..=10)
            .map(|i| {
                (
                    Same,
                    match i {
                        1 => "line1",
                        2 => "line2",
                        3 => "line3",
                        4 => "line4",
                        5 => "line5",
                        6 => "line6",
                        7 => "line7",
                        8 => "line8",
                        9 => "line9",
                        _ => "line10",
                    },
                )
            })
            .collect();
        assert_eq!(
            collapse_unchanged(ten),
            vec![(Same, "line1"), (Same, "..."), (Same, "line10")]
        );
    }

    #[test]
    fn collapse_unchanged_preserves_added_and_removed() {
        let diff = vec![
            (Same, "1"),
            (Same, "2"),
            (Same, "3"),
            (Same, "4"),
            (Removed, "old"),
            (Added, "new"),
            (Same, "5"),
            (Same, "6"),
        ];
        assert_eq!(
            collapse_unchanged(diff),
            vec![
                (Same, "1"),
                (Same, "..."),
                (Same, "4"),
                (Removed, "old"),
                (Added, "new"),
                (Same, "5"),
                (Same, "6"),
            ]
        );
    }

    #[test]
    fn diff_lines_collapsed_collapses_unchanged_sections() {
        let old = "a\nb\nc\nd\ne\nold_mid\nf\ng\nh\ni\nj";
        let new = "a\nb\nc\nd\ne\nnew_mid\nf\ng\nh\ni\nj";
        assert_eq!(
            diff_lines_collapsed(old, new),
            vec![
                (Same, "a"),
                (Same, "..."),
                (Same, "e"),
                (Removed, "old_mid"),
                (Added, "new_mid"),
                (Same, "f"),
                (Same, "..."),
                (Same, "j"),
            ]
        );
    }
}
