use super::*;

#[test]
fn search_regex_invalid_errors() {
    let tmp = TempDir::new().unwrap();
    ok(&tmp, &["boards", "--create", "Proj"]);
    let e = fail(&tmp, &["search", "[unclosed", "--regex"]);
    assert!(e.contains("Invalid regular expression"), "{e}");
}

#[test]
fn search_no_query_errors() {
    let tmp = TempDir::new().unwrap();
    let e = fail(&tmp, &["search"]);
    assert!(e.contains("Usage:") || e.contains("query"), "{e}");
}

#[test]
fn search_with_query_matches() {
    let tmp = TempDir::new().unwrap();
    ok(&tmp, &["boards", "--create", "Proj"]);
    ok(&tmp, &["lists", "Proj", "--create", "Tasks"]);
    ok(&tmp, &["cards", "Proj", "--create", "Tasks", "Find me"]);

    let out = ok(&tmp, &["search", "Find"]);
    assert!(out.contains("Find me"), "{out}");
    assert!(out.contains("Found 1 card"), "{out}");
}
