use super::*;

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

#[test]
fn unknown_command_errors() {
    let tmp = TempDir::new().unwrap();
    let e = fail(&tmp, &["badcommand"]);
    assert!(e.contains("Unknown command"), "{e}");
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
