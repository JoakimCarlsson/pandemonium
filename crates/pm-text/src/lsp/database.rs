//! The compilation database clangd is pointed at when it cannot find one.
//!
//! clangd looks for `compile_commands.json` in the directories above a file
//! and in their `build` directories, and nowhere else. A checkout that builds
//! one directory per preset — `build/debug`, `build/release` — keeps its
//! databases where clangd does not look, and every file is then parsed with a
//! guessed command and none of the checkout's include paths.

use pm_host::Location;
use std::path::PathBuf;

/// The file name clangd reads a compilation database from.
const DATABASE: &str = "compile_commands.json";

/// The directory holding the database of the checkout at `root`, when clangd
/// would not find one there on its own.
///
/// Of several presets' databases the one written last is taken: it is the
/// build the reader was most recently working with. Nothing is generated, and
/// no flag is invented: a checkout with no database gets none.
pub(super) fn beside_build(root: &Location) -> Option<PathBuf> {
    let fs = root.host.fs();
    if fs.is_file(root.join(DATABASE)) || fs.is_file(root.join("build").join(DATABASE)) {
        return None;
    }
    fs.read_dir(root.join("build"))
        .ok()?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter_map(|directory| {
            let written = fs.metadata(directory.join(DATABASE)).ok()?.modified()?;
            Some((written, directory))
        })
        .max_by_key(|(written, _)| *written)
        .map(|(_, directory)| directory)
}

/// What to tell the reader when the checkout at `root` has no compilation
/// database at all, so that clangd runs on a guessed command.
pub(super) fn missing(root: &Location) -> Option<String> {
    let fs = root.host.fs();
    let found = fs.is_file(root.join(DATABASE))
        || fs
            .read_dir(root.join("build"))
            .into_iter()
            .flatten()
            .filter_map(Result::ok)
            .any(|entry| fs.is_file(entry.path().join(DATABASE)))
        || fs.is_file(root.join("build").join(DATABASE));
    (!found).then(|| {
        format!(
            "no {DATABASE} under {} or its build directories: clangd parses every file with a guessed command and cannot resolve the checkout's headers. Generate one with the build, for CMake `-DCMAKE_EXPORT_COMPILE_COMMANDS=ON`.",
            root.display()
        )
    })
}
