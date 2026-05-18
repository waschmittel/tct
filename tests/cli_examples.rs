/// Integration tests that verify the CLI examples shown in README.md.
/// Each test gets its own TempDir via TCT_DATA_DIR, so tests are independent
/// and can run in parallel.
use std::process::{Command, Output};
use tempfile::TempDir;

// ── Helpers ───────────────────────────────────────────────────────────────────

fn tct(tmp: &TempDir, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_tct"))
        .args(args)
        .env("TCT_DATA_DIR", tmp.path())
        .output()
        .expect("failed to run tct binary")
}

/// Run a command, assert success, return stdout.
fn ok(tmp: &TempDir, args: &[&str]) -> String {
    let out = tct(tmp, args);
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    assert!(
        out.status.success(),
        "tct {} failed\nstderr: {}\nstdout: {}",
        args.join(" "),
        stderr,
        stdout,
    );
    stdout
}

/// Run a command, assert failure, return stderr.
fn fail(tmp: &TempDir, args: &[&str]) -> String {
    let out = tct(tmp, args);
    assert!(
        !out.status.success(),
        "tct {} should have failed but succeeded",
        args.join(" ")
    );
    String::from_utf8_lossy(&out.stderr).to_string()
}

/// Extract the last 8-char hex ID from listing output.
/// Listings include the board ID in header lines; taking the last ID picks the most
/// specific entity (the list, card, etc. being listed).
fn extract_id(output: &str) -> String {
    let mut last: Option<String> = None;
    for line in output.lines() {
        let mut search = line;
        while let Some(start) = search.find('[') {
            let rest = &search[start + 1..];
            if let Some(end) = rest.find(']') {
                let candidate = &rest[..end];
                if candidate.len() == 8 && candidate.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f')) {
                    last = Some(candidate.to_string());
                }
                search = &rest[end + 1..];
            } else {
                break;
            }
        }
    }
    last.unwrap_or_else(|| panic!("No 8-char hex ID found in output:\n{output}"))
}

// ── Board tests ───────────────────────────────────────────────────────────────

#[test]
fn boards_create_and_list() {
    let tmp = TempDir::new().unwrap();

    let out = ok(&tmp, &["boards", "--create", "My Project"]);
    assert!(out.contains("Created board 'My Project'"), "{out}");

    let out = ok(&tmp, &["boards"]);
    assert!(out.contains("My Project"), "{out}");
    assert!(out.contains("0 lists"), "{out}");
}

#[test]
fn boards_archive_restore_delete() {
    let tmp = TempDir::new().unwrap();
    ok(&tmp, &["boards", "--create", "My Project"]);

    let out = ok(&tmp, &["boards", "--archive", "My Project"]);
    assert!(out.contains("Archived board 'My Project'"), "{out}");

    let out = ok(&tmp, &["boards"]);
    assert!(out.contains("No active boards"), "{out}");

    let out = ok(&tmp, &["boards", "--archived"]);
    assert!(out.contains("My Project"), "{out}");

    let out = ok(&tmp, &["boards", "--restore", "My Project"]);
    assert!(out.contains("Restored board 'My Project'"), "{out}");

    let out = ok(&tmp, &["boards"]);
    assert!(out.contains("My Project"), "{out}");

    ok(&tmp, &["boards", "--archive", "My Project"]);
    let out = ok(&tmp, &["boards", "--delete", "My Project"]);
    assert!(out.contains("Permanently deleted board 'My Project'"), "{out}");

    let out = ok(&tmp, &["boards"]);
    assert!(out.contains("No active boards"), "{out}");
}

// ── List tests ────────────────────────────────────────────────────────────────

#[test]
fn lists_create_rename_delete() {
    let tmp = TempDir::new().unwrap();
    ok(&tmp, &["boards", "--create", "Proj"]);

    let out = ok(&tmp, &["lists", "Proj", "--create", "To Do"]);
    assert!(out.contains("Created list 'To Do' on board 'Proj'"), "{out}");

    let out = ok(&tmp, &["lists", "Proj"]);
    assert!(out.contains("To Do"), "{out}");

    let out = ok(&tmp, &["lists", "Proj", "--rename", "To Do", "Backlog"]);
    assert!(out.contains("Renamed list 'To Do' to 'Backlog'"), "{out}");

    let out = ok(&tmp, &["lists", "Proj"]);
    assert!(out.contains("Backlog"), "{out}");
    assert!(!out.contains("To Do"), "{out}");

    let out = ok(&tmp, &["lists", "Proj", "--delete", "Backlog"]);
    assert!(out.contains("Deleted list 'Backlog' and all its cards"), "{out}");

    let out = ok(&tmp, &["lists", "Proj"]);
    assert!(out.contains("(no lists)"), "{out}");
}

