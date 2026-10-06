//! Embeds the source commit and version label, and gives the Windows executable the editor's icon.

use std::path::Path;
use std::process::{Command, Stdio};

/// The editor's icon, relative to this crate.
const ICON: &str = "../../assets/brand/pandemonium.ico";

/// Records the source commit and version label, and embeds the icon when building for Windows.
fn main() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    println!("cargo:rustc-env=PANDEMONIUM_COMMIT={}", commit(&root));
    println!("cargo:rustc-env=PANDEMONIUM_VERSION={}", version());
    println!("cargo:rerun-if-env-changed=PANDEMONIUM_VERSION");
    println!("cargo:rerun-if-changed={ICON}");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    winresource::WindowsResource::new()
        .set_icon(ICON)
        .compile()
        .expect("could not embed the icon in the executable");
}

/// The version label to show: `PANDEMONIUM_VERSION` when set, else the crate version.
fn version() -> String {
    match std::env::var("PANDEMONIUM_VERSION") {
        Ok(label) if !label.is_empty() => label,
        _ => std::env::var("CARGO_PKG_VERSION").unwrap_or_default(),
    }
}

/// Runs Git in the source root, returning successful UTF-8 output only.
fn git(root: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .current_dir(root)
        .args(args)
        .stderr(Stdio::null())
        .output()
        .ok()?;
    output.status.success().then_some(())?;
    String::from_utf8(output.stdout).ok()
}

/// Watches an existing path without making absent Git metadata force rebuilds.
fn watch(path: &Path) {
    if path.exists() {
        println!("cargo:rerun-if-changed={}", path.display());
    }
}

/// Captures the short commit and dirty state, watching checkout and Git changes.
fn commit(root: &Path) -> String {
    let checkout = root.join(".git");
    if checkout.is_file() {
        watch(&checkout);
    }
    if !checkout.exists() {
        return String::new();
    }
    for name in ["HEAD", "index", "packed-refs"] {
        if let Some(path) = git(root, &["rev-parse", "--git-path", name]) {
            watch(&root.join(path.trim()));
        }
    }
    if let Some(reference) = git(root, &["symbolic-ref", "-q", "HEAD"])
        && let Some(path) = git(root, &["rev-parse", "--git-path", reference.trim()])
    {
        watch(&root.join(path.trim()));
    }
    if let Some(files) = git(
        root,
        &[
            "ls-files",
            "--cached",
            "--others",
            "--exclude-standard",
            "-z",
        ],
    ) {
        for file in files.split('\0').filter(|file| !file.is_empty()) {
            watch(&root.join(file));
        }
    }
    let Some(hash) = git(root, &["rev-parse", "--short=10", "HEAD"]) else {
        return String::new();
    };
    let Some(status) = git(root, &["status", "--porcelain"]) else {
        return String::new();
    };
    let hash = hash.trim();
    match status.is_empty() {
        true => hash.to_owned(),
        false => format!("{hash}-dirty"),
    }
}
