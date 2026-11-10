//! Git integration (ADR-0019): commit the data directory — and nothing else —
//! on request (`tct commit`, `C` in the board view) and, when enabled
//! (`tct --git enable`), when the TUI quits.
//!
//! The only module that spawns `git`. Every call is `git -C <data dir>` with
//! the pathspec `.`, so a commit can never contain a file outside the data
//! directory, whatever else is staged in the repository.

mod message;

use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use anyhow::{Context, Result, bail};

use crate::model::id::{BoardId, CardId};
use crate::storage::{board_store, card_store, paths};
use message::{BoardChange, CardChange};

/// Local git config key holding the opt-in. Local config (not a file in the
/// data directory) because the data directory may come from a checked-out
/// repository and must not be able to switch this on.
const CONFIG_KEY: &str = "tct.autocommit";

#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    /// The commit's subject line.
    Committed(String),
    Nothing,
}

fn git(dir: &Path) -> Command {
    let mut cmd = Command::new("git");
    cmd.arg("-C")
        .arg(dir)
        .stdin(Stdio::null())
        .env("GIT_TERMINAL_PROMPT", "0")
        // An inherited repository override would point git somewhere else.
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .env_remove("GIT_COMMON_DIR");
    cmd
}

fn output(dir: &Path, args: &[&str]) -> Result<Output> {
    git(dir)
        .args(args)
        .output()
        .context("could not run git (is it installed?)")
}

fn run_bytes(dir: &Path, args: &[&str]) -> Result<Vec<u8>> {
    let out = output(dir, args)?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        bail!(
            "git {}: {}",
            args[0],
            err.lines().next().unwrap_or("failed")
        );
    }
    Ok(out.stdout)
}

fn run(dir: &Path, args: &[&str]) -> Result<String> {
    Ok(String::from_utf8_lossy(&run_bytes(dir, args)?).into_owned())
}

fn data_dir() -> Result<PathBuf> {
    let dir = paths::base_dir();
    if !dir.is_dir() {
        bail!("data directory {} does not exist", dir.display());
    }
    Ok(dir)
}

/// The work tree root containing `dir`; `None` outside any work tree.
fn toplevel(dir: &Path) -> Result<Option<PathBuf>> {
    let out = output(dir, &["rev-parse", "--show-toplevel"])?;
    Ok(out
        .status
        .success()
        .then(|| PathBuf::from(String::from_utf8_lossy(&out.stdout).trim())))
}

/// Why git integration cannot work for `dir` — `Ok` when it can.
pub fn detect(dir: &Path) -> std::result::Result<(), String> {
    let top = match toplevel(dir) {
        Ok(Some(top)) => top,
        Ok(None) => return Err("not in a git repository".into()),
        Err(_) => return Err("git is not installed".into()),
    };
    // Committing "the data directory" would commit the whole repository.
    if let (Ok(a), Ok(b)) = (dir.canonicalize(), top.canonicalize())
        && a == b
    {
        return Err("the data directory is the repository root".into());
    }
    match output(dir, &["check-ignore", "-q", "--", ".version"]) {
        Ok(o) if o.status.code() == Some(0) => Err("the data directory is ignored by git".into()),
        _ => Ok(()),
    }
}