// ── Card tests ────────────────────────────────────────────────────────────────

#[test]
fn cards_create_list_show_edit() {
    let tmp = TempDir::new().unwrap();
    ok(&tmp, &["boards", "--create", "Proj"]);
    ok(&tmp, &["lists", "Proj", "--create", "To Do"]);
    ok(&tmp, &["lists", "Proj", "--create", "Done"]);

    let out = ok(&tmp, &["cards", "Proj", "--create", "To Do", "Fix the bug"]);
    assert!(out.contains("Created card 'Fix the bug' in list 'To Do' on board 'Proj'"), "{out}");

    let out = ok(&tmp, &["cards", "Proj"]);
    assert!(out.contains("Fix the bug"), "{out}");

    let out = ok(&tmp, &["cards", "Proj", "--list", "To Do"]);
    assert!(out.contains("Fix the bug"), "{out}");

    let out = ok(&tmp, &["cards", "Proj", "--show", "Fix"]);
    assert!(out.contains("Card:"), "{out}");
    assert!(out.contains("Fix the bug"), "{out}");

    let out = ok(&tmp, &["cards", "Proj", "--edit", "Fix", "--title", "Fix login bug", "--due", "2099-12-31"]);
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

// ── Checklist tests ───────────────────────────────────────────────────────────

#[test]
fn checklist_add_toggle_delete() {
    let tmp = TempDir::new().unwrap();
    ok(&tmp, &["boards", "--create", "Proj"]);
    ok(&tmp, &["lists", "Proj", "--create", "Tasks"]);
    ok(&tmp, &["cards", "Proj", "--create", "Tasks", "Write docs"]);

    let out = ok(&tmp, &["checklist", "Proj", "Write docs", "--add", "Draft outline"]);
    assert!(out.contains("Added checklist item 'Draft outline' to card 'Write docs'"), "{out}");

    ok(&tmp, &["checklist", "Proj", "Write docs", "--add", "Write content"]);

    let out = ok(&tmp, &["checklist", "Proj", "Write docs"]);
    assert!(out.contains("Draft outline"), "{out}");
    assert!(out.contains("Write content"), "{out}");
    assert!(out.contains("[ ]"), "{out}");

    let out = ok(&tmp, &["checklist", "Proj", "Write docs", "--toggle", "1"]);
    assert!(out.contains("done"), "{out}");

    let out = ok(&tmp, &["checklist", "Proj", "Write docs"]);
    assert!(out.contains("[x]"), "{out}");

    let out = ok(&tmp, &["checklist", "Proj", "Write docs", "--delete", "2"]);
    assert!(out.contains("Deleted checklist item 'Write content'"), "{out}");

    let out = ok(&tmp, &["checklist", "Proj", "Write docs"]);
    assert!(!out.contains("Write content"), "{out}");
    assert!(out.contains("Draft outline"), "{out}");
}

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
    assert!(out.contains("Assigned label 'bug' to card 'Fix bug'"), "{out}");

    // Label visible in card detail
    let out = ok(&tmp, &["cards", "Proj", "--show", "Fix bug"]);
    assert!(out.contains("Labels:"), "{out}");
    assert!(out.contains("bug"), "{out}");

    let out = ok(&tmp, &["labels", "Proj", "--remove", "Fix bug", "bug"]);
    assert!(out.contains("Removed label 'bug' from card 'Fix bug'"), "{out}");

    let out = ok(&tmp, &["labels", "Proj", "--delete", "urgent"]);
    assert!(out.contains("Deleted label 'urgent'"), "{out}");

    let out = ok(&tmp, &["labels", "Proj"]);
    assert!(!out.contains("urgent"), "{out}");
}

// ── --by-id tests ─────────────────────────────────────────────────────────────

