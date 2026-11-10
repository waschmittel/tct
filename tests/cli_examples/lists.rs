use super::*;

// ── List tests ────────────────────────────────────────────────────────────────

#[test]
fn lists_create_rename_delete() {
    let tmp = TempDir::new().unwrap();
    ok(&tmp, &["boards", "--create", "Proj"]);

    let out = ok(&tmp, &["lists", "Proj", "--create", "To Do"]);
    assert!(
        out.contains("Created list 'To Do' on board 'Proj'"),
        "{out}"
    );

    let out = ok(&tmp, &["lists", "Proj"]);
    assert!(out.contains("To Do"), "{out}");

    let out = ok(&tmp, &["lists", "Proj", "--rename", "To Do", "Backlog"]);
    assert!(out.contains("Renamed list 'To Do' to 'Backlog'"), "{out}");

    let out = ok(&tmp, &["lists", "Proj"]);
    assert!(out.contains("Backlog"), "{out}");
    assert!(!out.contains("To Do"), "{out}");

    let out = ok(&tmp, &["lists", "Proj", "--archive", "Backlog"]);
    assert!(out.contains("Archived list 'Backlog'"), "{out}");

    let out = ok(&tmp, &["lists", "Proj"]);
    assert!(out.contains("(no lists)"), "{out}");

    let out = ok(&tmp, &["lists", "Proj", "--delete", "Backlog"]);
    assert!(out.contains("Permanently deleted list 'Backlog'"), "{out}");
}

#[test]
fn lists_rename_nonexistent_errors() {
    let tmp = TempDir::new().unwrap();
    ok(&tmp, &["boards", "--create", "Proj"]);
    let e = fail(&tmp, &["lists", "Proj", "--rename", "ghost", "newname"]);
    assert!(e.contains("No list matches"), "{e}");
}

#[test]
fn lists_delete_nonexistent_errors() {
    let tmp = TempDir::new().unwrap();
    ok(&tmp, &["boards", "--create", "Proj"]);
    let e = fail(&tmp, &["lists", "Proj", "--delete", "ghost"]);
    assert!(e.contains("No list matches"), "{e}");
}

#[test]
fn deleting_list_removes_its_cards() {
    let tmp = TempDir::new().unwrap();
    ok(&tmp, &["boards", "--create", "Proj"]);
    ok(&tmp, &["lists", "Proj", "--create", "Tasks"]);
    ok(&tmp, &["cards", "Proj", "--create", "Tasks", "Doomed"]);
    ok(&tmp, &["lists", "Proj", "--archive", "Tasks"]);
    ok(&tmp, &["lists", "Proj", "--delete", "Tasks"]);

    // Card should no longer appear in archived either (it's deleted, not archived)
    let out = ok(&tmp, &["cards", "Proj", "--archived"]);
    assert!(!out.contains("Doomed"), "{out}");
}
