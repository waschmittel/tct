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

fn lines(s: &str) -> Vec<&str> {
    if s.is_empty() { Vec::new() } else { s.split('\n').collect() }
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
            vec![(Same, "a"), (Removed, "b"), (Added, "x"), (Same, "c"), (Added, "d")]
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
}
