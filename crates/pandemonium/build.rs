//! Lets the binary find the embedded browser's libraries without environment variables.

use std::path::PathBuf;

/// Where the browser engine's libraries are, if they can be found.
fn cef_dir() -> Option<PathBuf> {
    let configured = std::env::var_os("CEF_PATH").map(PathBuf::from);
    let cached = std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".cache/cef"));
    configured
        .into_iter()
        .chain(cached)
        .find(|dir| dir.join("libcef.so").exists())
}

/// Adds the directories the browser engine's libraries may be in to the binary's run-time search path on Linux: beside the binary, and where it was built against.
fn main() {
    println!("cargo::rerun-if-env-changed=CEF_PATH");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("linux") {
        return;
    }
    println!("cargo::rustc-link-arg=-Wl,-rpath,$ORIGIN");
    if let Some(dir) = cef_dir() {
        println!("cargo::rustc-link-arg=-Wl,-rpath,{}", dir.display());
    }
}
