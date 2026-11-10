use super::*;

// ── Label tests ───────────────────────────────────────────────────────────────

#[test]
fn labels_create_assign_remove_delete() {
    let tmp = TempDir::new().unwrap();
    ok(&tmp, &["boards", "--create", "Proj"]);
    ok(&tmp, &["lists", "Proj", "--create", "Tasks"]);
    ok(&tmp, &["cards", "Proj", "--create", "Tasks", "Fix bug"]);

    let out = ok(&tmp, &["labels", "Proj", "--create", "bug"]);
    assert!(out.contains("Created label 'bug' on board 'Proj'"), "{out}");

    ok(&tmp, &["labels", "Proj", "--create", "urgent"]);

    let out = ok(&tmp, &["labels", "Proj"]);
    assert!(out.contains("bug"), "{out}");
    assert!(out.contains("urgent"), "{out}");

    let out = ok(&tmp, &["labels", "Proj", "--assign", "Fix bug", "bug"]);
    assert!(
        out.contains("Assigned label 'bug' to card 'Fix bug'"),
        "{out}"
    );

    // Label visible in card detail
    let out = ok(&tmp, &["cards", "Proj", "--show", "Fix bug"]);
    assert!(out.contains("Labels:"), "{out}");
    assert!(out.contains("bug"), "{out}");

    let out = ok(&tmp, &["labels", "Proj", "--remove", "Fix bug", "bug"]);
    assert!(
        out.contains("Removed label 'bug' from card 'Fix bug'"),
        "{out}"
    );

    let out = ok(&tmp, &["labels", "Proj", "--delete", "urgent"]);
    assert!(out.contains("Deleted label 'urgent'"), "{out}");

    let out = ok(&tmp, &["labels", "Proj"]);
    assert!(!out.contains("urgent"), "{out}");
}

#[test]
fn labels_assign_duplicate_is_idempotent() {
    let tmp = TempDir::new().unwrap();
    ok(&tmp, &["boards", "--create", "Proj"]);
    ok(&tmp, &["lists", "Proj", "--create", "Tasks"]);
    ok(&tmp, &["cards", "Proj", "--create", "Tasks", "My card"]);
    ok(&tmp, &["labels", "Proj", "--create", "bug"]);
    ok(&tmp, &["labels", "Proj", "--assign", "My card", "bug"]);

    // Second assign should succeed (idempotent) and say "already assigned"
    let out = ok(&tmp, &["labels", "Proj", "--assign", "My card", "bug"]);
    assert!(out.contains("already assigned"), "{out}");
}

#[test]
fn labels_remove_not_assigned_is_ok() {
    let tmp = TempDir::new().unwrap();
    ok(&tmp, &["boards", "--create", "Proj"]);
    ok(&tmp, &["lists", "Proj", "--create", "Tasks"]);
    ok(&tmp, &["cards", "Proj", "--create", "Tasks", "My card"]);
    ok(&tmp, &["labels", "Proj", "--create", "bug"]);

    // Remove label that was never assigned — should succeed with info message
    let out = ok(&tmp, &["labels", "Proj", "--remove", "My card", "bug"]);
    assert!(out.contains("not assigned"), "{out}");
}

#[test]
fn labels_delete_nonexistent_errors() {
    let tmp = TempDir::new().unwrap();
    ok(&tmp, &["boards", "--create", "Proj"]);
    let e = fail(&tmp, &["labels", "Proj", "--delete", "ghost"]);
    assert!(e.contains("No label matches"), "{e}");
}

#[test]
fn labels_assign_nonexistent_label_errors() {
    let tmp = TempDir::new().unwrap();
    ok(&tmp, &["boards", "--create", "Proj"]);
    ok(&tmp, &["lists", "Proj", "--create", "Tasks"]);
    ok(&tmp, &["cards", "Proj", "--create", "Tasks", "Card"]);
    let e = fail(&tmp, &["labels", "Proj", "--assign", "Card", "ghost"]);
    assert!(e.contains("No label matches"), "{e}");
}

#[test]
fn deleting_label_removes_from_card() {
    let tmp = TempDir::new().unwrap();
    ok(&tmp, &["boards", "--create", "Proj"]);
    ok(&tmp, &["lists", "Proj", "--create", "Tasks"]);
    ok(&tmp, &["cards", "Proj", "--create", "Tasks", "Card"]);
    ok(&tmp, &["labels", "Proj", "--create", "bug"]);
    ok(&tmp, &["labels", "Proj", "--assign", "Card", "bug"]);

    // Verify assigned
    let out = ok(&tmp, &["cards", "Proj", "--show", "Card"]);
    assert!(out.contains("Labels:"), "{out}");

    // Delete the label
    ok(&tmp, &["labels", "Proj", "--delete", "bug"]);

    // Card should no longer have label
    let out = ok(&tmp, &["cards", "Proj", "--show", "Card"]);
    assert!(!out.contains("Labels:"), "{out}");
}
