//! Build script: derive the `TCT_VERSION` compile-time env var.
//!
//! Release builds happen on a pushed git tag (the tag commit is checked
//! out, so `git describe --tags --exact-match` succeeds) — the tag, with
//! any leading `v` stripped, becomes the version. Every other build has no
//! exact tag, so the version is `snap-<yyyy-mm-dd>`.

use std::process::Command;

fn main() {
    let version = exact_tag().unwrap_or_else(snapshot);
    println!("cargo:rustc-env=TCT_VERSION={version}");
    // Re-run if the checked-out commit or tags change.
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/refs/tags");
}

fn exact_tag() -> Option<String> {
    let output = Command::new("git")
        .args(["describe", "--tags", "--exact-match"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let tag = String::from_utf8(output.stdout).ok()?.trim().to_string();
    if tag.is_empty() {
        None
    } else {
        Some(tag.trim_start_matches('v').to_string())
    }
}

fn snapshot() -> String {
    format!("snap-{}", chrono::Utc::now().format("%Y-%m-%d"))
}