/// The explicit setting in this repository's local config, if any.
pub fn configured() -> Option<bool> {
    let dir = paths::base_dir();
    let out = output(
        &dir,
        &["config", "--local", "--type=bool", "--get", CONFIG_KEY],
    )
    .ok()?;
    match String::from_utf8_lossy(&out.stdout).trim() {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

/// Whether commit-on-quit is switched on (never errors: no git, no repo → off).
pub fn is_enabled() -> bool {
    configured() == Some(true)
}

/// Switch commit-on-quit on or off. Enabling fails with the reason when the
/// data directory is not suitable; disabling outside a repository is a no-op.
pub fn set_enabled(on: bool) -> Result<()> {
    let dir = data_dir()?;
    if on {
        if let Err(why) = detect(&dir) {
            bail!("cannot enable git integration: {why}");
        }
    } else if !matches!(toplevel(&dir), Ok(Some(_))) {
        return Ok(());
    }
    run(
        &dir,
        &[
            "config",
            "--local",
            CONFIG_KEY,
            if on { "true" } else { "false" },
        ],
    )?;
    Ok(())
}

/// A merge, rebase, cherry-pick or revert in progress — committing then
/// would corrupt it.
fn busy(dir: &Path) -> Result<Option<&'static str>> {
    const STATES: [(&str, &str); 5] = [
        ("MERGE_HEAD", "merge"),
        ("CHERRY_PICK_HEAD", "cherry-pick"),
        ("REVERT_HEAD", "revert"),
        ("rebase-merge", "rebase"),
        ("rebase-apply", "rebase"),
    ];
    let mut args = vec!["rev-parse"];
    for (name, _) in STATES {
        args.extend(["--git-path", name]);
    }
    let out = run(dir, &args)?;
    Ok(out
        .lines()
        .zip(STATES)
        .find(|(path, _)| dir.join(path).exists())
        .map(|(_, (_, what))| what))
}

/// `(status, path)` pairs of `git diff --name-status -z`.
fn parse_status(raw: &[u8]) -> Vec<(char, String)> {
    let mut parts = raw
        .split(|b| *b == 0)
        .filter(|p| !p.is_empty())
        .map(|p| String::from_utf8_lossy(p).into_owned());
    let mut out = Vec::new();
    while let (Some(status), Some(path)) = (parts.next(), parts.next()) {
        out.push((status.chars().next().unwrap_or('M'), path));
    }
    out
}

/// A file's content in `HEAD` (`rev` = "HEAD") or the index (`rev` = "").
fn blob(dir: &Path, rev: &str, path: &str) -> Option<String> {
    let out = output(dir, &["show", &format!("{rev}:./{path}")]).ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Group the changed paths by board and load old (HEAD) and new (index)
/// versions of what changed.
fn collect(dir: &Path, changed: &[(char, String)]) -> Vec<BoardChange> {
    let mut boards: BTreeMap<BoardId, BoardChange> = BTreeMap::new();
    for (_, path) in changed {
        let parts: Vec<&str> = path.split('/').collect();
        let [boards_dir, bid, file] = parts.as_slice() else {
            continue;
        };
        let Ok(board_id) = BoardId::parse(bid) else {
            continue;
        };
        if *boards_dir != "boards" {
            continue;
        }
        let entry = boards.entry(board_id.clone()).or_insert_with(|| {
            let meta_path = format!("boards/{bid}/board.json");
            let load = |rev| {
                blob(dir, rev, &meta_path)
                    .and_then(|t| board_store::parse_board(&board_id, &t).ok())
            };
            BoardChange {
                id: board_id.clone(),
                old: load("HEAD"),
                new: load(""),
                cards: Vec::new(),
            }
        });
        let card_id = file
            .strip_prefix("card-")
            .and_then(|f| f.strip_suffix(".json"))
            .and_then(|id| CardId::parse(id).ok());
        if let Some(card_id) = card_id {
            let load =
                |rev| blob(dir, rev, path).and_then(|t| card_store::parse_card(&card_id, &t).ok());
            let card = CardChange {
                old: load("HEAD"),
                new: load(""),
            };
            if card.old.is_some() || card.new.is_some() {
                entry.cards.push(card);
            }
        }
    }
    boards.into_values().collect()
}

/// Commit everything under the data directory with a generated message.
pub fn commit() -> Result<Outcome> {
    let dir = data_dir()?;
    if let Err(why) = detect(&dir) {
        bail!("cannot commit: {why}");
    }
    if let Some(what) = busy(&dir)? {
        bail!("a {what} is in progress; not committing");
    }
    run(&dir, &["add", "-A", "--", "."])?;
    let changed = parse_status(&run_bytes(
        &dir,
        &[
            "diff",
            "--cached",
            "--name-status",
            "-z",
            "--no-renames",
            "--relative",
            "--",
            ".",
        ],
    )?);
    if changed.is_empty() {
        return Ok(Outcome::Nothing);
    }

    let (subject, body) = message::message(&collect(&dir, &changed));
    let text = if body.is_empty() {
        subject.clone()
    } else {
        format!("{subject}\n\n{body}")
    };

    // Hooks and signing are skipped so an automatic commit can never block
    // on a prompt. The pathspec makes this a partial commit: anything else
    // staged in the repository stays staged and out of it.
    let mut child = git(&dir)
        .args([
            "commit",
            "--quiet",
            "--no-verify",
            "--no-gpg-sign",
            "-F",
            "-",
            "--",
            ".",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .context("could not run git (is it installed?)")?;
    child
        .stdin
        .take()
        .context("git stdin")?
        .write_all(text.as_bytes())?;
    let out = child.wait_with_output()?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        bail!("git commit: {}", err.lines().next().unwrap_or("failed"));
    }
    Ok(Outcome::Committed(subject))
}

/// The TUI's quit hook: commit when enabled. `Some(error)` when that failed.
pub fn commit_on_quit() -> Option<anyhow::Error> {
    if is_enabled() { commit().err() } else { None }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::card::Card;
    use crate::model::id::ListId;
    use crate::test_support::add_list;

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

    /// A fresh repository with `.tct` inside, `TCT_DATA_DIR` pointing at it.
    /// Returns the repository root to `f`.
    fn with_repo(f: impl FnOnce(&Path)) {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().canonicalize().unwrap();
        sh(&root, &["init", "-q"]);
        sh(&root, &["config", "user.name", "Tester"]);
        sh(&root, &["config", "user.email", "t@example.com"]);
        sh(&root, &["config", "commit.gpgsign", "true"]);
        let data = root.join(".tct");
        // SAFETY: tests run single-threaded.
        unsafe { std::env::set_var("TCT_DATA_DIR", &data) };
        board_store::init_data_dir().unwrap();
        f(&root);
        unsafe { std::env::remove_var("TCT_DATA_DIR") };
    }

    fn files_in_head(root: &Path) -> Vec<String> {
        sh(root, &["show", "--name-only", "--pretty=format:", "HEAD"])
            .lines()
            .filter(|l| !l.is_empty())
            .map(String::from)
            .collect()
    }

    fn subject(root: &Path) -> String {
        sh(root, &["log", "-1", "--pretty=%s"]).trim().to_string()
    }

    #[test]
    fn enable_disable_roundtrip() {
        with_repo(|_| {
            assert_eq!(configured(), None);
            assert!(!is_enabled());
            set_enabled(true).unwrap();
            assert!(is_enabled());
            set_enabled(false).unwrap();
            assert_eq!(configured(), Some(false));
            assert!(!is_enabled());
        });
    }

    #[test]
    fn enabling_needs_a_repo_that_does_not_ignore_the_data_dir() {
        with_repo(|root| {
            std::fs::write(root.join(".gitignore"), ".tct/\n").unwrap();
            let err = set_enabled(true).unwrap_err().to_string();
            assert!(err.contains("ignored by git"), "{err}");
        });
        // No repository at all (disabling is a harmless no-op there).
        crate::test_support::with_temp_dir(|| {
            let err = set_enabled(true).unwrap_err().to_string();
            assert!(err.contains("not in a git repository"), "{err}");
            set_enabled(false).unwrap();
            assert!(commit().is_err());
        });
    }

    #[test]
    fn data_dir_equal_to_repo_root_is_refused() {
        with_repo(|root| {
            unsafe { std::env::set_var("TCT_DATA_DIR", root) };
            let err = commit().unwrap_err().to_string();
            assert!(err.contains("repository root"), "{err}");
        });
    }

    #[test]
    fn commits_only_the_data_dir() {
        with_repo(|root| {
            std::fs::write(root.join("outside-staged.txt"), "a").unwrap();
            std::fs::write(root.join("outside-untracked.txt"), "b").unwrap();
            sh(root, &["add", "outside-staged.txt"]);
            board_store::create_board("Sprint".into()).unwrap();

            let out = commit().unwrap();
            assert!(matches!(out, Outcome::Committed(_)));
            let files = files_in_head(root);
            assert!(!files.is_empty());
            assert!(files.iter().all(|f| f.starts_with(".tct/")), "{files:?}");
            // The user's own staging is left exactly as it was.
            assert_eq!(
                sh(root, &["diff", "--cached", "--name-only"]).trim(),
                "outside-staged.txt"
            );
            assert!(sh(root, &["status", "--porcelain"]).contains("?? outside-untracked.txt"));
        });
    }

    #[test]
    fn commits_only_the_data_dir_after_the_first_commit_too() {
        with_repo(|root| {
            board_store::create_board("A".into()).unwrap();
            commit().unwrap();
            std::fs::write(root.join("code.rs"), "fn main() {}").unwrap();
            sh(root, &["add", "code.rs"]);
            board_store::create_board("B".into()).unwrap();
            commit().unwrap();
            let files = files_in_head(root);
            assert!(files.iter().all(|f| f.starts_with(".tct/")), "{files:?}");
        });
    }

    #[test]
    fn quit_commits_only_when_enabled() {
        with_repo(|root| {
            board_store::create_board("A".into()).unwrap();
            assert!(commit_on_quit().is_none());
            assert!(sh(root, &["status", "--porcelain"]).contains(".tct"));

            set_enabled(true).unwrap();
            assert!(commit_on_quit().is_none());
            assert_eq!(subject(root), "Board \"A\": created");
            assert!(sh(root, &["status", "--porcelain"]).is_empty());

            // A failure is returned, not swallowed.
            board_store::create_board("B".into()).unwrap();
            std::fs::write(root.join(".git/MERGE_HEAD"), "0".repeat(40)).unwrap();
            assert!(commit_on_quit().is_some());
        });
    }

    #[test]
    fn nothing_to_commit_when_clean() {
        with_repo(|_| {
            board_store::create_board("A".into()).unwrap();
            commit().unwrap();
            assert_eq!(commit().unwrap(), Outcome::Nothing);
        });
    }

    #[test]
    fn message_describes_a_card_move() {
        with_repo(|root| {
            let mut board = board_store::create_board("Sprint".into()).unwrap();
            let todo = add_list(&mut board, "To Do");
            let done = add_list(&mut board, "Done");
            let mut card = Card::new("Fix login".into());
            card.list_id = todo;
            card_store::save_card(&board.id, &card).unwrap();
            commit().unwrap();
            assert_eq!(subject(root), "Board \"Sprint\": created (1 card)");

            card.list_id = done;
            card_store::save_card(&board.id, &card).unwrap();
            let out = commit().unwrap();
            let expected = "Board \"Sprint\": card \"Fix login\" moved to \"Done\"";
            assert_eq!(out, Outcome::Committed(expected.into()));
            assert_eq!(subject(root), expected);
        });
    }

    #[test]
    fn untitled_boards_and_other_files_still_commit() {
        with_repo(|root| {
            // Only README/.version are new: no board to name.
            assert!(matches!(commit().unwrap(), Outcome::Committed(_)));
            assert_eq!(subject(root), "Update tct data");
        });
    }

    #[test]
    fn refuses_during_a_merge() {
        with_repo(|root| {
            board_store::create_board("A".into()).unwrap();
            std::fs::write(root.join(".git/MERGE_HEAD"), "0".repeat(40)).unwrap();
            let err = commit().unwrap_err().to_string();
            assert!(err.contains("merge is in progress"), "{err}");
        });
    }

    #[test]
    fn ignores_hooks_and_signing() {
        with_repo(|root| {
            let hook = root.join(".git/hooks/pre-commit");
            std::fs::create_dir_all(hook.parent().unwrap()).unwrap();
            std::fs::write(&hook, "#!/bin/sh\nexit 1\n").unwrap();
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755)).unwrap();
            }
            board_store::create_board("A".into()).unwrap();
            // `commit.gpgsign=true` with no key would fail otherwise.
            assert!(matches!(commit().unwrap(), Outcome::Committed(_)));
        });
    }

    #[test]
    fn deleted_board_is_named_from_head() {
        with_repo(|root| {
            let board = board_store::create_board("Gone".into()).unwrap();
            let _ = ListId::generate();
            commit().unwrap();
            board_store::delete_board(&board.id).unwrap();
            commit().unwrap();
            assert_eq!(subject(root), "Board \"Gone\": deleted");
        });
    }

    #[test]
    fn status_parsing_handles_nul_separated_pairs() {
        let raw = b"M\0boards/a/board.json\0A\0boards/a/card-x.json\0";
        assert_eq!(
            parse_status(raw),
            vec![
                ('M', "boards/a/board.json".to_string()),
                ('A', "boards/a/card-x.json".to_string())
            ]
        );
        assert!(parse_status(b"").is_empty());
    }
}