#[test]
fn by_id_board_and_list() {
    let tmp = TempDir::new().unwrap();
    ok(&tmp, &["boards", "--create", "Proj"]);
    ok(&tmp, &["lists", "Proj", "--create", "Tasks"]);
    ok(&tmp, &["cards", "Proj", "--create", "Tasks", "My task"]);

    // Resolve board ID from listing
    let listing = ok(&tmp, &["boards"]);
    let board_id = extract_id(&listing);

    // Address board by exact ID
    let out = ok(&tmp, &["lists", &board_id, "--by-id"]);
    assert!(out.contains("Tasks"), "{out}");

    // Resolve list ID
    let listing = ok(&tmp, &["lists", "Proj"]);
    let list_id = extract_id(&listing);

    // Filter cards by list ID (board also addressed by ID since --by-id applies to all args)
    let out = ok(&tmp, &["cards", &board_id, "--list", &list_id, "--by-id"]);
    assert!(out.contains("My task"), "{out}");

    // Resolve card ID
    let listing = ok(&tmp, &["cards", "Proj"]);
    let card_id = extract_id(&listing);

    // Show card by ID
    let out = ok(&tmp, &["cards", &board_id, "--show", &card_id, "--by-id"]);
    assert!(out.contains("My task"), "{out}");

}

#[test]
fn by_id_wrong_id_errors() {
    let tmp = TempDir::new().unwrap();
    ok(&tmp, &["boards", "--create", "Proj"]);
    ok(&tmp, &["lists", "Proj", "--create", "Tasks"]);
    ok(&tmp, &["cards", "Proj", "--create", "Tasks", "My task"]);

    // Get the board ID to use when --by-id is active (applies to all args including board)
    let listing = ok(&tmp, &["boards"]);
    let board_id = extract_id(&listing);

    let e = fail(&tmp, &["cards", &board_id, "--show", "00000000", "--by-id"]);
    assert!(e.contains("No card with ID '00000000'"), "{e}");

    let e = fail(&tmp, &["boards", "--archive", "00000000", "--by-id"]);
    assert!(e.contains("No active board with ID '00000000'"), "{e}");
}

// ── Error / ambiguity tests ───────────────────────────────────────────────────

#[test]
fn ambiguous_and_missing_name_errors() {
    let tmp = TempDir::new().unwrap();
    ok(&tmp, &["boards", "--create", "Alpha Project"]);
    ok(&tmp, &["boards", "--create", "Alpha Staging"]);

    // Ambiguous partial
    let e = fail(&tmp, &["boards", "--archive", "Alpha"]);
    assert!(e.contains("Multiple active boards match 'Alpha'"), "{e}");
    assert!(e.contains("Alpha Project"), "{e}");
    assert!(e.contains("Alpha Staging"), "{e}");

    // No match
    let e = fail(&tmp, &["boards", "--archive", "Zeta"]);
    assert!(e.contains("No active board matches 'Zeta'"), "{e}");
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
    ok(&tmp, &["checklist", "Proj", "My card", "--add", "only item"]);

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
fn cards_edit_no_fields_errors() {
    let tmp = TempDir::new().unwrap();
    ok(&tmp, &["boards", "--create", "Proj"]);
    ok(&tmp, &["lists", "Proj", "--create", "Tasks"]);
    ok(&tmp, &["cards", "Proj", "--create", "Tasks", "My card"]);

    let e = fail(&tmp, &["cards", "Proj", "--edit", "My card"]);
    assert!(e.contains("--title") || e.contains("--description") || e.contains("--due"), "{e}");
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

// ── Local .tct discovery test ─────────────────────────────────────────────────

#[test]
fn local_tct_dir_discovery() {
    let tmp = TempDir::new().unwrap();
    // Create a .tct dir inside tmp to simulate a project-local store
    let local_tct = tmp.path().join(".tct");
    std::fs::create_dir(&local_tct).unwrap();

    // Run tct from within tmp (no TCT_DATA_DIR, should find .tct in cwd)
    let out = Command::new(env!("CARGO_BIN_EXE_tct"))
        .args(["boards", "--create", "Local Board"])
        .current_dir(tmp.path())
        // no TCT_DATA_DIR set
        .env_remove("TCT_DATA_DIR")
        .output()
        .expect("failed to run tct");

    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    assert!(out.status.success(), "stderr: {stderr}\nstdout: {stdout}");
    assert!(stdout.contains("Created board 'Local Board'"), "{stdout}");

    // The board file should be inside tmp/.tct, not ~/.tct
    assert!(
        local_tct.join("boards").exists(),
        ".tct/boards should exist inside the local dir"
    );
}
