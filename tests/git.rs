/// End-to-end tests of the git integration through the real binary.
use std::path::Path;
use std::process::{Command, Output};
use tempfile::TempDir;

fn sh(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .unwrap();
    assert!(out.status.success(), "git {args:?}: {out:?}");
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// Run tct in `dir` against `dir/.tct`.
fn tct(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_tct"))
        .args(args)
        .current_dir(dir)
        .env("TCT_DATA_DIR", dir.join(".tct"))
        .output()
        .unwrap()
}

fn ok(dir: &Path, args: &[&str]) -> String {
    let out = tct(dir, args);
    assert!(out.status.success(), "tct {args:?}: {out:?}");
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn repo() -> (TempDir, std::path::PathBuf) {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().canonicalize().unwrap();
    sh(&root, &["init", "-q"]);
    sh(&root, &["config", "user.name", "Tester"]);
    sh(&root, &["config", "user.email", "t@example.com"]);
    (tmp, root)
}

#[test]
fn init_detects_a_repository_and_reports_it() {
    let (_tmp, root) = repo();
    let out = ok(&root, &["init"]);
    assert!(
        out.contains("Git integration: enabled (inside a git repository)"),
        "{out}"
    );
    assert_eq!(ok(&root, &["--git"]).trim(), "Git integration: enabled");
}

#[test]
fn init_outside_a_repository_is_disabled() {
    let tmp = tempfile::tempdir().unwrap();
    let out = ok(tmp.path(), &["init"]);
    assert!(
        out.contains("Git integration: disabled (not in a git repository)"),
        "{out}"
    );
    let status = ok(tmp.path(), &["--git"]);
    assert!(
        status.contains("disabled (not in a git repository)"),
        "{status}"
    );
}

#[test]
fn init_flag_overrides_detection() {
    let (_tmp, root) = repo();
    let out = ok(&root, &["init", "--git", "disable"]);
    assert!(
        out.contains("Git integration: disabled (as requested)"),
        "{out}"
    );
    // A re-init keeps an explicit setting.
    let out = ok(&root, &["init"]);
    assert!(
        out.contains("Git integration: disabled (already configured)"),
        "{out}"
    );
    let out = ok(&root, &["init", "--git", "enable"]);
    assert!(
        out.contains("Git integration: enabled (as requested)"),
        "{out}"
    );

    // Enabling where it cannot work fails — after the directory was made.
    let tmp = tempfile::tempdir().unwrap();
    let out = tct(tmp.path(), &["init", "--git", "enable"]);
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("not in a git repository"));
    assert!(tmp.path().join(".tct/boards").is_dir());

    let out = tct(&root, &["init", "--git", "maybe"]);
    assert!(!out.status.success());
}

#[test]
fn git_flag_toggles_and_rejects_garbage() {
    let (_tmp, root) = repo();
    ok(&root, &["init", "--git", "disable"]);
    assert_eq!(
        ok(&root, &["--git", "enable"]).trim(),
        "Git integration: enabled"
    );
    assert_eq!(
        ok(&root, &["--git", "disable"]).trim(),
        "Git integration: disabled"
    );
    assert!(!tct(&root, &["--git", "sometimes"]).status.success());
    assert!(!tct(&root, &["--git", "enable", "extra"]).status.success());
}

#[test]
fn cli_commands_never_commit_but_tct_commit_does() {
    let (_tmp, root) = repo();
    ok(&root, &["init"]);
    ok(&root, &["boards", "--create", "Sprint"]);
    ok(&root, &["lists", "Sprint", "--create", "To Do"]);
    ok(
        &root,
        &["cards", "Sprint", "--create", "To Do", "Fix login"],
    );
    // Autocommit is on, yet no commit happened.
    assert!(
        !root
            .join(".git/refs/heads")
            .read_dir()
            .is_ok_and(|mut d| d.next().is_some())
    );

    std::fs::write(root.join("outside.txt"), "x").unwrap();
    sh(&root, &["add", "outside.txt"]);

    let out = ok(&root, &["commit"]);
    assert!(out.starts_with("Committed: Board \"Sprint\":"), "{out}");
    let files = sh(&root, &["show", "--name-only", "--pretty=format:", "HEAD"]);
    assert!(
        files
            .lines()
            .filter(|l| !l.is_empty())
            .all(|l| l.starts_with(".tct/")),
        "{files}"
    );
    assert_eq!(
        sh(&root, &["diff", "--cached", "--name-only"]).trim(),
        "outside.txt"
    );

    assert_eq!(ok(&root, &["commit"]).trim(), "Nothing to commit");

    ok(
        &root,
        &[
            "cards",
            "Sprint",
            "--edit",
            "Fix login",
            "--title",
            "Fix logout",
        ],
    );
    let out = ok(&root, &["commit"]);
    assert_eq!(
        out.trim(),
        "Committed: Board \"Sprint\": card \"Fix logout\" renamed from \"Fix login\""
    );
}

#[test]
fn commit_works_with_autocommit_disabled_and_fails_outside_a_repo() {
    let (_tmp, root) = repo();
    ok(&root, &["init", "--git", "disable"]);
    ok(&root, &["boards", "--create", "A"]);
    assert!(ok(&root, &["commit"]).starts_with("Committed: "));

    let tmp = tempfile::tempdir().unwrap();
    ok(tmp.path(), &["init"]);
    let out = tct(tmp.path(), &["commit"]);
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("not in a git repository"));

    for extra in [["commit", "--bogus"], ["commit", "-m"]] {
        assert!(!tct(&root, &extra).status.success());
    }
    assert!(!tct(&root, &["commit", "-m", "x"]).status.success());
}
