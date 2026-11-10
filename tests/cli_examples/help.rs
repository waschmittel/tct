use super::*;

#[test]
fn help_via_long_flag() {
    let tmp = TempDir::new().unwrap();
    let out = ok(&tmp, &["--help"]);
    assert!(out.contains("Terminal Card Tracker"), "{out}");
}

#[test]
fn help_via_short_flag() {
    let tmp = TempDir::new().unwrap();
    let out = ok(&tmp, &["-h"]);
    assert!(out.contains("Terminal Card Tracker"), "{out}");
}

#[test]
fn help_via_subcommand() {
    let tmp = TempDir::new().unwrap();
    let out = ok(&tmp, &["help"]);
    assert!(out.contains("Terminal Card Tracker"), "{out}");
}

// ── --help topics ─────────────────────────────────────────────────────────────

#[test]
fn help_overview_is_short_and_lists_topics() {
    let tmp = TempDir::new().unwrap();
    let out = ok(&tmp, &["--help"]);
    for t in ["cli", "examples", "env", "readme", "user-guide"] {
        assert!(out.contains(&format!("--help {t}")), "{t}\n{out}");
    }
    assert!(out.contains("users and AI agents"), "{out}");
    assert!(
        !out.contains("COMMANDS:") && !out.contains("ENVIRONMENT:"),
        "{out}"
    );
}

#[test]
fn help_cli_and_env_topics_split_commands_from_environment() {
    let tmp = TempDir::new().unwrap();
    let cli = ok(&tmp, &["--help", "cli"]);
    assert!(cli.contains("FLAGS") && cli.contains("COMMANDS:") && cli.contains("--by-id"));
    assert!(!cli.contains("TCT_DATA_DIR"), "{cli}");
    let env = ok(&tmp, &["help", "env"]);
    assert!(env.contains("STORAGE:") && env.contains("ENVIRONMENT:") && env.contains("TCT_WEB"));
}

#[test]
fn help_readme_and_user_guide_print_the_repo_files() {
    let tmp = TempDir::new().unwrap();
    for (topic, file) in [
        ("readme", "README.md"),
        ("user-guide", "docs/user-guide.md"),
    ] {
        let want =
            std::fs::read_to_string(format!("{}/{file}", env!("CARGO_MANIFEST_DIR"))).unwrap();
        assert_eq!(
            ok(&tmp, &["--help", topic]).trim_end(),
            want.trim_end(),
            "{topic}"
        );
        // Position of the help word doesn't matter (`tct cards --help …`).
        assert_eq!(ok(&tmp, &["-h", topic]), ok(&tmp, &["help", topic]));
    }
}

#[test]
fn help_toc_and_section_select_parts_of_a_document() {
    let tmp = TempDir::new().unwrap();
    let toc = ok(&tmp, &["--help", "user-guide", "--toc"]);
    assert!(toc.contains("\n  CLI Reference\n"), "{toc}");
    assert!(!toc.contains("```"), "{toc}");
    let sec = ok(
        &tmp,
        &["--help", "user-guide", "--section", "cli reference"],
    );
    assert!(sec.starts_with("## CLI Reference"), "{sec}");
    assert!(!sec.contains("## Workflows"), "{sec}");
    let e = fail(
        &tmp,
        &["--help", "user-guide", "--section", "no such section"],
    );
    assert!(e.contains("No section matches"), "{e}");
    let e = fail(&tmp, &["--help", "bogus"]);
    assert!(
        e.contains("Unknown help topic") && e.contains("user-guide"),
        "{e}"
    );
}

#[test]
fn help_examples_all_run() {
    let tmp = TempDir::new().unwrap();
    let text = ok(&tmp, &["--help", "examples"]);
    let mut ran = 0;
    for line in text
        .lines()
        .map(str::trim)
        .filter(|l| l.starts_with("tct "))
    {
        let mut args = words(line);
        args.remove(0);
        // The ID example needs a real ID: resolve the board's.
        if args.contains(&"--by-id".to_string()) {
            let id = extract_id(&ok(&tmp, &["boards"]));
            args[1] = id;
        }
        if args.first().map(String::as_str) == Some("--help") {
            continue;
        }
        let refs: Vec<&str> = args.iter().map(String::as_str).collect();
        ok(&tmp, &refs);
        ran += 1;
    }
    assert!(ran >= 14, "ran only {ran}");
}
