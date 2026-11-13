use super::*;

#[test]
fn matrix_first_column_is_bold_and_normal_tables_are_not() {
    let bold = |page: &[Op], t: &str| {
        page.iter()
            .any(|op| matches!(op, Op::Text { text, style, .. } if text == t && style.bold))
    };
    let m = table_page("|  | A |\n|---|---|\n| row | 1 |");
    assert!(bold(&m, "row") && !bold(&m, "1") && bold(&m, "A"));
    let n = table_page("| H | A |\n|---|---|\n| row | 1 |");
    assert!(!bold(&n, "row"));
}

#[test]
fn matrix_corner_has_no_rule_above_and_no_line_left() {
    // Few rows: no row separators, so no top or bottom border either; only
    // the heavy rule under the header remains.
    let m = table_page(&matrix(3, 2));
    let hm = rules(&m, HEAVY_RULE);
    assert_eq!(hm.len(), 1, "just the header rule");
    // With row separators the top rule starts right of the open corner.
    let r = table_page(&matrix(3, 8));
    let (thin, header) = (rules(&r, MIDRULE), rules(&r, HEAVY_RULE)[0]);
    assert!(
        thin[0].1 > header.1 + 10.0,
        "top rule must start right of the corner"
    );
    assert_eq!(thin[0].2, header.2);
    assert!(
        thin.last().unwrap().1 <= header.1 + 0.1,
        "the bottom rule stays whole"
    );
    // Wide matrix: no outer vertical lines, left or right, on any row.
    let g = table_page(&matrix(5, 2));
    let thin = verticals(&g, MIDRULE);
    let (_, left, right) = rules(&g, HEAVY_RULE)[0];
    let (left, right) = (left.min(right), left.max(right));
    let inner = |l: &&(f32, f32, f32)| l.0 > left + 0.1 && l.0 < right - 0.1;
    let heavy = verticals(&g, HEAVY_RULE);
    let all: Vec<_> = thin.iter().chain(&heavy).collect();
    assert!(
        !thin.is_empty() && all.iter().all(inner),
        "outer line drawn"
    );
}

#[test]
fn matrix_labels_are_separated_from_contents_by_heavy_lines() {
    let m = table_page(&matrix(3, 2));
    // Heavy divider right of the labels, over header + body rows; no thin
    // column lines with only 2 content columns.
    assert_eq!(verticals(&m, HEAVY_RULE).len(), 3);
    assert!(verticals(&m, MIDRULE).is_empty());
    assert_eq!(
        rules(&m, MIDRULE).len(),
        0,
        "no row separators and no light header rule"
    );
    assert_eq!(
        rules(&m, HEAVY_RULE).len(),
        1,
        "header rule only; no top or bottom border"
    );
}

#[test]
fn matrix_column_lines_start_at_four_content_columns() {
    let thin = |cols| verticals(&table_page(&matrix(cols, 2)), MIDRULE).len();
    assert_eq!(thin(4), 0, "3 content columns");
    // Inner lines only (3 per row, 3 rows); the divider after the labels is heavy.
    assert_eq!(thin(5), 3 * 3, "4 content columns");
}

#[test]
fn matrix_row_separators_start_above_eight_rows_regardless_of_columns() {
    let seps = |cols, body| rules(&table_page(&matrix(cols, body)), MIDRULE).len();
    assert_eq!(seps(3, 6), 0, "7 rows in all");
    // 6 between the 7 body rows + the (now thin) top and bottom rules
    assert_eq!(seps(3, 7), 8, "8 rows");
    assert_eq!(seps(6, 7), 8, "column count is irrelevant");
    assert_eq!(seps(6, 3), 0, "few rows, many columns: no row separators");
}

#[test]
fn matrix_outer_rules_are_as_thin_as_row_separators_when_there_are_any() {
    let many = table_page(&matrix(3, 7));
    let few = table_page(&matrix(3, 3));
    // Heavy: only the header rule remains in the long one.
    assert_eq!(rules(&many, HEAVY_RULE).len(), 1);
    assert_eq!(
        rules(&few, HEAVY_RULE).len(),
        1,
        "only the header rule: no outer borders"
    );
    assert!(rules(&few, MIDRULE).is_empty());
    // A normal table keeps its outer borders.
    let plain = table_page(&matrix(3, 3).replacen("|  |", "| H |", 1));
    assert_eq!(rules(&plain, HEAVY_RULE).len(), 2);
}

#[test]
fn long_tables_repeat_their_header_on_every_page() {
    let mut card = Card::new("T".into());
    let rows: Vec<String> = (0..120).map(|i| format!("| r{i} | {i} |")).collect();
    card.description = format!("| Name | Qty |\n|---|--:|\n{}", rows.join("\n"));
    let pages = laid_out(&card);
    assert!(pages.len() >= 3);
    for (n, page) in pages.iter().enumerate() {
        assert_eq!(count(page, "Name"), 1, "page {n} lacks the repeated header");
    }
    assert_eq!(pages.iter().map(|p| count(p, "r119")).sum::<usize>(), 1);
}

#[test]
fn a_short_table_is_not_split_across_pages() {
    let mut card = Card::new("T".into());
    let rows: Vec<String> = (0..10).map(|i| format!("| r{i} | {i} |")).collect();
    card.description = format!(
        "{}\n\n| Name | Qty |\n|---|--:|\n{}",
        lines("filler", 38),
        rows.join("\n")
    );
    let pages = laid_out(&card);
    assert_eq!(page_of(&pages, "Name"), page_of(&pages, "r9"));
    assert_eq!(page_of(&pages, "Name"), 1);
}
