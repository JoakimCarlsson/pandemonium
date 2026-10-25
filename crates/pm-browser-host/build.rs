//! Lets the browser program find the engine's libraries beside it or in the place it was built against.

use std::path::PathBuf;

/// Where the engine's libraries were built against, if known.
fn cef_dir() -> Option<PathBuf> {
    let configured = std::env::var_os("CEF_PATH").map(PathBuf::from);
    let cached = std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".cache/cef"));
    configured
        .into_iter()
        .chain(cached)
        .find(|dir| dir.join("libcef.so").exists())
}

/// Adds the library directories to the program's run-time search path on Linux.
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
