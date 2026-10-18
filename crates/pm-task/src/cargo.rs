//! The tasks of a Cargo manifest.

use std::path::Path;

use crate::task::Task;

/// The tasks of the `Cargo.toml` in `root`, or none when it has none.
pub(crate) fn detect(root: &Path) -> Vec<Task> {
    let Ok(text) = std::fs::read_to_string(root.join("Cargo.toml")) else {
        return Vec::new();
    };
    let manifest: toml::Table = text.parse().unwrap_or_default();
    let packaged = manifest.contains_key("package");
    let everything = if packaged { "" } else { " --workspace" };
    let task = |name: &str, command: String, group: &str| {
        Task::new(name, command, group, "rust", "Cargo.toml")
    };
    let mut found = vec![
        task("cargo build", format!("cargo build{everything}"), "build"),
        task("cargo check", format!("cargo check{everything}"), "build"),
        task("cargo test", format!("cargo test{everything}"), "test"),
        task(
            "cargo clippy",
            format!("cargo clippy{everything} --all-targets"),
            "build",
        ),
    ];
    if packaged && (manifest.contains_key("bin") || root.join("src/main.rs").is_file()) {
        found.push(task("cargo run", "cargo run".to_owned(), "run"));
    }
    found
}
