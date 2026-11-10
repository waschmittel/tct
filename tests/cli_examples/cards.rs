use super::*;

// ── Card tests ────────────────────────────────────────────────────────────────

#[test]
fn cards_create_list_show_edit() {
    let tmp = TempDir::new().unwrap();
    ok(&tmp, &["boards", "--create", "Proj"]);
    ok(&tmp, &["lists", "Proj", "--create", "To Do"]);
    ok(&tmp, &["lists", "Proj", "--create", "Done"]);

    let out = ok(&tmp, &["cards", "Proj", "--create", "To Do", "Fix the bug"]);
    assert!(
        out.contains("Created card 'Fix the bug' in list 'To Do' on board 'Proj'"),
        "{out}"
    );

    let out = ok(&tmp, &["cards", "Proj"]);
    assert!(out.contains("Fix the bug"), "{out}");

    let out = ok(&tmp, &["cards", "Proj", "--list", "To Do"]);
    assert!(out.contains("Fix the bug"), "{out}");

    let out = ok(&tmp, &["cards", "Proj", "--show", "Fix"]);
    assert!(out.contains("Card:"), "{out}");
    assert!(out.contains("Fix the bug"), "{out}");

    let out = ok(
        &tmp,
        &[
            "cards",
            "Proj",
            "--edit",
            "Fix",
            "--title",
            "Fix login bug",
            "--due",
            "2099-12-31",
        ],
    );
    assert!(out.contains("Updated card 'Fix login bug'"), "{out}");

    let out = ok(&tmp, &["cards", "Proj", "--show", "Fix"]);
    assert!(out.contains("Fix login bug"), "{out}");
    assert!(out.contains("2099-12-31"), "{out}");
}

#[test]
fn cards_archive_restore_delete() {
    let tmp = TempDir::new().unwrap();
    ok(&tmp, &["boards", "--create", "Proj"]);
    ok(&tmp, &["lists", "Proj", "--create", "Tasks"]);
    ok(&tmp, &["cards", "Proj", "--create", "Tasks", "Fix bug"]);

    let out = ok(&tmp, &["cards", "Proj", "--archive", "Fix bug"]);
    assert!(out.contains("Archived card 'Fix bug'"), "{out}");

    let out = ok(&tmp, &["cards", "Proj", "--archived"]);
    assert!(out.contains("Fix bug"), "{out}");

    let out = ok(&tmp, &["cards", "Proj", "--restore", "Fix bug"]);
    assert!(out.contains("Restored card 'Fix bug'"), "{out}");

    ok(&tmp, &["cards", "Proj", "--archive", "Fix bug"]);
    let out = ok(&tmp, &["cards", "Proj", "--delete", "Fix bug"]);
    assert!(out.contains("Permanently deleted card 'Fix bug'"), "{out}");
}

#[test]
fn cards_edit_no_fields_errors() {
    let tmp = TempDir::new().unwrap();
    ok(&tmp, &["boards", "--create", "Proj"]);
    ok(&tmp, &["lists", "Proj", "--create", "Tasks"]);
    ok(&tmp, &["cards", "Proj", "--create", "Tasks", "My card"]);

    let e = fail(&tmp, &["cards", "Proj", "--edit", "My card"]);
    assert!(
        e.contains("--title") || e.contains("--description") || e.contains("--due"),
        "{e}"
    );
}

// ── Additional gap-filling tests ──────────────────────────────────────────────

#[test]
fn cards_archive_nonexistent_errors() {
    let tmp = TempDir::new().unwrap();
    ok(&tmp, &["boards", "--create", "Proj"]);
    ok(&tmp, &["lists", "Proj", "--create", "Tasks"]);

    let e = fail(&tmp, &["cards", "Proj", "--archive", "ghost"]);
    assert!(e.contains("No card matches"), "{e}");
}

