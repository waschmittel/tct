//! Integration tests that verify the CLI examples shown in README.md.
//! Each test gets its own TempDir via TCT_DATA_DIR, so tests are independent
//! and can run in parallel.

use std::process::{Command, Output};
use tempfile::TempDir;

mod boards;
mod cards;
mod checklist;
mod export;
mod help;
mod labels;
mod lists;
mod misc;
mod search;

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
                if candidate.len() == 8
                    && candidate
                        .chars()
                        .all(|c| matches!(c, '0'..='9' | 'a'..='f'))
                {
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

/// Split a shell-ish line into words (double quotes only).
fn words(line: &str) -> Vec<String> {
    let (mut out, mut cur, mut quoted, mut any) = (Vec::new(), String::new(), false, false);
    for c in line.chars() {
        match c {
            '"' => (quoted, any) = (!quoted, true),
            ' ' if !quoted => {
                if any {
                    out.push(std::mem::take(&mut cur));
                    any = false;
                }
            }
            c => (cur.push(c), any = true).0,
        }
    }
    if any {
        out.push(cur);
    }
    out
}
