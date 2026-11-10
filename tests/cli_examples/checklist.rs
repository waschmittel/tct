use super::*;

// ── Checklist tests ───────────────────────────────────────────────────────────

#[test]
fn checklist_add_toggle_delete() {
    let tmp = TempDir::new().unwrap();
    ok(&tmp, &["boards", "--create", "Proj"]);
    ok(&tmp, &["lists", "Proj", "--create", "Tasks"]);
    ok(&tmp, &["cards", "Proj", "--create", "Tasks", "Write docs"]);

    let out = ok(
        &tmp,
        &["checklist", "Proj", "Write docs", "--add", "Draft outline"],
    );
    assert!(
        out.contains("Added checklist item 'Draft outline' to card 'Write docs'"),
        "{out}"
    );

    ok(
        &tmp,
        &["checklist", "Proj", "Write docs", "--add", "Write content"],
    );

    let out = ok(&tmp, &["checklist", "Proj", "Write docs"]);
    assert!(out.contains("Draft outline"), "{out}");
    assert!(out.contains("Write content"), "{out}");
    assert!(out.contains("[ ]"), "{out}");

    let out = ok(&tmp, &["checklist", "Proj", "Write docs", "--toggle", "1"]);
    assert!(out.contains("done"), "{out}");

    let out = ok(&tmp, &["checklist", "Proj", "Write docs"]);
    assert!(out.contains("[x]"), "{out}");

    let out = ok(&tmp, &["checklist", "Proj", "Write docs", "--delete", "2"]);
    assert!(
        out.contains("Deleted checklist item 'Write content'"),
        "{out}"
    );

    let out = ok(&tmp, &["checklist", "Proj", "Write docs"]);
    assert!(!out.contains("Write content"), "{out}");
    assert!(out.contains("Draft outline"), "{out}");
}

// ── Checklist error-path tests ────────────────────────────────────────────────

#[test]
fn checklist_toggle_index_zero_errors() {
    let tmp = TempDir::new().unwrap();
    ok(&tmp, &["boards", "--create", "Proj"]);
    ok(&tmp, &["lists", "Proj", "--create", "Tasks"]);
    ok(&tmp, &["cards", "Proj", "--create", "Tasks", "My card"]);
    ok(&tmp, &["checklist", "Proj", "My card", "--add", "item"]);

    let e = fail(&tmp, &["checklist", "Proj", "My card", "--toggle", "0"]);
    assert!(e.contains("Index must be >= 1"), "{e}");
}

#[test]
fn checklist_toggle_out_of_range_errors() {
    let tmp = TempDir::new().unwrap();
    ok(&tmp, &["boards", "--create", "Proj"]);
    ok(&tmp, &["lists", "Proj", "--create", "Tasks"]);
    ok(&tmp, &["cards", "Proj", "--create", "Tasks", "My card"]);
    ok(
        &tmp,
        &["checklist", "Proj", "My card", "--add", "only item"],
    );

    let e = fail(&tmp, &["checklist", "Proj", "My card", "--toggle", "99"]);
    assert!(e.contains("out of range"), "{e}");
}

#[test]
fn checklist_delete_index_zero_errors() {
    let tmp = TempDir::new().unwrap();
    ok(&tmp, &["boards", "--create", "Proj"]);
    ok(&tmp, &["lists", "Proj", "--create", "Tasks"]);
    ok(&tmp, &["cards", "Proj", "--create", "Tasks", "My card"]);
    ok(&tmp, &["checklist", "Proj", "My card", "--add", "item"]);

    let e = fail(&tmp, &["checklist", "Proj", "My card", "--delete", "0"]);
    assert!(e.contains("Index must be >= 1"), "{e}");
}

#[test]
fn checklist_show_empty() {
    let tmp = TempDir::new().unwrap();
    ok(&tmp, &["boards", "--create", "Proj"]);
    ok(&tmp, &["lists", "Proj", "--create", "Tasks"]);
    ok(&tmp, &["cards", "Proj", "--create", "Tasks", "Card"]);

    let out = ok(&tmp, &["checklist", "Proj", "Card"]);
    assert!(out.contains("no items"), "{out}");
}

#[test]
fn checklist_delete_out_of_range_errors() {
    let tmp = TempDir::new().unwrap();
    ok(&tmp, &["boards", "--create", "Proj"]);
    ok(&tmp, &["lists", "Proj", "--create", "Tasks"]);
    ok(&tmp, &["cards", "Proj", "--create", "Tasks", "Card"]);
    ok(&tmp, &["checklist", "Proj", "Card", "--add", "item"]);

    let e = fail(&tmp, &["checklist", "Proj", "Card", "--delete", "99"]);
    assert!(e.contains("out of range"), "{e}");
}

#[test]
fn checklist_toggle_non_numeric_errors() {
    let tmp = TempDir::new().unwrap();
    ok(&tmp, &["boards", "--create", "Proj"]);
    ok(&tmp, &["lists", "Proj", "--create", "Tasks"]);
    ok(&tmp, &["cards", "Proj", "--create", "Tasks", "Card"]);
    ok(&tmp, &["checklist", "Proj", "Card", "--add", "item"]);

    let e = fail(&tmp, &["checklist", "Proj", "Card", "--toggle", "abc"]);
    assert!(e.contains("positive integer"), "{e}");
}
