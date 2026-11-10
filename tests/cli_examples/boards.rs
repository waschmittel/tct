use super::*;

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
    assert!(
        out.contains("Permanently deleted board 'My Project'"),
        "{out}"
    );

    let out = ok(&tmp, &["boards"]);
    assert!(out.contains("No active boards"), "{out}");
}

#[test]
fn boards_restore_nonexistent_errors() {
    let tmp = TempDir::new().unwrap();
    let e = fail(&tmp, &["boards", "--restore", "ghost"]);
    assert!(e.contains("No archived board"), "{e}");
}

#[test]
fn boards_delete_nonarchived_errors() {
    let tmp = TempDir::new().unwrap();
    ok(&tmp, &["boards", "--create", "Proj"]);
    // --delete only works on archived boards
    let e = fail(&tmp, &["boards", "--delete", "Proj"]);
    assert!(e.contains("No archived board"), "{e}");
}

#[test]
fn boards_create_assigns_distinct_accent_colors() {
    let tmp = TempDir::new().unwrap();
    // Two boards should not have the same accent color (auto-differentiation)
    ok(&tmp, &["boards", "--create", "Board1"]);
    ok(&tmp, &["boards", "--create", "Board2"]);
    ok(&tmp, &["boards", "--create", "Board3"]);

    // Read both board.json files and compare accent_color fields
    let boards_dir = tmp.path().join("boards");
    let mut colors: Vec<serde_json::Value> = Vec::new();
    for entry in std::fs::read_dir(&boards_dir).unwrap() {
        let path = entry.unwrap().path().join("board.json");
        let data = std::fs::read_to_string(&path).unwrap();
        let v: serde_json::Value = serde_json::from_str(&data).unwrap();
        colors.push(v["accent_color"].clone());
    }
    assert_eq!(colors.len(), 3);
    // At least one pair must differ
    assert!(
        !(colors[0] == colors[1] && colors[1] == colors[2]),
        "all three boards got same accent: {colors:?}"
    );
}
