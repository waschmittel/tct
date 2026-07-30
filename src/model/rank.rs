//! Fractional ranking, shared by the two ordered collections: Cards within a
//! List and Boards within the selector (ADR-0006/0008).
//!
//! A move sets the moved record's rank to the midpoint of its new neighbours,
//! so reordering rewrites exactly one file instead of reindexing the rest.

/// A rank that orders strictly between `prev` and `next`.
pub fn between(prev: Option<f64>, next: Option<f64>) -> f64 {
    match (prev, next) {
        (None, None) => 1.0,
        (Some(p), None) => p + 1.0,
        (None, Some(n)) => n - 1.0,
        (Some(p), Some(n)) => (p + n) / 2.0,
    }
}

/// The rank that places a record after everything in `existing`.
pub fn after_last(existing: impl IntoIterator<Item = f64>) -> f64 {
    between(existing.into_iter().reduce(f64::max), None)
}

/// Ascending rank order, with NaN treated as equal so a hand-edited file
/// cannot panic the sort.
pub fn cmp(a: f64, b: f64) -> std::cmp::Ordering {
    a.partial_cmp(&b).unwrap_or(std::cmp::Ordering::Equal)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn between_empty_is_one() {
        assert_eq!(between(None, None), 1.0);
    }

    #[test]
    fn between_appends_and_prepends() {
        assert_eq!(between(Some(3.0), None), 4.0);
        assert_eq!(between(None, Some(3.0)), 2.0);
    }

    #[test]
    fn between_takes_the_midpoint() {
        assert_eq!(between(Some(1.0), Some(2.0)), 1.5);
        assert!(between(Some(1.0), Some(1.5)) > 1.0);
    }

    #[test]
    fn after_last_follows_the_maximum() {
        assert_eq!(after_last([1.0, 5.0, 3.0]), 6.0);
        assert_eq!(after_last([]), 1.0);
    }

    #[test]
    fn cmp_orders_ascending_and_survives_nan() {
        assert_eq!(cmp(1.0, 2.0), std::cmp::Ordering::Less);
        assert_eq!(cmp(f64::NAN, 1.0), std::cmp::Ordering::Equal);
    }
}
