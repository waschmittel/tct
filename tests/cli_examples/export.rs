use super::*;

#[test]
fn cards_export_writes_a_pdf_into_the_chosen_folder() {
    let tmp = TempDir::new().unwrap();
    let out = TempDir::new().unwrap();
    ok(&tmp, &["boards", "--create", "Work"]);
    ok(&tmp, &["lists", "Work", "--create", "To Do"]);
    ok(
        &tmp,
        &["cards", "Work", "--create", "To Do", "Plan the Release"],
    );

    let stdout = ok(
        &tmp,
        &[
            "cards",
            "Work",
            "--export",
            "Plan",
            "--out",
            out.path().to_str().unwrap(),
        ],
    );
    assert!(stdout.contains("plan-the-release.pdf"), "{stdout}");
    let pdf = std::fs::read(out.path().join("plan-the-release.pdf")).unwrap();
    assert!(pdf.starts_with(b"%PDF-"));

    let named = out.path().join("custom");
    ok(
        &tmp,
        &[
            "cards",
            "Work",
            "--export",
            "Plan",
            "--out",
            named.to_str().unwrap(),
        ],
    );
    assert!(out.path().join("custom.pdf").exists());
}

#[test]
fn cards_export_defaults_to_mono_and_accepts_color_under_the_same_name() {
    let tmp = TempDir::new().unwrap();
    let out = TempDir::new().unwrap();
    ok(&tmp, &["boards", "--create", "Work"]);
    ok(&tmp, &["lists", "Work", "--create", "To Do"]);
    ok(
        &tmp,
        &["cards", "Work", "--create", "To Do", "Plan the Release"],
    );

    let dir = out.path().to_str().unwrap();
    let default = ok(&tmp, &["cards", "Work", "--export", "Plan", "--out", dir]);
    assert!(default.contains("plan-the-release.pdf"), "{default}");
    let explicit = ok(
        &tmp,
        &[
            "cards", "Work", "--export", "Plan", "--out", dir, "--style", "mono",
        ],
    );
    assert!(explicit.contains("plan-the-release.pdf"), "{explicit}");
    let color = ok(
        &tmp,
        &[
            "cards", "Work", "--export", "Plan", "--out", dir, "--style", "color",
        ],
    );
    assert!(color.contains("plan-the-release.pdf"), "{color}");
    assert_eq!(std::fs::read_dir(out.path()).unwrap().count(), 1);

    let err = fail(
        &tmp,
        &[
            "cards", "Work", "--export", "Plan", "--out", dir, "--style", "neon",
        ],
    );
    assert!(
        err.contains("unknown style 'neon'") && err.contains("mono or color"),
        "{err}"
    );
}

#[test]
fn cards_export_renders_matrix_tables_in_both_styles() {
    let tmp = TempDir::new().unwrap();
    let out = TempDir::new().unwrap();
    ok(&tmp, &["boards", "--create", "Work"]);
    ok(&tmp, &["lists", "Work", "--create", "To Do"]);
    ok(&tmp, &["cards", "Work", "--create", "To Do", "Matrix"]);
    let rows: String = (0..12)
        .map(|i| format!("| row {i} | {i} | {i} | {i} |\n"))
        .collect();
    let table = format!("|  | A | B | C |\n|---|---|---|---|\n{rows}");
    ok(
        &tmp,
        &["cards", "Work", "--edit", "Matrix", "--description", &table],
    );

    let dir = out.path().to_str().unwrap();
    let mut pdfs = Vec::new();
    for style in ["mono", "color"] {
        let file = out.path().join(format!("{style}.pdf"));
        ok(
            &tmp,
            &[
                "cards",
                "Work",
                "--export",
                "Matrix",
                "--out",
                file.to_str().unwrap(),
                "--style",
                style,
            ],
        );
        let pdf = std::fs::read(&file).unwrap();
        assert!(pdf.starts_with(b"%PDF-"), "{style}");
        pdfs.push(pdf);
    }
    assert_ne!(pdfs[0], pdfs[1], "styles differ");

    // A matrix lays out differently from the same table with a filled corner.
    ok(&tmp, &["cards", "Work", "--create", "To Do", "Plain"]);
    let plain = table.replacen("|  |", "| H |", 1);
    ok(
        &tmp,
        &["cards", "Work", "--edit", "Plain", "--description", &plain],
    );
    ok(&tmp, &["cards", "Work", "--export", "Plain", "--out", dir]);
    let plain_pdf = std::fs::read(out.path().join("plain.pdf")).unwrap();
    assert!(plain_pdf.starts_with(b"%PDF-"));
    assert_ne!(plain_pdf.len(), pdfs[0].len());
}