#[test]
fn cards_show_nonexistent_errors() {
    let tmp = TempDir::new().unwrap();
    ok(&tmp, &["boards", "--create", "Proj"]);
    ok(&tmp, &["lists", "Proj", "--create", "Tasks"]);

    let e = fail(&tmp, &["cards", "Proj", "--show", "ghost"]);
    assert!(e.contains("No card matches"), "{e}");
}

#[test]
fn cards_restore_nonexistent_errors() {
    let tmp = TempDir::new().unwrap();
    ok(&tmp, &["boards", "--create", "Proj"]);

    let e = fail(&tmp, &["cards", "Proj", "--restore", "ghost"]);
    assert!(e.contains("No archived card"), "{e}");
}

#[test]
fn cards_delete_nonexistent_errors() {
    let tmp = TempDir::new().unwrap();
    ok(&tmp, &["boards", "--create", "Proj"]);

    let e = fail(&tmp, &["cards", "Proj", "--delete", "ghost"]);
    assert!(e.contains("No archived card"), "{e}");
}

#[test]
fn cards_create_in_nonexistent_list_errors() {
    let tmp = TempDir::new().unwrap();
    ok(&tmp, &["boards", "--create", "Proj"]);
    let e = fail(&tmp, &["cards", "Proj", "--create", "ghost", "title"]);
    assert!(e.contains("No list matches"), "{e}");
}

#[test]
fn cards_edit_due_invalid_date_errors() {
    let tmp = TempDir::new().unwrap();
    ok(&tmp, &["boards", "--create", "Proj"]);
    ok(&tmp, &["lists", "Proj", "--create", "Tasks"]);
    ok(&tmp, &["cards", "Proj", "--create", "Tasks", "Card"]);

    let e = fail(
        &tmp,
        &["cards", "Proj", "--edit", "Card", "--due", "not-a-date"],
    );
    assert!(e.contains("Invalid date format"), "{e}");
}

#[test]
fn cards_edit_description() {
    let tmp = TempDir::new().unwrap();
    ok(&tmp, &["boards", "--create", "Proj"]);
    ok(&tmp, &["lists", "Proj", "--create", "Tasks"]);
    ok(&tmp, &["cards", "Proj", "--create", "Tasks", "Card"]);

    ok(
        &tmp,
        &[
            "cards",
            "Proj",
            "--edit",
            "Card",
            "--description",
            "new desc text",
        ],
    );
    let out = ok(&tmp, &["cards", "Proj", "--show", "Card"]);
    assert!(out.contains("new desc text"), "{out}");
}

#[test]
fn cards_edit_due_clear_via_none() {
    let tmp = TempDir::new().unwrap();
    ok(&tmp, &["boards", "--create", "Proj"]);
    ok(&tmp, &["lists", "Proj", "--create", "Tasks"]);
    ok(&tmp, &["cards", "Proj", "--create", "Tasks", "Card"]);

    ok(
        &tmp,
        &["cards", "Proj", "--edit", "Card", "--due", "2099-01-01"],
    );
    let out = ok(&tmp, &["cards", "Proj", "--show", "Card"]);
    assert!(out.contains("2099-01-01"), "{out}");

    ok(&tmp, &["cards", "Proj", "--edit", "Card", "--due", "none"]);
    let out = ok(&tmp, &["cards", "Proj", "--show", "Card"]);
    assert!(!out.contains("2099-01-01"), "{out}");
    assert!(!out.contains("Due:"), "{out}");
}

#[test]
fn cards_show_ambiguous_match_errors() {
    let tmp = TempDir::new().unwrap();
    ok(&tmp, &["boards", "--create", "Proj"]);
    ok(&tmp, &["lists", "Proj", "--create", "Tasks"]);
    ok(&tmp, &["cards", "Proj", "--create", "Tasks", "Alpha one"]);
    ok(&tmp, &["cards", "Proj", "--create", "Tasks", "Alpha two"]);

    let e = fail(&tmp, &["cards", "Proj", "--show", "Alpha"]);
    assert!(e.contains("Multiple cards match"), "{e}");
}
