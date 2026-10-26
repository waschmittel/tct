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

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        embed_windows_icon();
    }
}

/// Convert `assets/icon.ico` into a 32-bit Windows PE `.res` file and pass it
/// to the Windows linker so `tct.exe` carries the application icon without
/// requiring external `rc.exe` lookup or extra build-dependency crates.
fn embed_windows_icon() {
    println!("cargo:rerun-if-changed=assets/icon.ico");
    let ico = std::fs::read("assets/icon.ico").expect("failed to read assets/icon.ico");
    let res = ico_to_win32_res(&ico);
    let out_dir = std::path::PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR not set"));
    let res_path = out_dir.join("icon.res");
    std::fs::write(&res_path, res).expect("failed to write icon.res");
    println!("cargo:rustc-link-arg-bins={}", res_path.display());
}

fn ico_to_win32_res(ico: &[u8]) -> Vec<u8> {
    assert!(ico.len() >= 6, "invalid .ico header");
    let count = u16::from_le_bytes([ico[4], ico[5]]) as usize;
    assert!(ico.len() >= 6 + count * 16, "truncated .ico directory");

    let mut out = Vec::new();

    // 1. Mandatory 32-byte null resource header at offset 0 of a 32-bit .res file.
    write_res_header(&mut out, 0, 0, 0, 0, 0);

    // 2. RT_ICON (type = 3) record for each image frame.
    let mut group_data = Vec::with_capacity(6 + count * 14);
    group_data.extend_from_slice(&ico[0..6]);

    for i in 0..count {
        let entry = &ico[6 + i * 16..6 + (i + 1) * 16];
        let size = u32::from_le_bytes([entry[8], entry[9], entry[10], entry[11]]) as usize;
        let offset = u32::from_le_bytes([entry[12], entry[13], entry[14], entry[15]]) as usize;
        let icon_id = (i + 1) as u16;

        write_res_header(&mut out, size as u32, 3, icon_id, 0x1010, 0x0409);
        out.extend_from_slice(&ico[offset..offset + size]);
        pad_dword(&mut out);

        // GRPICONDIRENTRY: first 12 bytes of ICONDIRENTRY + 2-byte resource ID.
        group_data.extend_from_slice(&entry[0..12]);
        group_data.extend_from_slice(&icon_id.to_le_bytes());
    }

    // 3. RT_GROUP_ICON (type = 14) record (resource ID = 1).
    write_res_header(&mut out, group_data.len() as u32, 14, 1, 0x1030, 0x0409);
    out.extend_from_slice(&group_data);
    pad_dword(&mut out);

    out
}

fn write_res_header(
    out: &mut Vec<u8>,
    data_size: u32,
    res_type: u16,
    res_name: u16,
    memory_flags: u16,
    language_id: u16,
) {
    out.extend_from_slice(&data_size.to_le_bytes());
    out.extend_from_slice(&32u32.to_le_bytes()); // HeaderSize
    out.extend_from_slice(&0xffffu16.to_le_bytes());
    out.extend_from_slice(&res_type.to_le_bytes());
    out.extend_from_slice(&0xffffu16.to_le_bytes());
    out.extend_from_slice(&res_name.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes()); // DataVersion
    out.extend_from_slice(&memory_flags.to_le_bytes());
    out.extend_from_slice(&language_id.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes()); // Version
    out.extend_from_slice(&0u32.to_le_bytes()); // Characteristics
}

fn pad_dword(out: &mut Vec<u8>) {
    while !out.len().is_multiple_of(4) {
        out.push(0);
    }
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
